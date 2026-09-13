//! Shared native client-hook runner: CleverScript → styled [`VNode`].
//!
//! Used by desktop (Skia), gateway SSR, and tests. WASM/Android keep their
//! platform-specific hosts but follow the same multi-pass stability loop.

use crate::host::{Host, HostHandle, ModuleMapHost};
use crate::markdown_host;
use crate::run_state::{
    clear_handler_registry_for_vnode_pass, clear_run_state, consume_render_requested,
    register_handler, run_hook_persistent, with_isolated_hook_run,
};
use crate::style_resolve::{resolve_vnode_styles, Platform};
use crate::value::Value;
use crate::vdom::{value_to_vnode_with_handlers, VNode};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Result of running a client hook to a styled VNode tree.
pub struct HookRunResult {
    pub vnode: VNode,
    pub theme_yaml: String,
    /// Web stylesheet generated from used classNames (SSR / no-JS). Empty on native.
    pub css: String,
    pub stable_passes: u32,
}

/// Composite host: module map + virtual packages + outer fetch/navigate/create_element.
struct ClientHookHost {
    modules: Rc<RefCell<ModuleMapHost>>,
    outer: HostHandle,
    hooks: Option<Value>,
}

impl Host for ClientHookHost {
    fn fetch(&self, url: &str, options: Option<&Value>) -> Result<Value, String> {
        self.outer.borrow().fetch(url, options)
    }

    fn navigate(&self, path: &str) -> Result<(), String> {
        self.outer.borrow().navigate(path)
    }

    fn create_element(
        &self,
        tag: &str,
        props: &Value,
        children: &[Value],
    ) -> Result<Value, String> {
        match self.outer.borrow().create_element(tag, props, children) {
            Ok(v) => Ok(v),
            Err(_) => Ok(Value::Object(
                [
                    ("type".to_string(), Value::String("element".to_string())),
                    ("tag".to_string(), Value::String(tag.to_string())),
                    ("props".to_string(), props.clone()),
                    ("children".to_string(), Value::Array(children.to_vec())),
                ]
                .into_iter()
                .collect(),
            )),
        }
    }

    fn load_module(&self, path: &str) -> Result<Value, String> {
        if path == "react" || path == "react.js" {
            return Ok(react_module());
        }
        if path == "react/jsx-runtime" || path == "react/jsx-runtime.js" {
            return self.modules.borrow().load_module("react/jsx-runtime");
        }
        if path == "@clevertree/markdown"
            || path == "@clevertree/markdown.js"
            || path == "@clevertree/hook-transpiler"
            || path == "@clevertree/hook-transpiler.js"
        {
            return Ok(markdown_host::markdown_module());
        }
        if path == "@clevertree/video" || path == "@clevertree/video.js" {
            return Ok(crate::video_host::video_module());
        }
        if let Some(result) = crate::boot_shell_host::try_load_boot_shell_module(path) {
            return result;
        }
        if let Some(m) = crate::relay_modules::try_load_relay_module(path) {
            return Ok(m);
        }
        if let Some(result) = crate::style_host::try_load_style_module(path) {
            return result;
        }
        if path == "@hooks" || path == "./hooks.json" {
            if let Some(ref h) = self.hooks {
                return Ok(h.clone());
            }
            return Err("Hooks manifest not loaded".to_string());
        }
        self.modules.borrow().load_module(path)
    }

    fn log(&self, level: &str, message: &str) {
        self.outer.borrow().log(level, message);
    }
}

fn react_module() -> Value {
    let mut obj = HashMap::new();
    for name in ["useState", "useEffect", "useCallback", "useRef"] {
        obj.insert(name.to_string(), Value::ReactHook(name.to_string()));
    }
    Value::Object(obj)
}

