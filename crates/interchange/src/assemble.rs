//! Build a [`Project`](videocut_model::Project) from the neutral draft importers produce.

use std::collections::HashMap;

use videocut_model::{
    new_id, AudioStreamInfo, BinItemKind, BinNode, Clip, ClipKind, Marker, MarkerKind, MediaRef,
    Project, Sequence, SequenceSettings, Track, TrackKind, Transition, TransitionAlign,
    VideoStreamInfo,
};

pub struct DraftProject {
    pub name: String,
    pub sequences: Vec<DraftSequence>,
    pub warnings: Vec<String>,
}

pub struct DraftSequence {
    pub name: String,
    pub settings: SequenceSettings,
    pub tracks: Vec<DraftTrack>,
    pub markers: Vec<DraftMarker>,
}

pub struct DraftTrack {
    pub kind: TrackKind,
    pub clips: Vec<DraftClip>,
    pub transitions: Vec<DraftTransition>,
}

pub struct DraftClip {
    pub name: String,
    pub media_name: String,
    pub path: String,
    pub media_duration: i64,
    pub video: Option<VideoStreamInfo>,
    pub audio: Option<AudioStreamInfo>,
    pub timeline_in: i64,
    pub timeline_out: i64,
    pub source_in: i64,
    pub source_out: i64,
    pub speed: f64,
    pub reversed: bool,
    pub enabled: bool,
    pub kind: ClipKind,
    pub label_color: u8,
}

pub struct DraftTransition {
    pub effect_id: String,
    pub center_ticks: i64,
    pub duration_ticks: i64,
    pub alignment: TransitionAlign,
}

pub struct DraftMarker {
    pub time_ticks: i64,
    pub duration_ticks: i64,
    pub name: String,
    pub comment: String,
    pub color: u8,
    pub kind: MarkerKind,
}

struct MediaSlot {
    index: usize,
}

pub fn realize(draft: DraftProject) -> Project {
    let mut project = Project::new(draft.name);
    let mut media_index: HashMap<String, MediaSlot> = HashMap::new();
    let mut bin_children = Vec::new();

    for sequence in draft.sequences {
        let mut video_tracks = Vec::new();
        let mut audio_tracks = Vec::new();
        for track in sequence.tracks {
            let mut realized = Track::new(
                track_index(track.kind, &video_tracks, &audio_tracks),
                track.kind,
            );
            let mut clips = track.clips;
            clips.sort_by(|a, b| {
                a.timeline_in
                    .cmp(&b.timeline_in)
                    .then(a.timeline_out.cmp(&b.timeline_out))
            });
            for clip in clips {
                let media_id = ensure_media(&mut project, &mut media_index, &clip);
                let mut realized_clip = Clip::media(clip.name, media_id);
                realized_clip.kind = clip.kind;
                realized_clip.timeline_in = clip.timeline_in;
                realized_clip.timeline_out = clip.timeline_out;
                realized_clip.source_in = clip.source_in;
                realized_clip.source_out = clip.source_out;
                realized_clip.speed = clip.speed;
                realized_clip.reversed = clip.reversed;
                realized_clip.enabled = clip.enabled;
                realized_clip.label_color = clip.label_color;
                realized.clips.push(realized_clip);
            }
            let mut transitions = track.transitions;
            transitions.sort_by_key(|transition| transition.center_ticks);
            for transition in transitions {
                if transition.duration_ticks <= 0 {
                    continue;
                }
                realized.transitions.push(Transition {
                    id: new_id("trans"),
                    effect_id: transition.effect_id,
                    center_ticks: transition.center_ticks,
                    duration_ticks: transition.duration_ticks,
                    alignment: transition.alignment,
                });
            }
            match track.kind {
                TrackKind::Video => video_tracks.push(realized),
                TrackKind::Audio => audio_tracks.push(realized),
            }
        }
        let mut sequence_model = Sequence::new(sequence.name, sequence.settings);
        sequence_model.video_tracks = video_tracks;
        sequence_model.audio_tracks = audio_tracks;
        sequence_model.markers = sequence
            .markers
            .into_iter()
            .map(|marker| Marker {
                id: new_id("marker"),
                time_ticks: marker.time_ticks,
                duration_ticks: marker.duration_ticks,
                name: marker.name,
                comment: marker.comment,
                color: marker.color,
                kind: marker.kind,
            })
            .collect();
        bin_children.push(BinNode {
            id: new_id("bin"),
            name: sequence_model.name.clone(),
            kind: BinItemKind::Sequence,
            media_id: None,
            sequence_id: Some(sequence_model.id.clone()),
            children: Vec::new(),
        });
        project.sequences.push(sequence_model);
    }

    for media in &project.media {
        bin_children.push(BinNode {
            id: new_id("bin"),
            name: media.name.clone(),
            kind: BinItemKind::Clip,
            media_id: Some(media.id.clone()),
            sequence_id: None,
            children: Vec::new(),
        });
    }
    if !bin_children.is_empty() {
        project.bins.push(BinNode {
            id: new_id("bin"),
            name: project.name.clone(),
            kind: BinItemKind::Bin,
            media_id: None,
            sequence_id: None,
            children: bin_children,
        });
    }
    project
}

fn track_index(kind: TrackKind, video: &[Track], audio: &[Track]) -> u32 {
    match kind {
        TrackKind::Video => video.len() as u32,
        TrackKind::Audio => audio.len() as u32,
    }
}

fn ensure_media(
    project: &mut Project,
    index: &mut HashMap<String, MediaSlot>,
    clip: &DraftClip,
) -> String {
    let key = if clip.path.is_empty() {
        format!("name:{}", clip.media_name)
    } else {
        clip.path.clone()
    };
    if let Some(slot) = index.get(&key) {
        let media = &mut project.media[slot.index];
        if clip.media_duration > media.duration_ticks {
            media.duration_ticks = clip.media_duration;
        }
        if media.video.is_none() {
            media.video = clip.video.clone();
        }
        if media.audio.is_none() {
            media.audio = clip.audio.clone();
        }
        return media.id.clone();
    }
    let id = new_id("media");
    let name = if clip.media_name.is_empty() {
        clip.name.clone()
    } else {
        clip.media_name.clone()
    };
    project.media.push(MediaRef {
        id: id.clone(),
        name,
        path: clip.path.clone(),
        duration_ticks: clip.media_duration.max(clip.source_out).max(0),
        video: clip.video.clone(),
        audio: clip.audio.clone(),
        proxy_path: None,
    });
    index.insert(
        key,
        MediaSlot {
            index: project.media.len() - 1,
        },
    );
    id
}
