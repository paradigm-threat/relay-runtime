//! useEffect dependency scheduling and setter bail-out.

use relay_runtime::host::MockHost;
use relay_runtime::run_state::{clear_run_state, consume_render_requested, run_hook_persistent};
use relay_runtime::value::Value;
use relay_runtime::vdom::value_to_vnode;
use std::collections::HashMap;

fn react_module() -> Value {
    let mut obj = HashMap::new();
    obj.insert("useState".to_string(), Value::ReactHook("useState".to_string()));
    obj.insert("useEffect".to_string(), Value::ReactHook("useEffect".to_string()));
    Value::Object(obj)
}

fn text_from_result(v: &Value) -> String {
    let vnode = value_to_vnode(v).normalize();
    fn walk(v: &relay_runtime::vdom::VNode) -> String {
        match v {
            relay_runtime::vdom::VNode::Text(s) => s.clone(),
            relay_runtime::vdom::VNode::Element { children, .. } => {
                children.iter().map(walk).collect::<Vec<_>>().join("")
            }
            relay_runtime::vdom::VNode::Empty => String::new(),
        }
    }
    walk(&vnode)
}

#[test]
fn use_effect_empty_deps_runs_once_across_rerenders() {
    let source = r#"
import React, { useState, useEffect } from 'react'
export default function App() {
  const [runs, setRuns] = useState(0)
  useEffect(() => {
    setRuns(runs + 1)
  }, [])
  return <div>{runs}</div>
}
"#;
    let host = MockHost::new();
    host.borrow().register_module("react", react_module());
    clear_run_state();

    let r1 = run_hook_persistent(source, host.clone()).unwrap();
    assert_eq!(text_from_result(&r1), "0");

    let r2 = run_hook_persistent(source, host.clone()).unwrap();
    assert_eq!(text_from_result(&r2), "1", "effect should bump runs once");

    let r3 = run_hook_persistent(source, host.clone()).unwrap();
    assert_eq!(text_from_result(&r3), "1", "empty deps must not re-run effect");
}

#[test]
fn use_effect_dep_change_reruns() {
    let source = r#"
import React, { useState, useEffect } from 'react'
export default function App() {
  const [n, setN] = useState(0)
  const [logged, setLogged] = useState(-1)
  useEffect(() => {
    setLogged(n)
  }, [n])
  return <div>{[logged, setN]}</div>
}
"#;
    let host = MockHost::new();
    host.borrow().register_module("react", react_module());
    clear_run_state();

    let result = run_hook_persistent(source, host.clone()).unwrap();
    assert_eq!(text_from_result(&result), "-1");

    let result2 = run_hook_persistent(source, host.clone()).unwrap();
    assert_eq!(text_from_result(&result2), "0");

    let setter = extract_setter(&result2);
    setter.invoke_setter(&[Value::Number(1.0)]).unwrap();
    let _ = run_hook_persistent(source, host.clone()).unwrap();
    let result3 = run_hook_persistent(source, host.clone()).unwrap();
    assert_eq!(text_from_result(&result3), "1");
}

#[test]
fn setter_same_value_does_not_request_render() {
    let source = r#"
import React, { useState } from 'react'
export default function App() {
  const [n, setN] = useState(0)
  return <div>{[n, setN]}</div>
}
"#;
    let host = MockHost::new();
    host.borrow().register_module("react", react_module());
    clear_run_state();
    let result = run_hook_persistent(source, host).unwrap();
    let setter = extract_setter(&result);
    assert!(!consume_render_requested());
    setter.invoke_setter(&[Value::Number(0.0)]).unwrap();
    assert!(!consume_render_requested());
}

fn extract_setter(v: &Value) -> Value {
    if let Value::Object(o) = v {
        if let Some(Value::Array(ch)) = o.get("children") {
            if let Some(Value::Array(state)) = ch.first() {
                return state.get(1).cloned().unwrap_or(Value::Null);
            }
        }
    }
    panic!("no setter");
}
