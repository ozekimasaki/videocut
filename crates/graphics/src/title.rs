use videocut_model::{
    new_id, Clip, ClipKind, EffectInstance, Param, ParamValue, Point2, Rgba, Sequence, TrackKind,
};

use crate::error::GraphicsError;

/// Effect id stored on a title clip. VideoCut-owned, independent of other hosts.
pub const TITLE_EFFECT_ID: &str = "videocut.graphics.title";
/// Effect id stored on a lower-third clip.
pub const LOWER_THIRD_EFFECT_ID: &str = "videocut.graphics.lower-third";

pub mod param_id {
    pub const TEXT: &str = "text";
    pub const PRIMARY: &str = "primary";
    pub const SECONDARY: &str = "secondary";
    pub const FONT_FAMILY: &str = "fontFamily";
    pub const FONT_SIZE: &str = "fontSize";
    pub const COLOR: &str = "color";
    pub const ACCENT: &str = "accent";
    pub const ALIGN: &str = "align";
    pub const POSITION: &str = "position";
    pub const OPACITY: &str = "opacity";
    pub const BAR_HEIGHT: &str = "barHeight";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

impl TextAlign {
    fn as_str(self) -> &'static str {
        match self {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Right => "right",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "left" => TextAlign::Left,
            "right" => TextAlign::Right,
            _ => TextAlign::Center,
        }
    }
}

/// Normalized screen position uses `x` and `y` in the 0..1 range of the frame.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub font_family: String,
    pub font_size: f64,
    pub color: Rgba,
    pub align: TextAlign,
    pub position: Point2,
    pub opacity: f64,
}

impl TextStyle {
    pub fn title() -> Self {
        Self {
            font_family: "sans-serif".into(),
            font_size: 64.0,
            color: white(),
            align: TextAlign::Center,
            position: Point2 { x: 0.5, y: 0.42 },
            opacity: 1.0,
        }
    }

