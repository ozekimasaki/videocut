//! Map a project XML object graph onto [`videocut_model::Project`].

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use videocut_model::{
    effect_ids, AudioStreamInfo, BinItemKind, BinNode, Clip, ClipKind, EffectInstance, Interp,
    Keyframe, Marker, MarkerKind, MediaRef, Param, ParamValue, Point2, Project, Rational, Sequence,
    SequenceSettings, Track, TrackKind, Transition, TransitionAlign, VideoStreamInfo,
    TICKS_PER_SECOND,
};

use crate::xml::XmlElement;
use crate::PrprojWarning;

const KNOWN_TAGS: &[&str] = &[
    "Project",
    "RootProjectItem",
    "BinProjectItem",
    "ClipProjectItem",
    "SmartBinProjectItem",
    "Sequence",
    "VideoTrackGroup",
    "AudioTrackGroup",
    "DataTrackGroup",
    "VideoClipTrack",
    "AudioClipTrack",
    "AudioMixTrack",
    "VideoClipTrackItem",
    "AudioClipTrackItem",
    "SubClip",
    "VideoClip",
    "AudioClip",
    "MasterClip",
    "Media",
    "VideoStream",
    "AudioStream",
    "VideoMediaSource",
    "AudioMediaSource",
    "VideoSequenceSource",
    "AudioSequenceSource",
    "Markers",
    "Marker",
    "Link",
    "VideoComponentChain",
    "AudioComponentChain",
    "VideoFilterComponent",
    "VideoComponentParam",
    "PointComponentParam",
    "AudioComponentParam",
    "VideoTransitionTrackItem",
    "AudioTransitionTrackItem",
    "ClipLoggingInfo",
    "SecondaryContent",
    "AudioFader",
    "AudioMeter",
    "DefaultPanProcessor",
    "StereoToStereoPanProcessor",
    "AudioTrackInlet",
    "TimeRemapping",
    "ClipChannelGroupVectorSerializer",
];

const ID_PATHS: &[&[&str]] = &[
    &["Node", "Properties", "MZ.VideoCut.Id"],
    &["ProjectItem", "Node", "Properties", "MZ.VideoCut.Id"],
    &[
        "ClipTrackItem",
        "TrackItem",
        "Node",
        "Properties",
        "MZ.VideoCut.Id",
    ],
    &["ClipTrack", "Track", "Node", "Properties", "MZ.VideoCut.Id"],
    &[
        "TransitionTrackItem",
        "TrackItem",
        "Node",
        "Properties",
        "MZ.VideoCut.Id",
    ],
    &["Component", "Node", "Properties", "MZ.VideoCut.Id"],
];

struct Graph<'a> {
    by_id: HashMap<&'a str, &'a XmlElement>,
    by_uid: HashMap<&'a str, &'a XmlElement>,
    warnings: Vec<PrprojWarning>,
}

pub fn project_from_xml(root: &XmlElement) -> (Project, Vec<PrprojWarning>) {
    let mut graph = Graph {
        by_id: HashMap::new(),
        by_uid: HashMap::new(),
        warnings: Vec::new(),
    };
    for child in root.children.iter().filter_map(element_node) {
        if let Some(id) = child.get_attr("ObjectID") {
            if graph.by_id.insert(id, child).is_some() {
                graph.warn(child, format!("duplicate ObjectID {id}"));
            }
        }
        if let Some(uid) = child.get_attr("ObjectUID") {
            if graph.by_uid.insert(uid, child).is_some() {
                graph.warn(child, format!("duplicate ObjectUID {uid}"));
            }
        }
        if !KNOWN_TAGS.contains(&child.name.as_str()) {
            graph.warn(
                child,
                format!(
                    "unsupported object <{}> was kept in the document",
                    child.name
                ),
            );
        }
    }

    let project_element = root
        .children_named("Project")
        .find(|element| element.get_attr("ObjectID").is_some())
        .or_else(|| {
            root.child_named("Project")
                .and_then(|stub| graph.resolve(stub))
        });

    let Some(project_element) = project_element else {
        graph.warn_tag("Project", None, "document has no project object");
        return (Project::new("Untitled"), graph.warnings);
    };

    let mut project = Project {
        id: stable_id(project_element, "proj"),
        name: project_element
            .child_text("Name")
            .unwrap_or_else(|| "Untitled".into()),
        bins: Vec::new(),
        sequences: Vec::new(),
        media: Vec::new(),
    };

    if let Some(root_ref) = project_element.child_named("RootProjectItem") {
        if let Some(root_item) = graph.resolve(root_ref) {
            project.bins = graph.bin_children(root_item);
        } else {
            graph.warn(root_ref, "root bin reference does not resolve");
        }
    }

    let mut clip_ids: HashMap<String, String> = HashMap::new();
    for sequence_element in root.children_named("Sequence") {
        project
            .sequences
            .push(graph.sequence(sequence_element, &mut clip_ids));
    }
    graph.apply_links(root, &mut project, &clip_ids);
    project.media = graph.media_list(root);

    (project, graph.warnings)
}

