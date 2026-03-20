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

/// Run hook with persistent state. First call creates RunState; subsequent
/// calls with the same source reuse it so useState state persists.
pub fn run_hook_persistent(
    source: &str,
    host: HostHandle,
) -> Result<crate::value::Value, RuntimeError> {
    let program = crate::parse(source)?;
    let analysis = crate::analyze(&program).map_err(|e| RuntimeError::Runtime(e.to_string()))?;

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
        runtime.run()
    })
}

