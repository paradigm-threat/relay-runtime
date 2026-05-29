//! Resolve className + tag → style in relay-runtime core.
//!
//! **All platforms use flat inline style resolution.** No cascading, no CSS
//! injection. Web acts like native: className → resolved style per element.
//!
//! ## Style translation is in Rust (not Kotlin/JS)
//!
//! All style translation happens here and in `themed-styler`. Platform glue
//! (Android VNodeRenderer.kt, web host) only **applies** pre-resolved style
//! props; it does not translate className, convert units, or resolve theme
//! variables.
//!
//! See docs/CROSS_PLATFORM_OS_SUPPORT_PLAN.md §2.

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

/// Resolve className → style for each element. All platforms get flat inline
/// style (no cascading). Returns (resolved VNode, None, optional state JSON for getThemes).
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
    let state = build_styler_state(
        theme_yaml,
        display_density,
        scaled_density,
        theme_name,
        prefers_dark,
    );
    let resolved = resolve_element_styles_rec(vnode, &state, platform);
    let state_json = serde_json::to_string(&state.to_json()).unwrap_or_default();
    (resolved, None, Some(state_json))
}

fn resolve_element_styles_rec(
    vnode: VNode,
    state: &themed_styler::State,
    platform: Platform,
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
            if let Some(ref cn) = class_name {
                let classes: Vec<String> = cn
                    .trim()
                    .split_whitespace()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                if !classes.is_empty() {
                    let tag_sel = if tag == "fragment" { "div" } else { &tag };
                    let styles = match platform {
                        Platform::Web => state.web_styles_for(tag_sel, &classes),
                        Platform::Android | Platform::Ios | Platform::Desktop => {
                            state.android_styles_for(tag_sel, &classes)
                        }
                    };
                    for (k, v) in styles {
                        let s = value_to_style_string(&v);
                        if !s.is_empty() {
                            resolved_style.insert(k, s);
                        }
                    }
                }
            }
            if let Some(inline) = inline_style {
                for (k, v) in inline {
                    resolved_style.insert(k, v);
                }
            }
            if !resolved_style.is_empty() {
                props.insert("style".to_string(), PropValue::Style(resolved_style));
            }
            props.remove("className");

            let children: Vec<VNode> = children
                .into_iter()
                .map(|c| resolve_element_styles_rec(c, state, platform))
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