fn element_node(node: &crate::xml::XmlNode) -> Option<&XmlElement> {
    match node {
        crate::xml::XmlNode::Element(element) => Some(element),
        crate::xml::XmlNode::Text(_) | crate::xml::XmlNode::Comment(_) => None,
    }
}

impl<'a> Graph<'a> {
    fn warn(&mut self, element: &XmlElement, message: impl Into<String>) {
        self.warn_tag(
            &element.name,
            element.object_id().map(str::to_string),
            message,
        );
    }

    fn warn_tag(&mut self, tag: &str, object_id: Option<String>, message: impl Into<String>) {
        self.warnings.push(PrprojWarning {
            object_id,
            tag: tag.to_string(),
            message: message.into(),
        });
    }

    fn resolve(&self, element: &'a XmlElement) -> Option<&'a XmlElement> {
        if let Some(id) = element.get_attr("ObjectRef") {
            return self.by_id.get(id).copied();
        }
        if let Some(uid) = element.get_attr("ObjectURef") {
            return self.by_uid.get(uid).copied();
        }
        if element.get_attr("ObjectID").is_some() || element.get_attr("ObjectUID").is_some() {
            return Some(element);
        }
        None
    }

    fn resolve_warn(&mut self, element: &'a XmlElement) -> Option<&'a XmlElement> {
        let resolved = self.resolve(element);
        if resolved.is_none()
            && (element.get_attr("ObjectRef").is_some() || element.get_attr("ObjectURef").is_some())
        {
            self.warn(element, "object reference does not resolve");
        }
        resolved
    }

    fn bin_children(&mut self, item: &'a XmlElement) -> Vec<BinNode> {
        let Some(container) = item_container(item) else {
            return Vec::new();
        };
        let mut children: Vec<&XmlElement> = container
            .children_named("Items")
            .flat_map(|items| items.children_named("Item"))
            .collect();
        children.sort_by_key(|child| index_of(child));
        let mut nodes = Vec::new();
        for child in children {
            if let Some(resolved) = self.resolve_warn(child) {
                nodes.push(self.bin_node(resolved));
            }
        }
        nodes
    }

    fn bin_node(&mut self, item: &'a XmlElement) -> BinNode {
        if item.name == "SmartBinProjectItem" {
            self.warn(item, "search bin imported as a bin");
        }
        let name = item
            .path_text(&["ProjectItem", "Name"])
            .or_else(|| item.child_text("Name"))
            .unwrap_or_else(|| "Bin".into());
        let id = stable_id(item, "bin");
        if item.name == "BinProjectItem"
            || item.name == "RootProjectItem"
            || item.name == "SmartBinProjectItem"
        {
            return BinNode {
                id,
                name,
                kind: BinItemKind::Bin,
                media_id: None,
                sequence_id: None,
                children: self.bin_children(item),
            };
        }

        let master = item
            .child_named("MasterClip")
            .and_then(|reference| self.resolve_warn(reference));
        if let Some(sequence_id) = master.and_then(|master| self.sequence_uid(master)) {
            return BinNode {
                id,
                name,
                kind: BinItemKind::Sequence,
                media_id: None,
                sequence_id: Some(sequence_id),
                children: Vec::new(),
            };
        }
        let media_id = master.and_then(|master| self.media_id_of_master(master));
        BinNode {
            id,
            name,
            kind: BinItemKind::Clip,
            media_id,
            sequence_id: None,
            children: Vec::new(),
        }
    }

    fn sequence_uid(&mut self, master: &'a XmlElement) -> Option<String> {
        for clip_ref in master
            .child_named("Clips")
            .into_iter()
            .flat_map(|clips| clips.children_named("Clip"))
        {
            let Some(clip) = self.resolve_warn(clip_ref) else {
                continue;
            };
            let Some(core) = clip_core(clip) else {
                continue;
            };
            let Some(source_ref) = core.child_named("Source") else {
                continue;
            };
            let Some(source) = self.resolve_warn(source_ref) else {
                continue;
            };
            if let Some(sequence) = source
                .descend(&["SequenceSource", "Sequence"])
                .and_then(|reference| self.resolve_warn(reference))
            {
                return Some(stable_id(sequence, "seq"));
            }
        }
        None
    }

    fn media_id_of_master(&mut self, master: &'a XmlElement) -> Option<String> {
        self.media_of_master(master)
            .map(|media| stable_id(media, "media"))
    }

    fn media_of_master(&mut self, master: &'a XmlElement) -> Option<&'a XmlElement> {
        for clip_ref in master
            .child_named("Clips")
            .into_iter()
            .flat_map(|clips| clips.children_named("Clip"))
        {
            let Some(clip) = self.resolve_warn(clip_ref) else {
                continue;
            };
            let Some(core) = clip_core(clip) else {
                continue;
            };
            let Some(source_ref) = core.child_named("Source") else {
                continue;
            };
            let Some(source) = self.resolve_warn(source_ref) else {
                continue;
            };
            if let Some(media_ref) = source.descend(&["MediaSource", "Media"]) {
                if let Some(media) = self.resolve_warn(media_ref) {
                    return Some(media);
                }
            }
        }
        None
    }

    fn sequence(
        &mut self,
        element: &'a XmlElement,
        clip_ids: &mut HashMap<String, String>,
    ) -> Sequence {
        let mut settings = SequenceSettings {
            width: 0,
            height: 0,
            fps: Rational { num: 30, den: 1 },
            sample_rate: 48_000,
            audio_channels: 2,
            start_timecode_ticks: element
                .path_text(&["Node", "Properties", "MZ.Sequence.ZeroPoint"])
                .and_then(|text| text.parse().ok())
                .unwrap_or(0),
        };
        let mut video_tracks = Vec::new();
        let mut audio_tracks = Vec::new();
        for pair in element
            .child_named("TrackGroups")
            .into_iter()
            .flat_map(|groups| groups.children_named("TrackGroup"))
        {
            let Some(group_ref) = pair.child_named("Second") else {
                continue;
            };
            let Some(group) = self.resolve_warn(group_ref) else {
                continue;
            };
            match group.name.as_str() {
                "VideoTrackGroup" => {
                    if let Some((width, height)) =
                        parse_frame_rect(group.child_text("FrameRect").as_deref())
                    {
                        settings.width = width;
                        settings.height = height;
                    }
                    if let Some(ticks) = group
                        .path_text(&["TrackGroup", "FrameRate"])
                        .and_then(|text| text.parse::<i64>().ok())
                    {
                        settings.fps = fps_from_ticks(ticks);
                    }
                    video_tracks.extend(self.tracks(group, TrackKind::Video, clip_ids));
                }
                "AudioTrackGroup" => {
                    if let Some(ticks) = group
                        .path_text(&["TrackGroup", "FrameRate"])
                        .and_then(|text| text.parse::<i64>().ok())
                    {
                        if ticks > 0 {
                            settings.sample_rate = (TICKS_PER_SECOND / ticks) as u32;
                        }
                    }
                    if let Some(channels) = group
                        .child_text("NumAdaptiveChannels")
                        .and_then(|text| text.parse().ok())
                    {
                        settings.audio_channels = channels;
                    }
                    audio_tracks.extend(self.tracks(group, TrackKind::Audio, clip_ids));
                }
                "DataTrackGroup" => {
                    self.warn(group, "data track group is not imported");
                }
                other => {
                    self.warn(group, format!("unsupported track group <{other}>"));
                }
            }
        }
        if settings.width == 0 || settings.height == 0 {
            self.warn(element, "sequence has no frame size");
        }

        Sequence {
            id: stable_id(element, "seq"),
            name: element
                .child_text("Name")
                .unwrap_or_else(|| "Sequence".into()),
            settings,
            video_tracks,
            audio_tracks,
            markers: self.markers(element),
            captions: Vec::new(),
            multicam_angles: Vec::new(),
        }
    }

    fn tracks(
        &mut self,
        group: &'a XmlElement,
        kind: TrackKind,
        clip_ids: &mut HashMap<String, String>,
    ) -> Vec<Track> {
        let mut refs: Vec<&XmlElement> = group
            .descend(&["TrackGroup", "Tracks"])
            .into_iter()
            .flat_map(|tracks| tracks.children_named("Track"))
            .collect();
        refs.sort_by_key(|reference| index_of(reference));
        let mut tracks = Vec::new();
        for reference in refs {
            let Some(track) = self.resolve_warn(reference) else {
                continue;
            };
            let keep = matches!(
                (kind, track.name.as_str()),
                (TrackKind::Video, "VideoClipTrack") | (TrackKind::Audio, "AudioClipTrack")
            );
            if track.name == "AudioMixTrack" {
                self.warn(track, "mix track is not imported");
            } else if !keep {
                self.warn(track, "unsupported track was left in the document");
            }
            if keep {
                tracks.push(self.track(track, kind, clip_ids));
            }
        }
        tracks
    }

    fn track(
        &mut self,
        element: &'a XmlElement,
        kind: TrackKind,
        clip_ids: &mut HashMap<String, String>,
    ) -> Track {
        let body = element.descend(&["ClipTrack", "Track"]);
        let index = body
            .and_then(|track| track.child_text("Index"))
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);
        let mut clips = Vec::new();
        if let Some(items) = element.descend(&["ClipTrack", "ClipItems", "TrackItems"]) {
            let mut refs: Vec<&XmlElement> = items.children_named("TrackItem").collect();
            refs.sort_by_key(|reference| index_of(reference));
            for reference in refs {
                let Some(item) = self.resolve_warn(reference) else {
                    continue;
                };
                if let Some(clip) = self.clip(item, kind) {
                    if let Some(object_id) = item.get_attr("ObjectID") {
                        clip_ids.insert(object_id.to_string(), clip.id.clone());
                    }
                    clips.push(clip);
                }
            }
        }
        let transitions = self.transitions(element);
        let shy = body
            .and_then(|track| track.descend(&["Node", "Properties", "TL.SQTrackShy"]))
            .and_then(XmlElement::text)
            .map(|text| text == "1" || text == "true")
            .unwrap_or(false);
        let targeted = body
            .and_then(|track| track.descend(&["Node", "Properties", "MZ.TrackTargeted"]))
            .and_then(XmlElement::text)
            .map(|text| text != "0" && text != "false")
            .unwrap_or(true);
        Track {
            id: stable_id(element, "track"),
            index,
            kind,
            clips,
            transitions,
            locked: flag(body, "IsLocked"),
            muted: flag(body, "IsMuted") || self.fader_muted(element),
            hidden: shy,
            sync_lock: body
                .and_then(|track| track.child_text("IsSyncLocked"))
                .map(|text| text != "false")
                .unwrap_or(true),
            targeted,
        }
    }

    fn fader_muted(&mut self, track: &'a XmlElement) -> bool {
        let Some(components) = track.descend(&["AudioTrack", "ComponentOwner", "Components"])
        else {
            return false;
        };
        let Some(chain) = self.resolve(components) else {
            return false;
        };
        for component_ref in chain
            .descend(&["ComponentChain", "Components"])
            .into_iter()
            .flat_map(|list| list.children_named("Component"))
        {
            let Some(component) = self.resolve(component_ref) else {
                continue;
            };
            if component.child_text("Name").as_deref() == Some("Mute")
                && component.child_text("CurrentValue").as_deref() == Some("true")
            {
                return true;
            }
            let inner = component
                .child_named("Component")
                .or_else(|| component.descend(&["AudioComponent", "Component"]));
            let Some(inner) = inner else {
                continue;
            };
            for param_ref in inner
                .child_named("Params")
                .into_iter()
                .flat_map(|params| params.children_named("Param"))
            {
                let Some(param) = self.resolve(param_ref) else {
                    continue;
                };
                if param.child_text("Name").as_deref() == Some("Mute")
                    && param.child_text("CurrentValue").as_deref() == Some("true")
                {
                    return true;
                }
            }
        }
        false
    }

    fn clip(&mut self, element: &'a XmlElement, kind: TrackKind) -> Option<Clip> {
        let subclip_ref = element.descend(&["ClipTrackItem", "SubClip"])?;
        let subclip = self.resolve_warn(subclip_ref)?;
        let clip_ref = subclip.child_named("Clip")?;
        let clip_holder = self.resolve_warn(clip_ref)?;
        let core = clip_core(clip_holder)?;
        let speed = playback_speed(core);
        let reversed = core.child_text("PlayBackwards").as_deref() == Some("true") || speed < 0.0;
        let speed = speed.abs();
        let speed = if speed == 0.0 { 1.0 } else { speed };
        let timeline_in = element
            .path_text(&["ClipTrackItem", "TrackItem", "Start"])
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);
        let timeline_out = element
            .path_text(&["ClipTrackItem", "TrackItem", "End"])
            .and_then(|text| text.parse().ok())
            .unwrap_or(timeline_in);
        let source_in = core
            .child_text("InPoint")
            .and_then(|text| text.parse().ok())
            .map(|raw: i64| source_ticks(raw, speed))
            .unwrap_or(0);
        let source_out = core
            .child_text("OutPoint")
            .and_then(|text| text.parse().ok())
            .map(|raw: i64| source_ticks(raw, speed))
            .unwrap_or(source_in);
        let master = subclip
            .child_named("MasterClip")
            .and_then(|reference| self.resolve_warn(reference));
        let nested = master.and_then(|master| self.sequence_uid(master));
        let media_id = if nested.is_some() {
            None
        } else {
            master.and_then(|master| self.media_id_of_master(master))
        };
        let adjustment = clip_holder.child_text("AdjustmentLayer").as_deref() == Some("true");
        let stored_kind = element
            .path_text(&[
                "ClipTrackItem",
                "TrackItem",
                "Node",
                "Properties",
                "MZ.VideoCut.ClipKind",
            ])
            .or_else(|| clip_holder.path_text(&["Node", "Properties", "MZ.VideoCut.ClipKind"]));
        let clip_kind = match stored_kind.as_deref() {
            Some("nested") => ClipKind::Nested,
            Some("adjustment") => ClipKind::Adjustment,
            Some("title") => ClipKind::Title,
            Some("colorMatte") => ClipKind::ColorMatte,
            Some("caption") => ClipKind::Caption,
            Some("media") => ClipKind::Media,
            _ if nested.is_some() => ClipKind::Nested,
            _ if adjustment => ClipKind::Adjustment,
            _ => ClipKind::Media,
        };
        if core.child_named("MarkerOwner").is_some() {
            let markers = self.markers(core);
            if !markers.is_empty() {
                self.warn(
                    element,
                    "clip markers are kept in the document and not copied onto the sequence",
                );
            }
        }
        let _ = kind;
        let label = element
            .path_text(&[
                "ClipTrackItem",
                "TrackItem",
                "Node",
                "Properties",
                "MZ.VideoCut.Label",
            ])
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);
        Some(Clip {
            id: stable_id(element, "clip"),
            name: subclip.child_text("Name").unwrap_or_else(|| "Clip".into()),
            media_id,
            nested_sequence_id: nested,
            kind: clip_kind,
            timeline_in,
            timeline_out,
            source_in,
            source_out,
            speed,
            reversed,
            enabled: element
                .path_text(&["ClipTrackItem", "IsMuted"])
                .map(|text| text != "true")
                .unwrap_or(true),
            linked_ids: Vec::new(),
            effects: self.effects(element),
            label_color: label,
        })
    }

