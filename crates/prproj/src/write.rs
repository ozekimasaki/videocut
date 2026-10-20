//! Build a project XML object graph from [`videocut_model::Project`].

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::{json, Value};
use videocut_model::{
    effect_ids, BinItemKind, BinNode, Clip, ClipKind, EffectInstance, Interp, Marker, MarkerKind,
    MediaRef, Param, ParamValue, Project, Sequence, Track, TrackKind, Transition, TransitionAlign,
    TICKS_PER_SECOND,
};

use crate::xml::{PrprojError, XmlElement};

const PROJECT_CLASS: &str = "62ad66dd-0dcd-42da-a660-6d8fbde94876";
const ROOT_ITEM_CLASS: &str = "1c307a89-9318-47d7-a583-bf2553736543";
const BIN_CLASS: &str = "dbfd6653-24da-480e-a35e-ba45e9504e4b";
const ITEM_CLASS: &str = "cb4e0ed7-aca1-4171-8525-e3658dec06dd";
const SEQUENCE_CLASS: &str = "6a15d903-8739-11d5-af2d-9b7855ad8974";
const VIDEO_GROUP_CLASS: &str = "9e9abf7a-0918-49c2-91ae-991b5dde77bb";
const AUDIO_GROUP_CLASS: &str = "9b9238b9-53a8-4cc3-b03f-b36246d052e6";
const VIDEO_TRACK_CLASS: &str = "f68dcd81-8805-11d5-af2d-9bfa89d4ddd4";
const AUDIO_TRACK_CLASS: &str = "097f6203-99ae-11d5-84f2-8cf14bde7040";
const VIDEO_ITEM_CLASS: &str = "368b0406-29e3-4923-9fcd-094fbf9a1089";
const AUDIO_ITEM_CLASS: &str = "064ec682-9ba6-11d5-af2d-9ca32c7d6164";
const SUBCLIP_CLASS: &str = "e0c58dc9-dbdd-4166-aef7-5db7e3f22e84";
const VIDEO_CLIP_CLASS: &str = "9308dbef-2440-4acb-9ab2-953b9a4e82ec";
const AUDIO_CLIP_CLASS: &str = "b8830d03-de02-41ee-84ec-fe566dc70cd9";
const MASTER_CLASS: &str = "fb11c33a-b0a9-4465-aa94-b6d5db2628cf";
const MEDIA_CLASS: &str = "7a5c103e-f3ac-4391-b6b4-7cc3d2f9a7ff";
const VIDEO_STREAM_CLASS: &str = "a36e4719-3ec6-4a0c-ab11-8b4aab377aa5";
const AUDIO_STREAM_CLASS: &str = "0b5cf52f-2b85-4863-890b-8844b64ecfe9";
const VIDEO_SOURCE_CLASS: &str = "e64ddf74-8fac-4682-8aa8-0e0ca2248949";
const AUDIO_SOURCE_CLASS: &str = "f588da05-fc2a-4fbc-9383-74d653b379e3";
const VIDEO_SEQUENCE_SOURCE_CLASS: &str = "4752dfa9-7a7e-4a3b-a25b-cafde1a8d036";
const MARKERS_CLASS: &str = "bee50706-b524-416c-9f03-b596ce5f6866";
const MARKER_CLASS: &str = "a45508e0-3ff7-4d04-90a7-2e0dfff4c910";
const LINK_CLASS: &str = "149d4ea5-a7d4-4b34-9bb7-16d783904bf2";
const VIDEO_CHAIN_CLASS: &str = "0970e08a-f58f-4108-b29a-1a717b8e12e2";
const FILTER_CLASS: &str = "d10da199-beea-4dd1-b941-ed3a78766d50";
const SCALAR_PARAM_CLASS: &str = "fe47129e-6c94-4fc0-95d5-c056a517aaf3";
const POINT_PARAM_CLASS: &str = "ca81d347-309b-44d2-acc7-1c572efb973c";
const VIDEO_TRANSITION_CLASS: &str = "3eeaed31-f78e-4144-b8e8-077656517181";
const AUDIO_TRANSITION_CLASS: &str = "23f687b6-9c5a-42f4-bea1-e7b3e28e7082";
const VIDEO_MEDIA: &str = "228cda18-3625-4d2d-951e-348879e4ed93";
const AUDIO_MEDIA: &str = "80b8e3d5-6dca-4195-aefb-cb5f407ab009";

const MARKER_COLORS: [u64; 8] = [
    4_281_828_977,
    4_281_740_498,
    4_289_825_711,
    4_280_578_025,
    4_281_049_552,
    4_294_967_295,
    4_294_741_314,
    4_292_277_273,
];

struct Builder {
    next_id: i64,
    objects: Vec<XmlElement>,
}

struct MediaBinding {
    master_uid: String,
    video_source_id: Option<String>,
    audio_source_id: Option<String>,
}

struct Timeline<'a> {
    sequence: &'a Sequence,
    media: &'a HashMap<&'a str, MediaBinding>,
    sequence_uids: &'a HashMap<&'a str, String>,
    clip_ids: &'a mut HashMap<String, String>,
}

