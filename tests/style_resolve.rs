//! Style resolution tests: className → resolved style in Rust (not Kotlin).
//!
//! Style translation occurs in:
//! - relay-runtime: style_resolve::resolve_vnode_styles()
//! - themed-styler: State::web_styles_for() (Web), State::android_styles_for() (native)
//!
//! Platform glue only APPLIES pre-resolved style props; it does not
//! translate className or convert units.

use relay_runtime::{
    analyze, parse, runtime::Runtime, style_resolve::*, value_to_vnode, vdom::*,
};
use std::collections::HashMap;

/// Minimal theme with .bg-primary and .p-4-like selectors for testing.
/// Use r##"..."## so "#3b82f6" inside the string does not terminate the raw literal.
const MINIMAL_THEME: &str = r##"
themes:
  light:
    variables:
      primary: "#3b82f6"
    selectors:
      ".bg-primary":
        backgroundColor: "var(primary)"
      ".p-4":
        padding: 16
current_theme: light
"##;

fn vnode_with_classname(tag: &str, className: &str, children: Vec<VNode>) -> VNode {
    let mut props = HashMap::new();
    props.insert("className".to_string(), PropValue::String(className.to_string()));
    VNode::Element {
        tag: tag.to_string(),
        props,
        children,
        key: None,
    }
}

