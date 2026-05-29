//! Tier 1: Call stack and locals on runtime errors.

use relay_runtime::{analyze, parse, runtime::Runtime};
use relay_runtime::RuntimeError;

#[test]
fn runtime_error_includes_call_stack() {
    // Error in variable init so it propagates (not in JSX child which gets swallowed).
    let source = r#"
export default function App() {
  const x = null
  const z = x.foo
  return <div>ok</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run();
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let RuntimeError::RuntimeDetail(d) = &err {
        assert!(!d.call_stack.is_empty(), "call_stack should not be empty");
        assert!(d.call_stack.iter().any(|f| f == "App" || f.contains("default")));
    } else {
        panic!("expected RuntimeDetail, got {:?}", err);
    }
}

#[test]
fn runtime_error_includes_locals() {
    // Error in variable init - (1)() calls a number, "Not a function".
    let source = r#"
export default function App() {
  const count = 42
  const items = [1, 2]
  const bad = (1)()
  return <div>ok</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run();
    assert!(result.is_err());
    let err = result.unwrap_err();
    if let RuntimeError::RuntimeDetail(d) = &err {
        assert!(d.locals.contains_key("count"));
        assert!(d.locals.contains_key("items"));
    } else {
        panic!("expected RuntimeDetail, got {:?}", err);
    }
}

#[test]
fn error_json_has_call_stack_and_locals() {
    // Error in variable init - x is number, x() is "Not a function".
    let source = r#"
export default function App() {
  const x = 1
  const y = x()
  return <div>ok</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run();
    let err = result.unwrap_err();
    let json = err.to_error_json();
    let obj = json.as_object().expect("error json should be object");
    assert!(obj.contains_key("call_stack"));
    assert!(obj.contains_key("locals"));
    let stack = obj.get("call_stack").and_then(|v| v.as_array()).expect("call_stack array");
    assert!(!stack.is_empty());
}