pub fn project_to_xml(project: &Project) -> Result<XmlElement, PrprojError> {
    for sequence in &project.sequences {
        if sequence.settings.fps.num <= 0 || sequence.settings.fps.den <= 0 {
            return Err(PrprojError::FrameRate);
        }
    }
    let mut builder = Builder {
        next_id: 1,
        objects: Vec::new(),
    };
    let root_uid = format!("uid-{}", project.id);
    let mut project_object = object("Project", PROJECT_CLASS, "45", &id_num("1"))
        .child(properties_node(&[("MZ.VideoCut.Id", project.id.as_str())]))
        .text_child("Name", &project.name)
        .child(XmlElement::new("RootProjectItem").attr("ObjectURef", &root_uid));
    project_object.attrs.retain(|(key, _)| key != "ObjectID");
    project_object
        .attrs
        .insert(0, ("ObjectID".into(), "1".into()));

    let mut media_bindings = HashMap::new();
    let mut sequence_uids = HashMap::new();
    for sequence in &project.sequences {
        sequence_uids.insert(sequence.id.as_str(), format!("uid-{}", sequence.id));
    }
    for media in &project.media {
        media_bindings.insert(media.id.as_str(), builder.media(media));
    }
    for sequence in &project.sequences {
        builder.sequence_source_media(sequence, &sequence_uids);
    }

    builder.push(project_object);
    let root_items = builder.bin_items(&project.bins, &media_bindings, &sequence_uids);
    builder.push(root_bin(&root_uid, root_items));

    let mut clip_object_ids: HashMap<String, String> = HashMap::new();
    for sequence in &project.sequences {
        builder.sequence(&mut Timeline {
            sequence,
            media: &media_bindings,
            sequence_uids: &sequence_uids,
            clip_ids: &mut clip_object_ids,
        });
    }
    builder.links(project, &clip_object_ids);

    let mut root = XmlElement::new("PremiereData").attr("Version", "3");
    root.children.push(crate::xml::XmlNode::Element(
        XmlElement::new("Project").attr("ObjectRef", "1"),
    ));
    for object in builder.objects {
        root.children.push(crate::xml::XmlNode::Element(object));
    }
    Ok(root)
}

impl Builder {
    fn alloc(&mut self) -> String {
        self.next_id += 1;
        self.next_id.to_string()
    }

    fn push(&mut self, element: XmlElement) -> String {
        let id = element
            .get_attr("ObjectID")
            .or_else(|| element.get_attr("ObjectUID"))
            .unwrap_or("")
            .to_string();
        self.objects.push(element);
        id
    }

    fn push_id(&mut self, mut element: XmlElement) -> String {
        let id = self.alloc();
        element.attrs.insert(0, ("ObjectID".into(), id.clone()));
        self.objects.push(element);
        id
    }

    fn media(&mut self, media: &MediaRef) -> MediaBinding {
        let video_stream = media.video.as_ref().map(|video| {
            let ticks = ticks_per_frame(video.fps.num, video.fps.den);
            self.push_id(
                object_bare("VideoStream", VIDEO_STREAM_CLASS, "13")
                    .text_child("FrameRect", format!("0,0,{},{}", video.width, video.height))
                    .text_child("FrameRate", ticks.to_string())
                    .text_child("Duration", media.duration_ticks.to_string())
                    .text_child("CodecType", &video.codec),
            )
        });
        let audio_stream = media.audio.as_ref().map(|audio| {
            let ticks = if audio.sample_rate == 0 {
                TICKS_PER_SECOND / 48_000
            } else {
                TICKS_PER_SECOND / i64::from(audio.sample_rate)
            };
            self.push_id(
                object_bare("AudioStream", AUDIO_STREAM_CLASS, "5")
                    .text_child("FrameRate", ticks.to_string())
                    .text_child("ChannelCount", audio.channels.to_string())
                    .text_child("Duration", media.duration_ticks.to_string())
                    .text_child("CodecType", &audio.codec),
            )
        });
        let mut media_element = object(
            "Media",
            MEDIA_CLASS,
            "30",
            &Id::Uid(format!("uid-{}", media.id)),
        )
        .child(properties_node(&[("MZ.VideoCut.Id", media.id.as_str())]))
        .text_child("FilePath", &media.path)
        .text_child("ActualMediaFilePath", &media.path)
        .text_child("Title", &media.name);
        if let Some(id) = &video_stream {
            media_element =
                media_element.child(XmlElement::new("VideoStream").attr("ObjectRef", id));
        }
        if let Some(id) = &audio_stream {
            media_element =
                media_element.child(XmlElement::new("AudioStream").attr("ObjectRef", id));
        }
        let _media_key = self.push(media_element);

        let proxy_id = media.proxy_path.as_ref().map(|path| {
            self.push_id(
                object_bare("Media", MEDIA_CLASS, "30")
                    .text_child("IsProxy", "true")
                    .text_child("FilePath", path)
                    .text_child("Title", file_name(path)),
            )
        });

        let video_source_id = video_stream.as_ref().map(|stream_id| {
            let mut source = object_bare("VideoMediaSource", VIDEO_SOURCE_CLASS, "2").child(
                XmlElement::new("MediaSource")
                    .attr("Version", "3")
                    .child(content_with_proxy(proxy_id.as_deref()))
                    .child(
                        XmlElement::new("Media").attr("ObjectURef", format!("uid-{}", media.id)),
                    ),
            );
            source = source.text_child("OriginalDuration", media.duration_ticks.to_string());
            let _ = stream_id;
            self.push_id(source)
        });
        let audio_source_id = audio_stream.as_ref().map(|_| {
            self.push_id(
                object_bare("AudioMediaSource", AUDIO_SOURCE_CLASS, "2")
                    .child(
                        XmlElement::new("MediaSource")
                            .attr("Version", "3")
                            .child(content_with_proxy(proxy_id.as_deref()))
                            .child(
                                XmlElement::new("Media")
                                    .attr("ObjectURef", format!("uid-{}", media.id)),
                            ),
                    )
                    .text_child("OriginalDuration", media.duration_ticks.to_string()),
            )
        });

        let mut clips = XmlElement::new("Clips").attr("Version", "1");
        if let Some(source_id) = &video_source_id {
            let clip_id = self.push_id(full_video_clip(source_id, media.duration_ticks, false));
            clips = clips.child(
                XmlElement::new("Clip")
                    .attr("Index", "0")
                    .attr("ObjectRef", clip_id),
            );
        }
        if let Some(source_id) = &audio_source_id {
            let clip_id = self.push_id(full_audio_clip(source_id, media.duration_ticks));
            let index = clips.children.len().to_string();
            clips = clips.child(
                XmlElement::new("Clip")
                    .attr("Index", index)
                    .attr("ObjectRef", clip_id),
            );
        }
        let master_uid = format!("master-{}", media.id);
        self.push(
            object(
                "MasterClip",
                MASTER_CLASS,
                "12",
                &Id::Uid(master_uid.clone()),
            )
            .text_child("Name", &media.name)
            .child(clips),
        );
        MediaBinding {
            master_uid,
            video_source_id,
            audio_source_id,
        }
    }

