//! RunState persistence for re-renders.
//!
//! Stores program, analysis, component instances, and host across
//! run_hook_with_host calls so that useState state persists when re-running.

use crate::analysis::AnalysisResult;
use crate::ast::Program;
use crate::error::RuntimeError;
use crate::host::HostHandle;
use crate::runtime::{ComponentInstance, Runtime};
use crate::value::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

thread_local! {
    static RUN_STATE: RefCell<Option<RunState>> = RefCell::new(None);
    static HANDLER_REGISTRY: RefCell<HashMap<u32, Value>> = RefCell::new(HashMap::new());
    static NEXT_HANDLER_ID: Cell<u32> = Cell::new(1);
    /// Set when a setter runs; host checks via consume_render_requested() to re-run.
    static RENDER_REQUESTED: Cell<bool> = Cell::new(false);
    /// In-memory localStorage for hooks (getItem/setItem).
    static LOCAL_STORAGE: RefCell<std::collections::HashMap<String, String>> = RefCell::new(std::collections::HashMap::new());
    /// Host for lazy() during first run when RunState not yet set.
    pub(crate) static HOST_FOR_LAZY: RefCell<Option<HostHandle>> = RefCell::new(None);
}

/// Build Array object with isArray for injection into runtime env.
pub fn array_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "isArray".to_string(),
        Value::Native(|args: &[Value]| {
            let v = args.first();
            Ok(Value::Bool(matches!(v, Some(Value::Array(_)))))
        }),
    );
    Value::Object(obj)
}

/// Build Date object with now() for peer tab IDs etc.
pub fn date_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "now".to_string(),
        Value::Native(|_args: &[Value]| {
            Ok(Value::Number(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as f64,
            ))
        }),
    );
    Value::Object(obj)
}

/// Build Object with keys, entries for injection into runtime env.
pub fn object_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "keys".to_string(),
        Value::Native(|args: &[Value]| {
            let o = args.first().and_then(|v| if let Value::Object(x) = v { Some(x) } else { None }).ok_or("Object.keys requires object")?;
            let keys: Vec<Value> = o.keys().map(|k| Value::String(k.clone())).collect();
            Ok(Value::Array(keys))
        }),
    );
    obj.insert(
        "entries".to_string(),
        Value::Native(|args: &[Value]| {
            let o = args.first().and_then(|v| if let Value::Object(x) = v { Some(x) } else { None }).ok_or("Object.entries requires object")?;
            let entries: Vec<Value> = o.iter().map(|(k, v)| Value::Array(vec![Value::String(k.clone()), v.clone()])).collect();
            Ok(Value::Array(entries))
        }),
    );
    Value::Object(obj)
}

/// Build JSON object with parse/stringify for injection into runtime env.
pub fn json_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "parse".to_string(),
        Value::Native(|args: &[Value]| {
            let s = args
                .first()
                .and_then(|v| if let Value::String(x) = v { Some(x.as_str()) } else { None })
                .ok_or("JSON.parse requires string")?;
            let v: serde_json::Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
            Ok(Value::from_json(&v))
        }),
    );
    obj.insert(
        "stringify".to_string(),
        Value::Native(|args: &[Value]| {
            let v = args.first().ok_or("JSON.stringify requires value")?;
            Ok(Value::String(
                serde_json::to_string(&v.to_json()).unwrap_or_else(|_| "null".to_string()),
            ))
        }),
    );
    Value::Object(obj)
}

/// Build localStorage object for injection into runtime env.
pub fn localStorage_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "getItem".to_string(),
        Value::Native(|args: &[Value]| {
            let key = args
                .first()
                .and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None })
                .unwrap_or("");
            let val = LOCAL_STORAGE.with(|m| m.borrow().get(key).cloned());
            Ok(Value::String(val.unwrap_or_default()))
        }),
    );
    obj.insert(
        "setItem".to_string(),
        Value::Native(|args: &[Value]| {
            let key = args
                .get(0)
                .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                .unwrap_or_default();
            let val = args
                .get(1)
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => format!("{}", n),
                    Value::Bool(b) => format!("{}", b),
                    Value::Null => "null".to_string(),
                    _ => "".to_string(),
                })
                .unwrap_or_default();
            LOCAL_STORAGE.with(|m| m.borrow_mut().insert(key, val));
            Ok(Value::Null)
        }),
    );
    Value::Object(obj)
}

/// Shared map of module path -> component instance for useState persistence in loaded hooks.
pub type LoadedInstances = Rc<RefCell<HashMap<String, Rc<RefCell<ComponentInstance>>>>>;

struct RunState {
    program: Program,
    analysis: AnalysisResult,
    default_instance: Rc<RefCell<ComponentInstance>>,
    loaded_instances: LoadedInstances,
    host: HostHandle,
    /// Source string; when it changes, we reset.
    source: String,
}

/// Clear RunState (e.g. for tests or when loading a new app).
pub fn clear_run_state() {
    RUN_STATE.with(|r| *r.borrow_mut() = None);
    HANDLER_REGISTRY.with(|h| h.borrow_mut().clear());
    NEXT_HANDLER_ID.with(|n| n.set(1));
    RENDER_REQUESTED.with(|r| r.set(false));
}

/// Called when a setter runs; signals the host to re-run and re-render.
pub fn set_render_requested() {
    RENDER_REQUESTED.with(|r| r.set(true));
}

/// Returns true if a setter ran since last run, and clears the flag.
pub fn consume_render_requested() -> bool {
    RENDER_REQUESTED.with(|r| r.replace(false))
}