#[test]
fn style_resolution_removes_classname_on_android() {
    // Build VNode with className
    let vnode = vnode_with_classname("div", "flex p-4", vec![]);

    let (resolved, css, state_json) =
        resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, None, false);

    assert!(css.is_none());
    assert!(state_json.is_some()); // For getThemes

    match &resolved {
        VNode::Element { props, .. } => {
            // className MUST be removed (resolved into style)
            assert!(
                !props.contains_key("className"),
                "className must be removed after resolution"
            );
            // style MUST be present with resolved values
            assert!(
                props.contains_key("style"),
                "resolved style must be present"
            );
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_produces_expected_keys() {
    let vnode = vnode_with_classname("div", "flex gap-4", vec![]);

    let (resolved, _, _) = resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, None, false);

    match &resolved {
        VNode::Element { props, .. } => {
            let style = props
                .get("style")
                .and_then(|p| {
                    if let PropValue::Style(m) = p {
                        Some(m)
                    } else {
                        None
                    }
                })
                .expect("style prop");

            // flex → flexDirection, androidOrientation
            assert!(
                style.contains_key("flexDirection") || style.contains_key("androidOrientation"),
                "flex class must produce layout keys: {:?}",
                style.keys().collect::<Vec<_>>()
            );
            // gap-4 → gap (in px after conversion)
            assert!(
                style.contains_key("gap"),
                "gap-4 must produce gap: {:?}",
                style
            );
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_theme_selectors() {
    let vnode = vnode_with_classname("div", "bg-primary", vec![]);

    let (resolved, _, _) = resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, None, false);

    match &resolved {
        VNode::Element { props, .. } => {
            let style = props
                .get("style")
                .and_then(|p| {
                    if let PropValue::Style(m) = p {
                        Some(m)
                    } else {
                        None
                    }
                })
                .expect("style prop");

            assert!(
                style.contains_key("backgroundColor"),
                ".bg-primary from theme must resolve to backgroundColor: {:?}",
                style
            );
            let bg = style.get("backgroundColor").map(|s| s.as_str());
            assert_eq!(bg, Some("#3b82f6"), "primary var must resolve to hex");
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_web_same_as_native() {
    let vnode = vnode_with_classname("div", "flex bg-primary", vec![]);

    let (resolved, css, state_json) =
        resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Web, 1.0, None, false);

    assert!(css.is_none());
    assert!(state_json.is_some());

    match &resolved {
        VNode::Element { props, .. } => {
            // Web also removes className (flat inline style, no cascading)
            assert!(!props.contains_key("className"));
            assert!(props.contains_key("style"));
            if let Some(PropValue::Style(m)) = props.get("style") {
                assert!(m.contains_key("backgroundColor"));
                assert!(m.contains_key("flexDirection"));
            }
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_nested_children() {
    let child = vnode_with_classname("span", "text-base", vec![VNode::Text("hello".into())]);
    let parent = vnode_with_classname("div", "flex", vec![child]);

    let (resolved, _, _) = resolve_vnode_styles(parent, MINIMAL_THEME, Platform::Android, 1.0, None, false);

    match &resolved {
        VNode::Element { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                VNode::Element { props, .. } => {
                    assert!(!props.contains_key("className"));
                    assert!(props.contains_key("style"));
                    if let Some(PropValue::Style(m)) = props.get("style") {
                        assert!(
                            m.contains_key("fontSize"),
                            "text-base must produce fontSize: {:?}",
                            m
                        );
                    }
                }
                _ => panic!("expected child Element"),
            }
        }
        _ => panic!("expected parent Element"),
    }
}

#[test]
fn style_resolution_jsx_to_resolved_flow() {
    // Full flow: JSX → Value → VNode → resolve
    let source = r#"
export default function App() {
  return <div className="flex p-4 bg-primary"><span>hi</span></div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run().unwrap();
    let vnode = value_to_vnode(&result).normalize();

    let (resolved, _, _) = resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, None, false);

    match &resolved {
        VNode::Element { tag, props, children, .. } => {
            assert_eq!(tag, "div");
            assert!(!props.contains_key("className"));
            assert!(props.contains_key("style"));
            if let Some(PropValue::Style(m)) = props.get("style") {
                assert!(m.contains_key("backgroundColor"), "bg-primary resolved");
                assert!(
                    m.contains_key("padding") || m.contains_key("paddingTop"),
                    "p-4 resolved: {:?}",
                    m.keys().collect::<Vec<_>>()
                );
                assert!(
                    m.contains_key("flexDirection") || m.contains_key("androidOrientation"),
                    "flex resolved"
                );
            }
            assert_eq!(children.len(), 1);
            // JSX <div><span>hi</span></div> → div's child is span (Element), not raw text
            match &children[0] {
                VNode::Element { tag, children: inner, .. } => {
                    assert_eq!(tag, "span");
                    assert_eq!(inner.len(), 1);
                    if let VNode::Text(s) = &inner[0] {
                        assert_eq!(s, "hi");
                    } else {
                        panic!("expected span to contain text");
                    }
                }
                VNode::Text(s) => assert_eq!(s, "hi"),
                _ => panic!("expected span or text child"),
            }
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_web_vs_android_same_base_keys() {
    let vnode = vnode_with_classname("div", "flex p-4 bg-primary gap-4", vec![]);

    let (web_resolved, _, _) = resolve_vnode_styles(vnode.clone(), MINIMAL_THEME, Platform::Web, 1.0, None, false);
    let (android_resolved, _, _) = resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, None, false);

    let web_style = match &web_resolved {
        VNode::Element { props, .. } => props.get("style").and_then(|p| {
            if let PropValue::Style(m) = p {
                Some(m)
            } else {
                None
            }
        }),
        _ => None,
    }
    .expect("web style");

    let android_style = match &android_resolved {
        VNode::Element { props, .. } => props.get("style").and_then(|p| {
            if let PropValue::Style(m) = p {
                Some(m)
            } else {
                None
            }
        }),
        _ => None,
    }
    .expect("android style");

    for key in &["backgroundColor", "flexDirection", "padding", "paddingTop", "gap"] {
        assert!(
            web_style.contains_key(*key),
            "Web must have {}: {:?}",
            key,
            web_style.keys().collect::<Vec<_>>()
        );
        assert!(
            android_style.contains_key(*key) || (key == &"padding" && android_style.contains_key("paddingTop")),
            "Android must have {} or equivalent: {:?}",
            key,
            android_style.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        web_style.get("backgroundColor"),
        android_style.get("backgroundColor"),
        "Both platforms resolve theme colors identically"
    );
}