    fn sequence_source_media(
        &mut self,
        sequence: &Sequence,
        sequence_uids: &HashMap<&str, String>,
    ) {
        let uid = sequence_uids
            .get(sequence.id.as_str())
            .cloned()
            .unwrap_or_default();
        let source_id = self.push_id(
            object_bare("VideoSequenceSource", VIDEO_SEQUENCE_SOURCE_CLASS, "3").child(
                XmlElement::new("SequenceSource")
                    .attr("Version", "3")
                    .child(XmlElement::new("Sequence").attr("ObjectURef", uid)),
            ),
        );
        let clip_id = self.push_id(full_video_clip(&source_id, 0, false));
        let master_uid = format!("master-seq-{}", sequence.id);
        self.push(
            object("MasterClip", MASTER_CLASS, "12", &Id::Uid(master_uid))
                .text_child("Name", &sequence.name)
                .child(
                    XmlElement::new("Clips").attr("Version", "1").child(
                        XmlElement::new("Clip")
                            .attr("Index", "0")
                            .attr("ObjectRef", clip_id),
                    ),
                ),
        );
    }

    fn bin_items(
        &mut self,
        bins: &[BinNode],
        media: &HashMap<&str, MediaBinding>,
        sequences: &HashMap<&str, String>,
    ) -> XmlElement {
        let mut items = XmlElement::new("Items").attr("Version", "1");
        for (index, bin) in bins.iter().enumerate() {
            let uid = format!("uid-{}", bin.id);
            items = items.child(
                XmlElement::new("Item")
                    .attr("Index", index.to_string())
                    .attr("ObjectURef", &uid),
            );
            match bin.kind {
                BinItemKind::Bin => {
                    let children = self.bin_items(&bin.children, media, sequences);
                    self.push(bin_element(&uid, &bin.id, &bin.name, children));
                }
                BinItemKind::Clip => {
                    let master = bin
                        .media_id
                        .as_deref()
                        .and_then(|id| media.get(id))
                        .map(|binding| binding.master_uid.clone())
                        .unwrap_or_else(|| format!("master-missing-{}", bin.id));
                    self.push(clip_item(&uid, &bin.id, &bin.name, &master));
                }
                BinItemKind::Sequence => {
                    let master = format!("master-seq-{}", bin.sequence_id.as_deref().unwrap_or(""));
                    let _ = sequences;
                    self.push(clip_item(&uid, &bin.id, &bin.name, &master));
                }
            }
        }
        items
    }