    pub fn lower_third() -> Self {
        Self {
            font_family: "sans-serif".into(),
            font_size: 40.0,
            color: white(),
            align: TextAlign::Left,
            position: Point2 { x: 0.06, y: 0.78 },
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TitleSpec {
    pub text: String,
    pub timeline_in: i64,
    pub timeline_out: i64,
    pub style: TextStyle,
}

impl TitleSpec {
    pub fn new(text: impl Into<String>, timeline_in: i64, timeline_out: i64) -> Self {
        Self {
            text: text.into(),
            timeline_in,
            timeline_out,
            style: TextStyle::title(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LowerThirdSpec {
    pub primary: String,
    pub secondary: String,
    pub accent: Rgba,
    pub bar_height: f64,
    pub timeline_in: i64,
    pub timeline_out: i64,
    pub style: TextStyle,
}

impl LowerThirdSpec {
    pub fn new(
        primary: impl Into<String>,
        secondary: impl Into<String>,
        timeline_in: i64,
        timeline_out: i64,
    ) -> Self {
        Self {
            primary: primary.into(),
            secondary: secondary.into(),
            accent: Rgba {
                r: 0.93,
                g: 0.62,
                b: 0.28,
                a: 1.0,
            },
            bar_height: 0.12,
            timeline_in,
            timeline_out,
            style: TextStyle::lower_third(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TitleContent {
    pub text: String,
    pub style: TextStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LowerThirdContent {
    pub primary: String,
    pub secondary: String,
    pub accent: Rgba,
    pub bar_height: f64,
    pub style: TextStyle,
}

pub fn create_title_clip(spec: &TitleSpec) -> Result<Clip, GraphicsError> {
    validate_span(spec.timeline_in, spec.timeline_out)?;
    validate_style(&spec.style)?;
    let mut params = vec![p_str(param_id::TEXT, spec.text.clone())];
    params.extend(style_params(&spec.style));
    Ok(graphic_clip(
        spec.text.clone(),
        spec.timeline_in,
        spec.timeline_out,
        effect(TITLE_EFFECT_ID, params),
    ))
}

pub fn create_lower_third_clip(spec: &LowerThirdSpec) -> Result<Clip, GraphicsError> {
    validate_span(spec.timeline_in, spec.timeline_out)?;
    validate_style(&spec.style)?;
    validate_bar_height(spec.bar_height)?;
    let mut params = vec![
        p_str(param_id::PRIMARY, spec.primary.clone()),
        p_str(param_id::SECONDARY, spec.secondary.clone()),
        p_color(param_id::ACCENT, spec.accent),
        p_float(param_id::BAR_HEIGHT, spec.bar_height),
    ];
    params.extend(style_params(&spec.style));
    let effect = effect(LOWER_THIRD_EFFECT_ID, params);
    Ok(graphic_clip(
        spec.primary.clone(),
        spec.timeline_in,
        spec.timeline_out,
        effect,
    ))
}

pub fn add_title(
    sequence: &mut Sequence,
    track_id: &str,
    spec: TitleSpec,
) -> Result<String, GraphicsError> {
    let clip = create_title_clip(&spec)?;
    push_graphic(sequence, track_id, clip)
}

pub fn add_lower_third(
    sequence: &mut Sequence,
    track_id: &str,
    spec: LowerThirdSpec,
) -> Result<String, GraphicsError> {
    let clip = create_lower_third_clip(&spec)?;
    push_graphic(sequence, track_id, clip)
}

pub fn read_title(clip: &Clip) -> Result<TitleContent, GraphicsError> {
    let effect = require_effect(clip, TITLE_EFFECT_ID, GraphicsError::NotTitle)?;
    Ok(TitleContent {
        text: required_str(effect, param_id::TEXT)?,
        style: read_style(effect)?,
    })
}

pub fn read_lower_third(clip: &Clip) -> Result<LowerThirdContent, GraphicsError> {
    let effect = require_effect(clip, LOWER_THIRD_EFFECT_ID, GraphicsError::NotLowerThird)?;
    Ok(LowerThirdContent {
        primary: required_str(effect, param_id::PRIMARY)?,
        secondary: required_str(effect, param_id::SECONDARY)?,
        accent: color_or(
            effect,
            param_id::ACCENT,
            Rgba {
                r: 0.93,
                g: 0.62,
                b: 0.28,
                a: 1.0,
            },
        )?,
        bar_height: float_or(effect, param_id::BAR_HEIGHT, 0.12)?,
        style: read_style(effect)?,
    })
}

pub fn set_title_text(clip: &mut Clip, text: impl Into<String>) -> Result<(), GraphicsError> {
    let text = text.into();
    {
        let effect = clip
            .effects
            .iter_mut()
            .find(|fx| fx.effect_id == TITLE_EFFECT_ID)
            .ok_or(GraphicsError::NotTitle)?;
        set_param(effect, param_id::TEXT, ParamValue::Str(text.clone()));
    }
    clip.name = text;
    Ok(())
}

pub fn set_lower_third_text(
    clip: &mut Clip,
    primary: impl Into<String>,
    secondary: impl Into<String>,
) -> Result<(), GraphicsError> {
    let primary = primary.into();
    let secondary = secondary.into();
    {
        let effect = clip
            .effects
            .iter_mut()
            .find(|fx| fx.effect_id == LOWER_THIRD_EFFECT_ID)
            .ok_or(GraphicsError::NotLowerThird)?;
        set_param(effect, param_id::PRIMARY, ParamValue::Str(primary.clone()));
        set_param(effect, param_id::SECONDARY, ParamValue::Str(secondary));
    }
    clip.name = primary;
    Ok(())
}

fn push_graphic(
    sequence: &mut Sequence,
    track_id: &str,
    clip: Clip,
) -> Result<String, GraphicsError> {
    let id = clip.id.clone();
    let track = sequence
        .track_mut(track_id)
        .ok_or(GraphicsError::TrackNotFound)?;
    if track.kind != TrackKind::Video {
        return Err(GraphicsError::NotVideoTrack);
    }
    if track.locked {
        return Err(GraphicsError::TrackLocked);
    }
    track.clips.push(clip);
    Ok(id)
}

fn graphic_clip(name: String, timeline_in: i64, timeline_out: i64, effect: EffectInstance) -> Clip {
    Clip {
        id: new_id("clip"),
        name,
        media_id: None,
        nested_sequence_id: None,
        kind: ClipKind::Title,
        timeline_in,
        timeline_out,
        source_in: 0,
        source_out: timeline_out.saturating_sub(timeline_in),
        speed: 1.0,
        reversed: false,
        enabled: true,
        linked_ids: Vec::new(),
        effects: vec![effect],
        label_color: 0,
    }
}

fn effect(effect_id: &str, params: Vec<Param>) -> EffectInstance {
    EffectInstance {
        id: new_id("fx"),
        effect_id: effect_id.to_string(),
        enabled: true,
        params,
    }
}

fn style_params(style: &TextStyle) -> Vec<Param> {
    vec![
        p_str(param_id::FONT_FAMILY, style.font_family.clone()),
        p_float(param_id::FONT_SIZE, style.font_size),
        p_color(param_id::COLOR, style.color),
        p_str(param_id::ALIGN, style.align.as_str()),
        p_point(param_id::POSITION, style.position),
        p_float(param_id::OPACITY, style.opacity),
    ]
}

fn read_style(effect: &EffectInstance) -> Result<TextStyle, GraphicsError> {
    let defaults = if effect.effect_id == LOWER_THIRD_EFFECT_ID {
        TextStyle::lower_third()
    } else {
        TextStyle::title()
    };
    Ok(TextStyle {
        font_family: str_or(effect, param_id::FONT_FAMILY, &defaults.font_family)?,
        font_size: float_or(effect, param_id::FONT_SIZE, defaults.font_size)?,
        color: color_or(effect, param_id::COLOR, defaults.color)?,
        align: TextAlign::parse(&str_or(effect, param_id::ALIGN, defaults.align.as_str())?),
        position: point_or(effect, param_id::POSITION, defaults.position)?,
        opacity: float_or(effect, param_id::OPACITY, defaults.opacity)?,
    })
}

fn validate_span(timeline_in: i64, timeline_out: i64) -> Result<(), GraphicsError> {
    if timeline_in < 0 || timeline_out <= timeline_in {
        return Err(GraphicsError::InvalidRange);
    }
    Ok(())
}

fn validate_style(style: &TextStyle) -> Result<(), GraphicsError> {
    if !(style.font_size.is_finite() && style.font_size > 0.0) {
        return Err(GraphicsError::InvalidFontSize);
    }
    if !(style.opacity.is_finite() && (0.0..=1.0).contains(&style.opacity)) {
        return Err(GraphicsError::InvalidOpacity);
    }
    if !style.position.x.is_finite() || !style.position.y.is_finite() {
        return Err(GraphicsError::InvalidPosition);
    }
    Ok(())
}

fn validate_bar_height(bar_height: f64) -> Result<(), GraphicsError> {
    if !(bar_height.is_finite() && (0.0..=1.0).contains(&bar_height)) {
        return Err(GraphicsError::InvalidBarHeight);
    }
    Ok(())
}

fn require_effect<'a>(
    clip: &'a Clip,
    effect_id: &str,
    missing: GraphicsError,
) -> Result<&'a EffectInstance, GraphicsError> {
    clip.effects
        .iter()
        .find(|fx| fx.effect_id == effect_id)
        .ok_or(missing)
}

fn required_str(effect: &EffectInstance, id: &'static str) -> Result<String, GraphicsError> {
    match param_value(effect, id) {
        Some(ParamValue::Str(value)) => Ok(value.clone()),
        _ => Err(GraphicsError::InvalidGraphicParam(id)),
    }
}

fn str_or(
    effect: &EffectInstance,
    id: &'static str,
    default: &str,
) -> Result<String, GraphicsError> {
    match param_value(effect, id) {
        Some(ParamValue::Str(value)) => Ok(value.clone()),
        None => Ok(default.to_string()),
        Some(_) => Err(GraphicsError::InvalidGraphicParam(id)),
    }
}

fn float_or(effect: &EffectInstance, id: &'static str, default: f64) -> Result<f64, GraphicsError> {
    match param_value(effect, id) {
        Some(ParamValue::Float(value)) => Ok(*value),
        None => Ok(default),
        Some(_) => Err(GraphicsError::InvalidGraphicParam(id)),
    }
}

fn color_or(
    effect: &EffectInstance,
    id: &'static str,
    default: Rgba,
) -> Result<Rgba, GraphicsError> {
    match param_value(effect, id) {
        Some(ParamValue::Color(value)) => Ok(*value),
        None => Ok(default),
        Some(_) => Err(GraphicsError::InvalidGraphicParam(id)),
    }
}

fn point_or(
    effect: &EffectInstance,
    id: &'static str,
    default: Point2,
) -> Result<Point2, GraphicsError> {
    match param_value(effect, id) {
        Some(ParamValue::Point(value)) => Ok(*value),
        None => Ok(default),
        Some(_) => Err(GraphicsError::InvalidGraphicParam(id)),
    }
}

fn param_value<'a>(effect: &'a EffectInstance, id: &str) -> Option<&'a ParamValue> {
    effect
        .params
        .iter()
        .find(|param| param.id == id)
        .map(|param| &param.value)
}

fn set_param(effect: &mut EffectInstance, id: &str, value: ParamValue) {
    if let Some(param) = effect.params.iter_mut().find(|param| param.id == id) {
        param.value = value;
    } else {
        effect.params.push(Param {
            id: id.to_string(),
            value,
            keyframes: Vec::new(),
        });
    }
}

fn p_str(id: &str, value: impl Into<String>) -> Param {
    Param {
        id: id.to_string(),
        value: ParamValue::Str(value.into()),
        keyframes: Vec::new(),
    }
}

fn p_float(id: &str, value: f64) -> Param {
    Param {
        id: id.to_string(),
        value: ParamValue::Float(value),
        keyframes: Vec::new(),
    }
}

fn p_color(id: &str, value: Rgba) -> Param {
    Param {
        id: id.to_string(),
        value: ParamValue::Color(value),
        keyframes: Vec::new(),
    }
}

fn p_point(id: &str, value: Point2) -> Param {
    Param {
        id: id.to_string(),
        value: ParamValue::Point(value),
        keyframes: Vec::new(),
    }
}

fn white() -> Rgba {
    Rgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::{SequenceSettings, Track, TrackKind, TICKS_PER_SECOND};

    fn video_sequence() -> (Sequence, String) {
        let mut sequence = Sequence::new("seq", SequenceSettings::hd1080p30());
        let track = Track::new(0, TrackKind::Video);
        let id = track.id.clone();
        sequence.video_tracks.push(track);
        (sequence, id)
    }

    #[test]
    fn title_round_trip_keeps_text_and_style() {
        let spec = TitleSpec::new("Opening", 0, TICKS_PER_SECOND);
        let clip = create_title_clip(&spec).unwrap();
        assert_eq!(clip.kind, ClipKind::Title);
        assert_eq!(clip.name, "Opening");
        assert_eq!(clip.source_out, TICKS_PER_SECOND);
        let content = read_title(&clip).unwrap();
        assert_eq!(content.text, "Opening");
        assert_eq!(content.style, TextStyle::title());
        assert!(read_lower_third(&clip).is_err());
    }

    #[test]
    fn lower_third_sits_in_the_lower_half_and_keeps_both_lines() {
        let spec = LowerThirdSpec::new("Aoki", "Editor", 0, TICKS_PER_SECOND);
        let clip = create_lower_third_clip(&spec).unwrap();
        let content = read_lower_third(&clip).unwrap();
        assert_eq!(content.primary, "Aoki");
        assert_eq!(content.secondary, "Editor");
        assert!(content.style.position.y > 0.5);
        assert_eq!(content.style.align, TextAlign::Left);
        assert!(read_title(&clip).is_err());
    }

    #[test]
    fn editing_text_keeps_the_other_parameters() {
        let mut title = create_title_clip(&TitleSpec::new("Before", 0, TICKS_PER_SECOND)).unwrap();
        set_title_text(&mut title, "After").unwrap();
        assert_eq!(title.name, "After");
        assert_eq!(read_title(&title).unwrap().text, "After");
        assert_eq!(read_title(&title).unwrap().style.font_size, 64.0);

        let mut lower =
            create_lower_third_clip(&LowerThirdSpec::new("Name", "Role", 0, TICKS_PER_SECOND))
                .unwrap();
        let accent = read_lower_third(&lower).unwrap().accent;
        set_lower_third_text(&mut lower, "New", "Updated").unwrap();
        let content = read_lower_third(&lower).unwrap();
        assert_eq!(content.primary, "New");
        assert_eq!(content.secondary, "Updated");
        assert_eq!(content.accent, accent);
        assert_eq!(lower.name, "New");
        assert!(set_title_text(&mut lower, "nope").is_err());
        assert!(set_lower_third_text(&mut title, "nope", "nope").is_err());
    }

    #[test]
    fn add_title_rejects_audio_locked_missing_and_inverted_ranges() {
        let (mut sequence, video_id) = video_sequence();
        let mut audio = Track::new(0, TrackKind::Audio);
        let audio_id = audio.id.clone();
        audio.locked = false;
        sequence.audio_tracks.push(audio);
        let spec = TitleSpec::new("Title", 0, TICKS_PER_SECOND);
        assert_eq!(
            add_title(&mut sequence, "missing", spec.clone()).unwrap_err(),
            GraphicsError::TrackNotFound
        );
        assert_eq!(
            add_title(&mut sequence, &audio_id, spec.clone()).unwrap_err(),
            GraphicsError::NotVideoTrack
        );
        sequence.track_mut(&video_id).unwrap().locked = true;
        assert_eq!(
            add_title(&mut sequence, &video_id, spec.clone()).unwrap_err(),
            GraphicsError::TrackLocked
        );
        assert!(sequence.video_tracks[0].clips.is_empty());
        sequence.track_mut(&video_id).unwrap().locked = false;
        assert!(add_title(&mut sequence, &video_id, TitleSpec::new("Bad", 10, 10)).is_err());
        let id = add_title(&mut sequence, &video_id, spec).unwrap();
        add_lower_third(
            &mut sequence,
            &video_id,
            LowerThirdSpec::new("Name", "Role", 0, TICKS_PER_SECOND),
        )
        .unwrap();
        assert_eq!(sequence.video_tracks[0].clips.len(), 2);
        assert_eq!(sequence.video_tracks[0].clips[0].id, id);
    }

    #[test]
    fn title_serializes_with_the_shared_model_schema() {
        let clip = create_title_clip(&TitleSpec::new("Hello", 0, TICKS_PER_SECOND)).unwrap();
        let json = serde_json::to_value(&clip).unwrap();
        assert_eq!(json["kind"], "title");
        assert_eq!(json["timelineIn"], 0);
        assert_eq!(json["effects"][0]["effectId"], TITLE_EFFECT_ID);
        let params = json["effects"][0]["params"].as_array().unwrap();
        let text = params.iter().find(|param| param["id"] == "text").unwrap();
        assert_eq!(text["value"]["type"], "str");
        assert_eq!(text["value"]["value"], "Hello");
        let restored: Clip = serde_json::from_value(json).unwrap();
        assert_eq!(read_title(&restored).unwrap().text, "Hello");
    }

    #[test]
    fn missing_style_params_use_defaults_and_invalid_style_is_rejected() {
        let mut clip = create_title_clip(&TitleSpec::new("Hello", 0, TICKS_PER_SECOND)).unwrap();
        clip.effects[0]
            .params
            .retain(|param| param.id != param_id::FONT_SIZE);
        assert_eq!(read_title(&clip).unwrap().style.font_size, 64.0);

        let mut spec = TitleSpec::new("Hello", 0, TICKS_PER_SECOND);
        spec.style.font_size = 0.0;
        assert_eq!(
            create_title_clip(&spec).unwrap_err(),
            GraphicsError::InvalidFontSize
        );
        spec.style = TextStyle::title();
        spec.style.opacity = 1.5;
        assert_eq!(
            create_title_clip(&spec).unwrap_err(),
            GraphicsError::InvalidOpacity
        );
        spec.style = TextStyle::title();
        spec.style.position.x = f64::NAN;
        assert_eq!(
            create_title_clip(&spec).unwrap_err(),
            GraphicsError::InvalidPosition
        );
        let mut lower = LowerThirdSpec::new("A", "B", 0, TICKS_PER_SECOND);
        lower.bar_height = 2.0;
        assert_eq!(
            create_lower_third_clip(&lower).unwrap_err(),
            GraphicsError::InvalidBarHeight
        );
    }

    #[test]
    fn media_clip_is_not_a_title() {
        let clip = Clip::media("clip", "media-1");
        assert_eq!(read_title(&clip).unwrap_err(), GraphicsError::NotTitle);
    }
}
