//! Style resolution tests: className → resolved style in Rust (not Kotlin).
//!
//! Style translation occurs in:
//! - relay-runtime: style_resolve::resolve_vnode_styles()
//! - themed-styler: State::web_styles_for() (Web), State::android_styles_for() (native)
//!
//! Platform glue only APPLIES pre-resolved style props; it does not
//! translate className or convert units.

use relay_runtime::{
    analyze, parse, runtime::Runtime, run_state::{clear_handler_registry_for_vnode_pass, clear_run_state, run_hook_persistent, register_handler},
    style_resolve::*, value::Value, value_to_vnode, vdom::{value_to_vnode_with_handlers, *}, Host, HostHandle,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

struct NullHost;

impl Host for NullHost {
    fn fetch(&self, _url: &str, _options: Option<&Value>) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn navigate(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }
    fn create_element(
        &self,
        _tag: &str,
        _props: &Value,
        _children: &[Value],
    ) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn load_module(&self, _path: &str) -> Result<Value, String> {
        Err("no modules".into())
    }
}

fn null_host() -> HostHandle {
    Rc::new(RefCell::new(NullHost))
}

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

/// Two named themes to verify `theme_name` selects the correct entry.
const THEME_LIGHT_DARK: &str = r##"
themes:
  light:
    selectors:
      ".tone":
        color: "#111111"
  dark:
    selectors:
      ".tone":
        color: "#eeeeee"
current_theme: light
"##;

#[test]
fn web_dual_theme_css_emits_html_dark_and_light_prefixes() {
    let vnode = vnode_with_classname("div", "tone", vec![]);
    let (_resolved, css, _) = resolve_vnode_styles(
        vnode,
        THEME_LIGHT_DARK,
        Platform::Web,
        1.0,
        1.0,
        Some("light"),
        false,
    );
    let css = css.expect("web dual css");
    assert!(
        css.contains("html.dark") && css.contains("html.light"),
        "css_for_web_system must force both html.dark and html.light: {css}"
    );
    assert!(
        css.contains("prefers-color-scheme: dark"),
        "system dual CSS must include prefers-color-scheme: {css}"
    );
    assert!(
        css.contains("#111111") && css.contains("#eeeeee"),
        "both theme colors must appear: {css}"
    );
}

#[test]
fn android_theme_name_selects_dark_flat_styles() {
    let vnode = vnode_with_classname("div", "tone", vec![]);
    let (light_vn, light_css, _) = resolve_vnode_styles(
        vnode.clone(),
        THEME_LIGHT_DARK,
        Platform::Android,
        1.0,
        1.0,
        Some("light"),
        false,
    );
    let (dark_vn, dark_css, _) = resolve_vnode_styles(
        vnode,
        THEME_LIGHT_DARK,
        Platform::Android,
        1.0,
        1.0,
        Some("dark"),
        true,
    );
    assert!(light_css.is_none(), "Android must not inject CSS sheets");
    assert!(dark_css.is_none(), "Android must not inject CSS sheets");
    let light_color = match &light_vn {
        VNode::Element { props, .. } => props.get("style").and_then(|p| {
            if let PropValue::Style(m) = p {
                m.get("color").cloned()
            } else {
                None
            }
        }),
        _ => None,
    };
    let dark_color = match &dark_vn {
        VNode::Element { props, .. } => props.get("style").and_then(|p| {
            if let PropValue::Style(m) = p {
                m.get("color").cloned()
            } else {
                None
            }
        }),
        _ => None,
    };
    assert_ne!(
        light_color, dark_color,
        "SetThemeAndRender path depends on theme_name flipping flat styles; light={light_color:?} dark={dark_color:?}"
    );
}

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
        resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, 1.0, None, false);

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

    let (resolved, _, _) = resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, 1.0, None, false);

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

    let (resolved, _, _) = resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, 1.0, None, false);

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
fn style_resolution_web_keeps_classname_and_emits_css() {
    let vnode = vnode_with_classname("div", "flex bg-primary", vec![]);

    let (resolved, css, state_json) =
        resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Web, 1.0, 1.0, None, false);

    assert!(state_json.is_some());
    let css = css.expect("web must emit stylesheet");
    assert!(
        css.contains(".bg-primary") || css.contains("background-color"),
        "CSS should include theme class rules: {css}"
    );
    assert!(
        css.contains(".flex") || css.contains("display:flex") || css.contains("flex-direction"),
        "CSS should include flex utility: {css}"
    );

    match &resolved {
        VNode::Element { props, .. } => {
            assert!(
                props.contains_key("className"),
                "web must keep className for stylesheet targeting"
            );
            // Theme/class styles are in CSS, not flattened into inline style.
            if let Some(PropValue::Style(m)) = props.get("style") {
                assert!(
                    !m.contains_key("backgroundColor"),
                    "web must not flatten theme colors into inline style: {:?}",
                    m
                );
            }
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_nested_children() {
    let child = vnode_with_classname("span", "text-base", vec![VNode::Text("hello".into())]);
    let parent = vnode_with_classname("div", "flex", vec![child]);

    let (resolved, _, _) = resolve_vnode_styles(parent, MINIMAL_THEME, Platform::Android, 1.0, 1.0, None, false);

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

    let (resolved, _, _) = resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, 1.0, None, false);

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
fn style_resolution_web_vs_android_theme_parity_via_css() {
    let vnode = vnode_with_classname("div", "flex p-4 bg-primary gap-4", vec![]);

    let (web_resolved, web_css, _) =
        resolve_vnode_styles(vnode.clone(), MINIMAL_THEME, Platform::Web, 1.0, 1.0, None, false);
    let (android_resolved, _, _) =
        resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Android, 1.0, 1.0, None, false);

    let web_css = web_css.expect("web css");
    assert!(web_css.contains("background-color") || web_css.contains(".bg-primary"));

    match &web_resolved {
        VNode::Element { props, .. } => {
            assert!(props.contains_key("className"));
        }
        _ => panic!("expected web Element"),
    }

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

    assert!(
        android_style.contains_key("backgroundColor"),
        "Android must flatten theme: {:?}",
        android_style.keys().collect::<Vec<_>>()
    );
    assert!(
        android_style.contains_key("flexDirection") || android_style.contains_key("androidOrientation"),
        "Android must flatten flex: {:?}",
        android_style.keys().collect::<Vec<_>>()
    );
}

#[test]
fn style_resolution_theme_name_selects_dark() {
    let vnode = vnode_with_classname("div", "tone", vec![]);
    let (resolved, _, _) =
        resolve_vnode_styles(vnode, THEME_LIGHT_DARK, Platform::Android, 1.0, 1.0, Some("dark"), false);
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
                .expect("style");
            assert_eq!(style.get("color").map(|s| s.as_str()), Some("#eeeeee"));
        }
        _ => panic!("expected Element"),
    }
}

