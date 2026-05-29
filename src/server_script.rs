//! One-shot CleverScript execution for server hooks (e.g. QUERY) without `RUN_STATE` persistence.

use crate::ast::{ExportDefaultDecl, Statement};
use crate::error::RuntimeError;
use crate::host::HostHandle;
use crate::runtime::Runtime;
use crate::run_state::{array_value, date_value, json_value, localStorage_value, object_value};
use crate::value::Value;

/// Run a CleverScript module whose **default export** is a function. The first parameter is bound
/// to `context` (as [`Value::from_json`]). When the export lists a **second** parameter, it is set
/// to `hook_next_fn.clone()` if `hook_next_fn` is `Some`, else [`Value::Null`] — Express-style
/// `export default function run(ctx, next) { return next(); }` on GET hooks passes a native
/// `next` from the gateway. Hooks with only `(ctx)` ignore the extra `hook_next_fn` argument at
/// binding time (the second parameter is omitted from the signature).
///
/// Returns the value of the first `return` in that function (typically `{ rows: [...] }` for QUERY).
///
/// When `relay` is `Some`, it is installed as the global `Relay` object (legacy / tests). The
/// relay-gateway server hooks instead pass `None` and expose **`@relay/git`** / **`@relay/image`**
/// only through [`Host::load_module`](crate::host::Host::load_module).
///
/// Does not use [`crate::run_state::run_hook_persistent`] so it does not touch thread-local hook
/// state used by client render persistence.
pub fn run_query_script(
    source: &str,
    host: HostHandle,
    context: &serde_json::Value,
    relay: Option<Value>,
    hook_next_fn: Option<Value>,
) -> Result<Value, RuntimeError> {
    let program = crate::parse(source)?;
    let analysis = crate::analyze(&program)?;
    let mut runtime = Runtime::new(program, analysis).with_host(host);
    {
        let mut env = runtime.env.borrow_mut();
        env.set("localStorage", localStorage_value());
        env.set("Array", array_value());
        env.set("JSON", json_value());
        env.set("Object", object_value());
        env.set("Date", date_value());
        match relay {
            Some(r) => env.set("Relay", r),
            None => env.set("Relay", Value::Null),
        }
    }
    bind_default_export_params(&runtime, context, hook_next_fn.as_ref())?;
    runtime.run()
}

fn bind_default_export_params(
    runtime: &Runtime,
    context: &serde_json::Value,
    hook_next_fn: Option<&Value>,
) -> Result<(), RuntimeError> {
    for stmt in &runtime.program.body {
        if let Statement::ExportDefault { declaration } = stmt {
            if let ExportDefaultDecl::Function { params, .. } = declaration.as_ref() {
                if let Some(p) = params.first() {
                    runtime
                        .env
                        .borrow_mut()
                        .set(&p.name, Value::from_json(context));
                }
                if let Some(p2) = params.get(1) {
                    let next_val = hook_next_fn
                        .cloned()
                        .unwrap_or(Value::Null);
                    runtime.env.borrow_mut().set(&p2.name, next_val);
                }
            }
            break;
        }
    }
    Ok(())
}