    fn sequence(&mut self, timeline: &mut Timeline<'_>) {
        let sequence = timeline.sequence;
        let settings = &sequence.settings;
        let frame_ticks = ticks_per_frame(settings.fps.num, settings.fps.den);
        let audio_ticks = if settings.sample_rate == 0 {
            TICKS_PER_SECOND / 48_000
        } else {
            TICKS_PER_SECOND / i64::from(settings.sample_rate)
        };
        let video_group = self.track_group(
            timeline,
            &sequence.video_tracks,
            TrackKind::Video,
            frame_ticks,
        );
        let audio_group = self.track_group(
            timeline,
            &sequence.audio_tracks,
            TrackKind::Audio,
            audio_ticks,
        );
        let uid = timeline
            .sequence_uids
            .get(sequence.id.as_str())
            .cloned()
            .unwrap_or_else(|| format!("uid-{}", sequence.id));
        let mut element =
            object("Sequence", SEQUENCE_CLASS, "11", &Id::Uid(uid)).child(properties_node(&[
                ("MZ.VideoCut.Id", sequence.id.as_str()),
                (
                    "MZ.Sequence.ZeroPoint",
                    &settings.start_timecode_ticks.to_string(),
                ),
            ]));
        element = element.text_child("Name", &sequence.name);
        element = element.child(marker_owner(self.markers(&sequence.markers)));
        element = element.child(
            XmlElement::new("TrackGroups")
                .attr("Version", "1")
                .child(
                    XmlElement::new("TrackGroup")
                        .attr("Version", "1")
                        .attr("Index", "0")
                        .child(XmlElement::new("Second").attr("ObjectRef", video_group))
                        .child(XmlElement::new("First").with_text(VIDEO_MEDIA)),
                )
                .child(
                    XmlElement::new("TrackGroup")
                        .attr("Version", "1")
                        .attr("Index", "1")
                        .child(XmlElement::new("Second").attr("ObjectRef", audio_group))
                        .child(XmlElement::new("First").with_text(AUDIO_MEDIA)),
                ),
        );
        self.push(element);
    }

    fn track_group(
        &mut self,
        timeline: &mut Timeline<'_>,
        tracks: &[Track],
        kind: TrackKind,
        frame_ticks: i64,
    ) -> String {
        let settings = &timeline.sequence.settings;
        let mut list = XmlElement::new("Tracks").attr("Version", "1");
        for track in tracks {
            let track_id = self.track(timeline, track, kind);
            list = list.child(
                XmlElement::new("Track")
                    .attr("Index", track.index.to_string())
                    .attr("ObjectRef", track_id),
            );
        }
        let (tag, class, version) = match kind {
            TrackKind::Video => ("VideoTrackGroup", VIDEO_GROUP_CLASS, "8"),
            TrackKind::Audio => ("AudioTrackGroup", AUDIO_GROUP_CLASS, "6"),
        };
        let mut group = object_bare(tag, class, version).child(
            XmlElement::new("TrackGroup")
                .attr("Version", "1")
                .child(XmlElement::new("Node").attr("Version", "1"))
                .text_child("NextTrackID", (tracks.len() + 1).to_string())
                .child(list)
                .text_child("FrameRate", frame_ticks.to_string()),
        );
        if kind == TrackKind::Video {
            group = group
                .text_child(
                    "FrameRect",
                    format!("0,0,{},{}", settings.width, settings.height),
                )
                .text_child("PixelAspectRatio", "1,1");
        } else {
            group = group.text_child("NumAdaptiveChannels", settings.audio_channels.to_string());
        }
        self.push_id(group)
    }

    fn track(&mut self, timeline: &mut Timeline<'_>, track: &Track, kind: TrackKind) -> String {
        let mut items = XmlElement::new("TrackItems").attr("Version", "1");
        for (index, clip) in track.clips.iter().enumerate() {
            let object_id = self.clip(timeline, clip, kind);
            timeline.clip_ids.insert(clip.id.clone(), object_id.clone());
            items = items.child(
                XmlElement::new("TrackItem")
                    .attr("Index", index.to_string())
                    .attr("ObjectRef", object_id),
            );
        }
        let mut transitions = XmlElement::new("TrackItems").attr("Version", "1");
        for (index, transition) in track.transitions.iter().enumerate() {
            let object_id = self.transition(transition, kind);
            transitions = transitions.child(
                XmlElement::new("TrackItem")
                    .attr("Index", index.to_string())
                    .attr("ObjectRef", object_id),
            );
        }
        let media_type = match kind {
            TrackKind::Video => VIDEO_MEDIA,
            TrackKind::Audio => AUDIO_MEDIA,
        };
        let body = XmlElement::new("Track")
            .attr("Version", "3")
            .child(properties_node(&[
                ("MZ.VideoCut.Id", track.id.as_str()),
                ("TL.SQTrackShy", if track.hidden { "1" } else { "0" }),
                ("MZ.TrackTargeted", if track.targeted { "1" } else { "0" }),
            ]))
            .text_child(
                "IsSyncLocked",
                if track.sync_lock { "true" } else { "false" },
            )
            .text_child("Index", track.index.to_string())
            .text_child("MediaType", media_type)
            .text_child("IsMuted", if track.muted { "true" } else { "false" })
            .text_child("IsLocked", if track.locked { "true" } else { "false" })
            .text_child("ID", (track.index + 1).to_string());
        let clip_track = XmlElement::new("ClipTrack")
            .attr("Version", "2")
            .child(body)
            .child(
                XmlElement::new("TransitionItems")
                    .attr("Version", "3")
                    .child(transitions)
                    .text_child("MediaType", media_type),
            )
            .child(
                XmlElement::new("ClipItems")
                    .attr("Version", "3")
                    .child(items)
                    .text_child("MediaType", media_type),
            );
        let (tag, class, version) = match kind {
            TrackKind::Video => ("VideoClipTrack", VIDEO_TRACK_CLASS, "1"),
            TrackKind::Audio => ("AudioClipTrack", AUDIO_TRACK_CLASS, "6"),
        };
        let mut element = object_bare(tag, class, version);
        if kind == TrackKind::Audio {
            element = element.child(XmlElement::new("AudioTrack").attr("Version", "11"));
        }
        element = element.child(clip_track);
        self.push_id(element)
    }

