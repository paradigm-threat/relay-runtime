//! Integration tests: Paradigm Threat files corpus hook (`get.jsx`) + `Layout.jsx`.
//! Fixtures mirror `paradigm-threat-files` under `tests/fixtures/` — sync when editing the PT repo.

use relay_runtime::host::{host_fetch_object, Host, HostHandle, MockHost};
use relay_runtime::markdown_host;
use relay_runtime::run_state::{
    clear_handler_registry_for_vnode_pass, clear_run_state, consume_render_requested,
    invoke_handler, register_handler, run_hook_persistent,
};
use relay_runtime::style_resolve::{resolve_vnode_styles, Platform};
use relay_runtime::value::Value;
use relay_runtime::vdom::{value_to_vnode_with_handlers, PropValue, VNode};
use relay_runtime::{analyze, parse, value_to_vnode, Runtime};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Minimal theme for Layout `className` resolution (flex layout + border token).
const LAYOUT_THEME_YAML: &str = r##"
themes:
  light:
    variables:
      border-color: "#e5e7eb"
    selectors:
      ".flex":
        display: "flex"
      ".flex-col":
        flexDirection: "column"
      ".h-full":
        height: "100%"
current_theme: light
"##;

fn react_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert(
        "useState".to_string(),
        Value::ReactHook("useState".to_string()),
    );
    obj.insert(
        "useEffect".to_string(),
        Value::ReactHook("useEffect".to_string()),
    );
    obj.insert(
        "useCallback".to_string(),
        Value::ReactHook("useCallback".to_string()),
    );
    Value::Object(obj)
}

fn hooks_fixture_object() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "relayClientBaseUrl".to_string(),
        Value::String("http://fixture.repo".to_string()),
    );
    m.insert("relayPathSlash".to_string(), Value::String("/".to_string()));
    Value::Object(m)
}

fn compile_layout_exports() -> Value {
    const LAYOUT_SRC: &str =
        include_str!("fixtures/paradigm-threat-files/hooks/client/components/Layout.jsx");
    let program = parse(LAYOUT_SRC).expect("parse Layout.jsx");
    let analysis = analyze(&program).expect("analyze Layout.jsx");
    let h = MockHost::new();
    let mut rt = Runtime::new(program, analysis).with_host(h);
    rt.get_module_exports_for_path("hooks/client/components/Layout.jsx")
        .expect("Layout module exports")
}

/// Host matching WASM wiring: `@hooks`, `@clevertree/markdown`, Layout path; fetch returns markdown for `README.md`.
struct ParadigmFixtureHost {
    /// Shared with tests so `fetch_calls` can be asserted.
    inner: Rc<RefCell<MockHost>>,
    layout_exports: Value,
    hooks_obj: Value,
}

impl Host for ParadigmFixtureHost {
    fn fetch(&self, url: &str, options: Option<&Value>) -> Result<Value, String> {
        if url.contains(".md") {
            self.inner
                .borrow()
                .fetch_calls
                .borrow_mut()
                .push((url.to_string(), options.cloned()));
            return Ok(host_fetch_object(
                true,
                200.0,
                "# Fixture README\n\nRendered **markdown** from tests.",
                None,
            ));
        }
        let g = self.inner.borrow();
        Host::fetch(&*g, url, options)
    }

    fn navigate(&self, path: &str) -> Result<(), String> {
        let g = self.inner.borrow();
        Host::navigate(&*g, path)
    }

    fn create_element(
        &self,
        tag: &str,
        props: &Value,
        children: &[Value],
    ) -> Result<Value, String> {
        let g = self.inner.borrow();
        Host::create_element(&*g, tag, props, children)
    }

    fn load_module(&self, path: &str) -> Result<Value, String> {
        match path {
            "react" | "react.js" => Ok(react_module()),
            "@hooks" | "./hooks.json" => Ok(self.hooks_obj.clone()),
            "@clevertree/markdown" | "@clevertree/markdown.js" => {
                Ok(markdown_host::markdown_module())
            }
            "hooks/client/components/Layout.jsx" => Ok(self.layout_exports.clone()),
            _ => {
                let g = self.inner.borrow();
                Host::load_module(&*g, path)
            }
        }
    }

    fn log(&self, level: &str, message: &str) {
        let g = self.inner.borrow();
        Host::log(&*g, level, message)
    }
}

