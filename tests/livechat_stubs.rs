//! Layer-1 LiveChat / ChatApp stubs — optional props must not throw.
//! Fixtures: `tests/fixtures/paradigm-threat-files/hooks/client/native/`.
//! Regression: CleverScript throws on missing keys (`Property 'origin' not found`).

use relay_runtime::host::{Host, HostHandle, MockHost};
use relay_runtime::run_state::{clear_run_state, get_host_for_lazy, run_hook_persistent};
use relay_runtime::value::Value;
use relay_runtime::vdom::{value_to_vnode, PropValue, VNode};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn react_module() -> Value {
    let mut obj = HashMap::new();
    for name in ["useState", "useEffect", "useCallback", "useRef"] {
        obj.insert(name.to_string(), Value::ReactHook(name.to_string()));
    }
    Value::Object(obj)
}

fn tip_modules() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert(
        "hooks/client/propOr.jsx".to_string(),
        include_str!("fixtures/paradigm-threat-files/hooks/client/propOr.jsx").to_string(),
    );
    m.insert(
        "hooks/client/native/LiveChat.js".to_string(),
        include_str!("fixtures/paradigm-threat-files/hooks/client/native/LiveChat.js").to_string(),
    );
    m.insert(
        "hooks/client/native/ChatApp.js".to_string(),
        include_str!("fixtures/paradigm-threat-files/hooks/client/native/ChatApp.js").to_string(),
    );
    m
}

struct ChatStubHost {
    inner: Rc<RefCell<MockHost>>,
    modules: HashMap<String, String>,
    cache: RefCell<HashMap<String, Value>>,
}

impl ChatStubHost {
    fn new() -> HostHandle {
        Rc::new(RefCell::new(ChatStubHost {
            inner: MockHost::new(),
            modules: tip_modules(),
            cache: RefCell::new(HashMap::new()),
        }))
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
        self.cache
            .borrow_mut()
            .insert(path.to_string(), exports.clone());
        Ok(exports)
    }
}

impl Host for ChatStubHost {
    fn fetch(
        &self,
        url: &str,
        options: Option<&Value>,
    ) -> Result<Value, String> {
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
        if self.modules.contains_key(path) {
            let host = get_host_for_lazy()
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

fn prop_str<'a>(props: &'a HashMap<String, PropValue>, key: &str) -> Option<&'a str> {
    props.get(key).and_then(|p| match p {
        PropValue::String(s) => Some(s.as_str()),
        _ => None,
    })
}

fn find_by_class<'a>(vnode: &'a VNode, class_name: &str) -> Option<&'a VNode> {
    match vnode {
        VNode::Element { props, children, .. } => {
            if prop_str(props, "className") == Some(class_name) {
                return Some(vnode);
            }
            for c in children {
                if let Some(found) = find_by_class(c, class_name) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn el_props(vnode: &VNode) -> &HashMap<String, PropValue> {
    match vnode {
        VNode::Element { props, .. } => props,
        _ => panic!("expected element"),
    }
}

#[test]
fn livechat_mounts_with_no_optional_props() {
    clear_run_state();
    let host = ChatStubHost::new();
    // Bare <LiveChat /> — no origin/channel/path/mode/prefix (the production crash case).
    let src = r#"
import LiveChat from 'hooks/client/native/LiveChat.js'
export default function App() {
  return <LiveChat />
}
"#;
    let result = run_hook_persistent(src, host).expect("LiveChat without optional props must not throw");
    let vnode = value_to_vnode(&result).normalize();
    let host_el = find_by_class(&vnode, "fb-live-chat-host").expect("fb-live-chat-host vnode");
    let props = el_props(host_el);
    assert_eq!(
        prop_str(props, "data-relay-park-native"),
        Some("fb-live-chat")
    );
    assert_eq!(prop_str(props, "data-path"), Some("/"));
    assert_eq!(prop_str(props, "data-mode"), Some("embed"));
    assert_eq!(prop_str(props, "data-channel"), Some(""));
    assert_eq!(prop_str(props, "data-prefix"), Some(""));
    assert_eq!(prop_str(props, "data-chat-origin"), Some(""));
}

#[test]
fn livechat_mounts_with_typical_embed_props() {
    clear_run_state();
    let host = ChatStubHost::new();
    let src = r#"
import LiveChat from 'hooks/client/native/LiveChat.js'
export default function App() {
  return <LiveChat prefix="pt" channel="" path="/articles/foo.md" mode="embed" />
}
"#;
    let result = run_hook_persistent(src, host).expect("LiveChat with embed props");
    let vnode = value_to_vnode(&result).normalize();
    let host_el = find_by_class(&vnode, "fb-live-chat-host").expect("fb-live-chat-host");
    let props = el_props(host_el);
    assert_eq!(prop_str(props, "data-prefix"), Some("pt"));
    assert_eq!(prop_str(props, "data-path"), Some("/articles/foo.md"));
    assert_eq!(prop_str(props, "data-mode"), Some("embed"));
    // origin / chatOrigin still omitted — must default empty, not throw
    assert_eq!(prop_str(props, "data-chat-origin"), Some(""));
}

#[test]
fn chatapp_mounts_with_no_channel_prop() {
    clear_run_state();
    let host = ChatStubHost::new();
    let src = r#"
import ChatApp from 'hooks/client/native/ChatApp.js'
export default function App() {
  return <ChatApp />
}
"#;
    let result = run_hook_persistent(src, host).expect("ChatApp without channel must not throw");
    let vnode = value_to_vnode(&result).normalize();
    let host_el = find_by_class(&vnode, "fb-chat-app-host").expect("fb-chat-app-host");
    let props = el_props(host_el);
    assert_eq!(
        prop_str(props, "data-relay-park-native"),
        Some("fb-chat-app")
    );
    assert_eq!(prop_str(props, "data-channel"), Some("pt"));
}
