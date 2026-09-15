//! Layer-1 DirNav expand/collapse + Nav menu — mini fixtures, mocked QUERY.
//! Fixtures: `tests/fixtures/paradigm-threat-files/` (sync from park tip).

use relay_runtime::host::{host_fetch_object, Host, HostHandle, MockHost};
use relay_runtime::run_state::{
    clear_handler_registry_for_vnode_pass, clear_run_state, consume_render_requested,
    invoke_handler, register_handler, run_hook_persistent,
};
use relay_runtime::session_theme::{set_client_live, set_client_live_phase};
use relay_runtime::style_host::try_load_style_module;
use relay_runtime::value::Value;
use relay_runtime::vdom::{value_to_vnode, value_to_vnode_with_handlers, PropValue, VNode};
use relay_runtime::{markdown_host, relay_modules};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const CATALOG_JSON: &str = r#"{
  "results": [
    {"path": "blog/a.md", "title": "Blog A", "content_kind": "blog"},
    {"path": "blog/nested/b.md", "title": "Nested B", "content_kind": "blog"},
    {"path": "science/x.md", "title": "Science X", "content_kind": "article"}
  ]
}"#;

fn react_module() -> Value {
    let mut obj = HashMap::new();
    for name in ["useState", "useEffect", "useCallback", "useRef"] {
        obj.insert(name.to_string(), Value::ReactHook(name.to_string()));
    }
    Value::Object(obj)
}

fn hooks_fixture_object() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "relayClientBaseUrl".to_string(),
        Value::String("http://fixture.repo".to_string()),
    );
    m.insert("relayPathSlash".to_string(), Value::String("/".to_string()));
    // Park SSR default — DirNav shows placeholder when phase < 2.
    m.insert("relayHostMode".to_string(), Value::String("ssr".to_string()));
    Value::Object(m)
}

fn tip_modules() -> HashMap<String, String> {
    let mut m = HashMap::new();
    let add = |map: &mut HashMap<String, String>, path: &str, src: &str| {
        map.insert(path.to_string(), src.to_string());
    };
    add(
        &mut m,
        "hooks/client/components/DirNav.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/components/DirNav.jsx"),
    );
    add(
        &mut m,
        "hooks/client/components/Nav.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/components/Nav.jsx"),
    );
    add(
        &mut m,
        "hooks/client/components/Icons.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/components/Icons.jsx"),
    );
    add(
        &mut m,
        "hooks/client/components/FeelingLucky.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/components/FeelingLucky.jsx"),
    );
    add(
        &mut m,
        "hooks/client/components/DirectoryIndex.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/components/DirectoryIndex.jsx"),
    );
    add(
        &mut m,
        "hooks/client/hostMode.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/hostMode.jsx"),
    );
    add(
        &mut m,
        "hooks/client/propOr.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/propOr.jsx"),
    );
    add(
        &mut m,
        "hooks/client/loadArticles.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/loadArticles.jsx"),
    );
    add(
        &mut m,
        "hooks/client/jsonUtil.jsx",
        include_str!("fixtures/paradigm-threat-files/hooks/client/jsonUtil.jsx"),
    );
    m
}

/// Host: QUERY catalog + ModuleMap tip sources + style/relay builtins.
struct ParkComponentHost {
    inner: Rc<RefCell<MockHost>>,
    modules: HashMap<String, String>,
    cache: RefCell<HashMap<String, Value>>,
    hooks_obj: Value,
}

impl ParkComponentHost {
    fn new() -> (HostHandle, Rc<RefCell<MockHost>>) {
        let inner = MockHost::new();
        let host = Rc::new(RefCell::new(ParkComponentHost {
            inner: inner.clone(),
            modules: tip_modules(),
            cache: RefCell::new(HashMap::new()),
            hooks_obj: hooks_fixture_object(),
        }));
        (host, inner)
    }