fn paradigm_host_handle(
    layout_exports: Value,
    hooks_obj: Value,
) -> (HostHandle, Rc<RefCell<MockHost>>) {
    let inner = MockHost::new();
    let fixture = ParadigmFixtureHost {
        inner: inner.clone(),
        layout_exports,
        hooks_obj,
    };
    let host: HostHandle = Rc::new(RefCell::new(fixture));
    (host, inner)
}

const GET_HOOK: &str = include_str!("fixtures/paradigm-threat-files/hooks/client/get.jsx");

fn vnode_text(v: &VNode) -> String {
    match v {
        VNode::Text(s) => s.clone(),
        VNode::Element { children, .. } => children.iter().map(vnode_text).collect(),
        VNode::Empty => String::new(),
    }
}

fn find_element_by_id<'a>(vnode: &'a VNode, id: &str) -> Option<&'a VNode> {
    match vnode {
        VNode::Element { props, children, .. } => {
            let matches = props.get("id").and_then(|p| {
                if let PropValue::String(s) = p {
                    Some(s.as_str() == id)
                } else {
                    None
                }
            }) == Some(true);
            if matches {
                return Some(vnode);
            }
            for c in children {
                if let Some(found) = find_element_by_id(c, id) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Find a `button` whose subtree text equals `label`; return its `onClick` handler id.
fn button_on_click_id(vnode: &VNode, label: &str) -> Option<u32> {
    match vnode {
        VNode::Element { tag, props, children, .. } if tag == "button" => {
            if vnode_text(vnode).trim() == label {
                return props.get("onClick").and_then(|p| {
                    if let PropValue::HandlerId(id) = p {
                        Some(*id)
                    } else {
                        None
                    }
                });
            }
        }
        VNode::Element { children, .. } => {
            for c in children {
                if let Some(id) = button_on_click_id(c, label) {
                    return Some(id);
                }
            }
        }
        _ => {}
    }
    None
}

#[test]
fn paradigm_files_get_hook_renders_header_layout_and_markdown() {
    clear_run_state();
    let layout_exports = compile_layout_exports();
    let hooks_obj = hooks_fixture_object();
    let (host, mock_for_assert) = paradigm_host_handle(layout_exports, hooks_obj);

    let result = run_hook_persistent(GET_HOOK, host.clone()).expect("run_hook_persistent");
    let vnode = value_to_vnode(&result).normalize();
    let text = vnode.text_content();

    assert!(
        text.contains("Paradigm Threat"),
        "expected title in snapshot text: {}",
        text
    );
    assert!(text.contains("README.md"), "expected path label: {}", text);
    assert!(
        text.contains("Fixture README"),
        "expected fetched markdown heading: {}",
        text
    );
    assert!(
        text.contains("Rendered"),
        "expected markdown body fragment: {}",
        text
    );
    assert!(
        text.contains("Up") && text.contains("Home"),
        "expected Layout nav labels: {}",
        text
    );

    let fetch_urls: Vec<(String, Option<Value>)> = mock_for_assert
        .borrow()
        .fetch_calls
        .borrow()
        .clone();
    assert!(
        fetch_urls.iter().any(|(u, _)| {
            u.contains("README.md") && u.starts_with("http://fixture.repo")
        }),
        "expected fetch to README under relayClientBaseUrl, got {:?}",
        fetch_urls
    );

}

#[test]
fn paradigm_files_get_hook_home_button_navigates() {
    clear_run_state();
    let layout_exports = compile_layout_exports();
    let hooks_obj = hooks_fixture_object();
    let (host, _) = paradigm_host_handle(layout_exports, hooks_obj);

    let src = r#"
import React, { useState } from 'react'
import Layout from 'hooks/client/components/Layout.jsx'
export default function T() {
  const rp = useState('subdir/note.md')
  const routePath = rp[0]
  const setRoute = rp[1]
  return (
    <Layout
      filePath={routePath}
      onUp={() => setRoute('README.md')}
      onHome={() => setRoute('README.md')}
    >
      <span>body</span>
    </Layout>
  )
}
"#;

    let result = run_hook_persistent(src, host.clone()).expect("mini get layout");
    let before = value_to_vnode(&result).normalize().text_content();
    assert!(before.contains("subdir/note.md"), "initial path: {}", before);

    clear_handler_registry_for_vnode_pass();
    let vnode_handlers =
        value_to_vnode_with_handlers(&result, &mut |v| register_handler(v)).normalize();
    let home_id = button_on_click_id(&vnode_handlers, "Home").expect("Home onClick id");
    invoke_handler(home_id).expect("invoke Home");
    assert!(
        consume_render_requested(),
        "Home should request re-render via setRoute"
    );
    let after = run_hook_persistent(src, host).expect("re-run after Home");
    let after_text = value_to_vnode(&after).normalize().text_content();
    assert!(
        after_text.contains("README.md"),
        "Home navigates to README.md: {}",
        after_text
    );
}

#[test]
fn paradigm_files_layout_component_renders_file_row_and_buttons() {
    clear_run_state();
    let layout_exports = compile_layout_exports();
    let hooks_obj = hooks_fixture_object();
    let (host, _) = paradigm_host_handle(layout_exports, hooks_obj);

    let src = r#"
import Layout from 'hooks/client/components/Layout.jsx'
export default function T() {
  return (
    <Layout filePath="subdir/file.md" onUp={() => {}} onHome={() => {}}>
      <span>inner</span>
    </Layout>
  )
}
"#;

    let result = run_hook_persistent(src, host).expect("layout wrapper");
    let vnode = value_to_vnode(&result).normalize();
    let text = vnode.text_content();

    assert!(text.contains("subdir/file.md"), "path label: {}", text);
    assert!(
        text.contains("Up") && text.contains("Home"),
        "nav buttons: {}",
        text
    );
    assert!(text.contains("inner"), "child slot: {}", text);
}

#[test]
fn paradigm_files_layout_classname_resolves_to_inline_style() {
    clear_run_state();
    let layout_exports = compile_layout_exports();
    let hooks_obj = hooks_fixture_object();
    let (host, _) = paradigm_host_handle(layout_exports, hooks_obj);

    let src = r#"
import Layout from 'hooks/client/components/Layout.jsx'
export default function T() {
  return (
    <Layout filePath="x.md" onUp={() => {}} onHome={() => {}}>
      <span>inner</span>
    </Layout>
  )
}
"#;

    let result = run_hook_persistent(src, host).expect("layout render");
    clear_handler_registry_for_vnode_pass();
    let vnode = value_to_vnode_with_handlers(&result, &mut |v| register_handler(v)).normalize();
    let (resolved, _, _) =
        resolve_vnode_styles(vnode, LAYOUT_THEME_YAML, Platform::Web, 1.0, 1.0, None, false);

    let layout_el = find_element_by_id(&resolved, "layout").expect("#layout element");
    match layout_el {
        VNode::Element { props, .. } => {
            assert!(
                !props.contains_key("className"),
                "className must be resolved away: {:?}",
                props.keys().collect::<Vec<_>>()
            );
            let style = props
                .get("style")
                .and_then(|p| {
                    if let PropValue::Style(m) = p {
                        Some(m)
                    } else {
                        None
                    }
                })
                .expect("resolved style on #layout");
            assert!(
                style.contains_key("display") || style.contains_key("flexDirection"),
                "flex classes must produce layout keys: {:?}",
                style
            );
        }
        _ => panic!("expected Element"),
    }
}

#[test]
fn paradigm_layout_buttons_invoke_handlers() {
    clear_run_state();
    let layout_exports = compile_layout_exports();
    let hooks_obj = hooks_fixture_object();
    let (host, mock_for_assert) = paradigm_host_handle(layout_exports, hooks_obj);

    let src = r#"
import Layout from 'hooks/client/components/Layout.jsx'
export default function T() {
  return (
    <Layout
      filePath="a/b/c.md"
      onUp={() => navigate('/up-from-layout')}
      onHome={() => navigate('/home-from-layout')}
    >
      <span>inner</span>
    </Layout>
  )
}
"#;

    let result = run_hook_persistent(src, host).expect("layout with navigate handlers");
    clear_handler_registry_for_vnode_pass();
    let vnode = value_to_vnode_with_handlers(&result, &mut |v| register_handler(v)).normalize();

    let up_id = button_on_click_id(&vnode, "Up").expect("Up onClick id");
    invoke_handler(up_id).expect("invoke Up");
    let mock_ref = mock_for_assert.borrow();
    let nav = mock_ref.navigate_calls.borrow();
    assert_eq!(nav.last().map(|s| s.as_str()), Some("/up-from-layout"));
    drop(nav);
    drop(mock_ref);

    let home_id = button_on_click_id(&vnode, "Home").expect("Home onClick id");
    invoke_handler(home_id).expect("invoke Home");
    let mock_ref = mock_for_assert.borrow();
    let nav = mock_ref.navigate_calls.borrow();
    assert_eq!(nav.len(), 2);
    assert_eq!(nav[1], "/home-from-layout");
}
