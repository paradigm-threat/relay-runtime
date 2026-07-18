//! WASM exports for relay-runtime.
//!
//! Only compiled for `target_arch = "wasm32"`.
//! Includes theme styling (built-in from themed-styler).

#![cfg(target_arch = "wasm32")]

use crate::host::{Host, HostHandle, ModuleMapHost};
use crate::run_state;
use crate::runtime::Runtime;
use crate::style_resolve::{resolve_vnode_styles, Platform};
use crate::value::Value;
use crate::vdom::collect_usage_from_vnode_json;
use crate::vdom::value_to_vnode_with_handlers;
use crate::error::RuntimeError;
use crate::{analyze, parse_with_path, value_to_vnode};
use js_sys::JSON;
use serde_json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::window;


thread_local! {
    static INJECTED_CSS: RefCell<String> = RefCell::new(String::new());
    /// Last state passed to render_css_for_web; used by setCurrentTheme and getThemes.
    static LAST_STATE: RefCell<String> = RefCell::new(String::new());
    /// Last run_hook timestamp (ms) for frequent re-run detection.
    static LAST_RUN_MS: RefCell<f64> = RefCell::new(0.0);
    /// Messages pushed from host via push_message_from_host. Consumed by getPendingMessages.
    static MESSAGE_QUEUE: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);
    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn console_warn(s: &str);
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(s: &str);
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn performance_now() -> f64;
}

static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Enable or disable verbose mode. When enabled, run_hook logs each hook execution,
/// and the host should log fetches and file loads.
#[wasm_bindgen]
pub fn set_verbose(enabled: bool) {
    VERBOSE.store(enabled, Ordering::SeqCst);
}

fn log_error(msg: &str) {
    console_error(msg);
}

#[allow(dead_code)]
fn log_warn(msg: &str) {
    console_warn(msg);
}

fn log_verbose(msg: &str) {
    if VERBOSE.load(Ordering::SeqCst) {
        console_log(msg);
    }
}

fn runtime_error_to_js(e: RuntimeError) -> JsValue {
    let (msg, json_str) = match &e {
        RuntimeError::ParseDetail(d) => (
            format!("{}", e),
            serde_json::to_string(&d.to_json()).unwrap_or_else(|_| format!("{}", e)),
        ),
        RuntimeError::RuntimeDetail(_) => {
            // Pass full JSON for host debug UI (call_stack, locals).
            let json = e.to_error_json();
            let s = serde_json::to_string(&json).unwrap_or_else(|_| format!("{}", e));
            (format!("{}", e), s)
        }
        _ => {
            let json = e.to_error_json();
            let s = serde_json::to_string(&json).unwrap_or_else(|_| format!("{}", e));
            (format!("{}", e), s)
        }
    };
    log_error(&format!("[relay-runtime] {}", msg));
    JsValue::from_str(&json_str)
}

/// Run hook source and return VNode tree as JSON for host to apply to DOM.
///
/// Returns JSON string of the normalized VNode, or error on failure.
/// For parse errors, the error string is JSON `{message, line, column, file?, source_line?}` for advanced UI.
/// Errors and warnings are forwarded to the browser console.
#[wasm_bindgen]
pub fn run_hook(source: &str) -> Result<String, JsValue> {
    run_hook_with_file(source, "")
}

/// Run hook with optional filename for verbose parse errors (file:line:column).
#[wasm_bindgen]
pub fn run_hook_with_file(source: &str, file: &str) -> Result<String, JsValue> {
    #[cfg(feature = "wasm")]
    console_error_panic_hook::set_once();

    check_frequent_rerun();
    let program = parse_with_path(source, if file.is_empty() { None::<String> } else { Some(file.to_string()) })
        .map_err(runtime_error_to_js)?;
    let analysis = analyze(&program).map_err(|e| {
        let msg = format!("[relay-runtime] Analysis error: {}", e);
        log_error(&msg);
        JsValue::from_str(&msg)
    })?;

    let default_name = analysis.default_export.as_deref().unwrap_or("<default>");
    log_verbose(&format!("[relay-runtime] run_hook: {}", default_name));

    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run().map_err(|e| {
        let msg = format!("[relay-runtime] Runtime error: {}", e);
        log_error(&msg);
        JsValue::from_str(&msg)
    })?;
    let vnode = value_to_vnode(&result).normalize();

    serde_json::to_string(&vnode).map_err(|e| {
        let msg = format!("[relay-runtime] Serialize error: {}", e);
        log_error(&msg);
        JsValue::from_str(&msg)
    })
}