    fn load_tip_module(&self, path: &str, self_host: HostHandle) -> Result<Value, String> {
        if let Some(cached) = self.cache.borrow().get(path).cloned() {
            return Ok(cached);
        }
        let source = self
            .modules
            .get(path)
            .cloned()
            .ok_or_else(|| format!("Module not found: {path}"))?;
        let program = relay_runtime::parse(&source).map_err(|e| format!("Parse {path}: {e}"))?;
        let analysis =
            relay_runtime::analyze(&program).map_err(|e| format!("Analyze {path}: {e}"))?;
        let mut runtime = relay_runtime::Runtime::new(program, analysis).with_host(self_host);
        relay_runtime::run_state::inject_standard_builtins(&runtime);
        let exports = runtime
            .get_module_exports_for_path(path)
            .map_err(|e| format!("{e}"))?;
        self.cache.borrow_mut().insert(path.to_string(), exports.clone());
        Ok(exports)
    }
}

impl Host for ParkComponentHost {
    fn fetch(&self, url: &str, options: Option<&Value>) -> Result<Value, String> {
        let method = options
            .and_then(|o| match o {
                Value::Object(map) => map.get("method"),
                _ => None,
            })
            .and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or("GET");
        if method.eq_ignore_ascii_case("QUERY") {
            return Ok(host_fetch_object(true, 200.0, CATALOG_JSON, None));
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
        if path == "react" || path == "react.js" {
            return Ok(react_module());
        }
        if path == "@hooks" || path == "./hooks.json" {
            return Ok(self.hooks_obj.clone());
        }
        if path == "@clevertree/markdown" || path == "@clevertree/markdown.js" {
            return Ok(markdown_host::markdown_module());
        }
        if let Some(m) = relay_modules::try_load_relay_module(path) {
            return Ok(m);
        }
        if let Some(result) = try_load_style_module(path) {
            return result;
        }
        // Re-enter via HostHandle stored in HOST_FOR_LAZY / Runtime — use a temporary
        // Rc wrap: callers always go through HostHandle, so we need the outer handle.
        // Module loads use Runtime.with_host(self_host) from load_tip_module; here we
        // only have &self. Reconstruct via inner mock's pattern: get from run_state.
        if self.modules.contains_key(path) {
            let host = relay_runtime::run_state::get_host_for_lazy()
                .ok_or_else(|| format!("No host for tip module load: {path}"))?;
            return self.load_tip_module(path, host);
        }
        let g = self.inner.borrow();
        Host::load_module(&*g, path)
    }

    fn log(&self, level: &str, message: &str) {
        let g = self.inner.borrow();
        Host::log(&*g, level, message)
    }
}

fn vnode_text(v: &VNode) -> String {
    match v {
        VNode::Text(s) => s.clone(),
        VNode::Element { children, .. } => children.iter().map(vnode_text).collect(),
        VNode::Empty => String::new(),
    }
}

fn prop_str<'a>(props: &'a HashMap<String, PropValue>, key: &str) -> Option<&'a str> {
    props.get(key).and_then(|p| match p {
        PropValue::String(s) => Some(s.as_str()),
        _ => None,
    })
}

fn find_by_data_pt_path<'a>(vnode: &'a VNode, path: &str) -> Option<&'a VNode> {
    match vnode {
        VNode::Element { props, children, .. } => {
            if prop_str(props, "data-pt-path") == Some(path) {
                return Some(vnode);
            }
            for c in children {
                if let Some(found) = find_by_data_pt_path(c, path) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn data_pt_open(vnode: &VNode) -> Option<&str> {
    match vnode {
        VNode::Element { props, .. } => prop_str(props, "data-pt-open"),
        _ => None,
    }
}

/// First chevron button under a directory node (aria-label Expand/Collapse folder).
fn chevron_on_click_id(node: &VNode) -> Option<u32> {
    match node {
        VNode::Element { tag, props, children, .. } if tag == "button" => {
            let label = prop_str(props, "aria-label").unwrap_or("");
            if label.contains("folder") {
                return props.get("onClick").and_then(|p| {
                    if let PropValue::HandlerId(id) = p {
                        Some(*id)
                    } else {
                        None
                    }
                });
            }
            for c in children {
                if let Some(id) = chevron_on_click_id(c) {
                    return Some(id);
                }
            }
            None
        }
        VNode::Element { children, .. } => {
            for c in children {
                if let Some(id) = chevron_on_click_id(c) {
                    return Some(id);
                }
            }
            None
        }
        _ => None,
    }
}

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
            for c in children {
                if let Some(id) = button_on_click_id(c, label) {
                    return Some(id);
                }
            }
            None
        }
        VNode::Element { children, .. } => {
            for c in children {
                if let Some(id) = button_on_click_id(c, label) {
                    return Some(id);
                }
            }
            None
        }
        _ => None,
    }
}

