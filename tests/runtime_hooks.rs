//! Phase 3: Hooks runtime tests
//! Hooks require import from 'react'.

use relay_runtime::host::MockHost;
use relay_runtime::{analyze, parse, runtime::Runtime, value::Value};
use std::collections::HashMap;

fn react_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("useState".to_string(), Value::ReactHook("useState".to_string()));
    obj.insert("useEffect".to_string(), Value::ReactHook("useEffect".to_string()));
    obj.insert("useCallback".to_string(), Value::ReactHook("useCallback".to_string()));
    Value::Object(obj)
}

#[test]
fn runtime_use_state() {
    let source = r#"
import React, { useState } from 'react'
export default function App() {
  const [a, setA] = useState(0)
  return <div>{[a, setA]}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let host = MockHost::new();
    host.borrow().register_module("react", react_module());
    let mut runtime = Runtime::new(program, analysis).with_host(host);

    let result = runtime.run().unwrap();
    let obj = match &result {
        Value::Object(o) => o,
        _ => panic!("expected JSX object, got {:?}", result),
    };
    let children = match obj.get("children") {
        Some(Value::Array(c)) => c,
        _ => panic!("expected children array, got {:?}", obj.get("children")),
    };
    let state_arr = match children.first() {
        Some(Value::Array(a)) => a,
        c => panic!("expected [value, setter] as first child, got {:?}", c),
    };
    assert_eq!(state_arr.len(), 2);
    assert!(matches!(state_arr[0], Value::Number(0.0)));
    let setter = state_arr[1].clone();

    runtime.call_value(&setter, &[Value::Number(1.0)]).unwrap();

    let result2 = runtime.run().unwrap();
    let obj2 = match &result2 {
        Value::Object(o) => o,
        _ => panic!("expected JSX object, got {:?}", result2),
    };
    let children2 = match obj2.get("children") {
        Some(Value::Array(c)) => c,
        _ => panic!("expected children array"),
    };
    let state_arr2 = match children2.first() {
        Some(Value::Array(a)) => a,
        _ => panic!("expected [value, setter] after rerun"),
    };
    assert_eq!(state_arr2[0], Value::Number(1.0));
}