    fn clip(&mut self, timeline: &Timeline<'_>, clip: &Clip, kind: TrackKind) -> String {
        let settings = &timeline.sequence.settings;
        let speed = if clip.speed == 0.0 {
            1.0
        } else {
            clip.speed.abs()
        };
        let (source_id, master_uid) =
            self.clip_source(clip, kind, timeline.media, timeline.sequence_uids);
        let raw_in = raw_ticks(clip.source_in, speed);
        let raw_out = raw_ticks(clip.source_out, speed);
        let mut core = XmlElement::new("Clip")
            .attr("Version", "14")
            .text_child("OutPoint", raw_out.to_string())
            .text_child("InPoint", raw_in.to_string());
        if (speed - 1.0).abs() > f64::EPSILON {
            core = core.text_child("PlaybackSpeed", format_number(speed));
        }
        if clip.reversed {
            core = core.text_child("PlayBackwards", "true");
        }
        if let Some(source_id) = source_id {
            core = core.child(XmlElement::new("Source").attr("ObjectRef", source_id));
        }
        let frame_ticks = ticks_per_frame(settings.fps.num, settings.fps.den);
        core = core.text_child("FrameRate", frame_ticks.to_string());
        let (holder_tag, holder_class) = match kind {
            TrackKind::Video => ("VideoClip", VIDEO_CLIP_CLASS),
            TrackKind::Audio => ("AudioClip", AUDIO_CLIP_CLASS),
        };
        let mut holder = object_bare(holder_tag, holder_class, "9").child(core);
        if clip.kind == ClipKind::Adjustment || matches_kind_name(clip.kind, "adjustment") {
            holder = holder.text_child("AdjustmentLayer", "true");
        }
        holder = holder.child(properties_node(&[(
            "MZ.VideoCut.ClipKind",
            kind_name(clip.kind),
        )]));
        let holder_id = self.push_id(holder);
        let subclip_id = self.push_id(
            object_bare("SubClip", SUBCLIP_CLASS, "3")
                .child(XmlElement::new("MasterClip").attr("ObjectURef", master_uid))
                .child(XmlElement::new("Clip").attr("ObjectRef", holder_id))
                .text_child("Name", &clip.name),
        );
        let chain_id = self.component_chain(&clip.effects);
        let media_type = match kind {
            TrackKind::Video => VIDEO_MEDIA,
            TrackKind::Audio => AUDIO_MEDIA,
        };
        let (item_tag, item_class, item_version) = match kind {
            TrackKind::Video => ("VideoClipTrackItem", VIDEO_ITEM_CLASS, "5"),
            TrackKind::Audio => ("AudioClipTrackItem", AUDIO_ITEM_CLASS, "6"),
        };
        let mut item = object_bare(item_tag, item_class, item_version).child(
            XmlElement::new("ClipTrackItem")
                .attr("Version", "4")
                .child(
                    XmlElement::new("TrackItem")
                        .attr("Version", "3")
                        .child(properties_node(&[
                            ("MZ.VideoCut.Id", clip.id.as_str()),
                            ("MZ.VideoCut.Label", &clip.label_color.to_string()),
                            ("MZ.VideoCut.ClipKind", kind_name(clip.kind)),
                        ]))
                        .text_child("End", clip.timeline_out.to_string())
                        .text_child("Start", clip.timeline_in.to_string())
                        .text_child("MediaType", media_type),
                )
                .child(
                    XmlElement::new("ComponentOwner")
                        .attr("Version", "1")
                        .child(XmlElement::new("Components").attr("ObjectRef", chain_id)),
                )
                .text_child("IsMuted", if clip.enabled { "false" } else { "true" })
                .child(XmlElement::new("SubClip").attr("ObjectRef", subclip_id)),
        );
        if kind == TrackKind::Video {
            item = item.text_child(
                "FrameRect",
                format!("0,0,{},{}", settings.width, settings.height),
            );
        }
        self.push_id(item)
    }

    fn clip_source(
        &self,
        clip: &Clip,
        kind: TrackKind,
        media: &HashMap<&str, MediaBinding>,
        sequence_uids: &HashMap<&str, String>,
    ) -> (Option<String>, String) {
        if let Some(sequence_id) = clip.nested_sequence_id.as_deref() {
            let master = format!("master-seq-{sequence_id}");
            let _ = sequence_uids;
            return (None, master);
        }
        if let Some(media_id) = clip.media_id.as_deref() {
            if let Some(binding) = media.get(media_id) {
                let source = match kind {
                    TrackKind::Video => binding.video_source_id.clone(),
                    TrackKind::Audio => binding
                        .audio_source_id
                        .clone()
                        .or_else(|| binding.video_source_id.clone()),
                };
                return (source, binding.master_uid.clone());
            }
        }
        (None, format!("master-missing-{}", clip.id))
    }