fn button_by_aria_label(vnode: &VNode, label: &str) -> Option<u32> {
    match vnode {
        VNode::Element { tag, props, children, .. } if tag == "button" => {
            if prop_str(props, "aria-label") == Some(label) {
                return props.get("onClick").and_then(|p| {
                    if let PropValue::HandlerId(id) = p {
                        Some(*id)
                    } else {
                        None
                    }
                });
            }
            for c in children {
                if let Some(id) = button_by_aria_label(c, label) {
                    return Some(id);
                }
            }
            None
        }
        VNode::Element { children, .. } => {
            for c in children {
                if let Some(id) = button_by_aria_label(c, label) {
                    return Some(id);
                }
            }
            None
        }
        _ => None,
    }
}

fn has_aria_label(vnode: &VNode, label: &str) -> bool {
    match vnode {
        VNode::Element { props, children, .. } => {
            if prop_str(props, "aria-label") == Some(label) {
                return true;
            }
            children.iter().any(|c| has_aria_label(c, label))
        }
        _ => false,
    }
}

fn has_data_attr(vnode: &VNode, key: &str) -> bool {
    match vnode {
        VNode::Element { props, children, .. } => {
            if props.contains_key(key) {
                return true;
            }
            children.iter().any(|c| has_data_attr(c, key))
        }
        _ => false,
    }
}

fn node_has_active_file(vnode: &VNode) -> bool {
    match vnode {
        VNode::Element { props, children, .. } => {
            if let Some(cls) = prop_str(props, "className") {
                if cls.contains("pt-dir-file-active") {
                    return true;
                }
            }
            children.iter().any(node_has_active_file)
        }
        _ => false,
    }
}

fn kids_have_file_text(node: &VNode, needle: &str) -> bool {
    vnode_text(node).contains(needle)
}

/// Run until useEffect/setState quiesce (max 16 passes). Returns last Value + vnode with handlers.
fn run_until_stable(src: &str, host: HostHandle) -> (Value, VNode) {
    let mut last = run_hook_persistent(src, host.clone()).expect("run_hook_persistent");
    for _ in 0..16 {
        if !consume_render_requested() {
            break;
        }
        last = run_hook_persistent(src, host.clone()).expect("re-run");
    }
    let _ = consume_render_requested();
    clear_handler_registry_for_vnode_pass();
    let vnode = value_to_vnode_with_handlers(&last, &mut |v| register_handler(v)).normalize();
    (last, vnode)
}

const DIRNAV_SRC: &str = r#"
import DirNav from 'hooks/client/components/DirNav.jsx'
export default function T() {
  return <DirNav currentPath="/blog/nested/b.md" />
}
"#;

const NAV_SRC: &str = r#"
import Nav from 'hooks/client/components/Nav.jsx'
export default function T() {
  return <Nav currentPath="/blog/nested/b.md" />
}
"#;

#[test]
fn dirnav_ssr_placeholder_when_phase_zero() {
    clear_run_state();
    // phase stays 0
    let (host, _) = ParkComponentHost::new();
    let result = run_hook_persistent(DIRNAV_SRC, host).expect("ssr DirNav");
    let vnode = value_to_vnode(&result).normalize();
    let text = vnode.text_content();
    assert!(
        text.contains("Directory loads") || has_data_attr(&vnode, "data-pt-dirnav-ssr"),
        "expected SSR placeholder: {text}"
    );
    assert!(
        find_by_data_pt_path(&vnode, "blog").is_none(),
        "SSR must not mount catalog tree"
    );
}

