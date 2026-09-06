//! JNI exports for relay-runtime on Android.
//!
//! Mirrors the WASM API: run_hook_with_host, invoke_handler, consume_render_requested, etc.

#![cfg(target_os = "android")]

use crate::host::{fetch_options_json, Host, HostHandle, ModuleMapHost};
use crate::markdown_host;
use crate::run_state;
use crate::style_resolve::{resolve_vnode_styles, Platform};
use crate::vdom::{collect_usage_from_vnode_json, value_to_vnode_with_handlers, VNode};
use crate::value::Value;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jstring};
use jni::JNIEnv;
use serde_json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use themed_styler::{get_default_state_json, get_theme_list_json, register_theme_json, render_css_for_web, set_theme_json};

thread_local! {
    /// Set for the duration of `nativeRunHookWithHost` so [`AndroidHost::fetch`] can call into Java.
    static ANDROID_HOOK_JVM: RefCell<Option<jni::JavaVM>> = RefCell::new(None);
    static INJECTED_CSS: RefCell<String> = RefCell::new(String::new());
    static LAST_STATE: RefCell<String> = RefCell::new(String::new());
    /// Theme name selected by relayRuntime.setTheme() — used on next runRender.
    static CURRENT_THEME: RefCell<String> = RefCell::new("default".to_string());
    /// Last effective theme YAML used for resolve (session or legacy host).
    static LAST_THEME_YAML: RefCell<String> = RefCell::new(String::new());
    /// Last display density from JNI (for setThemeAndRender re-resolve).
    static LAST_DISPLAY_DENSITY: RefCell<f32> = RefCell::new(2.0);
    static LAST_SCALED_DENSITY: RefCell<f32> = RefCell::new(2.0);
    /// Last system dark preference from JNI (UI_MODE_NIGHT).
    static LAST_PREFERS_DARK: RefCell<bool> = RefCell::new(false);
}

/// Hard limits for JNI string payloads (defensive).
const MAX_HOOK_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_MODULE_MAP_JSON_BYTES: usize = 32 * 1024 * 1024;
const MAX_THEME_YAML_BYTES: usize = 2 * 1024 * 1024;
const MAX_HOOKS_MANIFEST_JSON_BYTES: usize = 4 * 1024 * 1024;

fn jstring_to_string(env: &mut JNIEnv, input: JString) -> Option<String> {
    if input.is_null() {
        return None;
    }
    env.get_string(&input).ok().and_then(|j| j.to_str().ok().map(|s| s.to_string()))
}

fn new_jstring(env: &mut JNIEnv, value: &str) -> jstring {
    env.new_string(value).ok().map(|j| j.into_raw()).unwrap_or(std::ptr::null_mut())
}

fn value_to_json_string(v: &Value) -> Result<String, String> {
    serde_json::to_string(&v.to_json()).map_err(|e| e.to_string())
}

