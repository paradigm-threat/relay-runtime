//! JSX rendering tests: expression interpolation, mixed text/expr, numbers.
//!
//! Covers: {expr}, "text {expr}", numbers, commas, equals in JSX children.

use relay_runtime::host::MockHost;
use relay_runtime::value::Value;
use relay_runtime::{analyze, parse, runtime::Runtime, value_to_vnode};
use std::collections::HashMap;

fn react_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("useState".to_string(), Value::ReactHook("useState".to_string()));
    obj.insert("useEffect".to_string(), Value::ReactHook("useEffect".to_string()));
    obj.insert("useCallback".to_string(), Value::ReactHook("useCallback".to_string()));
    Value::Object(obj)
}

fn run_and_text(source: &str) -> String {
    run_and_text_with_host(source, false)
}

fn run_and_text_with_host(source: &str, use_react: bool) -> String {
    let program = parse(source).expect("parse");
    let analysis = analyze(&program).expect("analyze");
    let mut runtime = if use_react {
        let host = MockHost::new();
        host.borrow().register_module("react", react_module());
        Runtime::new(program, analysis).with_host(host)
    } else {
        Runtime::new(program, analysis)
    };
    let result = runtime.run().expect("run");
    let vnode = value_to_vnode(&result).normalize();
    vnode.text_content()
}

#[test]
fn jsx_simple_text() {
    let text = run_and_text(
        r#"
export default function App() {
  return <div>hello</div>
}
"#,
    );
    assert_eq!(text, "hello");
}

#[test]
fn jsx_expression_number() {
    let text = run_and_text(
        r#"
export default function App() {
  const count = 0
  return <div>{count}</div>
}
"#,
    );
    assert_eq!(text, "0");
}

#[test]
fn jsx_text_and_expression() {
    let text = run_and_text(
        r#"
export default function App() {
  const count = 5
  return <div>Count: {count}</div>
}
"#,
    );
    assert_eq!(text, "Count: 5");
}

#[test]
fn jsx_expression_string() {
    let text = run_and_text(
        r#"
export default function App() {
  const name = "world"
  return <div>Hello {name}</div>
}
"#,
    );
    assert_eq!(text, "Hello world");
}

#[test]
fn jsx_use_state_count_display() {
    let text = run_and_text_with_host(
        r#"
import React, { useState } from 'react'
export default function App() {
  const [count, setCount] = useState(0)
  return <div>Count: {count}</div>
}
"#,
        true,
    );
    assert_eq!(text, "Count: 0");
}

#[test]
fn jsx_multiple_expressions_with_comma() {
    let text = run_and_text(
        r#"
export default function App() {
  const a = 1
  const b = 2
  return <div>a={a}, b={b}</div>
}
"#,
    );
    assert_eq!(text, "a=1, b=2");
}

#[test]
fn jsx_nullish_coalescing_display() {
    let text = run_and_text(
        r#"
export default function App() {
  const a = null ?? "default"
  const b = 0 ?? "fallback"
  return <div>?? nullish: a={a}, b={b}</div>
}
"#,
    );
    assert_eq!(text, "?? nullish: a=default, b=0");
}

#[test]
fn jsx_optional_chaining_display() {
    let text = run_and_text(
        r#"
export default function App() {
  const obj = { x: 1, nested: { y: 2 } }
  const d = obj?.x
  const e = obj?.nested?.y
  return <div>?. optional: d={d}, e={e}</div>
}
"#,
    );
    assert_eq!(text, "?. optional: d=1, e=2");
}

#[test]
fn jsx_while_loop_sum_display() {
    let text = run_and_text(
        r#"
export default function App() {
  let sum = 0
  let i = 0
  while (i < 5) {
    sum = sum + i
    i++
  }
  return <div>while sum(0-4)={sum}</div>
}
"#,
    );
    assert_eq!(text, "while sum(0-4)=10");
}