#[test]
fn dirnav_expand_to_current_seeds_ancestors_and_active_file() {
    clear_run_state();
    set_client_live_phase(2);
    let (host, _) = ParkComponentHost::new();
    let (_val, vnode) = run_until_stable(DIRNAV_SRC, host);

    let blog = find_by_data_pt_path(&vnode, "blog").expect("blog folder node");
    assert_eq!(data_pt_open(blog), Some("1"), "blog should be open after seed");
    let nested = find_by_data_pt_path(&vnode, "blog/nested").expect("nested folder");
    assert_eq!(
        data_pt_open(nested),
        Some("1"),
        "blog/nested should be open after seed"
    );
    assert!(
        kids_have_file_text(nested, "Nested B") || kids_have_file_text(blog, "Nested B"),
        "children remounted with Nested B: {}",
        vnode_text(&vnode)
    );
    assert!(
        node_has_active_file(&vnode),
        "current file should be pt-dir-file-active: {}",
        vnode_text(&vnode)
    );
}

#[test]
fn dirnav_collapse_all_and_expand_to_current() {
    clear_run_state();
    set_client_live_phase(2);
    let (host, _) = ParkComponentHost::new();
    let (_val, vnode) = run_until_stable(DIRNAV_SRC, host.clone());

    let collapse_id = button_on_click_id(&vnode, "Collapse all").expect("Collapse all");
    invoke_handler(collapse_id).expect("invoke Collapse all");
    assert!(consume_render_requested(), "Collapse should request re-render");
    let (_val, after_collapse) = run_until_stable(DIRNAV_SRC, host.clone());

    let blog = find_by_data_pt_path(&after_collapse, "blog").expect("blog after collapse");
    assert_eq!(data_pt_open(blog), Some("0"), "blog closed after Collapse all");
    assert!(
        !kids_have_file_text(blog, "Nested B"),
        "kids unmounted after collapse: {}",
        vnode_text(blog)
    );

    let expand_id =
        button_on_click_id(&after_collapse, "Expand to current").expect("Expand to current");
    invoke_handler(expand_id).expect("invoke Expand to current");
    assert!(consume_render_requested(), "Expand to current should re-render");
    let (_val, after_expand) = run_until_stable(DIRNAV_SRC, host);

    let blog2 = find_by_data_pt_path(&after_expand, "blog").expect("blog after expand");
    assert_eq!(data_pt_open(blog2), Some("1"));
    let nested = find_by_data_pt_path(&after_expand, "blog/nested").expect("nested restored");
    assert_eq!(data_pt_open(nested), Some("1"));
    assert!(
        kids_have_file_text(nested, "Nested B") || kids_have_file_text(blog2, "Nested B"),
        "kids remounted after Expand to current"
    );
}

#[test]
fn dirnav_chevron_toggles_kids_remount() {
    clear_run_state();
    set_client_live_phase(2);
    let (host, _) = ParkComponentHost::new();
    let (_val, vnode) = run_until_stable(DIRNAV_SRC, host.clone());

    // Start from collapsed so chevron clearly remounts kids.
    let collapse_id = button_on_click_id(&vnode, "Collapse all").expect("Collapse all");
    invoke_handler(collapse_id).expect("collapse");
    let _ = consume_render_requested();
    let (_val, closed) = run_until_stable(DIRNAV_SRC, host.clone());

    let blog = find_by_data_pt_path(&closed, "blog").expect("blog closed");
    assert_eq!(data_pt_open(blog), Some("0"));
    let chevron = chevron_on_click_id(blog).expect("chevron under blog");
    invoke_handler(chevron).expect("expand via chevron");
    assert!(consume_render_requested(), "chevron should request re-render");
    let (_val, opened) = run_until_stable(DIRNAV_SRC, host.clone());

    let blog_open = find_by_data_pt_path(&opened, "blog").expect("blog open");
    assert_eq!(data_pt_open(blog_open), Some("1"));
    assert!(
        kids_have_file_text(blog_open, "Blog A")
            || kids_have_file_text(blog_open, "nested")
            || find_by_data_pt_path(&opened, "blog/nested").is_some(),
        "chevron remounts kids: {}",
        vnode_text(blog_open)
    );

    // Toggle closed again.
    let blog2 = find_by_data_pt_path(&opened, "blog").expect("blog");
    let chevron2 = chevron_on_click_id(blog2).expect("chevron to collapse");
    invoke_handler(chevron2).expect("collapse via chevron");
    let _ = consume_render_requested();
    let (_val, closed2) = run_until_stable(DIRNAV_SRC, host);
    let blog3 = find_by_data_pt_path(&closed2, "blog").expect("blog");
    assert_eq!(data_pt_open(blog3), Some("0"));
    assert!(
        !kids_have_file_text(blog3, "Blog A"),
        "kids unmounted after second chevron: {}",
        vnode_text(blog3)
    );
}