    fn effects(&mut self, item: &'a XmlElement) -> Vec<EffectInstance> {
        let Some(chain_ref) = item.descend(&["ClipTrackItem", "ComponentOwner", "Components"])
        else {
            return Vec::new();
        };
        let Some(chain) = self.resolve_warn(chain_ref) else {
            return Vec::new();
        };
        let mut refs: Vec<&XmlElement> = chain
            .descend(&["ComponentChain", "Components"])
            .into_iter()
            .flat_map(|list| list.children_named("Component"))
            .collect();
        refs.sort_by_key(|reference| index_of(reference));
        let mut effects = Vec::new();
        for reference in refs {
            if let Some(component) = self.resolve_warn(reference) {
                effects.push(self.effect(component));
            }
        }
        effects
    }

    fn effect(&mut self, element: &'a XmlElement) -> EffectInstance {
        let match_name = element
            .child_text("MatchName")
            .or_else(|| element.path_text(&["Component", "MatchName"]))
            .unwrap_or_default();
        let display = element
            .path_text(&["Component", "DisplayName"])
            .unwrap_or_default();
        let effect_id = if match_name == "AE.ADBE Opacity" || display == "Opacity" {
            effect_ids::OPACITY.to_string()
        } else if match_name == "AE.ADBE Motion" || display == "Motion" {
            effect_ids::MOTION.to_string()
        } else {
            self.warn(
                element,
                format!("effect {match_name} is kept with its match name"),
            );
            if match_name.is_empty() {
                display
            } else {
                match_name
            }
        };
        let enabled = element
            .path_text(&["Component", "Bypass"])
            .map(|text| text != "true")
            .unwrap_or(true);
        let inner = element
            .child_named("Component")
            .or_else(|| element.descend(&["AudioComponent", "Component"]));
        let mut params = Vec::new();
        if let Some(inner) = inner {
            let mut refs: Vec<&XmlElement> = inner
                .child_named("Params")
                .into_iter()
                .flat_map(|list| list.children_named("Param"))
                .collect();
            refs.sort_by_key(|reference| index_of(reference));
            for reference in refs {
                let Some(param) = self.resolve_warn(reference) else {
                    continue;
                };
                params.push(self.param(param));
            }
        }
        EffectInstance {
            id: stable_id(element, "fx"),
            effect_id,
            enabled,
            params,
        }
    }