fn inject_jsx_runtime(host: &Rc<RefCell<ModuleMapHost>>) {
    host.borrow_mut().add_module(
        "react/jsx-runtime",
        r#"export function jsx(type, props) {
  const p = props || {};
  const ch = p.children;
  const children = ch == null ? [] : (Array.isArray(ch) ? ch : [ch]);
  delete p.children;
  const tag = typeof type === 'string' ? type : 'div';
  return { type: 'element', tag, props: p, children };
}
export function jsxs(type, props) { return jsx(type, props); }
export function Fragment(props) { return props.children; }"#
            .to_string(),
    );
}

fn build_composite(
    host: HostHandle,
    modules: HashMap<String, String>,
    hooks: Option<Value>,
) -> Rc<RefCell<ClientHookHost>> {
    let module_host = ModuleMapHost::new(modules);
    inject_jsx_runtime(&module_host);
    let composite = Rc::new(RefCell::new(ClientHookHost {
        modules: module_host.clone(),
        outer: host,
        hooks,
    }));
    module_host
        .borrow()
        .set_parent_host(composite.clone() as HostHandle);
    composite
}

fn vnode_from_hook_result(
    result: Value,
    theme_yaml: &str,
    platform: Platform,
) -> Result<(VNode, String, String), String> {
    clear_handler_registry_for_vnode_pass();
    let vnode = value_to_vnode_with_handlers(&result, &mut |v| register_handler(v)).normalize();
    let effective_yaml = crate::session_theme::effective_theme_yaml(theme_yaml);
    crate::session_theme::store_unresolved_vnode(vnode.clone());
    // Honor theme YAML `current_theme` / session theme; do not force light.
    let (resolved, inject_css, _) = resolve_vnode_styles(
        vnode,
        &effective_yaml,
        platform,
        1.0,
        1.0,
        None,
        false,
    );
    Ok((
        resolved,
        effective_yaml,
        inject_css.unwrap_or_default(),
    ))
}

fn run_one_pass(
    source: &str,
    composite: Rc<RefCell<ClientHookHost>>,
    theme_yaml: &str,
    platform: Platform,
) -> Result<HookRunResult, String> {
    let result = run_hook_persistent(source, composite as HostHandle).map_err(|e| format!("{e}"))?;
    let (vnode, theme_yaml, css) = vnode_from_hook_result(result, theme_yaml, platform)?;
    Ok(HookRunResult {
        vnode,
        theme_yaml,
        css,
        stable_passes: 1,
    })
}

/// Clear hook component state (call when opening a new repo or hook source).
pub fn reset_hook_session() {
    clear_run_state();
}

/// Run hook once (or until stable if effects fire within a single pass cycle).
pub fn run_hook_to_vnode(
    source: &str,
    host: HostHandle,
    theme_yaml: &str,
    modules: HashMap<String, String>,
    platform: Platform,
) -> Result<HookRunResult, String> {
    run_hook_to_vnode_stable(source, host, theme_yaml, modules, None, platform, true)
}

/// Run hook until useEffect/setState quiesce (max 16 passes).
///
/// When `isolated` is true (SSR / multi-threaded hosts), acquires the global
/// hook-run lock and clears thread-local [`RunState`](crate::run_state) around the run.
pub fn run_hook_to_vnode_stable(
    source: &str,
    host: HostHandle,
    theme_yaml: &str,
    modules: HashMap<String, String>,
    hooks_manifest: Option<Value>,
    platform: Platform,
    reset: bool,
) -> Result<HookRunResult, String> {
    run_hook_to_vnode_stable_inner(
        source,
        host,
        theme_yaml,
        modules,
        hooks_manifest,
        platform,
        reset,
        false,
    )
}

/// Like [`run_hook_to_vnode_stable`] but serializes concurrent callers and clears TLS state.
pub fn run_hook_to_vnode_stable_isolated(
    source: &str,
    host: HostHandle,
    theme_yaml: &str,
    modules: HashMap<String, String>,
    hooks_manifest: Option<Value>,
    platform: Platform,
) -> Result<HookRunResult, String> {
    with_isolated_hook_run(|| {
        run_hook_to_vnode_stable_inner(
            source,
            host,
            theme_yaml,
            modules,
            hooks_manifest,
            platform,
            true,
            true,
        )
    })
}