/// Minimum ms between runs before warning about frequent re-runs.
const RERUN_WARN_THRESHOLD_MS: f64 = 100.0;

fn check_frequent_rerun() {
    let now = performance_now();
    let last = LAST_RUN_MS.with(|c| *c.borrow());
    if last > 0.0 && (now - last) < RERUN_WARN_THRESHOLD_MS {
        console_warn(&format!(
            "[relay-runtime] Frequent re-run detected ({:.0}ms since last). Consider debouncing or batching updates.",
            now - last
        ));
    }
    LAST_RUN_MS.with(|c| *c.borrow_mut() = now);
}

/// Run hook with a module map for dynamic import() support.
/// module_map_json: JSON object mapping path -> source, e.g. {"../foo.jsx": "export default..."}
/// theme_yaml: legacy host theme; pass "" — hooks load theme via `@clevertree/style` StyleContext.
/// hooks_manifest_json: JSON string of {hooks: [{id, name, path}, ...]}. Pass "" if none. Hook can import("@hooks").
/// file: Optional filename for parse errors (e.g. "app.jsx"). Pass "" for none.
#[wasm_bindgen]
pub fn run_hook_with_host(
    source: &str,
    module_map_json: &str,
    theme_yaml: &str,
    hooks_manifest_json: &str,
    file: &str,
) -> Result<String, JsValue> {
    #[cfg(feature = "wasm")]
    console_error_panic_hook::set_once();

    check_frequent_rerun();
    INJECTED_CSS.with(|c| *c.borrow_mut() = String::new());

    let default_name = {
        let program = parse_with_path(source, if file.is_empty() { None::<String> } else { Some(file.to_string()) })
            .map_err(runtime_error_to_js)?;
        let analysis = analyze(&program).map_err(|e| {
            let msg = format!("[relay-runtime] Analysis error: {}", e);
            log_error(&msg);
            JsValue::from_str(&msg)
        })?;
        analysis.default_export.as_deref().unwrap_or("<default>").to_string()
    };
    log_verbose(&format!("[relay-runtime] run_hook_with_host: {}", default_name));

    let modules: HashMap<String, String> = serde_json::from_str(module_map_json)
        .unwrap_or_else(|_| HashMap::new());
    let module_count = modules.len();
    let inner = ModuleMapHost::new(modules);

    let hooks = if hooks_manifest_json.is_empty() {
        None
    } else {
        serde_json::from_str::<serde_json::Value>(hooks_manifest_json)
            .ok()
            .map(|v| Value::from_json(&v))
    };

    let wasm_host = Rc::new(RefCell::new(WasmHost {
        inner: inner.clone(),
        hooks,
    }));
    inner.borrow().set_parent_host(wasm_host.clone());

    let result = run_state::run_hook_persistent(source, wasm_host).map_err(|e| {
        let msg = format!("[relay-runtime] Runtime error: {}", e);
        log_error(&msg);
        JsValue::from_str(&msg)
    })?;
    run_state::clear_handler_registry_for_vnode_pass();
    let vnode = value_to_vnode_with_handlers(&result, &mut |v| run_state::register_handler(v)).normalize();
    let effective_yaml = crate::session_theme::effective_theme_yaml(theme_yaml);
    crate::session_theme::store_unresolved_vnode(vnode.clone());
    let (resolved, inject_css, state_json) = resolve_vnode_styles(
        vnode,
        &effective_yaml,
        Platform::Web,
        1.0,
        1.0,
        None,
        false,
    );
    if let Some(css) = inject_css {
        INJECTED_CSS.with(|c| *c.borrow_mut() = css);
    }
    if let Some(json) = state_json {
        LAST_STATE.with(|c| *c.borrow_mut() = json);
    }

    // Debug: when result normalizes to Empty, log what we got to help diagnose
    if matches!(&resolved, crate::vdom::VNode::Empty) {
        let result_preview = serde_json::to_string(&result.to_json()).unwrap_or_else(|_| "?".into());
        let preview = if result_preview.len() > 500 {
            format!("{}...", &result_preview[..500])
        } else {
            result_preview
        };
        console_warn(&format!(
            "[relay-runtime] Hook returned empty vnode. Raw result: {}",
            preview
        ));
        let hook_count = serde_json::from_str::<serde_json::Value>(hooks_manifest_json)
            .ok()
            .and_then(|v| v.get("hooks").and_then(|h| h.as_array()).map(|a| a.len()))
            .unwrap_or(0);
        console_warn(&format!(
            "[relay-runtime] Context: {} modules in map, {} hooks in manifest",
            module_count, hook_count
        ));
    }

    serde_json::to_string(&resolved).map_err(|e| {
        let msg = format!("[relay-runtime] Serialize error: {}", e);
        log_error(&msg);
        JsValue::from_str(&msg)
    })
}

