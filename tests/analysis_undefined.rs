//! Phase 4: Undefined variable detection.
//! TDD: analyze() reports undefined variable references.

use relay_runtime::{analyze, parse};

#[test]
fn undefined_variable_in_expression_reported() {
    let source = r#"
export default function App() {
  const x = typos
  return <div>{x}</div>
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program);

    assert!(result.is_err(), "analyze should fail for undefined 'typos'");
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.to_lowercase().contains("typos") || msg.to_lowercase().contains("undefined"));
}

#[test]
fn defined_variable_passes() {
    let source = r#"
export default function App() {
  const x = 1
  return <div>{x}</div>
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program);
    assert!(result.is_ok(), "analyze should pass when all vars defined");
}

#[test]
fn import_binding_counted_as_defined() {
    let source = r#"
import React, { useState } from 'react'
export default function App() {
  const [a, setA] = useState(0)
  return <div>{a}</div>
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program);
    assert!(result.is_ok(), "imports and hook results should be defined");
}
