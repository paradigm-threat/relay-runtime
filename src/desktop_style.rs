//! Normalize resolved web-style maps into desktop-ready px strings.
//!
//! Desktop glue (`relay-runtime-desktop`) expects numeric px values and font tokens.

use std::collections::HashMap;

pub const FONT_SANS: &str = "relay-system-sans";
pub const FONT_MONO: &str = "relay-system-mono";

const REM_BASE: f32 = 16.0;

const TEXT_BODY_TAGS: &[&str] = &["p", "li", "h1", "h2", "h3", "h4", "h5", "h6", "pre", "blockquote"];

const BLOCK_TAGS: &[&str] = &[
    "div", "p", "h1", "h2", "h3", "h4", "h5", "h6", "main", "section", "article", "header",
    "footer", "aside", "nav", "ul", "ol", "li", "pre", "blockquote", "code",
];

/// Block-level tags that should fill their flex parent on desktop (mirrors themed-styler UA defaults).
pub fn is_desktop_block_tag(tag: &str) -> bool {
    BLOCK_TAGS.iter().any(|t| t.eq_ignore_ascii_case(tag))
}

/// Materialize browser-like computed defaults missing from flat inline style resolution.
pub fn apply_desktop_ui_defaults(
    tag: &str,
    style: &mut HashMap<String, String>,
    parent_font_px: f32,
) {
    if !style.contains_key("fontSize") {
        let px = if parent_font_px > 0.0 {
            parent_font_px
        } else {
            REM_BASE
        };
        style.insert("fontSize".to_string(), format_px(px));
    }
    if !style.contains_key("width") && is_desktop_block_tag(tag) {
        style.insert("width".to_string(), "100%".to_string());
    }
    if !style.contains_key("lineHeight")
        && TEXT_BODY_TAGS.iter().any(|t| t.eq_ignore_ascii_case(tag))
    {
        style.insert("lineHeight".to_string(), "1.5".to_string());
    }
}

/// Convert resolved inline styles to desktop px props.
pub fn normalize_desktop_styles(
    style: HashMap<String, String>,
    parent_font_px: f32,
) -> HashMap<String, String> {
    let parent_font = if parent_font_px > 0.0 {
        parent_font_px
    } else {
        REM_BASE
    };
    let mut out = HashMap::with_capacity(style.len());
    for (k, v) in style {
        let val = v.trim().to_string();
        let normalized = match k.as_str() {
            "fontSize" => normalize_font_size(&val, parent_font),
            "lineHeight" => normalize_line_height(&val, parent_font),
            "fontWeight" => normalize_font_weight(&val),
            "fontFamily" => normalize_font_family(&val),
            "width" | "height" | "minWidth" | "minHeight" | "maxWidth" | "maxHeight"
            | "padding" | "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft"
            | "margin" | "marginTop" | "marginRight" | "marginBottom" | "marginLeft"
            | "borderWidth" | "borderTopWidth" | "borderRightWidth" | "borderBottomWidth"
            | "borderLeftWidth" | "borderRadius" | "gap" | "rowGap" | "columnGap"
            | "letterSpacing" | "top" | "right" | "bottom" | "left" => {
                normalize_length(&val, parent_font)
            }
            _ => val,
        };
        if !normalized.is_empty() {
            out.insert(k, normalized);
        }
    }
    expand_box_model_shorthands(&mut out, parent_font);
    out
}

/// Font size in px after normalization (for parent cascade).
pub fn font_size_px_from_style(style: &HashMap<String, String>, parent_font_px: f32) -> f32 {
    style
        .get("fontSize")
        .and_then(|s| parse_length_px(s, parent_font_px))
        .unwrap_or(if parent_font_px > 0.0 {
            parent_font_px
        } else {
            REM_BASE
        })
}

fn normalize_font_size(value: &str, parent_font_px: f32) -> String {
    parse_length_px(value, parent_font_px)
        .map(format_px)
        .unwrap_or_else(|| format_px(REM_BASE))
}

fn normalize_line_height(value: &str, parent_font_px: f32) -> String {
    let trimmed = value.trim();
    if let Ok(n) = trimmed.parse::<f32>() {
        return format_px(n * parent_font_px);
    }
    parse_length_px(trimmed, parent_font_px)
        .map(format_px)
        .unwrap_or_else(|| value.to_string())
}

fn normalize_font_weight(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "bold" | "bolder" => "700".to_string(),
        "normal" | "lighter" => "400".to_string(),
        other => {
            if other.parse::<f32>().is_ok() {
                other.to_string()
            } else {
                "400".to_string()
            }
        }
    }
}

fn normalize_font_family(value: &str) -> String {
    let lower = value.to_lowercase();
    if lower.contains("mono") {
        FONT_MONO.to_string()
    } else {
        FONT_SANS.to_string()
    }
}

fn normalize_length(value: &str, parent_font_px: f32) -> String {
    if value == "100%" {
        return value.to_string();
    }
    if value == "auto" || value.ends_with('%') {
        return value.to_string();
    }
    parse_length_px(value, parent_font_px)
        .map(format_px)
        .unwrap_or_else(|| value.to_string())
}

fn parse_length_px(value: &str, parent_font_px: f32) -> Option<f32> {
    let trimmed = value.trim();
    if trimmed == "100%" || trimmed == "auto" {
        return None;
    }
    if trimmed.ends_with("rem") {
        let rem = trimmed.trim_end_matches("rem").trim().parse::<f32>().ok()?;
        return Some(rem * REM_BASE);
    }
    if trimmed.ends_with("em") {
        let em = trimmed.trim_end_matches("em").trim().parse::<f32>().ok()?;
        return Some(em * parent_font_px);
    }
    if trimmed.ends_with("px") {
        return trimmed.trim_end_matches("px").trim().parse().ok();
    }
    trimmed.parse().ok()
}