#[test]
fn nav_menu_open_and_close() {
    clear_run_state();
    set_client_live(true);
    let (host, _) = ParkComponentHost::new();
    let (_val, vnode) = run_until_stable(NAV_SRC, host.clone());

    assert!(
        !has_aria_label(&vnode, "Mobile menu"),
        "menu closed initially"
    );
    let open_id = button_by_aria_label(&vnode, "Open menu")
        .or_else(|| button_on_click_id(&vnode, "Menu"))
        .expect("Open menu handler");
    invoke_handler(open_id).expect("open menu");
    assert!(
        consume_render_requested(),
        "menu open should re-render (phase={})",
        relay_runtime::session_theme::get_client_live_phase()
    );
    let (_val, opened) = run_until_stable(NAV_SRC, host.clone());
    assert!(
        has_aria_label(&opened, "Mobile menu"),
        "mobile dialog after open: {}",
        vnode_text(&opened)
    );

    let close_id = button_by_aria_label(&opened, "Close menu").expect("Close menu handler");
    invoke_handler(close_id).expect("close menu");
    assert!(consume_render_requested(), "menu close should re-render");
    let (_val, closed) = run_until_stable(NAV_SRC, host);
    assert!(
        !has_aria_label(&closed, "Mobile menu"),
        "mobile dialog gone after close"
    );
}

const DIR_INDEX_SRC: &str = r#"
import DirectoryIndex from 'hooks/client/components/DirectoryIndex.jsx'
export default function T() {
  return <DirectoryIndex path="blog" />
}
"#;

#[test]
fn directory_index_ssr_placeholder_when_phase_zero() {
    clear_run_state();
    let (host, _) = ParkComponentHost::new();
    let result = run_hook_persistent(DIR_INDEX_SRC, host).expect("DirectoryIndex SSR");
    let vnode = value_to_vnode(&result).normalize();
    let text = vnode.text_content();
    assert!(
        text.contains("loads after") || has_data_attr(&vnode, "data-pt-autoindex-ssr"),
        "expected SSR placeholder: {text}"
    );
    assert!(
        !text.contains("Blog A"),
        "SSR must not QUERY catalog rows: {text}"
    );
}

#[test]
fn directory_index_lists_categories_and_documents_at_phase_three() {
    clear_run_state();
    set_client_live_phase(3);
    let (host, _) = ParkComponentHost::new();
    let (_val, vnode) = run_until_stable(DIR_INDEX_SRC, host);
    let text = vnode_text(&vnode);
    assert!(
        text.contains("Categories"),
        "expected Categories section: {text}"
    );
    assert!(
        text.contains("nested") || text.contains("Nested"),
        "expected subfolder link: {text}"
    );
    assert!(
        text.contains("Documents"),
        "expected Documents section: {text}"
    );
    assert!(
        text.contains("Blog A"),
        "expected file title from catalog: {text}"
    );
    assert!(
        !text.contains("Could not load"),
        "must not show article error: {text}"
    );
}
