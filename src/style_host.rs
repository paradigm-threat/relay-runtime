//! Virtual `@clevertree/style` host module: **`StyleContext`** theme loader.
//!
//! Hooks wrap UI in `<StyleContext src="hooks/client/theme.yaml">` so theme YAML is
//! fetched by the hook (via `fetch`) and stored in [`session_theme`] before children render.

use crate::host::{Host, HostHandle, ModuleMapHost};
use crate::run_state::set_render_requested;
use crate::session_theme::{looks_like_theme_yaml, set_session_theme_yaml};
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;

const STYLE_CONTEXT_SOURCE: &str = include_str!("style_context.jsx");

thread_local! {
    static STYLE_EXPORTS_CACHE: RefCell<Option<Value>> = RefCell::new(None);
}

/// Natives imported by embedded `style_context.jsx`.
pub fn style_internal_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "setSessionThemeYaml".to_string(),
        Value::Native(|args: &[Value]| {
            let yaml = args
                .first()
                .and_then(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .ok_or("setSessionThemeYaml requires string")?;
            if !looks_like_theme_yaml(yaml) {
                return Err("setSessionThemeYaml: invalid theme YAML".to_string());
            }
            if crate::session_theme::get_session_theme_yaml() == yaml {
                return Ok(Value::Null);
            }
            set_session_theme_yaml(yaml);
            set_render_requested();
            Ok(Value::Null)
        }),
    );
    m.insert(
        "looksLikeThemeYaml".to_string(),
        Value::Native(|args: &[Value]| {
            let text = args
                .first()
                .and_then(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            Ok(Value::Bool(looks_like_theme_yaml(text)))
        }),
    );
    Value::Object(m)
}

/// Load (and cache) exports for `@clevertree/style`.
pub fn load_style_exports(host: HostHandle) -> Result<Value, String> {
    STYLE_EXPORTS_CACHE.with(|cache| {
        if let Some(ref exports) = *cache.borrow() {
            return Ok(exports.clone());
        }
        let mut modules = HashMap::new();
        modules.insert(
            "@clevertree/style".to_string(),
            STYLE_CONTEXT_SOURCE.to_string(),
        );
        let inner = ModuleMapHost::new(modules);
        inner.borrow().set_parent_host(host);
        let exports = inner.borrow().load_module("@clevertree/style")?;
        *cache.borrow_mut() = Some(exports.clone());
        Ok(exports)
    })
}

/// Resolve `@clevertree/style` / `@clevertree/style-internal` in host `load_module`.
pub fn try_load_style_module(path: &str) -> Option<Result<Value, String>> {
    if path == "@clevertree/style-internal" || path == "@clevertree/style-internal.js" {
        return Some(Ok(style_internal_module()));
    }
    if path == "@clevertree/style" || path == "@clevertree/style.js" {
        let host = crate::run_state::get_host_for_lazy()
            .ok_or_else(|| "No host for @clevertree/style".to_string());
        return Some(host.and_then(|h| load_style_exports(h)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_state::{clear_run_state, consume_render_requested};

    const MINIMAL_THEME: &str = "themes:\n  light:\n    selectors: {}\ncurrent_theme: light\n";

    #[test]
    fn style_internal_module_exports_natives() {
        let exports = style_internal_module();
        let obj = match exports {
            Value::Object(m) => m,
            _ => panic!("expected object exports"),
        };
        assert!(obj.contains_key("setSessionThemeYaml"));
        assert!(obj.contains_key("looksLikeThemeYaml"));
    }

    #[test]
    fn set_session_theme_yaml_native_requests_render() {
        clear_run_state();
        let m = style_internal_module();
        let set_fn = match &m {
            Value::Object(map) => map.get("setSessionThemeYaml").cloned().unwrap(),
            _ => panic!("expected object"),
        };
        if let Value::Native(f) = set_fn {
            f(&[Value::String(MINIMAL_THEME.to_string())]).expect("set theme");
            assert!(consume_render_requested());
        }
        clear_run_state();
    }
}