const FAN_EDITS_LIBRARY_THEME: &str = r##"
themes:
  light:
    selectors:
      ".library-shell":
        display: "flex"
        flexDirection: "row"
        backgroundColor: "#f5f4f0"
      ".library-nav":
        backgroundColor: "#ebe9e4"
        width: "280px"
current_theme: light
"##;

#[test]
fn fan_edits_library_shell_classname_resolves_on_web() {
    let src = r#"
export default function FanEditsLibrary() {
  return (
    <div className="library-shell">
      <aside className="library-nav">
        <button className="nav-link">Library home</button>
      </aside>
      <main className="library-main">
        <h2>Home</h2>
      </main>
    </div>
  )
}
"#;
    clear_run_state();
    let result = run_hook_persistent(src, null_host()).expect("hook run");
    clear_handler_registry_for_vnode_pass();
    let vnode = value_to_vnode_with_handlers(&result, &mut |v| register_handler(v)).normalize();
    let (resolved, css, _) = resolve_vnode_styles(
        vnode,
        FAN_EDITS_LIBRARY_THEME,
        Platform::Web,
        1.0,
        1.0,
        None,
        false,
    );
    let css = css.expect("web css");
    assert!(
        css.contains("display:flex") || css.contains(".library-shell"),
        "library-shell flex must be in CSS: {css}"
    );
    assert!(
        css.contains("#ebe9e4") || css.contains("background-color:#ebe9e4"),
        "library-nav color must be in CSS: {css}"
    );
    match &resolved {
        VNode::Element { props, .. } => {
            assert!(
                props.contains_key("className"),
                "web keeps className: {:?}",
                props.keys().collect::<Vec<_>>()
            );
        }
        _ => panic!("expected Element root"),
    }
}