    fn param(&mut self, element: &'a XmlElement) -> Param {
        let name = element.child_text("Name").unwrap_or_default();
        let id = match name.as_str() {
            "Opacity" => "opacity".to_string(),
            "Position" => "position".to_string(),
            "Scale" => "scale".to_string(),
            "Anchor Point" => "anchor".to_string(),
            "Rotation" => "rotation".to_string(),
            _ => {
                let stored = element.path_text(&["Node", "Properties", "MZ.VideoCut.ParamId"]);
                if stored.is_none() && !name.is_empty() {
                    self.warn(
                        element,
                        format!("parameter {name} is kept by its stored name"),
                    );
                }
                stored.unwrap_or_else(|| {
                    if name.is_empty() {
                        "param".into()
                    } else {
                        name.clone()
                    }
                })
            }
        };
        let keyframes = parse_keyframes(element.child_text("Keyframes").as_deref(), &name);
        let value = element
            .child_text("CurrentValue")
            .or_else(|| start_key_value(element.child_text("StartKeyframe").as_deref()))
            .map(|text| param_value(&name, &text))
            .unwrap_or(ParamValue::Float(0.0));
        Param {
            id,
            value,
            keyframes,
        }
    }

    fn transitions(&mut self, track: &'a XmlElement) -> Vec<Transition> {
        let Some(items) = track.descend(&["ClipTrack", "TransitionItems", "TrackItems"]) else {
            return Vec::new();
        };
        let mut refs: Vec<&XmlElement> = items.children_named("TrackItem").collect();
        refs.sort_by_key(|reference| index_of(reference));
        let mut transitions = Vec::new();
        for reference in refs {
            if let Some(item) = self.resolve_warn(reference) {
                transitions.push(self.transition(item));
            }
        }
        transitions
    }

