//! Virtual `@clevertree/style` host module: **`StyleContext`** theme loader.
//!
//! Hooks wrap UI in `<StyleContext src="hooks/client/theme.yaml">` so theme YAML is
//! fetched by the hook (via `fetch`) and stored in [`session_theme`] before children render.

use crate::host::{Host, HostHandle, ModuleMapHost};
use crate::run_state::set_render_requested;
use crate::session_theme::{
    get_client_live_phase, get_session_theme_yaml, is_client_live, is_client_live_phase,
    looks_like_theme_yaml, patch_current_theme_yaml, set_forced_theme_css, set_session_theme_yaml,
};
use crate::style_resolve::css_for_forced_theme;
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
    m.insert(
        "isClientLive".to_string(),
        Value::Native(|_args: &[Value]| Ok(Value::Bool(is_client_live()))),
    );
    m.insert(
        "getClientLivePhase".to_string(),
        Value::Native(|_args: &[Value]| Ok(Value::Number(get_client_live_phase() as f64))),
    );
    m.insert(
        "isClientLivePhase".to_string(),
        Value::Native(|args: &[Value]| {
            let min = args
                .first()
                .and_then(|v| match v {
                    Value::Number(n) => Some(*n as u8),
                    Value::String(s) => s.parse::<u8>().ok(),
                    _ => None,
                })
                .unwrap_or(1);
            Ok(Value::Bool(is_client_live_phase(min)))
        }),
    );
    m.insert(
        "getCurrentTheme".to_string(),
        Value::Native(|_args: &[Value]| {
            Ok(Value::String(crate::session_theme::get_current_theme_name()))
        }),
    );
    m.insert(
        "setCurrentTheme".to_string(),
        Value::Native(|args: &[Value]| {
            let name = args
                .first()
                .and_then(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .ok_or("setCurrentTheme requires theme name string")?
                .trim()
                .to_string();
            if name.is_empty() {
                return Err("setCurrentTheme: empty theme name".to_string());
            }
            let yaml = get_session_theme_yaml();
            let patched = patch_current_theme_yaml(&yaml, &name);
            if looks_like_theme_yaml(&patched) {
                set_session_theme_yaml(&patched);
            } else if !yaml.is_empty() {
                // Keep prior session; still force CSS from name when possible.
            } else {
                set_session_theme_yaml(&format!("current_theme: {name}\n"));
            }
            let css = css_for_forced_theme(&get_session_theme_yaml(), &name);
            if !css.trim().is_empty() {
                set_forced_theme_css(&css);
            }
            crate::session_theme::set_current_theme_name(&name);
            set_render_requested();
            Ok(Value::String(css))
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
        assert!(obj.contains_key("setCurrentTheme"));
        assert!(obj.contains_key("getCurrentTheme"));
        assert!(obj.contains_key("isClientLive"));
        assert!(obj.contains_key("getClientLivePhase"));
        assert!(obj.contains_key("isClientLivePhase"));
    }

    #[test]
    fn client_live_phase_natives_match_session_store() {
        use crate::session_theme::{clear_client_live, set_client_live_phase};
        clear_run_state();
        clear_client_live();
        let m = style_internal_module();
        let obj = match m {
            Value::Object(map) => map,
            _ => panic!("expected object"),
        };
        let get_phase = match obj.get("getClientLivePhase").cloned().unwrap() {
            Value::Native(f) => f,
            _ => panic!("getClientLivePhase"),
        };
        let is_phase = match obj.get("isClientLivePhase").cloned().unwrap() {
            Value::Native(f) => f,
            _ => panic!("isClientLivePhase"),
        };
        let is_live = match obj.get("isClientLive").cloned().unwrap() {
            Value::Native(f) => f,
            _ => panic!("isClientLive"),
        };
        assert_eq!(get_phase(&[]).unwrap(), Value::Number(0.0));
        assert_eq!(is_live(&[]).unwrap(), Value::Bool(false));
        assert_eq!(
            is_phase(&[Value::Number(1.0)]).unwrap(),
            Value::Bool(false)
        );
        set_client_live_phase(2);
        assert_eq!(get_phase(&[]).unwrap(), Value::Number(2.0));
        assert_eq!(is_live(&[]).unwrap(), Value::Bool(true));
        assert_eq!(
            is_phase(&[Value::Number(2.0)]).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            is_phase(&[Value::String("3".into())]).unwrap(),
            Value::Bool(false)
        );
        // Missing arg defaults to min=1 (shell).
        assert_eq!(is_phase(&[]).unwrap(), Value::Bool(true));
        clear_client_live();
        clear_run_state();
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