fn run_hook_to_vnode_stable_inner(
    source: &str,
    host: HostHandle,
    theme_yaml: &str,
    modules: HashMap<String, String>,
    hooks_manifest: Option<Value>,
    platform: Platform,
    reset: bool,
    _already_isolated: bool,
) -> Result<HookRunResult, String> {
    if reset {
        reset_hook_session();
    }
    if crate::ssr_cancel::ssr_is_cancelled() {
        return Err("SSR cancelled".into());
    }
    let composite = build_composite(host, modules, hooks_manifest);
    let mut last = run_one_pass(source, composite.clone(), theme_yaml, platform)?;
    let mut passes: u32 = 1;
    for _ in 0..16 {
        if crate::ssr_cancel::ssr_is_cancelled() {
            return Err("SSR cancelled".into());
        }
        if !consume_render_requested() {
            break;
        }
        last = run_one_pass(source, composite.clone(), theme_yaml, platform)?;
        passes += 1;
    }
    if crate::ssr_cancel::ssr_is_cancelled() {
        return Err("SSR cancelled".into());
    }
    consume_render_requested();
    Ok(HookRunResult {
        vnode: last.vnode,
        theme_yaml: last.theme_yaml,
        css: last.css,
        stable_passes: passes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::MockHost;
    use crate::html::vnode_to_html;

    #[test]
    fn mini_hook_renders_text() {
        let source = r#"
export default function App() {
  return <div className="x"><span>hello-ssr</span></div>
}
"#;
        let host = MockHost::new() as HostHandle;
        let result = run_hook_to_vnode_stable(
            source,
            host,
            "",
            HashMap::new(),
            None,
            Platform::Web,
            true,
        )
        .expect("run");
        assert!(
            result.vnode.text_content().contains("hello-ssr"),
            "got: {}",
            result.vnode.text_content()
        );
        assert!(result.stable_passes >= 1);
    }

    #[test]
    fn isolated_runs_reset_handler_ids() {
        let source = r#"
export default function App() {
  return <button onClick={() => {}}>Click</button>
}
"#;
        let host = MockHost::new() as HostHandle;
        let a = run_hook_to_vnode_stable_isolated(
            source,
            host.clone(),
            "",
            HashMap::new(),
            None,
            Platform::Web,
        )
        .expect("a");
        let b = run_hook_to_vnode_stable_isolated(
            source,
            host,
            "",
            HashMap::new(),
            None,
            Platform::Web,
        )
        .expect("b");
        // Both should stamp handler id starting at 1 after isolated clear.
        let ha = vnode_to_html(&a.vnode, &crate::html::HtmlRenderOptions::default());
        let hb = vnode_to_html(&b.vnode, &crate::html::HtmlRenderOptions::default());
        assert!(ha.contains(r#"data-relay-hid="1""#), "{ha}");
        assert!(hb.contains(r#"data-relay-hid="1""#), "{hb}");
    }

    #[test]
    fn concurrent_isolated_runs_do_not_panic() {
        use std::sync::Arc;
        use std::thread;

        let source = Arc::new(
            r#"
export default function App() {
  return <div>parallel</div>
}
"#
            .to_string(),
        );
        let mut handles = vec![];
        for _ in 0..8 {
            let source = Arc::clone(&source);
            handles.push(thread::spawn(move || {
                let host = MockHost::new() as HostHandle;
                run_hook_to_vnode_stable_isolated(
                    &source,
                    host,
                    "",
                    HashMap::new(),
                    None,
                    Platform::Web,
                )
                .map(|r| r.vnode.text_content())
            }));
        }
        for h in handles {
            let text = h.join().expect("thread").expect("run");
            assert!(text.contains("parallel"), "{text}");
        }
    }
}
