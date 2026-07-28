//! Resolve className + tag → style in relay-runtime core.
//!
//! ## Platform behavior
//!
//! - **Web**: Keep `className` on elements; emit a real stylesheet via
//!   `themed_styler::State::css_for_web` so SSR/no-JS browsers get media
//!   queries, `:hover`, and `prefers-color-scheme`. Author `style={{…}}` stays
//!   as inline style only.
//! - **Android / iOS / Desktop**: Flat inline style resolution (no cascading).
//!   Platform glue only **applies** pre-resolved style props.
//!
//! See docs/CROSS_PLATFORM_OS_SUPPORT_PLAN.md §2.

use crate::desktop_style::{
    apply_desktop_ui_defaults, font_size_px_from_style, normalize_desktop_styles,
};
use crate::vdom::{PropValue, VNode};
use serde_json;
use std::collections::HashMap;

/// Platform determines which styler variant (web vs android) for unit conversion.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Web,
    Android,
    Ios,
    Desktop,
}

impl Platform {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "web" => Platform::Web,
            "android" => Platform::Android,
            "ios" => Platform::Ios,
            "desktop" => Platform::Desktop,
            _ => Platform::Web,
        }
    }
}

/// Build single-theme CSS for a forced appearance (JS theme toggle).
/// Prefer this over `css_for_web` after `setCurrentTheme`.
pub fn css_for_forced_theme(theme_yaml: &str, theme_name: &str) -> String {
    let name = theme_name.trim().to_lowercase();
    if name.is_empty() {
        return String::new();
    }
    let prefers_dark = name == "dark";
    let state = build_styler_state(theme_yaml, 1.0, 1.0, Some(&name), prefers_dark);
    let css = state.css_for_web();
    if css.trim().is_empty() {
        String::new()
    } else {
        css
    }
}

/// Resolve className → style (native) or className + stylesheet (web).
///
/// Returns `(resolved VNode, optional CSS for web injection, optional state JSON)`.
/// - theme_name: "light", "dark", or "default" (uses prefers_dark for default)
/// - prefers_dark: when theme_name is "default", picks light vs dark
/// - scaled_density: Android `scaledDensity` for sp→px; often equals display_density on web.
pub fn resolve_vnode_styles(
    vnode: VNode,
    theme_yaml: &str,
    platform: Platform,
    display_density: f32,
    scaled_density: f32,
    theme_name: Option<&str>,
    prefers_dark: bool,
) -> (VNode, Option<String>, Option<String>) {
    let mut state = build_styler_state(
        theme_yaml,
        display_density,
        scaled_density,
        theme_name,
        prefers_dark,
    );
    let resolved = match platform {
        Platform::Web => {
            // Document chrome selectors apply even when the hook root is <main>.
            state.register_tags(["html".to_string(), "body".to_string()]);
            resolve_element_styles_web(vnode, &mut state)
        }
        other => resolve_element_styles_native(vnode, &state, other, 16.0),
    };
    let inject_css = match platform {
        Platform::Web => {
            let css = state.css_for_web();
            if css.trim().is_empty() {
                None
            } else {
                Some(css)
            }
        }
        _ => None,
    };
    let state_json = serde_json::to_string(&state.to_json()).unwrap_or_default();
    (resolved, inject_css, Some(state_json))
}