// Re-export theme styling (built into relay-runtime)
pub use themed_styler::{
    get_default_state_json, get_theme_list_json, register_theme_json, render_css_for_web,
    set_theme_json,
};

/// Return CSS injected by the hook via injectStyles(). Call after run_hook_with_host.
/// Prefers a forced theme stylesheet from `setCurrentTheme` when present.
#[wasm_bindgen]
pub fn get_injected_css() -> String {
    if let Some(forced) = crate::session_theme::peek_forced_theme_css() {
        return forced;
    }
    INJECTED_CSS.with(|c| c.borrow().clone())
}

/// Get theme list JSON from last state. Call after run_hook_with_host.
#[wasm_bindgen]
pub fn get_themes() -> String {
    LAST_STATE.with(|c| {
        let state = c.borrow();
        if state.is_empty() {
            "[]".to_string()
        } else {
            get_theme_list_json(state.as_str())
        }
    })
}

/// Parse Markdown to a JSON AST using **md2ast** (same crate as gateway should use for parity).
///
/// Returns a JSON array of tagged nodes (`{ "type": "element", ... }` | `{ "type": "text", ... }`).
/// `allowed_html_tags_json` is a JSON array of lowercase tag names for inline HTML in markdown
/// (e.g. `["span","div","math"]`). Use `[]` to disallow raw HTML tags in source.
#[wasm_bindgen]
pub fn markdown_to_ast_json(markdown: &str, allowed_html_tags_json: &str) -> Result<String, JsValue> {
    let options =
        crate::markdown_host::transpile_options_from_allowed_json(allowed_html_tags_json);
    crate::markdown_host::markdown_to_ast_json(markdown, &options).map_err(|e| JsValue::from_str(&e))
}

fn do_post_message_to_host(payload_json: &str) -> Result<(), String> {
    let window = window().ok_or_else(|| "no window".to_string())?;
    let payload: JsValue = JSON::parse(payload_json).unwrap_or_else(|_| JsValue::from_str(payload_json));
    let envelope = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str("type"), &JsValue::from_str("relay-runtime"));
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str("payload"), &payload);
    window.post_message(&envelope.into(), "*").map_err(|e| format!("postMessage failed: {:?}", e))
}

/// Post a message to the host via window.postMessage. Host receives via addEventListener("message", ...).
/// Payload is wrapped as { type: "relay-runtime", payload } so host can filter.
#[wasm_bindgen]
pub fn post_message_to_host(payload_json: &str) -> Result<(), JsValue> {
    do_post_message_to_host(payload_json).map_err(|e| JsValue::from_str(&e))
}

/// Push a message from host into the runtime queue. Call from host's message handler when forwarding.
/// Returns JSON array of pending messages and clears the queue.
#[wasm_bindgen]
pub fn push_message_from_host(payload_json: &str) {
    MESSAGE_QUEUE.with(|q| q.borrow_mut().push(payload_json.to_string()));
}

fn get_pending_messages_inner() -> String {
    MESSAGE_QUEUE.with(|q| {
        let mut queue = q.borrow_mut();
        let arr: Vec<serde_json::Value> = queue
            .drain(..)
            .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s)))
            .collect();
        serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
    })
}