fn themed_styler_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("get_default_state_json".to_string(), Value::Native(|_args: &[Value]| Ok(Value::String(get_default_state_json()))));
    obj.insert(
        "buildThemeJson".to_string(),
        Value::Native(|args: &[Value]| {
            let name = args.get(0).and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }).ok_or("buildThemeJson requires (name, themeData)")?;
            let theme_data = args.get(1).ok_or("buildThemeJson requires (name, themeData)")?;
            let o = serde_json::json!({ "name": name, "theme": theme_data.to_json() });
            Ok(Value::String(serde_json::to_string(&o).map_err(|e| e.to_string())?))
        }),
    );
    obj.insert(
        "register_theme_json".to_string(),
        Value::Native(|args: &[Value]| {
            let state = args.get(0).and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None }).ok_or("register_theme_json requires (stateJson, themeJson)")?;
            let theme = args.get(1).and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None }).ok_or("register_theme_json requires (stateJson, themeJson)")?;
            Ok(Value::String(register_theme_json(state, theme)))
        }),
    );
    obj.insert(
        "set_theme_json".to_string(),
        Value::Native(|args: &[Value]| {
            let state = args.get(0).and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None }).ok_or("set_theme_json requires (stateJson, themeName)")?;
            let name = args.get(1).and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None }).ok_or("set_theme_json requires (stateJson, themeName)")?;
            Ok(Value::String(set_theme_json(state, name)))
        }),
    );
    obj.insert(
        "mergeUsageIntoState".to_string(),
        Value::Native(|args: &[Value]| {
            let state = args.get(0).and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }).ok_or("mergeUsageIntoState requires (stateJson, ...)")?;
            let tags: Vec<String> = args.get(1).and_then(|v| if let Value::Array(a) = v { Some(a.iter().filter_map(|x| if let Value::String(s) = x { Some(s.clone()) } else { None }).collect()) } else { None }).unwrap_or_default();
            let classes: Vec<String> = args.get(2).and_then(|v| if let Value::Array(a) = v { Some(a.iter().filter_map(|x| if let Value::String(s) = x { Some(s.clone()) } else { None }).collect()) } else { None }).unwrap_or_default();
            let tag_classes: Vec<String> = args.get(3).and_then(|v| if let Value::Array(a) = v { Some(a.iter().filter_map(|x| if let Value::String(s) = x { Some(s.clone()) } else { None }).collect()) } else { None }).unwrap_or_default();
            let mut obj: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&state).map_err(|e| e.to_string())?;
            let mut all_tags: Vec<String> = vec!["body".into(), "h1".into(), "p".into(), "div".into(), "pre".into(), "span".into(), "button".into()];
            all_tags.extend(tags);
            all_tags.sort();
            all_tags.dedup();
            let mut all_classes: Vec<String> = vec!["test-section".into(), "test-header".into(), "test-output".into(), "test-json".into(), "json-toggle".into(), "json-content".into(), "error".into(), "visible".into(), "theme-toggle".into(), "theme-toggle-btn".into()];
            all_classes.extend(classes);
            all_classes.sort();
            all_classes.dedup();
            obj.insert("used_tags".to_string(), serde_json::Value::Array(all_tags.into_iter().map(serde_json::Value::String).collect()));
            obj.insert("used_classes".to_string(), serde_json::Value::Array(all_classes.into_iter().map(serde_json::Value::String).collect()));
            obj.insert("used_tag_classes".to_string(), serde_json::Value::Array(tag_classes.into_iter().map(serde_json::Value::String).collect()));
            Ok(Value::String(serde_json::to_string(&serde_json::Value::Object(obj)).map_err(|e| e.to_string())?))
        }),
    );
    obj.insert(
        "render_css_for_web".to_string(),
        Value::Native(|args: &[Value]| {
            let state = args.get(0).and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None }).ok_or("render_css_for_web requires stateJson")?;
            LAST_STATE.with(|c| *c.borrow_mut() = state.to_string());
            Ok(Value::String(render_css_for_web(state)))
        }),
    );
    obj.insert(
        "injectStyles".to_string(),
        Value::Native(|args: &[Value]| {
            let css = args.get(0).and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }).unwrap_or_default();
            INJECTED_CSS.with(|c| *c.borrow_mut() = css);
            Ok(Value::Null)
        }),
    );
    obj.insert(
        "collectUsage".to_string(),
        Value::Native(|args: &[Value]| {
            let vnode_json = args.get(0).and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None }).ok_or("collectUsage requires vnodeJson string")?;
            Ok(Value::String(collect_usage_from_vnode_json(vnode_json).map_err(|e| format!("collectUsage error: {}", e))?))
        }),
    );
    obj.insert(
        "parseJson".to_string(),
        Value::Native(|args: &[Value]| {
            let s = args.get(0).and_then(|v| if let Value::String(x) = v { Some(x.as_str()) } else { None }).ok_or("parseJson requires string")?;
            let v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
            Ok(Value::from_json(&v))
        }),
    );
    obj.insert(
        "stringifyJson".to_string(),
        Value::Native(|args: &[Value]| {
            let v = args.get(0).ok_or("stringifyJson requires value")?;
            Ok(Value::String(value_to_json_string(v).map_err(|e| e.to_string())?))
        }),
    );
    obj.insert(
        "objectEntries".to_string(),
        Value::Native(|args: &[Value]| {
            let obj = args.get(0).and_then(|v| if let Value::Object(o) = v { Some(o) } else { None }).ok_or("objectEntries requires object")?;
            let entries: Vec<Value> = obj.iter().map(|(k, v)| Value::Array(vec![Value::String(k.clone()), v.clone()])).collect();
            Ok(Value::Array(entries))
        }),
    );
    obj.insert(
        "arrayConcat".to_string(),
        Value::Native(|args: &[Value]| {
            let a = args.get(0).and_then(|v| if let Value::Array(x) = v { Some(x.clone()) } else { None }).unwrap_or_default();
            let b = args.get(1).and_then(|v| if let Value::Array(x) = v { Some(x.clone()) } else { None }).unwrap_or_default();
            let mut out = a;
            out.extend(b);
            Ok(Value::Array(out))
        }),
    );
    Value::Object(obj)
}