    fn transition(&mut self, element: &'a XmlElement) -> Transition {
        let start: i64 = element
            .path_text(&["TransitionTrackItem", "TrackItem", "Start"])
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);
        let end: i64 = element
            .path_text(&["TransitionTrackItem", "TrackItem", "End"])
            .and_then(|text| text.parse().ok())
            .unwrap_or(start);
        let duration = end.saturating_sub(start);
        let offset: i64 = element
            .path_text(&["TransitionTrackItem", "Alignment"])
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);
        let alignment = if duration > 0 && offset == 0 {
            TransitionAlign::Start
        } else if duration > 0 && offset == duration {
            TransitionAlign::End
        } else {
            TransitionAlign::Center
        };
        let match_name = element
            .path_text(&["TransitionTrackItem", "MatchName"])
            .unwrap_or_default();
        let display = element
            .path_text(&["TransitionTrackItem", "DisplayName"])
            .unwrap_or_default();
        let effect_id = if is_cross_dissolve(&match_name, &display) {
            effect_ids::CROSS_DISSOLVE.to_string()
        } else {
            self.warn(
                element,
                format!("transition {match_name} is kept with its match name"),
            );
            if match_name.is_empty() {
                display
            } else {
                match_name
            }
        };
        Transition {
            id: stable_id(element, "tr"),
            effect_id,
            center_ticks: start + duration / 2,
            duration_ticks: duration,
            alignment,
        }
    }

    fn markers(&mut self, owner: &'a XmlElement) -> Vec<Marker> {
        let Some(reference) = owner.descend(&["MarkerOwner", "Markers"]) else {
            return Vec::new();
        };
        let Some(collection) = self.resolve_warn(reference) else {
            return Vec::new();
        };
        let mut pairs: Vec<&XmlElement> = collection
            .child_named("Markers")
            .into_iter()
            .flat_map(|list| list.children_named("Marker"))
            .collect();
        pairs.sort_by_key(|pair| index_of(pair));
        let mut markers = Vec::new();
        for pair in pairs {
            let Some(reference) = pair.child_named("Second") else {
                continue;
            };
            if let Some(marker) = self.resolve_warn(reference) {
                markers.push(self.marker(marker));
            }
        }
        markers
    }

    fn marker(&mut self, element: &'a XmlElement) -> Marker {
        let raw = element
            .child_text("DVAMarker")
            .unwrap_or_else(|| "{}".into());
        let value: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        let data = value.get("DVAMarker").cloned().unwrap_or(value);
        let name = json_str(&data, "mName");
        let comment = json_str(&data, "mComment");
        let type_name = json_str(&data, "mType");
        let kind = match type_name.as_str() {
            "Chapter" => MarkerKind::Chapter,
            "Segmentation" => MarkerKind::Segmentation,
            "Caption" => MarkerKind::Caption,
            "Comment" | "" => MarkerKind::Comment,
            other => {
                self.warn(
                    element,
                    format!("marker type {other} imported as a comment"),
                );
                MarkerKind::Comment
            }
        };
        let time_ticks = data
            .pointer("/mStartTime/ticks")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let duration_ticks = data
            .pointer("/mDuration/ticks")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let color = marker_color(&data).unwrap_or(default_marker_color(kind));
        let id = data
            .get("mMarkerID")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| stable_id(element, "marker"));
        Marker {
            id,
            time_ticks,
            duration_ticks,
            name,
            comment,
            color,
            kind,
        }
    }

    fn apply_links(
        &mut self,
        root: &'a XmlElement,
        project: &mut Project,
        clip_ids: &HashMap<String, String>,
    ) {
        for link in root.children_named("Link") {
            let mut members = Vec::new();
            for reference in link
                .descend(&["TrackItemGroup", "TrackItems"])
                .into_iter()
                .flat_map(|items| items.children_named("TrackItem"))
            {
                let Some(object_id) = reference.get_attr("ObjectRef") else {
                    continue;
                };
                if let Some(clip_id) = clip_ids.get(object_id) {
                    members.push(clip_id.clone());
                } else {
                    self.warn(
                        link,
                        format!("link points at unknown track item {object_id}"),
                    );
                }
            }
            for clip_id in &members {
                let others: Vec<String> = members
                    .iter()
                    .filter(|other| *other != clip_id)
                    .cloned()
                    .collect();
                if let Some(clip) = project
                    .sequences
                    .iter_mut()
                    .find_map(|sequence| sequence.clip_mut(clip_id))
                {
                    for other in others {
                        if !clip.linked_ids.contains(&other) {
                            clip.linked_ids.push(other);
                        }
                    }
                }
            }
        }
    }

    fn media_list(&mut self, root: &'a XmlElement) -> Vec<MediaRef> {
        let mut seen = HashSet::new();
        let mut media = Vec::new();
        for element in root.children_named("Media") {
            if element.child_text("IsProxy").as_deref() == Some("true") {
                continue;
            }
            let id = stable_id(element, "media");
            if !seen.insert(id.clone()) {
                continue;
            }
            media.push(self.media_ref(element, root));
        }
        media
    }

    fn media_ref(&mut self, element: &'a XmlElement, root: &'a XmlElement) -> MediaRef {
        let path = element
            .child_text("FilePath")
            .or_else(|| element.child_text("ActualMediaFilePath"))
            .unwrap_or_default();
        let name = element
            .child_text("Title")
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| file_name(&path));
        let video = element
            .child_named("VideoStream")
            .and_then(|reference| self.resolve(reference))
            .map(video_info);
        let audio = element
            .child_named("AudioStream")
            .and_then(|reference| self.resolve(reference))
            .map(audio_info);
        let duration_ticks = video
            .as_ref()
            .map(|_| {
                stream_duration(
                    element
                        .child_named("VideoStream")
                        .and_then(|r| self.resolve(r)),
                )
            })
            .into_iter()
            .chain(audio.as_ref().map(|_| {
                stream_duration(
                    element
                        .child_named("AudioStream")
                        .and_then(|r| self.resolve(r)),
                )
            }))
            .max()
            .unwrap_or(0);
        let proxy_path = proxy_path(element, root, &self.by_id, &self.by_uid);
        MediaRef {
            id: stable_id(element, "media"),
            name,
            path,
            duration_ticks,
            video,
            audio,
            proxy_path,
        }
    }
}

