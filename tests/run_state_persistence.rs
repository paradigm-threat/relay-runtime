//! Phase A: RunState persistence across runs.
//!
//! When run_hook_with_host is called twice with the same source, the second run
//! must reuse component instances (and thus useState state) from the first run.
//!
//! This test uses relay_runtime::run_state to simulate the WASM flow: each call
//! to run_hook_persistent may create a fresh Runtime, but RunState persists
//! default_instance so the second run sees updated state.

use relay_runtime::host::MockHost;
use relay_runtime::run_state::{clear_run_state, run_hook_persistent};
use relay_runtime::value::Value;

fn react_module() -> Value {
    let mut obj = std::collections::HashMap::new();
    obj.insert("useState".to_string(), Value::ReactHook("useState".to_string()));
    obj.insert("useEffect".to_string(), Value::ReactHook("useEffect".to_string()));
    obj.insert("useCallback".to_string(), Value::ReactHook("useCallback".to_string()));
    Value::Object(obj)
}

#[test]
fn run_state_persists_use_state_across_runs() {
    let source = r#"
import React, { useState } from 'react'
export default function App() {
  const [count, setCount] = useState(0)
  return <div>{[count, setCount]}</div>
}
"#;
    let host = MockHost::new();
    host.borrow().register_module("react", react_module());

    clear_run_state();

    // First run
    let result1 = run_hook_persistent(source, host.clone()).unwrap();
    let (count1, setter) = extract_count_and_setter(&result1);
    assert_eq!(count1, 0.0);

    // Simulate user click: call setter (updates the cell in RunState's default_instance)
    setter.invoke_setter(&[Value::Number(1.0)]).unwrap();

    // Second run - must see updated state (persisted via RunState)
    let result2 = run_hook_persistent(source, host).unwrap();
    let (count2, _) = extract_count_and_setter(&result2);
    assert_eq!(count2, 1.0, "Second run should see useState(1), not 0");
}

fn extract_count_and_setter(v: &Value) -> (f64, Value) {
    if let Value::Object(o) = v {
        if let Some(Value::Array(children)) = o.get("children") {
            if let Some(Value::Array(state_arr)) = children.first() {
                let count = match state_arr.first() {
                    Some(Value::Number(n)) => *n,
                    _ => -1.0,
                };
                let setter = state_arr.get(1).cloned().unwrap_or(Value::Null);
                return (count, setter);
            }
        }
    }
    panic!("Could not extract count and setter from result: {:?}", v);
}