/// Get pending messages from host (pushed via push_message_from_host) and clear the queue.
/// Returns JSON array of message payloads (parsed when valid JSON, else as string).
#[wasm_bindgen]
pub fn get_pending_messages() -> String {
    get_pending_messages_inner()
}

/// Invoke a handler by ID (from VNode props with type "Handler"). Used when the host
/// receives a click or other event and needs to run the callback (e.g. onClick).
#[wasm_bindgen]
pub fn invoke_handler(handler_id: u32) -> Result<(), JsValue> {
    run_state::invoke_handler(handler_id).map_err(|e| {
        let msg = format!("[relay-runtime] invoke_handler error: {}", e);
        log_error(&msg);
        JsValue::from_str(&msg)
    })?;
    Ok(())
}

/// Returns true if a setter ran since last run (host should re-run and re-render), and clears the flag.
#[wasm_bindgen]
pub fn consume_render_requested() -> bool {
    run_state::consume_render_requested()
}

/// Set current theme, update LAST_STATE, and return single-theme CSS for immediate apply.
#[wasm_bindgen]
pub fn set_theme_and_render(theme_name: &str) -> String {
    let name = theme_name.trim();
    if name.is_empty() {
        return String::new();
    }
    let yaml = crate::session_theme::get_session_theme_yaml();
    let patched = crate::session_theme::patch_current_theme_yaml(&yaml, name);
    if crate::session_theme::looks_like_theme_yaml(&patched) {
        crate::session_theme::set_session_theme_yaml(&patched);
    }
    let css = crate::style_resolve::css_for_forced_theme(&patched, name);
    if !css.trim().is_empty() {
        crate::session_theme::set_forced_theme_css(&css);
        INJECTED_CSS.with(|c| *c.borrow_mut() = css.clone());
    }
    crate::session_theme::set_current_theme_name(name);
    LAST_STATE.with(|c| {
        let state = c.borrow().clone();
        if !state.is_empty() {
            *c.borrow_mut() = set_theme_json(&state, name);
        }
    });
    crate::run_state::set_render_requested();
    css
}

/// Mark the embed client as live (post-hydrate / CSR). CleverScript `isClientLive()`.
#[wasm_bindgen]
pub fn set_client_live(live: bool) {
    crate::session_theme::set_client_live(live);
}

/// Whether interactive controls should enable (`isClientLive()` in CleverScript).
#[wasm_bindgen]
pub fn is_client_live() -> bool {
    crate::session_theme::is_client_live()
}

/// Last theme name passed to `setCurrentTheme` / `set_theme_and_render`.
#[wasm_bindgen]
pub fn get_current_theme_name() -> String {
    crate::session_theme::get_current_theme_name()
}