fn react_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("useState".to_string(), Value::ReactHook("useState".to_string()));
    obj.insert("useEffect".to_string(), Value::ReactHook("useEffect".to_string()));
    obj.insert("useCallback".to_string(), Value::ReactHook("useCallback".to_string()));
    obj.insert("useRef".to_string(), Value::ReactHook("useRef".to_string()));
    // lazy(fn): sync eager load - fn() returns module, we return default export
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
            let module = rt.call_value(
                &Value::Function {
                    params,
                    body,
                    env,
                    is_async,
                    component_hooks,
                    component_module_path,
                },
                &[],
            ).map_err(|e| e.to_string())?;
            let default = match &module {
                Value::Object(o) => o.get("default").cloned(),
                _ => None,
            };
            Ok(default.unwrap_or(module))
        }),
    );
    // Suspense: passthrough - render children (no async boundary)
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
                    computed: false,
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


fn relay_runtime_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("getThemes".to_string(), Value::Native(|_args: &[Value]| Ok(Value::String(get_themes()))));
    obj.insert(
        "setTheme".to_string(),
        Value::Native(|args: &[Value]| {
            let name = args.first().and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }).unwrap_or_else(|| "default".to_string());
            CURRENT_THEME.with(|c| *c.borrow_mut() = name.clone());
            run_state::set_render_requested();
            Ok(Value::Null)
        }),
    );
    obj.insert(
        "getCurrentTheme".to_string(),
        Value::Native(|_args: &[Value]| {
            let name = CURRENT_THEME.with(|c| c.borrow().clone());
            Ok(Value::String(name))
        }),
    );
    obj.insert(
        "postMessage".to_string(),
        Value::Native(|args: &[Value]| {
            let payload = args.first().map(|v| value_to_json_string(v)).unwrap_or(Ok("null".to_string()))?;
            let _ = payload;
            Ok(Value::Null)
        }),
    );
    obj.insert(
        "getPendingMessages".to_string(),
        Value::Native(|_args: &[Value]| Ok(Value::Array(vec![]))),
    );
    Value::Object(obj)
}

fn get_themes() -> String {
    LAST_STATE.with(|c| {
        let state = c.borrow();
        if state.is_empty() {
            "[]".to_string()
        } else {
            get_theme_list_json(state.as_str())
        }
    })
}

/// Clears [`ANDROID_HOOK_JVM`] when dropped (end of hook run).
struct AndroidHookJvmGuard;

impl AndroidHookJvmGuard {
    fn arm(jvm: jni::JavaVM) -> Self {
        ANDROID_HOOK_JVM.with(|c| *c.borrow_mut() = Some(jvm));
        Self
    }
}

impl Drop for AndroidHookJvmGuard {
    fn drop(&mut self) {
        ANDROID_HOOK_JVM.with(|c| *c.borrow_mut() = None);
    }
}

