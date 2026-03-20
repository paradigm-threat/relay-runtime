//! Phase B: RENDER_REQUESTED flag and consume_render_requested.

use relay_runtime::host::MockHost;
use relay_runtime::run_state::{clear_run_state, consume_render_requested, run_hook_persistent};
use relay_runtime::value::Value;
use std::collections::HashMap;

fn react_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("useState".to_string(), Value::ReactHook("useState".to_string()));
    obj.insert("useEffect".to_string(), Value::ReactHook("useEffect".to_string()));
    obj.insert("useCallback".to_string(), Value::ReactHook("useCallback".to_string()));
    Value::Object(obj)
}

#[test]
fn setter_sets_render_requested() {
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

    let result = run_hook_persistent(source, host.clone()).unwrap();
    let (_, setter) = extract_count_and_setter(&result);
    assert!(!consume_render_requested());

    setter.invoke_setter(&[Value::Number(1.0)]).unwrap();
    assert!(consume_render_requested());
    assert!(!consume_render_requested()); // consumed, now false
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
    panic!("Could not extract count and setter");
}
