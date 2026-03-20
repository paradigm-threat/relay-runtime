//! WASM exports for relay-runtime.
//!
//! Only compiled for `target_arch = "wasm32"`.
//! Includes theme styling (built-in from themed-styler).

#![cfg(target_arch = "wasm32")]

use crate::host::{Host, HostHandle, ModuleMapHost};
use crate::run_state;
use crate::runtime::Runtime;
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
    let msg = match &e {
        RuntimeError::ParseDetail(d) => match serde_json::to_string(&d.to_json()) {
            Ok(s) => s,
            Err(_) => format!("{}", e),
        },
        _ => format!("{}", e),
    };
    log_error(&format!("[relay-runtime] {}", msg));
    JsValue::from_str(&msg)
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
/// theme_yaml: YAML string (e.g. from theme.yaml). Pass "" if none. Hook can import("@theme").
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

    let theme = if theme_yaml.is_empty() {
        None
    } else {
        serde_yaml::from_str::<serde_json::Value>(theme_yaml)
            .ok()
            .map(|v| Value::from_json(&v))
    };

    let hooks = if hooks_manifest_json.is_empty() {
        None
    } else {
        serde_json::from_str::<serde_json::Value>(hooks_manifest_json)
            .ok()
            .map(|v| Value::from_json(&v))
    };

    let wasm_host = Rc::new(RefCell::new(WasmHost {
        inner: inner.clone(),
        theme,
        hooks,
    }));
    inner.borrow().set_parent_host(wasm_host.clone());

    let result = run_state::run_hook_persistent(source, wasm_host).map_err(|e| {
        let msg = format!("[relay-runtime] Runtime error: {}", e);
        log_error(&msg);
        JsValue::from_str(&msg)
    })?;
    let vnode = value_to_vnode_with_handlers(&result, &mut |v| run_state::register_handler(v)).normalize();

    // Debug: when result normalizes to Empty, log what we got to help diagnose
    if matches!(vnode, crate::vdom::VNode::Empty) {
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

    serde_json::to_string(&vnode).map_err(|e| {
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
#[wasm_bindgen]
pub fn get_injected_css() -> String {
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

/// Set current theme and return new CSS. Updates LAST_STATE and INJECTED_CSS.
#[wasm_bindgen]
pub fn set_theme_and_render(theme_name: &str) -> String {
    let (new_state, css) = LAST_STATE.with(|c| {
        let state = c.borrow().clone();
        if state.is_empty() {
            return (String::new(), String::new());
        }
        let new_state = set_theme_json(&state, theme_name);
        let css = render_css_for_web(&new_state);
        *c.borrow_mut() = new_state.clone();
        (new_state, css)
    });
    if !new_state.is_empty() {
        INJECTED_CSS.with(|c| *c.borrow_mut() = css.clone());
    }
    css
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
        Value::Native(|args: &[Value]| {
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

/// Module for import('react'): useState, useEffect, useCallback as ReactHook values.
fn react_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("useState".to_string(), Value::ReactHook("useState".to_string()));
    obj.insert("useEffect".to_string(), Value::ReactHook("useEffect".to_string()));
    obj.insert("useCallback".to_string(), Value::ReactHook("useCallback".to_string()));
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

/// Host that adds @theme, @hooks, relay-runtime/themed-styler, and @clevertree/relay-runtime.
struct WasmHost {
    inner: HostHandle,
    theme: Option<Value>,
    hooks: Option<Value>,
}

impl Host for WasmHost {
    fn fetch(&self, url: &str, options: Option<&Value>) -> Result<Value, String> {
        self.inner.borrow().fetch(url, options)
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
        if path == "@theme" || path == "./theme.yaml" {
            if let Some(ref t) = self.theme {
                return Ok(t.clone());
            }
            return Err("Theme not loaded".to_string());
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
        self.inner.borrow().load_module(path)
    }
}