fn proxy_path(
    media: &XmlElement,
    root: &XmlElement,
    by_id: &HashMap<&str, &XmlElement>,
    by_uid: &HashMap<&str, &XmlElement>,
) -> Option<String> {
    if let Some(path) = media.child_text("ProxyPath") {
        return Some(path);
    }
    let media_id = media.get_attr("ObjectID");
    let media_uid = media.get_attr("ObjectUID");
    for source in root.children.iter().filter_map(element_node) {
        if source.name != "VideoMediaSource" && source.name != "AudioMediaSource" {
            continue;
        }
        let points_here = source
            .descend(&["MediaSource", "Media"])
            .and_then(|reference| {
                reference
                    .get_attr("ObjectRef")
                    .zip(media_id)
                    .map(|(id, media_id)| id == media_id)
                    .or_else(|| {
                        reference
                            .get_attr("ObjectURef")
                            .zip(media_uid)
                            .map(|(uid, media_uid)| uid == media_uid)
                    })
            })
            .unwrap_or(false);
        if !points_here {
            continue;
        }
        let Some(proxy_ref) = source.descend(&["MediaSource", "Content", "ProxyMedia"]) else {
            continue;
        };
        let proxy = proxy_ref
            .get_attr("ObjectRef")
            .and_then(|id| by_id.get(id).copied())
            .or_else(|| {
                proxy_ref
                    .get_attr("ObjectURef")
                    .and_then(|uid| by_uid.get(uid).copied())
            });
        if let Some(path) = proxy.and_then(|element| element.child_text("FilePath")) {
            return Some(path);
        }
    }
    None
}