    fn component_chain(&mut self, effects: &[EffectInstance]) -> String {
        let mut list = XmlElement::new("Components").attr("Version", "1");
        for (index, effect) in effects.iter().enumerate() {
            let id = self.effect(effect);
            list = list.child(
                XmlElement::new("Component")
                    .attr("Index", index.to_string())
                    .attr("ObjectRef", id),
            );
        }
        self.push_id(
            object_bare("VideoComponentChain", VIDEO_CHAIN_CLASS, "1").child(
                XmlElement::new("ComponentChain")
                    .attr("Version", "2")
                    .child(XmlElement::new("Node").attr("Version", "1"))
                    .child(list),
            ),
        )
    }

    fn effect(&mut self, effect: &EffectInstance) -> String {
        let (match_name, display) = effect_names(&effect.effect_id);
        let mut params = XmlElement::new("Params").attr("Version", "1");
        for (index, param) in effect.params.iter().enumerate() {
            let id = self.param(param);
            params = params.child(
                XmlElement::new("Param")
                    .attr("Index", index.to_string())
                    .attr("ObjectRef", id),
            );
        }
        self.push_id(
            object_bare("VideoFilterComponent", FILTER_CLASS, "9")
                .child(
                    XmlElement::new("Component")
                        .attr("Version", "6")
                        .child(properties_node(&[("MZ.VideoCut.Id", effect.id.as_str())]))
                        .child(params)
                        .text_child("DisplayName", display)
                        .text_child("Bypass", if effect.enabled { "false" } else { "true" }),
                )
                .text_child("MatchName", match_name),
        )
    }

    fn param(&mut self, param: &Param) -> String {
        let (tag, class, version, name) = param_identity(&param.id, &param.value);
        let current = format_param(&param.value);
        let mut element = object_bare(tag, class, version)
            .child(properties_node(&[(
                "MZ.VideoCut.ParamId",
                param.id.as_str(),
            )]))
            .text_child("Name", name)
            .text_child("CurrentValue", &current)
            .text_child(
                "StartKeyframe",
                format!("-91445760000000000,{current},0,0,0,0,0,0"),
            );
        if !param.keyframes.is_empty() {
            let mut encoded = String::new();
            for key in &param.keyframes {
                let code = match key.interp {
                    Interp::Linear => 0,
                    Interp::Bezier => 1,
                    Interp::Hold => 4,
                };
                encoded.push_str(&format!(
                    "{},{},{code},0,0,0,0,0;",
                    key.time_ticks,
                    format_param(&key.value)
                ));
            }
            element = element.text_child("Keyframes", encoded);
        }
        self.push_id(element)
    }

    fn transition(&mut self, transition: &Transition, kind: TrackKind) -> String {
        let duration = transition.duration_ticks;
        let start = transition.center_ticks - duration / 2;
        let end = start + duration;
        let offset = match transition.alignment {
            TransitionAlign::Start => 0,
            TransitionAlign::Center => duration / 2,
            TransitionAlign::End => duration,
        };
        let (incoming, outgoing) = match transition.alignment {
            TransitionAlign::Start => ("true", "false"),
            TransitionAlign::End => ("false", "true"),
            TransitionAlign::Center => ("true", "true"),
        };
        let (match_name, display) = transition_names(&transition.effect_id);
        let media_type = match kind {
            TrackKind::Video => VIDEO_MEDIA,
            TrackKind::Audio => AUDIO_MEDIA,
        };
        let (tag, class, version) = match kind {
            TrackKind::Video => ("VideoTransitionTrackItem", VIDEO_TRANSITION_CLASS, "6"),
            TrackKind::Audio => ("AudioTransitionTrackItem", AUDIO_TRANSITION_CLASS, "4"),
        };
        self.push_id(
            object_bare(tag, class, version).child(
                XmlElement::new("TransitionTrackItem")
                    .attr("Version", "3")
                    .child(
                        XmlElement::new("TrackItem")
                            .attr("Version", "3")
                            .child(properties_node(&[(
                                "MZ.VideoCut.Id",
                                transition.id.as_str(),
                            )]))
                            .text_child("Start", start.to_string())
                            .text_child("End", end.to_string())
                            .text_child("MediaType", media_type),
                    )
                    .text_child("Alignment", offset.to_string())
                    .text_child("DisplayName", display)
                    .text_child("MatchName", match_name)
                    .text_child("HasOutgoingClip", outgoing)
                    .text_child("HasIncomingClip", incoming),
            ),
        )
    }

    fn markers(&mut self, markers: &[Marker]) -> String {
        let mut list = XmlElement::new("Markers").attr("Version", "1");
        for (index, marker) in markers.iter().enumerate() {
            let id = self.marker(marker);
            list = list.child(
                XmlElement::new("Marker")
                    .attr("Index", index.to_string())
                    .child(XmlElement::new("First").with_text(&marker.id))
                    .child(XmlElement::new("Second").attr("ObjectRef", id)),
            );
        }
        self.push_id(
            object_bare("Markers", MARKERS_CLASS, "4")
                .child(list)
                .text_child("ByGUID", "byGUID"),
        )
    }