/// `RelayRuntimeHostHttp.hostHttpFetchSync(url, optionsJson)` → JSON string matching web `__relay_host_fetch`.
fn android_host_http_fetch_sync(url: &str, options_json: &str) -> Result<String, String> {
    use jni::objects::JValue;
    ANDROID_HOOK_JVM.with(|cell| {
        let vm = cell.borrow();
        let vm = vm.as_ref().ok_or_else(|| {
            "Host.fetch outside nativeRunHookWithHost (JavaVM not set)".to_string()
        })?;
        let mut env = vm
            .attach_current_thread_as_daemon()
            .map_err(|e| format!("attach_current_thread: {:?}", e))?;
        let cls = env.find_class("com/clevertree/relayruntime/RelayRuntimeHostHttp").map_err(|_| {
            "com.clevertree.relayruntime.RelayRuntimeHostHttp not on classpath (see relay-runtime/support/android/)"
                .to_string()
        })?;
        let ju = env.new_string(url).map_err(|e| format!("new_string url: {:?}", e))?;
        let jo = env
            .new_string(options_json)
            .map_err(|e| format!("new_string options: {:?}", e))?;
        let out = env
            .call_static_method(
                cls,
                "hostHttpFetchSync",
                "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&ju), JValue::Object(&jo)],
            )
            .map_err(|e| format!("hostHttpFetchSync: {:?}", e))?;
        let jstr = jni::objects::JString::from(
            out.l()
                .map_err(|e| format!("hostHttpFetchSync return: {:?}", e))?,
        );
        let s = env
            .get_string(&jstr)
            .map_err(|e| format!("get_string: {:?}", e))?;
        Ok(s.into())
    })
}

struct AndroidHost {
    inner: HostHandle,
    hooks: Option<Value>,
}

impl AndroidHost {
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

impl Host for AndroidHost {
    fn log(&self, level: &str, message: &str) {
        println!("[relay-runtime:{}] {}", level, message);
    }

    fn fetch(&self, url: &str, options: Option<&Value>) -> Result<Value, String> {
        let opts = fetch_options_json(options);
        let resolved = self.resolve_fetch_url(url);
        let json_str = android_host_http_fetch_sync(&resolved, &opts)?;
        let j: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
        Ok(Value::from_json(&j))
    }
    fn navigate(&self, _path: &str) -> Result<(), String> {
        Err("navigate not implemented on Android".to_string())
    }
    fn create_element(&self, tag: &str, props: &Value, children: &[Value]) -> Result<Value, String> {
        // Build element Value (JSX typically uses eval_jsx_element which doesn't call this;
        // this path is for explicit createElement() calls)
        let mut obj = HashMap::new();
        obj.insert("type".to_string(), Value::String("element".to_string()));
        obj.insert("tag".to_string(), Value::String(tag.to_string()));
        obj.insert("props".to_string(), props.clone());
        obj.insert("children".to_string(), Value::Array(children.to_vec()));
        Ok(Value::Object(obj))
    }
    fn load_module(&self, path: &str) -> Result<Value, String> {
        if path == "react" || path == "react.js" {
            return Ok(react_module());
        }
        if path == "relay-runtime/themed-styler" || path == "relay-runtime/themed-styler.js" {
            return Ok(themed_styler_module());
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
        if path == "@clevertree/relay-runtime" || path == "relay-runtime" {
            return Ok(relay_runtime_module());
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
        self.inner.borrow().load_module(path)
    }
}

/// Run hook with module map, theme, and hooks manifest. Returns VNode JSON or throws on error.
/// theme_name: "light", "dark", or "default" (uses prefers_dark for default)
/// prefers_dark: when theme is "default", true = dark mode
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeRunHookWithHost(
    mut env: JNIEnv,
    _class: JClass,
    source: JString,
    module_map_json: JString,
    theme_yaml: JString,
    hooks_manifest_json: JString,
    file: JString,
    theme_name: JString,
    prefers_dark: jboolean,
    display_density: jni::sys::jfloat,
    scaled_density: jni::sys::jfloat,
) -> jstring {
    let source = match jstring_to_string(&mut env, source) {
        Some(s) => s,
        None => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "source was null");
            return std::ptr::null_mut();
        }
    };
    if source.len() > MAX_HOOK_SOURCE_BYTES {
        let _ = env.throw_new(
            "java/lang/IllegalArgumentException",
            &format!("source exceeds {} bytes", MAX_HOOK_SOURCE_BYTES),
        );
        return std::ptr::null_mut();
    }
    let module_map_json = jstring_to_string(&mut env, module_map_json).unwrap_or_else(|| "{}".to_string());
    if module_map_json.len() > MAX_MODULE_MAP_JSON_BYTES {
        let _ = env.throw_new(
            "java/lang/IllegalArgumentException",
            &format!("moduleMapJson exceeds {} bytes", MAX_MODULE_MAP_JSON_BYTES),
        );
        return std::ptr::null_mut();
    }
    let theme_yaml = jstring_to_string(&mut env, theme_yaml).unwrap_or_default();
    if theme_yaml.len() > MAX_THEME_YAML_BYTES {
        let _ = env.throw_new(
            "java/lang/IllegalArgumentException",
            &format!("themeYaml exceeds {} bytes", MAX_THEME_YAML_BYTES),
        );
        return std::ptr::null_mut();
    }
    let hooks_manifest_json = jstring_to_string(&mut env, hooks_manifest_json).unwrap_or_else(|| "{}".to_string());
    if hooks_manifest_json.len() > MAX_HOOKS_MANIFEST_JSON_BYTES {
        let _ = env.throw_new(
            "java/lang/IllegalArgumentException",
            &format!("hooksManifestJson exceeds {} bytes", MAX_HOOKS_MANIFEST_JSON_BYTES),
        );
        return std::ptr::null_mut();
    }
    let file = jstring_to_string(&mut env, file).unwrap_or_default();
    let theme_name_str = jstring_to_string(&mut env, theme_name)
        .filter(|s| !s.is_empty())
        .or_else(|| Some(CURRENT_THEME.with(|c| c.borrow().clone())))
        .unwrap_or_else(|| "default".to_string());
    let prefers_dark_bool = prefers_dark == jni::sys::JNI_TRUE;

