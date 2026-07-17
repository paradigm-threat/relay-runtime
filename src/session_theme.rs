//! Session-scoped theme YAML set by hook [`StyleContext`](crate::style_host) (`@clevertree/style`).
//!
//! Hosts pass an empty `theme_yaml` to `run_hook_with_host`; resolution uses
//! [`effective_theme_yaml`] which prefers the session store.

use crate::vdom::VNode;
use std::cell::RefCell;

thread_local! {
    static SESSION_THEME_YAML: RefCell<String> = RefCell::new(String::new());
    static LAST_UNRESOLVED_VNODE: RefCell<Option<VNode>> = RefCell::new(None);
}

/// True when `text` looks like themed-styler YAML (not JSON 404 bodies).
pub fn looks_like_theme_yaml(text: &str) -> bool {
    let s = text.trim();
    if s.is_empty() || s.starts_with('{') {
        return false;
    }
    s.contains("themes:")
        || s.contains("selectors:")
        || s.contains("current_theme:")
}

pub fn get_session_theme_yaml() -> String {
    SESSION_THEME_YAML.with(|c| c.borrow().clone())
}

pub fn set_session_theme_yaml(yaml: &str) {
    SESSION_THEME_YAML.with(|c| *c.borrow_mut() = yaml.to_string());
}

pub fn clear_session_theme() {
    SESSION_THEME_YAML.with(|c| c.borrow_mut().clear());
    LAST_UNRESOLVED_VNODE.with(|c| *c.borrow_mut() = None);
}

/// Host YAML when non-empty; otherwise session theme from `StyleContext`.
pub fn effective_theme_yaml(host_yaml: &str) -> String {
    if !host_yaml.trim().is_empty() {
        return host_yaml.to_string();
    }
    get_session_theme_yaml()
}

pub fn store_unresolved_vnode(vnode: VNode) {
    LAST_UNRESOLVED_VNODE.with(|c| *c.borrow_mut() = Some(vnode));
}

pub fn peek_unresolved_vnode() -> Option<VNode> {
    LAST_UNRESOLVED_VNODE.with(|c| c.borrow().clone())
}

pub fn take_unresolved_vnode() -> Option<VNode> {
    LAST_UNRESOLVED_VNODE.with(|c| c.borrow_mut().take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_theme_yaml_rejects_json() {
        assert!(!looks_like_theme_yaml(r#"{"error":"not found"}"#));
        assert!(looks_like_theme_yaml("themes:\n  light:\n    selectors: {}\ncurrent_theme: light\n"));
    }

    #[test]
    fn effective_theme_yaml_prefers_host() {
        clear_session_theme();
        set_session_theme_yaml("themes:\n  light:\n    selectors: {}\ncurrent_theme: light\n");
        assert!(effective_theme_yaml("current_theme: dark\n").contains("dark"));
        clear_session_theme();
    }

    #[test]
    fn effective_theme_yaml_uses_session_when_host_empty() {
        clear_session_theme();
        set_session_theme_yaml("current_theme: light\n");
        assert_eq!(effective_theme_yaml(""), "current_theme: light\n");
        clear_session_theme();
    }
}
