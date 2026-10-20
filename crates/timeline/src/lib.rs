//! Sequence edit engine for VideoCut.
//!
//! Time is [`videocut_model::TICKS_PER_SECOND`] ticks per second. Edits that
//! fail return [`EditError`] and leave the sequence as it was.
//!
//! Linked clips share timeline edits. Targeted tracks receive insert, overwrite,
//! lift, and extract. Sync lock ripples the other tracks so picture and sound
//! stay aligned.

#![forbid(unsafe_code)]

mod edit;
mod error;
mod geom;

pub use edit::{
    add_marker, add_multicam_angle, collect_snap_points, extract, freeze_frame, insert, lift,
    link_clips, move_marker, nest, overwrite, razor, remove_marker, ripple_delete, ripple_trim,
    roll_trim, set_duration, set_reversed, set_speed, set_sync_lock, set_track_locked,
    set_track_targeted, slide, slip, snap_hit, snap_ticks, switch_multicam_angle, unlink_clip,
    MarkerDraft, Placement, SnapHit, SnapKind, TrimSide,
};
pub use error::EditError;
pub use geom::source_at;