fn format_px(px: f32) -> String {
    if (px - px.round()).abs() < 0.01 {
        format!("{}px", px.round() as i32)
    } else {
        format!("{px:.2}px")
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct BoxEdges {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

/// Expand CSS padding/margin shorthands into single-value side props.
///
/// Themed-styler may copy a multi-token shorthand (e.g. `"14px 20px"`) onto each side key;
/// drop those invalid side entries and materialize correct per-side px strings.
fn expand_box_model_shorthands(styles: &mut HashMap<String, String>, parent_font_px: f32) {
    for key in [
        "paddingTop",
        "paddingRight",
        "paddingBottom",
        "paddingLeft",
        "marginTop",
        "marginRight",
        "marginBottom",
        "marginLeft",
    ] {
        if styles
            .get(key)
            .is_some_and(|v| v.split_whitespace().count() > 1)
        {
            styles.remove(key);
        }
    }
    expand_box_shorthand(styles, "padding", parent_font_px, [
        "paddingTop",
        "paddingRight",
        "paddingBottom",
        "paddingLeft",
    ]);
    expand_box_shorthand(styles, "margin", parent_font_px, [
        "marginTop",
        "marginRight",
        "marginBottom",
        "marginLeft",
    ]);
}

fn expand_box_shorthand(
    styles: &mut HashMap<String, String>,
    shorthand: &str,
    parent_font_px: f32,
    sides: [&str; 4],
) {
    let Some(raw) = styles.get(shorthand).cloned() else {
        return;
    };
    let edges = parse_box_shorthand(&raw, parent_font_px);
    styles.insert(sides[0].into(), format_px(edges.top));
    styles.insert(sides[1].into(), format_px(edges.right));
    styles.insert(sides[2].into(), format_px(edges.bottom));
    styles.insert(sides[3].into(), format_px(edges.left));
    styles.remove(shorthand);
}

fn parse_box_shorthand(raw: &str, parent_font_px: f32) -> BoxEdges {
    let parts: Vec<_> = raw.split_whitespace().collect();
    let px = |token: &str| parse_length_px(token, parent_font_px).unwrap_or(0.0);
    match parts.len() {
        1 => {
            let v = px(parts[0]);
            BoxEdges {
                top: v,
                right: v,
                bottom: v,
                left: v,
            }
        }
        2 => {
            let v = px(parts[0]);
            let h = px(parts[1]);
            BoxEdges {
                top: v,
                right: h,
                bottom: v,
                left: h,
            }
        }
        3 => {
            let t = px(parts[0]);
            let h = px(parts[1]);
            let b = px(parts[2]);
            BoxEdges {
                top: t,
                right: h,
                bottom: b,
                left: h,
            }
        }
        _ if parts.len() >= 4 => BoxEdges {
            top: px(parts[0]),
            right: px(parts[1]),
            bottom: px(parts[2]),
            left: px(parts[3]),
        },
        _ => BoxEdges::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_shorthand_expands_to_single_value_sides() {
        let s = HashMap::from([
            ("padding".to_string(), "14px 20px".to_string()),
            ("paddingLeft".to_string(), "14px 20px".to_string()),
            ("paddingTop".to_string(), "14px 20px".to_string()),
        ]);
        let out = normalize_desktop_styles(s, REM_BASE);
        assert_eq!(out.get("paddingTop").map(String::as_str), Some("14px"));
        assert_eq!(out.get("paddingBottom").map(String::as_str), Some("14px"));
        assert_eq!(out.get("paddingLeft").map(String::as_str), Some("20px"));
        assert_eq!(out.get("paddingRight").map(String::as_str), Some("20px"));
        assert!(!out.contains_key("padding"));
    }

    #[test]
    fn rem_font_size_to_px() {
        let mut s = HashMap::from([("fontSize".to_string(), "0.875rem".to_string())]);
        let out = normalize_desktop_styles(s.clone(), REM_BASE);
        assert_eq!(out.get("fontSize").map(String::as_str), Some("14px"));
    }

    #[test]
    fn bold_weight_normalized() {
        let s = HashMap::from([("fontWeight".to_string(), "bold".to_string())]);
        let out = normalize_desktop_styles(s, REM_BASE);
        assert_eq!(out.get("fontWeight").map(String::as_str), Some("700"));
    }

    #[test]
    fn system_ui_maps_to_token() {
        let s = HashMap::from([(
            "fontFamily".to_string(),
            "system-ui, sans-serif".to_string(),
        )]);
        let out = normalize_desktop_styles(s, REM_BASE);
        assert_eq!(out.get("fontFamily").map(String::as_str), Some(FONT_SANS));
    }

    #[test]
    fn max_width_normalized() {
        let s = HashMap::from([("maxWidth".to_string(), "720px".to_string())]);
        let out = normalize_desktop_styles(s, REM_BASE);
        assert_eq!(out.get("maxWidth").map(String::as_str), Some("720px"));
    }

    #[test]
    fn apply_defaults_materializes_font_and_block_width() {
        let mut style = HashMap::from([("color".to_string(), "#111".to_string())]);
        apply_desktop_ui_defaults("p", &mut style, 16.0);
        assert_eq!(style.get("fontSize").map(String::as_str), Some("16px"));
        assert_eq!(style.get("width").map(String::as_str), Some("100%"));
    }

    #[test]
    fn apply_defaults_skips_existing_font_size() {
        let mut style = HashMap::from([("fontSize".to_string(), "14px".to_string())]);
        apply_desktop_ui_defaults("span", &mut style, 16.0);
        assert_eq!(style.get("fontSize").map(String::as_str), Some("14px"));
        assert!(!style.contains_key("width"));
    }
}
