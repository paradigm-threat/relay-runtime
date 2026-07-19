//! Session-scoped theme YAML set by hook [`StyleContext`](crate::style_host) (`@clevertree/style`).
//!
//! Hosts pass an empty `theme_yaml` to `run_hook_with_host`; resolution uses
//! [`effective_theme_yaml`] which prefers the session store.

use crate::vdom::VNode;
use std::cell::{Cell, RefCell};

thread_local! {
    static SESSION_THEME_YAML: RefCell<String> = RefCell::new(String::new());
    static LAST_UNRESOLVED_VNODE: RefCell<Option<VNode>> = RefCell::new(None);
    /// True only in the browser after SSR hydrate (or CSR mount) succeeds.
    /// CleverScript gates interactive controls on this so SSR HTML stays inert for no-JS.
    static CLIENT_LIVE: Cell<bool> = Cell::new(false);
    /// Optional forced single-theme CSS (from `setCurrentTheme`); preferred by get_injected_css.
    static FORCED_THEME_CSS: RefCell<String> = RefCell::new(String::new());
    static CURRENT_THEME_NAME: RefCell<String> = RefCell::new(String::new());
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
    FORCED_THEME_CSS.with(|c| c.borrow_mut().clear());
    clear_client_live();
}

pub fn is_client_live() -> bool {
    CLIENT_LIVE.with(|c| c.get())
}

pub fn set_client_live(live: bool) {
    CLIENT_LIVE.with(|c| c.set(live));
}

pub fn clear_client_live() {
    set_client_live(false);
}

pub fn set_forced_theme_css(css: &str) {
    FORCED_THEME_CSS.with(|c| *c.borrow_mut() = css.to_string());
}

pub fn peek_forced_theme_css() -> Option<String> {
    FORCED_THEME_CSS.with(|c| {
        let s = c.borrow().clone();
        if s.trim().is_empty() {
            None
        } else {
            Some(s)
        }
    })
}

pub fn clear_forced_theme_css() {
    FORCED_THEME_CSS.with(|c| c.borrow_mut().clear());
}

pub fn set_current_theme_name(name: &str) {
    CURRENT_THEME_NAME.with(|c| *c.borrow_mut() = name.trim().to_string());
}

pub fn get_current_theme_name() -> String {
    CURRENT_THEME_NAME.with(|c| c.borrow().clone())
}

/// Patch or append `current_theme: <name>` in theme YAML.
pub fn patch_current_theme_yaml(yaml: &str, theme_name: &str) -> String {
    let name = theme_name.trim();
    if name.is_empty() {
        return yaml.to_string();
    }
    let marker = "current_theme:";
    if let Some(idx) = yaml.find(marker) {
        let after = idx + marker.len();
        let mut stop = after;
        let bytes = yaml.as_bytes();
        while stop < bytes.len() && bytes[stop] != b'\n' {
            stop += 1;
        }
        let mut out = String::with_capacity(yaml.len() + name.len() + 1);
        out.push_str(&yaml[..after]);
        out.push(' ');
        out.push_str(name);
        out.push_str(&yaml[stop..]);
        out
    } else if yaml.is_empty() {
        format!("current_theme: {name}\n")
    } else {
        let mut out = yaml.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("current_theme: ");
        out.push_str(name);
        out.push('\n');
        out
    }
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
    fn patch_current_theme_yaml_replaces_and_appends() {
        let patched = patch_current_theme_yaml(
            "themes:\n  light:\n    selectors: {}\ncurrent_theme: light\n",
            "dark",
        );
        assert!(patched.contains("current_theme: dark"), "{patched}");
        assert!(!patched.contains("current_theme: light"), "{patched}");
        let appended = patch_current_theme_yaml("themes:\n  light:\n    selectors: {}\n", "dark");
        assert!(appended.contains("current_theme: dark"), "{appended}");
    }

    #[test]
    fn client_live_defaults_false() {
        clear_client_live();
        assert!(!is_client_live());
        set_client_live(true);
        assert!(is_client_live());
        clear_client_live();
    }
}
