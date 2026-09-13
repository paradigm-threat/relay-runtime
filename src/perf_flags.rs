//! Runtime performance strategy flags (`RELAY_PERF_*`).
//!
//! Preferred defaults keep fleet-safe behavior; alternate strategies stay available by env.
//! Catalog: `docs/PERF_FEATURE_FLAGS.md`. Not migration dual-stacks (see `docs/POLICY.md`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn flag_cache() -> &'static Mutex<HashMap<&'static str, bool>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, bool>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn parse_bool_env(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(s) => {
            let t = s.trim();
            if t.is_empty() {
                return default;
            }
            match t.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => true,
                "0" | "false" | "no" | "off" => false,
                _ => default,
            }
        }
        Err(_) => default,
    }
}

fn cached_flag(name: &'static str, default: bool) -> bool {
    // Process-lifetime cache so hot loops don't re-read env every iteration.
    // Tests that mutate env should call [`clear_perf_flag_cache_for_tests`].
    let Ok(mut guard) = flag_cache().lock() else {
        return parse_bool_env(name, default);
    };
    if let Some(v) = guard.get(name) {
        return *v;
    }
    let v = parse_bool_env(name, default);
    guard.insert(name, v);
    v
}

/// Clear cached env reads (unit tests only).
pub fn clear_perf_flag_cache_for_tests() {
    if let Ok(mut guard) = flag_cache().lock() {
        guard.clear();
    }
}

/// True when this thread is an SSR worker (`install_ssr_cancel` set a flag Arc).
pub fn is_ssr_thread() -> bool {
    crate::ssr_cancel::ssr_cancel_installed()
}

/// `RELAY_PERF_CS_CANCEL_IN_LOOPS` — default true.
pub fn cs_cancel_in_loops() -> bool {
    cached_flag("RELAY_PERF_CS_CANCEL_IN_LOOPS", true)
}

/// `RELAY_PERF_MD_CANCEL_BEFORE_PARSE` — default true.
pub fn md_cancel_before_parse() -> bool {
    cached_flag("RELAY_PERF_MD_CANCEL_BEFORE_PARSE", true)
}

/// `RELAY_PERF_MD_SSR_SKIP_ANCHOR_ONCLICK` — default true (applied when SSR thread).
pub fn md_ssr_skip_anchor_onclick() -> bool {
    cached_flag("RELAY_PERF_MD_SSR_SKIP_ANCHOR_ONCLICK", true)
}

/// `RELAY_PERF_MD_STUB_OVERRIDES_NOOP` — default true (getter reserved for wiring).
pub fn md_stub_overrides_noop() -> bool {
    cached_flag("RELAY_PERF_MD_STUB_OVERRIDES_NOOP", true)
}

/// `RELAY_PERF_MD_SSR_FAST_PATH` — default false (reserved).
pub fn md_ssr_fast_path() -> bool {
    cached_flag("RELAY_PERF_MD_SSR_FAST_PATH", false)
}

/// `RELAY_PERF_CACHED_GET_PREFER_MARKDOWN` — default true.
pub fn cached_get_prefer_markdown() -> bool {
    cached_flag("RELAY_PERF_CACHED_GET_PREFER_MARKDOWN", true)
}

/// `RELAY_PERF_MD_NESTED_LOUD_FAIL` — default true (reserved).
pub fn md_nested_loud_fail() -> bool {
    cached_flag("RELAY_PERF_MD_NESTED_LOUD_FAIL", true)
}

/// `RELAY_PERF_MD_FRAGMENT_CACHE` — default false (reserved).
pub fn md_fragment_cache() -> bool {
    cached_flag("RELAY_PERF_MD_FRAGMENT_CACHE", false)
}

/// `RELAY_PERF_ARRAY_HOST_BUILDER` — default false (reserved).
pub fn array_host_builder() -> bool {
    cached_flag("RELAY_PERF_ARRAY_HOST_BUILDER", false)
}

/// `RELAY_PERF_FUSE_SSR_SERIALIZE` — default false (reserved).
pub fn fuse_ssr_serialize() -> bool {
    cached_flag("RELAY_PERF_FUSE_SSR_SERIALIZE", false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn defaults_when_unset() {
        let _g = env_lock().lock().unwrap();
        std::env::remove_var("RELAY_PERF_CS_CANCEL_IN_LOOPS");
        std::env::remove_var("RELAY_PERF_MD_SSR_FAST_PATH");
        clear_perf_flag_cache_for_tests();
        assert!(cs_cancel_in_loops());
        assert!(!md_ssr_fast_path());
        assert!(md_cancel_before_parse());
        assert!(md_ssr_skip_anchor_onclick());
        assert!(cached_get_prefer_markdown());
    }

    #[test]
    fn env_overrides() {
        let _g = env_lock().lock().unwrap();
        std::env::set_var("RELAY_PERF_CS_CANCEL_IN_LOOPS", "0");
        std::env::set_var("RELAY_PERF_MD_SSR_FAST_PATH", "true");
        clear_perf_flag_cache_for_tests();
        assert!(!cs_cancel_in_loops());
        assert!(md_ssr_fast_path());
        std::env::remove_var("RELAY_PERF_CS_CANCEL_IN_LOOPS");
        std::env::remove_var("RELAY_PERF_MD_SSR_FAST_PATH");
        clear_perf_flag_cache_for_tests();
    }
}