fn inject_styles_native(args: &[Value]) -> Result<Value, String> {
    let css = args
        .first()
        .and_then(|v| {
            if let Value::String(s) = v {
                Some(s.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();
    INJECTED_CSS.with(|c| *c.borrow_mut() = css);
    Ok(Value::Null)
}

fn themed_styler_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert(
        "get_default_state_json".to_string(),
        Value::Native(|_args: &[Value]| {
            let s = get_default_state_json();
            Ok(Value::String(s))
        }),
    );
    obj.insert(
        "buildThemeJson".to_string(),
        Value::Native(|args: &[Value]| {
            let name = args
                .get(0)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .ok_or("buildThemeJson requires (name, themeData)")?;
            let theme_data = args.get(1).ok_or("buildThemeJson requires (name, themeData)")?;
            let obj = serde_json::json!({ "name": name, "theme": theme_data.to_json() });
            Ok(Value::String(
                serde_json::to_string(&obj).map_err(|e| e.to_string())?,
            ))
        }),
    );
    obj.insert(
        "register_theme_json".to_string(),
        Value::Native(|args: &[Value]| {
            let state = args
                .get(0)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("register_theme_json requires (stateJson, themeJson)")?;
            let theme = args
                .get(1)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("register_theme_json requires (stateJson, themeJson)")?;
            let out = register_theme_json(state, theme);
            Ok(Value::String(out))
        }),
    );
    obj.insert(
        "set_theme_json".to_string(),
        Value::Native(|args: &[Value]| {
            let state = args
                .get(0)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("set_theme_json requires (stateJson, themeName)")?;
            let name = args
                .get(1)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("set_theme_json requires (stateJson, themeName)")?;
            let out = set_theme_json(state, name);
            Ok(Value::String(out))
        }),
    );
    obj.insert(
        "mergeUsageIntoState".to_string(),
        Value::Native(|args: &[Value]| {
            let state = args
                .get(0)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .ok_or("mergeUsageIntoState requires (stateJson, usedTags, usedClasses, usedTagClasses)")?;
            let tags = args.get(1).and_then(|v| {
                if let Value::Array(a) = v {
                    Some(a.iter().filter_map(|x| {
                        if let Value::String(s) = x {
                            Some(s.clone())
                        } else {
                            None
                        }
                    }).collect::<Vec<_>>())
                } else {
                    None
                }
            }).unwrap_or_default();
            let classes = args.get(2).and_then(|v| {
                if let Value::Array(a) = v {
                    Some(a.iter().filter_map(|x| {
                        if let Value::String(s) = x {
                            Some(s.clone())
                        } else {
                            None
                        }
                    }).collect::<Vec<_>>())
                } else {
                    None
                }
            }).unwrap_or_default();
            let tag_classes: Vec<String> = args.get(3).and_then(|v| {
                if let Value::Array(a) = v {
                    Some(a.iter().filter_map(|x| {
                        if let Value::String(s) = x {
                            Some(s.clone())
                        } else {
                            None
                        }
                    }).collect::<Vec<_>>())
                } else {
                    None
                }
            }).unwrap_or_default();
            let mut obj: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&state).map_err(|e| e.to_string())?;
            let mut all_tags: Vec<String> = vec![
                "body".into(), "h1".into(), "p".into(), "div".into(), "pre".into(),
                "span".into(), "button".into(),
            ];
            all_tags.extend(tags);
            all_tags.sort();
            all_tags.dedup();
            let mut all_classes: Vec<String> = vec![
                "test-section".into(), "test-header".into(), "test-output".into(),
                "test-json".into(), "json-toggle".into(), "json-content".into(),
                "error".into(), "visible".into(),
                "theme-toggle".into(), "theme-toggle-btn".into(),
            ];
            all_classes.extend(classes);
            all_classes.sort();
            all_classes.dedup();
            obj.insert("used_tags".to_string(), serde_json::Value::Array(
                all_tags.into_iter().map(serde_json::Value::String).collect()
            ));
            obj.insert("used_classes".to_string(), serde_json::Value::Array(
                all_classes.into_iter().map(serde_json::Value::String).collect()
            ));
            obj.insert("used_tag_classes".to_string(), serde_json::Value::Array(
                tag_classes.into_iter().map(serde_json::Value::String).collect()
            ));
            let out = serde_json::to_string(&serde_json::Value::Object(obj)).map_err(|e| e.to_string())?;
            Ok(Value::String(out))
        }),
    );
    obj.insert(
        "render_css_for_web".to_string(),
        Value::Native(|args: &[Value]| {
            let state = args
                .get(0)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("render_css_for_web requires stateJson")?;
            LAST_STATE.with(|c| *c.borrow_mut() = state.to_string());
            let css = render_css_for_web(state);
            Ok(Value::String(css))
        }),
    );
    obj.insert(
        "injectStyles".to_string(),
        Value::Native(inject_styles_native),
    );
    obj.insert(
        "collectUsage".to_string(),
        Value::Native(|args: &[Value]| {
            let vnode_json = args
                .get(0)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("collectUsage requires vnodeJson string")?;
            let usage = collect_usage_from_vnode_json(vnode_json)
                .map_err(|e| format!("collectUsage error: {}", e))?;
            Ok(Value::String(usage))
        }),
    );
    obj.insert(
        "parseJson".to_string(),
        Value::Native(|args: &[Value]| {
            let s = args
                .get(0)
                .and_then(|v| {
                    if let Value::String(x) = v {
                        Some(x.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("parseJson requires string")?;
            let v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
            Ok(Value::from_json(&v))
        }),
    );
    obj.insert(
        "stringifyJson".to_string(),
        Value::Native(|args: &[Value]| {
            let v = args.get(0).ok_or("stringifyJson requires value")?;
            let s = value_to_json_string(v).map_err(|e| e.to_string())?;
            Ok(Value::String(s))
        }),
    );
    obj.insert(
        "objectEntries".to_string(),
        Value::Native(|args: &[Value]| {
            let obj = args
                .get(0)
                .and_then(|v| {
                    if let Value::Object(o) = v {
                        Some(o)
                    } else {
                        None
                    }
                })
                .ok_or("objectEntries requires object")?;
            let entries: Vec<Value> = obj
                .iter()
                .map(|(k, v)| {
                    Value::Array(vec![Value::String(k.clone()), v.clone()])
                })
                .collect();
            Ok(Value::Array(entries))
        }),
    );
    obj.insert(
        "arrayConcat".to_string(),
        Value::Native(|args: &[Value]| {
            let a = args.get(0).and_then(|v| {
                if let Value::Array(x) = v {
                    Some(x.clone())
                } else {
                    None
                }
            }).unwrap_or_default();
            let b = args.get(1).and_then(|v| {
                if let Value::Array(x) = v {
                    Some(x.clone())
                } else {
                    None
                }
            }).unwrap_or_default();
            let mut out = a;
            out.extend(b);
            Ok(Value::Array(out))
        }),
    );
    Value::Object(obj)
}

fn value_to_json_string(v: &Value) -> Result<String, String> {
    serde_json::to_string(&v.to_json()).map_err(|e| e.to_string())
}

/// Module for import('react'): useState, useEffect, useCallback, useRef, lazy, Suspense.
fn react_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("useState".to_string(), Value::ReactHook("useState".to_string()));
    obj.insert("useEffect".to_string(), Value::ReactHook("useEffect".to_string()));
    obj.insert("useCallback".to_string(), Value::ReactHook("useCallback".to_string()));
    obj.insert("useRef".to_string(), Value::ReactHook("useRef".to_string()));
    obj.insert(
        "lazy".to_string(),
        Value::Native(|args: &[Value]| {
            let factory = args.first().ok_or("lazy requires factory fn")?;
            let Value::Function { params, body, env, is_async, component_hooks, component_module_path } = factory.clone() else {
                return Err("lazy factory must be function".to_string());
            };
            let host = crate::run_state::get_host_for_lazy().ok_or("lazy requires host")?;
            let program = crate::ast::Program { body: vec![] };
            let analysis = crate::analysis::AnalysisResult::default();
            let mut rt = crate::runtime::Runtime::new(program, analysis).with_host(host);
            let module = rt
                .call_value(
                    &Value::Function {
                        params,
                        body,
                        env,
                        is_async,
                        component_hooks,
                        component_module_path,
                    },
                    &[],
                )
                .map_err(|e| e.to_string())?;
            let default = match &module {
                Value::Object(o) => o.get("default").cloned(),
                _ => None,
            };
            Ok(default.unwrap_or(module))
        }),
    );
    obj.insert(
        "Suspense".to_string(),
        Value::Function {
            params: vec![crate::ast::Param {
                name: "props".to_string(),
                rest: false,
                default: None,
                type_annotation: None,
            }],
            body: vec![crate::ast::Statement::Return {
                argument: Some(Box::new(crate::ast::Expr::Member {
                    object: Box::new(crate::ast::Expr::Identifier {
                        name: "props".to_string(),
                    }),
                    property: Box::new(crate::ast::Expr::Identifier {
                        name: "children".to_string(),
                    }),
                    optional: false,
                })),
            }],
            env: crate::value::Env::new(),
            is_async: false,
            component_hooks: Some(0),
            component_module_path: None,
        },
    );
    Value::Object(obj)
}

/// Module for @clevertree/relay-runtime: getThemes, postMessage, getPendingMessages.
fn relay_runtime_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert(
        "getThemes".to_string(),
        Value::Native(|_args: &[Value]| {
            let list = get_themes();
            Ok(Value::String(list))
        }),
    );
    obj.insert(
        "postMessage".to_string(),
        Value::Native(|args: &[Value]| {
            let payload = args.first().map(|v| value_to_json_string(v)).unwrap_or(Ok("null".to_string()))?;
            do_post_message_to_host(&payload).map(|_| Value::Null)
        }),
    );
    obj.insert(
        "getPendingMessages".to_string(),
        Value::Native(|_args: &[Value]| {
            let json = get_pending_messages_inner();
            let v: serde_json::Value = serde_json::from_str(&json).unwrap_or(serde_json::Value::Array(vec![]));
            Ok(Value::from_json(&v))
        }),
    );
    Value::Object(obj)
}

/// Call `globalThis.__relay_host_fetch(url, initJson)` installed by relay-client-web.
/// Returns a JSON string parsed into [`Value`] (must include `ok`, `status`, `text` per [`Host::fetch`]).
fn wasm_host_fetch_via_js(url: &str, options: Option<&Value>) -> Result<Value, String> {
    let init_json = crate::host::fetch_options_json(options);
    let g = js_sys::global();
    let key = JsValue::from_str("__relay_host_fetch");
    let v = js_sys::Reflect::get(&g, &key).map_err(|_| "Reflect.get __relay_host_fetch failed".to_string())?;
    if v.is_undefined() || v.is_null() {
        return Err("__relay_host_fetch is not defined (installRelayHostFetch from relay-runtime-web)".to_string());
    }
    let func = v
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "__relay_host_fetch is not a function".to_string())?;
    let r = func
        .call2(&JsValue::NULL, &JsValue::from_str(url), &JsValue::from_str(&init_json))
        .map_err(|e| format!("__relay_host_fetch threw: {:?}", e))?;
    let s = r
        .as_string()
        .ok_or_else(|| "__relay_host_fetch must return a JSON string".to_string())?;
    let j: serde_json::Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    Ok(Value::from_json(&j))
}

/// Host that adds @clevertree/style, @hooks, relay-runtime/themed-styler, and @clevertree/relay-runtime.
struct WasmHost {
    inner: HostHandle,
    hooks: Option<Value>,
}

impl WasmHost {
    fn resolve_fetch_url(&self, url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            return url.to_string();
        }
        if let Some(Value::Object(map)) = self.hooks.as_ref() {
            if let Some(Value::String(base)) = map.get("relayClientBaseUrl") {
                let b = base.trim_end_matches('/');
                let rel = url.trim_start_matches('/');
                if !b.is_empty() && !rel.is_empty() {
                    return format!("{b}/{rel}");
                }
            }
        }
        url.to_string()
    }
}

impl Host for WasmHost {
    fn log(&self, level: &str, message: &str) {
        let line = format!("[hook] {}", message);
        match level {
            "warn" => console_warn(&line),
            "error" => console_error(&line),
            _ => console_log(&line),
        }
    }

    fn fetch(&self, url: &str, options: Option<&Value>) -> Result<Value, String> {
        wasm_host_fetch_via_js(&self.resolve_fetch_url(url), options)
    }
    fn navigate(&self, path: &str) -> Result<(), String> {
        self.inner.borrow().navigate(path)
    }
    fn create_element(
        &self,
        tag: &str,
        props: &Value,
        children: &[Value],
    ) -> Result<Value, String> {
        self.inner
            .borrow()
            .create_element(tag, props, children)
    }
    fn load_module(&self, path: &str) -> Result<Value, String> {
        if path == "react" || path == "react.js" {
            return Ok(react_module());
        }
        if path == "relay-runtime/themed-styler" || path == "relay-runtime/themed-styler.js" {
            return Ok(themed_styler_module());
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
        if path == "@clevertree/relay-runtime" || path == "relay-runtime" {
            return Ok(relay_runtime_module());
        }
        if path == "@clevertree/markdown"
            || path == "@clevertree/markdown.js"
            || path == "@clevertree/hook-transpiler"
            || path == "@clevertree/hook-transpiler.js"
        {
            return Ok(crate::markdown_host::markdown_module());
        }
        if path == "@clevertree/video" || path == "@clevertree/video.js" {
            return Ok(crate::video_host::video_module());
        }
        self.inner.borrow().load_module(path)
    }
}