#[test]
fn fan_edits_library_nav_resolves_on_web() {
    let vnode = vnode_with_classname("aside", "library-nav", vec![]);
    let (resolved, css, _) = resolve_vnode_styles(
        vnode,
        FAN_EDITS_LIBRARY_THEME,
        Platform::Web,
        1.0,
        1.0,
        None,
        false,
    );
    let css = css.expect("web css");
    assert!(
        css.contains("#ebe9e4") || css.contains("background-color:#ebe9e4"),
        "library-nav color must be in CSS: {css}"
    );
    match &resolved {
        VNode::Element { props, .. } => {
            assert!(props.contains_key("className"));
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_theme_name_selects_light() {
    let vnode = vnode_with_classname("div", "tone", vec![]);
    let (resolved, _, _) =
        resolve_vnode_styles(vnode, THEME_LIGHT_DARK, Platform::Android, 1.0, 1.0, Some("light"), true);
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
                .expect("style");
            assert_eq!(style.get("color").map(|s| s.as_str()), Some("#111111"));
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_desktop_normalizes_rem_font_size() {
    let vnode = VNode::Element {
        tag: "span".into(),
        props: HashMap::from([
            (
                "className".into(),
                PropValue::String("text-base".into()),
            ),
            (
                "style".into(),
                PropValue::Style(HashMap::from([(
                    "fontSize".into(),
                    "0.875rem".into(),
                )])),
            ),
        ]),
        children: vec![],
        key: None,
    };
    let (resolved, _, _) =
        resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Desktop, 1.0, 1.0, None, false);
    match &resolved {
        VNode::Element { props, .. } => {
            let style = props.get("style").and_then(|p| {
                if let PropValue::Style(m) = p {
                    Some(m)
                } else {
                    None
                }
            }).expect("style");
            assert_eq!(style.get("fontSize").map(String::as_str), Some("14px"));
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_desktop_font_weight_bold() {
    let vnode = VNode::Element {
        tag: "span".into(),
        props: HashMap::from([(
            "style".into(),
            PropValue::Style(HashMap::from([(
                "fontWeight".into(),
                "bold".into(),
            )])),
        )]),
        children: vec![],
        key: None,
    };
    let (resolved, _, _) =
        resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Desktop, 1.0, 1.0, None, false);
    match &resolved {
        VNode::Element { props, .. } => {
            let style = props.get("style").and_then(|p| {
                if let PropValue::Style(m) = p {
                    Some(m)
                } else {
                    None
                }
            }).expect("style");
            assert_eq!(style.get("fontWeight").map(String::as_str), Some("700"));
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_desktop_maps_system_ui_font() {
    let vnode = VNode::Element {
        tag: "div".into(),
        props: HashMap::from([(
            "style".into(),
            PropValue::Style(HashMap::from([(
                "fontFamily".into(),
                "system-ui, sans-serif".into(),
            )])),
        )]),
        children: vec![],
        key: None,
    };
    let (resolved, _, _) =
        resolve_vnode_styles(vnode, MINIMAL_THEME, Platform::Desktop, 1.0, 1.0, None, false);
    match &resolved {
        VNode::Element { props, .. } => {
            let style = props.get("style").and_then(|p| {
                if let PropValue::Style(m) = p {
                    Some(m)
                } else {
                    None
                }
            }).expect("style");
            assert_eq!(
                style.get("fontFamily").map(String::as_str),
                Some("relay-system-sans")
            );
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn style_resolution_desktop_materializes_block_width_and_cascaded_font() {
    let child = VNode::Element {
        tag: "p".into(),
        props: HashMap::new(),
        children: vec![VNode::Text("body text".into())],
        key: None,
    };
    let parent = VNode::Element {
        tag: "div".into(),
        props: HashMap::from([(
            "style".into(),
            PropValue::Style(HashMap::from([("fontSize".into(), "18px".into())])),
        )]),
        children: vec![child],
        key: None,
    };
    let (resolved, _, _) =
        resolve_vnode_styles(parent, MINIMAL_THEME, Platform::Desktop, 1.0, 1.0, None, false);
    match &resolved {
        VNode::Element { children, .. } => match &children[0] {
            VNode::Element { props, .. } => {
                let style = props.get("style").and_then(|p| {
                    if let PropValue::Style(m) = p {
                        Some(m)
                    } else {
                        None
                    }
                }).expect("p style");
                assert_eq!(style.get("width").map(String::as_str), Some("100%"));
                assert_eq!(style.get("fontSize").map(String::as_str), Some("18px"));
            }
            _ => panic!("expected p child"),
        },
        _ => panic!("expected Element"),
    }
}

/// Fan-edits theme sets body `<p>` to 15px; web must receive that verbatim (no desktop normalize).
const FAN_EDITS_P_THEME: &str = r#"
themes:
  light:
    selectors:
      "p":
        fontSize: "15px"
        marginBottom: "8px"
current_theme: light
"#;

#[test]
fn fan_edits_theme_p_font_size_15px_on_web_and_desktop() {
    let vnode = VNode::Element {
        tag: "p".into(),
        props: HashMap::new(),
        children: vec![VNode::Text("body".into())],
        key: None,
    };
    // Desktop still flattens tag theme styles into inline props.
    let (desktop, _, _) = resolve_vnode_styles(
        vnode.clone(),
        FAN_EDITS_P_THEME,
        Platform::Desktop,
        1.0,
        1.0,
        Some("light"),
        false,
    );
    match &desktop {
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
                .expect("desktop style");
            assert_eq!(style.get("fontSize").map(String::as_str), Some("15px"));
        }
        _ => panic!("expected Element"),
    }

    // Web emits tag theme styles into the stylesheet for no-JS browsers.
    let (_, web_css, _) = resolve_vnode_styles(
        vnode,
        FAN_EDITS_P_THEME,
        Platform::Web,
        1.0,
        1.0,
        Some("light"),
        false,
    );
    let css = web_css.expect("web css");
    assert!(
        css.contains("font-size:15px") || css.contains("font-size:15"),
        "p font-size must be in CSS: {css}"
    );
}
