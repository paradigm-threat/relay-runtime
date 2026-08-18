//! Virtual `@relay/ui/RelayBootShell` — CSR loading overlay; SSR/hydrate returns children.

use crate::host::{Host, HostHandle, ModuleMapHost};
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;

const RELAY_BOOT_SHELL_SOURCE: &str = include_str!("relay_boot_shell.jsx");

thread_local! {
    static BOOT_SHELL_EXPORTS_CACHE: RefCell<Option<Value>> = RefCell::new(None);
}

/// Load (and cache) default export for `@relay/ui/RelayBootShell`.
pub fn load_relay_boot_shell_exports(host: HostHandle) -> Result<Value, String> {
    BOOT_SHELL_EXPORTS_CACHE.with(|cache| {
        if let Some(ref exports) = *cache.borrow() {
            return Ok(exports.clone());
        }
        let mut modules = HashMap::new();
        modules.insert(
            "@relay/ui/RelayBootShell".to_string(),
            RELAY_BOOT_SHELL_SOURCE.to_string(),
        );
        let inner = ModuleMapHost::new(modules);
        inner.borrow().set_parent_host(host);
        let exports = inner.borrow().load_module("@relay/ui/RelayBootShell")?;
        *cache.borrow_mut() = Some(exports.clone());
        Ok(exports)
    })
}

/// Resolve `@relay/ui/RelayBootShell` in host `load_module`.
pub fn try_load_boot_shell_module(path: &str) -> Option<Result<Value, String>> {
    if !is_boot_shell_path(path) {
        return None;
    }
    let host = crate::run_state::get_host_for_lazy()
        .ok_or_else(|| "No host for @relay/ui/RelayBootShell".to_string());
    Some(host.and_then(|h| load_relay_boot_shell_exports(h)))
}

fn is_boot_shell_path(path: &str) -> bool {
    let p = path.trim_end_matches('/');
    p == "@relay/ui/RelayBootShell"
        || p == "@relay/ui/RelayBootShell.js"
        || p == "@relay/ui/RelayBootShell.jsx"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::MockHost;
    use crate::hook_runner::run_hook_to_vnode_stable_isolated;
    use crate::style_resolve::Platform;
    use crate::vdom::VNode;
    use std::collections::HashMap;

    fn run_boot_shell_hook(import_path: &str, host_mode: Option<&str>) -> HookRunOk {
        crate::run_state::clear_run_state();
        BOOT_SHELL_EXPORTS_CACHE.with(|c| *c.borrow_mut() = None);
        let host = MockHost::new();
        let source = format!(
            r#"
import RelayBootShell from '{import_path}'
export default function App() {{
  return <RelayBootShell brand="Test"><span>hi</span></RelayBootShell>
}}
"#
        );
        let mut hooks = HashMap::new();
        if let Some(mode) = host_mode {
            hooks.insert("relayHostMode".to_string(), Value::String(mode.to_string()));
        }
        run_hook_to_vnode_stable_isolated(
            &source,
            host,
            "",
            HashMap::new(),
            Some(Value::Object(hooks)),
            Platform::Web,
        )
        .unwrap_or_else(|e| panic!("boot shell import {import_path} should resolve: {e}"))
    }

    fn vnode_text(v: &VNode) -> String {
        match v {
            VNode::Text(s) => s.clone(),
            VNode::Element { children, .. } => children.iter().map(vnode_text).collect(),
            VNode::Empty => String::new(),
        }
    }

    fn vnode_class_names(v: &VNode) -> String {
        match v {
            VNode::Element { props, children, .. } => {
                let mut s = match props.get("className") {
                    Some(crate::vdom::PropValue::String(c)) => c.clone(),
                    _ => String::new(),
                };
                for c in children {
                    let more = vnode_class_names(c);
                    if !more.is_empty() {
                        if !s.is_empty() {
                            s.push(' ');
                        }
                        s.push_str(&more);
                    }
                }
                s
            }
            _ => String::new(),
        }
    }

    type HookRunOk = crate::hook_runner::HookRunResult;

    #[test]
    fn boot_shell_import_resolves() {
        let result = run_boot_shell_hook("@relay/ui/RelayBootShell", None);
        assert_eq!(vnode_text(&result.vnode), "hi");
        assert!(
            !vnode_class_names(&result.vnode).contains("relay-boot-shell"),
            "SSR default must not cover children with a loading overlay"
        );
    }

    #[test]
    fn boot_shell_csr_shows_overlay_before_live() {
        let result = run_boot_shell_hook("@relay/ui/RelayBootShell", Some("csr"));
        assert!(vnode_class_names(&result.vnode).contains("relay-boot-shell"));
        assert_ne!(vnode_text(&result.vnode), "hi");
    }

    #[test]
    fn boot_shell_jsx_path_resolves() {
        let result = run_boot_shell_hook("@relay/ui/RelayBootShell.jsx", None);
        assert!(!matches!(result.vnode, VNode::Empty));
    }

    #[test]
    fn is_boot_shell_path_accepts_variants() {
        assert!(is_boot_shell_path("@relay/ui/RelayBootShell"));
        assert!(is_boot_shell_path("@relay/ui/RelayBootShell.js"));
        assert!(is_boot_shell_path("@relay/ui/RelayBootShell.jsx"));
        assert!(is_boot_shell_path("@relay/ui/RelayBootShell/"));
        assert!(!is_boot_shell_path("@relay/ui"));
        assert!(!is_boot_shell_path("hooks/client/BootShell.jsx"));
    }
}