/// Web: keep className, record usage for CSS generation, pass through author inline styles only.
fn resolve_element_styles_web(vnode: VNode, state: &mut themed_styler::State) -> VNode {
    match vnode {
        VNode::Element {
            tag,
            props,
            children,
            key,
        } => {
            let class_name = props.get("className").and_then(|p| {
                if let PropValue::String(s) = p {
                    Some(s.clone())
                } else {
                    None
                }
            });
            let tag_sel = if tag == "fragment" {
                "div"
            } else {
                tag.as_str()
            };
            if tag != "fragment" {
                state.register_tags([tag_sel.to_string()]);
            }
            if let Some(ref cn) = class_name {
                let classes: Vec<String> = cn
                    .trim()
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                if !classes.is_empty() {
                    state.register_tailwind_classes(classes.iter().cloned());
                    for c in &classes {
                        state.register_tag_class(tag_sel, c.clone());
                    }
                }
            }
            // Keep author `style={{…}}`; do not flatten theme/class into inline.
            // className stays so the generated stylesheet can target it.

            let children: Vec<VNode> = children
                .into_iter()
                .map(|c| resolve_element_styles_web(c, state))
                .collect();

            VNode::Element {
                tag,
                props,
                children,
                key,
            }
        }
        other => other,
    }
}

/// Native/desktop: flatten className + theme into inline style; strip className.
fn resolve_element_styles_native(
    vnode: VNode,
    state: &themed_styler::State,
    platform: Platform,
    parent_font_px: f32,
) -> VNode {
    match vnode {
        VNode::Element {
            tag,
            mut props,
            children,
            key,
        } => {
            let class_name = props.get("className").and_then(|p| {
                if let PropValue::String(s) = p {
                    Some(s.clone())
                } else {
                    None
                }
            });
            let inline_style = props.remove("style").and_then(|p| {
                if let PropValue::Style(m) = p {
                    Some(m)
                } else {
                    None
                }
            });

            let mut resolved_style = HashMap::new();
            let tag_sel = if tag == "fragment" { "div" } else { &tag };
            let classes: Vec<String> = class_name
                .as_ref()
                .map(|cn| {
                    cn.trim()
                        .split_whitespace()
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();
            let styles = match platform {
                Platform::Desktop => state.web_styles_for(tag_sel, &classes),
                Platform::Android | Platform::Ios => state.android_styles_for(tag_sel, &classes),
                Platform::Web => unreachable!("web uses resolve_element_styles_web"),
            };
            for (k, v) in styles {
                let s = value_to_style_string(&v);
                if !s.is_empty() {
                    resolved_style.insert(k, s);
                }
            }
            if let Some(inline) = inline_style {
                for (k, v) in inline {
                    resolved_style.insert(k, v);
                }
            }

            if platform == Platform::Desktop {
                resolved_style = normalize_desktop_styles(resolved_style, parent_font_px);
                apply_desktop_ui_defaults(&tag, &mut resolved_style, parent_font_px);
            }

            let element_font_px = if platform == Platform::Desktop {
                font_size_px_from_style(&resolved_style, parent_font_px)
            } else {
                parent_font_px
            };

            if !resolved_style.is_empty() {
                props.insert("style".to_string(), PropValue::Style(resolved_style));
            }
            props.remove("className");

            let children: Vec<VNode> = children
                .into_iter()
                .map(|c| resolve_element_styles_native(c, state, platform, element_font_px))
                .collect();

            VNode::Element {
                tag,
                props,
                children,
                key,
            }
        }
        other => other,
    }
}

fn build_styler_state(
    theme_yaml: &str,
    display_density: f32,
    scaled_density: f32,
    theme_name: Option<&str>,
    prefers_dark: bool,
) -> themed_styler::State {
    let theme_json = if theme_yaml.is_empty() {
        "{}".to_string()
    } else {
        serde_yaml::from_str::<serde_json::Value>(theme_yaml)
            .ok()
            .and_then(|v| serde_json::to_string(&v).ok())
            .unwrap_or_else(|| "{}".to_string())
    };
    let mut state = themed_styler::build_state_from_theme_json(&theme_json);
    state.display_density = display_density;
    state.scaled_density = scaled_density;
    if let Some(name) = theme_name {
        let name = name.trim().to_lowercase();
        if !name.is_empty() && state.themes.contains_key(&name) {
            state.current_theme = name.clone();
        }
    }
    state.prefers_color_scheme = Some(if prefers_dark { "dark" } else { "light" }.to_string());
    state
}

fn value_to_style_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}