    fn marker(&mut self, marker: &Marker) -> String {
        let type_name = match marker.kind {
            MarkerKind::Comment => "Comment",
            MarkerKind::Chapter => "Chapter",
            MarkerKind::Segmentation => "Segmentation",
            MarkerKind::Caption => "Caption",
        };
        let mut blob = BTreeMap::new();
        if !marker.comment.is_empty() {
            blob.insert(
                "mComment".to_string(),
                Value::String(marker.comment.clone()),
            );
        }
        let packed = MARKER_COLORS
            .get(marker.color as usize)
            .copied()
            .unwrap_or(MARKER_COLORS[0]);
        blob.insert(
            "mCuePointList".into(),
            json!([{
                "mKey": format!("keywordExtDVAv1_{}", marker.id),
                "mValue": json!({"color": packed}).to_string()
            }]),
        );
        if marker.duration_ticks != 0 {
            blob.insert("mDuration".into(), json!({"ticks": marker.duration_ticks}));
        }
        blob.insert("mMarkerID".into(), Value::String(marker.id.clone()));
        blob.insert("mName".into(), Value::String(marker.name.clone()));
        blob.insert("mStartTime".into(), json!({"ticks": marker.time_ticks}));
        blob.insert("mType".into(), Value::String(type_name.into()));
        let text =
            serde_json::to_string(&json!({"DVAMarker": Value::Object(blob.into_iter().collect())}))
                .unwrap_or_else(|_| "{}".into());
        self.push_id(object_bare("Marker", MARKER_CLASS, "3").text_child("DVAMarker", text))
    }

    fn links(&mut self, project: &Project, clip_object_ids: &HashMap<String, String>) {
        let mut clips: BTreeMap<&str, &Clip> = BTreeMap::new();
        for sequence in &project.sequences {
            for track in sequence
                .video_tracks
                .iter()
                .chain(sequence.audio_tracks.iter())
            {
                for clip in &track.clips {
                    clips.insert(clip.id.as_str(), clip);
                }
            }
        }
        let mut seen = BTreeSet::new();
        let ids: Vec<&str> = clips.keys().copied().collect();
        for id in ids {
            if !seen.insert(id) {
                continue;
            }
            let mut group = Vec::new();
            let mut stack = vec![id];
            while let Some(current) = stack.pop() {
                if group.contains(&current) {
                    continue;
                }
                group.push(current);
                seen.insert(current);
                if let Some(clip) = clips.get(current) {
                    for linked in &clip.linked_ids {
                        stack.push(linked.as_str());
                    }
                }
            }
            if group.len() < 2 {
                continue;
            }
            group.sort_unstable();
            let mut items = XmlElement::new("TrackItems").attr("Version", "1");
            for (index, member) in group.iter().enumerate() {
                let Some(object_id) = clip_object_ids.get(*member) else {
                    continue;
                };
                items = items.child(
                    XmlElement::new("TrackItem")
                        .attr("Index", index.to_string())
                        .attr("ObjectRef", object_id),
                );
            }
            self.push_id(
                object_bare("Link", LINK_CLASS, "1").child(
                    XmlElement::new("TrackItemGroup")
                        .attr("Version", "1")
                        .child(XmlElement::new("Node").attr("Version", "1"))
                        .child(items),
                ),
            );
        }
    }
}

enum Id {
    Num(String),
    Uid(String),
}

fn id_num(value: &str) -> Id {
    Id::Num(value.to_string())
}

fn object(name: &str, class_id: &str, version: &str, id: &Id) -> XmlElement {
    let element = XmlElement::new(name)
        .attr("ClassID", class_id)
        .attr("Version", version);
    match id {
        Id::Num(value) => element.attr("ObjectID", value),
        Id::Uid(value) => element.attr("ObjectUID", value),
    }
}

fn object_bare(name: &str, class_id: &str, version: &str) -> XmlElement {
    XmlElement::new(name)
        .attr("ClassID", class_id)
        .attr("Version", version)
}

fn properties_node(pairs: &[(&str, &str)]) -> XmlElement {
    let mut properties = XmlElement::new("Properties").attr("Version", "1");
    for (key, value) in pairs {
        properties = properties.text_child(*key, *value);
    }
    XmlElement::new("Node")
        .attr("Version", "1")
        .child(properties)
}

fn content_with_proxy(proxy_id: Option<&str>) -> XmlElement {
    let mut content = XmlElement::new("Content").attr("Version", "6");
    if let Some(proxy_id) = proxy_id {
        content = content.child(XmlElement::new("ProxyMedia").attr("ObjectRef", proxy_id));
    }
    content
}

fn full_video_clip(source_id: &str, duration: i64, _adjustment: bool) -> XmlElement {
    object_bare("VideoClip", VIDEO_CLIP_CLASS, "9").child(
        XmlElement::new("Clip")
            .attr("Version", "14")
            .text_child("OutPoint", duration.to_string())
            .text_child("InPoint", "0")
            .child(XmlElement::new("Source").attr("ObjectRef", source_id)),
    )
}