/// Host for lazy() imports; from RunState when set, else HOST_FOR_LAZY.
pub fn get_host_for_lazy() -> Option<HostHandle> {
    RUN_STATE
        .with(|r| r.borrow().as_ref().map(|rs| rs.host.clone()))
        .or_else(|| HOST_FOR_LAZY.with(|h| h.borrow().clone()))
}

/// Clear handler IDs accumulated during the previous VNode walk so only the **current** tree’s
/// handlers remain registered. Call immediately before each `value_to_vnode_with_handlers` pass
/// after `run_hook_persistent` returns (same hook source, re-render).
pub fn clear_handler_registry_for_vnode_pass() {
    HANDLER_REGISTRY.with(|h| h.borrow_mut().clear());
    NEXT_HANDLER_ID.with(|n| n.set(1));
}

/// Register a handler (Function or Setter) and return its ID for use in VNode props.
pub fn register_handler(v: &Value) -> u32 {
    let id = NEXT_HANDLER_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    HANDLER_REGISTRY.with(|h| h.borrow_mut().insert(id, v.clone()));
    id
}

/// Invoke a handler by ID. Uses RunState's Runtime to call the handler.
pub fn invoke_handler(id: u32) -> Result<Value, RuntimeError> {
    let handler = HANDLER_REGISTRY
        .with(|h| h.borrow().get(&id).cloned())
        .ok_or_else(|| RuntimeError::Runtime(format!("Handler {} not found", id)))?;

    RUN_STATE.with(|r| {
        let state = r.borrow();
        let rs = state.as_ref().ok_or_else(|| {
            RuntimeError::Runtime("No RunState (invoke_handler called before run)".to_string())
        })?;
        let mut runtime = Runtime::new(rs.program.clone(), rs.analysis.clone())
            .with_host(rs.host.clone())
            .with_loaded_instances(rs.loaded_instances.clone());
        runtime.default_instance = Some(rs.default_instance.clone());
        runtime.call_value(&handler, &[])
    })
}

/// Invoke a CleverScript/native function value with explicit arguments (e.g. per-anchor `href`).
/// Used by markdown `anchorOnClick` wrappers; same RunState / `Runtime` setup as [`invoke_handler`].
pub fn invoke_handler_value(f: &Value, args: &[Value]) -> Result<Value, RuntimeError> {
    RUN_STATE.with(|r| {
        let state = r.borrow();
        let rs = state.as_ref().ok_or_else(|| {
            RuntimeError::Runtime("No RunState (invoke_handler_value called before run)".to_string())
        })?;
        let mut runtime = Runtime::new(rs.program.clone(), rs.analysis.clone())
            .with_host(rs.host.clone())
            .with_loaded_instances(rs.loaded_instances.clone());
        runtime.default_instance = Some(rs.default_instance.clone());
        runtime.call_value(f, args)
    })
}

/// Run hook with persistent state. First call creates RunState; subsequent
/// calls with the same source reuse it so useState state persists.
pub fn run_hook_persistent(
    source: &str,
    host: HostHandle,
) -> Result<crate::value::Value, RuntimeError> {
    let program = crate::parse(source)?;
    let analysis = crate::analyze(&program).map_err(|e| RuntimeError::Runtime(e.to_string()))?;

    HOST_FOR_LAZY.with(|h| *h.borrow_mut() = Some(host.clone()));

    RUN_STATE.with(|r| {
        let mut state = r.borrow_mut();
        if let Some(ref rs) = *state {
            if rs.source != source {
                *state = None;
                HANDLER_REGISTRY.with(|h| h.borrow_mut().clear());
                NEXT_HANDLER_ID.with(|n| n.set(1));
            }
        }

        if state.is_none() {
            HANDLER_REGISTRY.with(|h| h.borrow_mut().clear());
            NEXT_HANDLER_ID.with(|n| n.set(1));
            let loaded = Rc::new(RefCell::new(HashMap::new()));
            let mut runtime = Runtime::new(program.clone(), analysis.clone())
                .with_host(host.clone())
                .with_loaded_instances(loaded.clone());
            runtime.env.borrow_mut().set("localStorage", localStorage_value());
            runtime.env.borrow_mut().set("Array", array_value());
            runtime.env.borrow_mut().set("JSON", json_value());
            runtime.env.borrow_mut().set("Object", object_value());
            runtime.env.borrow_mut().set("Date", date_value());
            let result = runtime.run()?;
            let default_instance = runtime
                .default_instance
                .clone()
                .expect("run creates default_instance");
            *state = Some(RunState {
                program,
                analysis,
                default_instance,
                loaded_instances: loaded,
                host,
                source: source.to_string(),
            });
            return Ok(result);
        }

        let rs = state.as_ref().unwrap();
        let mut runtime = Runtime::new(rs.program.clone(), rs.analysis.clone())
            .with_host(rs.host.clone())
            .with_loaded_instances(rs.loaded_instances.clone());
        runtime.default_instance = Some(rs.default_instance.clone());
        runtime.env.borrow_mut().set("localStorage", localStorage_value());
        runtime.env.borrow_mut().set("Array", array_value());
        runtime.env.borrow_mut().set("JSON", json_value());
        runtime.env.borrow_mut().set("Object", object_value());
        runtime.env.borrow_mut().set("Date", date_value());
        runtime.run()
    })
}

#[cfg(test)]
mod handler_registry_tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn clear_handler_registry_for_vnode_pass_resets_ids() {
        clear_handler_registry_for_vnode_pass();
        let a = register_handler(&Value::Native(|_| Ok(Value::Null)));
        let b = register_handler(&Value::Native(|_| Ok(Value::Null)));
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        clear_handler_registry_for_vnode_pass();
        let c = register_handler(&Value::Native(|_| Ok(Value::Null)));
        assert_eq!(c, 1);
    }
}