fn video_info(stream: &XmlElement) -> VideoStreamInfo {
    let (width, height) =
        parse_frame_rect(stream.child_text("FrameRect").as_deref()).unwrap_or((0, 0));
    let ticks = stream
        .child_text("FrameRate")
        .and_then(|text| text.parse().ok())
        .unwrap_or(TICKS_PER_SECOND / 30);
    VideoStreamInfo {
        width,
        height,
        fps: fps_from_ticks(ticks),
        codec: stream.child_text("CodecType").unwrap_or_default(),
    }
}

fn audio_info(stream: &XmlElement) -> AudioStreamInfo {
    let ticks = stream
        .child_text("FrameRate")
        .and_then(|text| text.parse::<i64>().ok())
        .unwrap_or(TICKS_PER_SECOND / 48_000);
    let sample_rate = if ticks > 0 {
        (TICKS_PER_SECOND / ticks) as u32
    } else {
        48_000
    };
    let channels = stream
        .child_text("ChannelCount")
        .and_then(|text| text.parse().ok())
        .unwrap_or(2);
    AudioStreamInfo {
        sample_rate,
        channels,
        codec: stream.child_text("CodecType").unwrap_or_default(),
    }
}

fn stream_duration(stream: Option<&XmlElement>) -> i64 {
    stream
        .and_then(|stream| stream.child_text("Duration"))
        .and_then(|text| text.parse().ok())
        .unwrap_or(0)
}

fn item_container(item: &XmlElement) -> Option<&XmlElement> {
    item.child_named("ProjectItemContainer")
        .or_else(|| item.descend(&["ProjectItem", "ProjectItemContainer"]))
}

fn clip_core(element: &XmlElement) -> Option<&XmlElement> {
    element.child_named("Clip").or_else(|| {
        element
            .children
            .iter()
            .find_map(element_node)
            .and_then(|child| child.child_named("Clip"))
    })
}

fn stable_id(element: &XmlElement, prefix: &str) -> String {
    for path in ID_PATHS {
        if let Some(id) = element.path_text(path) {
            return id;
        }
    }
    if let Some(uid) = element.get_attr("ObjectUID") {
        return format!("{prefix}-{uid}");
    }
    if let Some(id) = element.get_attr("ObjectID") {
        return format!("{prefix}-{id}");
    }
    format!("{prefix}-anon")
}

fn index_of(element: &XmlElement) -> i64 {
    element
        .get_attr("Index")
        .and_then(|text| text.parse().ok())
        .unwrap_or(0)
}