fn full_audio_clip(source_id: &str, duration: i64) -> XmlElement {
    object_bare("AudioClip", AUDIO_CLIP_CLASS, "8").child(
        XmlElement::new("Clip")
            .attr("Version", "14")
            .text_child("OutPoint", duration.to_string())
            .text_child("InPoint", "0")
            .child(XmlElement::new("Source").attr("ObjectRef", source_id)),
    )
}

fn root_bin(uid: &str, items: XmlElement) -> XmlElement {
    object(
        "RootProjectItem",
        ROOT_ITEM_CLASS,
        "1",
        &Id::Uid(uid.to_string()),
    )
    .child(
        XmlElement::new("ProjectItem")
            .attr("Version", "1")
            .child(properties_node(&[("MZ.VideoCut.Id", "root")]))
            .text_child("Name", "Root"),
    )
    .child(
        XmlElement::new("ProjectItemContainer")
            .attr("Version", "1")
            .child(items),
    )
}

fn bin_element(uid: &str, id: &str, name: &str, items: XmlElement) -> XmlElement {
    object("BinProjectItem", BIN_CLASS, "1", &Id::Uid(uid.to_string()))
        .child(
            XmlElement::new("ProjectItem")
                .attr("Version", "1")
                .child(properties_node(&[("MZ.VideoCut.Id", id)]))
                .text_child("Name", name),
        )
        .child(
            XmlElement::new("ProjectItemContainer")
                .attr("Version", "1")
                .child(items),
        )
}

fn clip_item(uid: &str, id: &str, name: &str, master_uid: &str) -> XmlElement {
    object(
        "ClipProjectItem",
        ITEM_CLASS,
        "1",
        &Id::Uid(uid.to_string()),
    )
    .child(
        XmlElement::new("ProjectItem")
            .attr("Version", "1")
            .child(properties_node(&[("MZ.VideoCut.Id", id)]))
            .text_child("Name", name),
    )
    .child(XmlElement::new("MasterClip").attr("ObjectURef", master_uid))
}

fn marker_owner(markers_id: String) -> XmlElement {
    XmlElement::new("MarkerOwner")
        .attr("Version", "1")
        .child(XmlElement::new("Markers").attr("ObjectRef", markers_id))
}

fn kind_name(kind: ClipKind) -> &'static str {
    match kind {
        ClipKind::Media => "media",
        ClipKind::Nested => "nested",
        ClipKind::Adjustment => "adjustment",
        ClipKind::Title => "title",
        ClipKind::ColorMatte => "colorMatte",
        ClipKind::Caption => "caption",
    }
}

fn matches_kind_name(kind: ClipKind, name: &str) -> bool {
    kind_name(kind) == name
}

fn effect_names(effect_id: &str) -> (&str, &str) {
    if effect_id == effect_ids::OPACITY {
        ("AE.ADBE Opacity", "Opacity")
    } else if effect_id == effect_ids::MOTION {
        ("AE.ADBE Motion", "Motion")
    } else {
        (effect_id, effect_id)
    }
}

fn transition_names(effect_id: &str) -> (&str, &str) {
    if effect_id == effect_ids::CROSS_DISSOLVE {
        ("AE.ADBE Cross Dissolve New", "Cross Dissolve")
    } else {
        (effect_id, effect_id)
    }
}

fn param_identity<'a>(
    id: &'a str,
    value: &ParamValue,
) -> (&'static str, &'static str, &'static str, &'a str) {
    let name = match id {
        "opacity" => "Opacity",
        "position" => "Position",
        "scale" => "Scale",
        "anchor" => "Anchor Point",
        "rotation" => "Rotation",
        other => other,
    };
    match value {
        ParamValue::Point(_) => ("PointComponentParam", POINT_PARAM_CLASS, "4", name),
        ParamValue::Float(_)
        | ParamValue::Int(_)
        | ParamValue::Bool(_)
        | ParamValue::Color(_)
        | ParamValue::Str(_) => ("VideoComponentParam", SCALAR_PARAM_CLASS, "8", name),
    }
}

fn format_param(value: &ParamValue) -> String {
    match value {
        ParamValue::Float(number) => format_number(*number),
        ParamValue::Int(number) => number.to_string(),
        ParamValue::Bool(flag) => if *flag { "true" } else { "false" }.to_string(),
        ParamValue::Point(point) => {
            format!("{}:{}", format_number(point.x), format_number(point.y))
        }
        ParamValue::Color(color) => format!("{},{},{},{}", color.r, color.g, color.b, color.a),
        ParamValue::Str(text) => text.clone(),
    }
}

fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}.", value as i64)
    } else {
        let text = format!("{value}");
        if text.contains('.') {
            text
        } else {
            format!("{text}.")
        }
    }
}

fn raw_ticks(source: i64, speed: f64) -> i64 {
    if (speed - 1.0).abs() < f64::EPSILON {
        source
    } else {
        (source as f64 * speed).round() as i64
    }
}

fn ticks_per_frame(num: i32, den: i32) -> i64 {
    if num <= 0 || den <= 0 {
        return TICKS_PER_SECOND / 30;
    }
    (TICKS_PER_SECOND as i128 * i128::from(den) / i128::from(num)) as i64
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("media")
        .to_string()
}