    let mut d_density = display_density;
    if !d_density.is_finite() || d_density <= 0.0 {
        d_density = 1.0;
    }
    let mut s_density = scaled_density;
    if !s_density.is_finite() || s_density <= 0.0 {
        s_density = d_density;
    }
    LAST_DISPLAY_DENSITY.with(|c| *c.borrow_mut() = d_density);
    LAST_SCALED_DENSITY.with(|c| *c.borrow_mut() = s_density);
    LAST_PREFERS_DARK.with(|c| *c.borrow_mut() = prefers_dark_bool);

    INJECTED_CSS.with(|c| *c.borrow_mut() = String::new());

    let modules: HashMap<String, String> = serde_json::from_str(&module_map_json).unwrap_or_else(|_| HashMap::new());
    let inner = ModuleMapHost::new(modules);

    let hooks = if hooks_manifest_json.is_empty() {
        None
    } else {
        serde_json::from_str::<serde_json::Value>(&hooks_manifest_json)
            .ok()
            .map(|v| Value::from_json(&v))
    };

    let android_host = Rc::new(RefCell::new(AndroidHost {
        inner: inner.clone(),
        hooks,
    }));
    inner.borrow().set_parent_host(android_host.clone());

    let jvm = match env.get_java_vm() {
        Ok(v) => v,
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                &format!("get_java_vm failed: {:?}", e),
            );
            return std::ptr::null_mut();
        }
    };
    let _jvm_guard = AndroidHookJvmGuard::arm(jvm);

    match run_state::run_hook_persistent(&source, android_host) {
        Ok(result) => {
            run_state::clear_handler_registry_for_vnode_pass();
            let vnode = value_to_vnode_with_handlers(&result, &mut |v| run_state::register_handler(v)).normalize();
            let theme_arg = if theme_name_str.is_empty() { None } else { Some(theme_name_str.as_str()) };
            let effective_yaml = crate::session_theme::effective_theme_yaml(&theme_yaml);
            crate::session_theme::store_unresolved_vnode(vnode.clone());
            let (resolved, _, state_opt) = resolve_vnode_styles(
                vnode,
                &effective_yaml,
                Platform::Android,
                d_density,
                s_density,
                theme_arg,
                prefers_dark_bool,
            );
            LAST_THEME_YAML.with(|c| *c.borrow_mut() = effective_yaml);
            if let Some(ref state_json) = state_opt {
                LAST_STATE.with(|c| *c.borrow_mut() = state_json.clone());
            }
            match serde_json::to_string(&resolved) {
                Ok(json) => new_jstring(&mut env, &json),
                Err(e) => {
                    let _ = env.throw_new("java/lang/RuntimeException", &format!("Serialize error: {}", e));
                    std::ptr::null_mut()
                }
            }
        }
        Err(e) => {
            let json = e.to_error_json();
            let msg = format!("RELAY_ERROR_JSON:{}", serde_json::to_string(&json).unwrap_or_else(|_| "{}".into()));
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            std::ptr::null_mut()
        }
    }
}