fn flag(element: Option<&XmlElement>, name: &str) -> bool {
    element
        .and_then(|element| element.child_text(name))
        .as_deref()
        == Some("true")
}

fn playback_speed(core: &XmlElement) -> f64 {
    core.child_text("PlaybackSpeed")
        .and_then(|text| text.parse().ok())
        .unwrap_or(1.0)
}

fn source_ticks(raw: i64, speed: f64) -> i64 {
    if speed == 0.0 || (speed - 1.0).abs() < f64::EPSILON {
        return raw;
    }
    (raw as f64 / speed) as i64
}

fn parse_frame_rect(value: Option<&str>) -> Option<(u32, u32)> {
    let value = value?;
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    Some((parts[2].trim().parse().ok()?, parts[3].trim().parse().ok()?))
}

fn fps_from_ticks(ticks_per_frame: i64) -> Rational {
    if ticks_per_frame <= 0 {
        return Rational { num: 30, den: 1 };
    }
    let ticks = ticks_per_frame as i128;
    let second = TICKS_PER_SECOND as i128;
    let divisor = gcd(second, ticks);
    let num = second / divisor;
    let den = ticks / divisor;
    if num > i32::MAX as i128 || den > i32::MAX as i128 {
        return Rational { num: 30, den: 1 };
    }
    Rational {
        num: num as i32,
        den: den as i32,
    }
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a.abs()
}

fn is_cross_dissolve(match_name: &str, display: &str) -> bool {
    let match_name = match_name.to_ascii_lowercase();
    let display = display.to_ascii_lowercase();
    match_name.contains("cross dissolve") || display.contains("cross dissolve")
}

fn param_value(name: &str, text: &str) -> ParamValue {
    if text.contains(':') && !text.contains(',') {
        let mut parts = text.splitn(2, ':');
        let x = parts
            .next()
            .and_then(|part| part.parse().ok())
            .unwrap_or(0.0);
        let y = parts
            .next()
            .and_then(|part| part.parse().ok())
            .unwrap_or(0.0);
        return ParamValue::Point(Point2 { x, y });
    }
    if name == "Position" || name == "Anchor Point" {
        let mut parts = text.splitn(2, ':');
        let x = parts
            .next()
            .and_then(|part| part.parse().ok())
            .unwrap_or(0.0);
        let y = parts
            .next()
            .and_then(|part| part.parse().ok())
            .unwrap_or(0.0);
        return ParamValue::Point(Point2 { x, y });
    }
    if text == "true" || text == "false" {
        return ParamValue::Bool(text == "true");
    }
    if let Ok(value) = text.parse::<i64>() {
        return ParamValue::Int(value);
    }
    if let Ok(value) = text.parse::<f64>() {
        return ParamValue::Float(value);
    }
    ParamValue::Str(text.to_string())
}

fn start_key_value(text: Option<&str>) -> Option<String> {
    let text = text?;
    let mut parts = text.split(',');
    let _time = parts.next()?;
    parts.next().map(str::to_string)
}

fn parse_keyframes(text: Option<&str>, name: &str) -> Vec<Keyframe> {
    let Some(text) = text else {
        return Vec::new();
    };
    if text.trim().is_empty() {
        return Vec::new();
    }
    text.split(';')
        .filter(|entry| !entry.trim().is_empty())
        .filter_map(|entry| {
            let mut fields = entry.split(',');
            let time: i64 = fields.next()?.trim().parse().ok()?;
            if time < 0 {
                return None;
            }
            let value = fields.next()?.trim();
            let code: i32 = fields
                .next()
                .and_then(|field| field.trim().parse().ok())
                .unwrap_or(0);
            let interp = match code {
                1 => Interp::Bezier,
                4 => Interp::Hold,
                _ => Interp::Linear,
            };
            Some(Keyframe {
                time_ticks: time,
                value: param_value(name, value),
                interp,
            })
        })
        .collect()
}

fn json_str(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn marker_color(data: &Value) -> Option<u8> {
    let cues = data.get("mCuePointList")?.as_array()?;
    for cue in cues {
        let raw = cue.get("mValue")?.as_str()?;
        let packed = serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|value| value.get("color").and_then(Value::as_u64))?;
        if let Some(index) = color_index(packed) {
            return Some(index);
        }
    }
    None
}

fn color_index(packed: u64) -> Option<u8> {
    const PALETTE: [u64; 8] = [
        4_281_828_977,
        4_281_740_498,
        4_289_825_711,
        4_280_578_025,
        4_281_049_552,
        4_294_967_295,
        4_294_741_314,
        4_292_277_273,
    ];
    PALETTE
        .iter()
        .position(|color| *color == packed)
        .map(|index| index as u8)
}

fn default_marker_color(kind: MarkerKind) -> u8 {
    match kind {
        MarkerKind::Comment => 0,
        MarkerKind::Chapter => 1,
        MarkerKind::Segmentation => 2,
        MarkerKind::Caption => 0,
    }
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("media")
        .to_string()
}
