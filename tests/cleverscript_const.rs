//! Phase 1.1: Enforce const immutability.
//! TDD: Tests first. Reassigning a const variable must throw a runtime error.

use relay_runtime::host::MockHost;
use relay_runtime::{analyze, parse, runtime::Runtime};
use std::collections::HashMap;

fn react_module() -> relay_runtime::value::Value {
    let mut obj = HashMap::new();
    obj.insert(
        "useState".to_string(),
        relay_runtime::value::Value::ReactHook("useState".to_string()),
    );
    relay_runtime::value::Value::Object(obj)
}

#[test]
fn const_reassignment_throws() {
    let source = r#"
import React, { useState } from 'react'
export default function App() {
  const x = 1
  x = 2
  return <div>{x}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let host = MockHost::new();
    host.borrow().register_module("react", react_module());
    let mut runtime = Runtime::new(program, analysis).with_host(host);

    let result = runtime.run();
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("const") && msg.contains("reassign"),
        "expected const reassignment error, got: {}",
        msg
    );
}

#[test]
fn const_in_block_reassignment_throws() {
    let source = r#"
export default function App() {
  if (true) {
    const y = 10
    y = 20
  }
  return <div>ok</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run();
    assert!(result.is_err());
}

#[test]
fn let_reassignment_succeeds() {
    let source = r#"
export default function App() {
  let x = 1
  x = 2
  return <div>{x}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run().unwrap();
    let obj = match &result {
        relay_runtime::value::Value::Object(o) => o,
        _ => panic!("expected object, got {:?}", result),
    };
    let children = match obj.get("children") {
        Some(relay_runtime::value::Value::Array(c)) => c,
        _ => panic!("expected children array"),
    };
    assert_eq!(children.first(), Some(&relay_runtime::value::Value::Number(2.0)));
}