/// Invoke handler by ID. Call when user taps a view with onClick etc.
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeInvokeHandler(
    mut env: JNIEnv,
    _class: JClass,
    handler_id: jni::sys::jint,
) -> jboolean {
    match run_state::invoke_handler(handler_id as u32) {
        Ok(_) => jni::sys::JNI_TRUE,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &format!("{}", e));
            jni::sys::JNI_FALSE
        }
    }
}

/// Returns true if a setState ran since last run (host should re-run and re-render). Clears the flag.
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeConsumeRenderRequested(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if run_state::consume_render_requested() {
        jni::sys::JNI_TRUE
    } else {
        jni::sys::JNI_FALSE
    }
}

/// Set current theme without triggering render. Call on init to restore saved theme.
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeSetTheme(
    mut env: JNIEnv,
    _class: JClass,
    theme_name: JString,
) {
    if let Some(name) = jstring_to_string(&mut env, theme_name) {
        if !name.is_empty() {
            CURRENT_THEME.with(|c| *c.borrow_mut() = name);
        }
    }
}

/// Get current theme name (from setTheme). Used by host before runHookWithHost.
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeGetCurrentTheme(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let name = CURRENT_THEME.with(|c| c.borrow().clone());
    new_jstring(&mut env, &name)
}

/// Return CSS injected by the hook via injectStyles(). May be empty on Android (uses native styles).
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeGetInjectedCss(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let css = INJECTED_CSS.with(|c| c.borrow().clone());
    new_jstring(&mut env, &css)
}

/// Get theme list JSON. Call after runHookWithHost.
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeGetThemes(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    new_jstring(&mut env, &get_themes())
}

/// Set current theme. On Android: re-resolves stored VNode with new theme, returns VNode JSON
/// (no hook re-run = one rerender). Call render() on the returned JSON.
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeSetThemeAndRender(
    mut env: JNIEnv,
    _class: JClass,
    theme_name: JString,
) -> jstring {
    let name = jstring_to_string(&mut env, theme_name).unwrap_or_default();
    CURRENT_THEME.with(|c| *c.borrow_mut() = name.clone());
    let vnode_json = {
        let theme_yaml = LAST_THEME_YAML.with(|c| c.borrow().clone());
        let vnode_opt = crate::session_theme::peek_unresolved_vnode();
        match vnode_opt {
            Some(vnode) if !theme_yaml.is_empty() => {
                let theme_arg = if name.is_empty() { None } else { Some(name.as_str()) };
                let display_d = LAST_DISPLAY_DENSITY.with(|c| *c.borrow());
                let scaled_d = LAST_SCALED_DENSITY.with(|c| *c.borrow());
                let prefers_dark = LAST_PREFERS_DARK.with(|c| *c.borrow());
                let (resolved, _, state_opt) = resolve_vnode_styles(
                    vnode,
                    &theme_yaml,
                    Platform::Android,
                    display_d,
                    scaled_d,
                    theme_arg,
                    prefers_dark,
                );
                if let Some(s) = state_opt {
                    LAST_STATE.with(|c| *c.borrow_mut() = s);
                }
                serde_json::to_string(&resolved).unwrap_or_default()
            }
            _ => String::new(),
        }
    };
    new_jstring(&mut env, &vnode_json)
}

/// Progressive hydrate phase 0..=4 (same ladder as web `set_client_live_phase`).
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeSetClientLivePhase(
    _env: JNIEnv,
    _class: JClass,
    phase: jint,
) {
    let p = if phase < 0 { 0u8 } else { (phase as u32).min(255) as u8 };
    crate::session_theme::set_client_live_phase(p);
}

#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeGetClientLivePhase(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    crate::session_theme::get_client_live_phase() as jint
}

/// Backward-compatible shell-live flag (phase ≥ 1).
#[no_mangle]
pub extern "system" fn Java_com_clevertree_relayruntime_RelayRuntime_nativeSetClientLive(
    _env: JNIEnv,
    _class: JClass,
    live: jboolean,
) {
    crate::session_theme::set_client_live(live != 0);
}

