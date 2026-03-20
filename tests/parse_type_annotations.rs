//! Phase 5: Optional type annotations.
//! TDD: Parse type annotations, validate at runtime.

use relay_runtime::value::Value;
use relay_runtime::{analyze, parse, runtime::Runtime};

#[test]
fn typed_variable_parse_and_run() {
    let source = r#"
export default function App() {
  let x: number = 5
  return <div>{x}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run().unwrap();
    let obj = match &result {
        Value::Object(o) => o,
        _ => panic!("expected object"),
    };
    let children = match obj.get("children") {
        Some(Value::Array(c)) => c,
        _ => panic!("expected children"),
    };
    assert_eq!(children.first(), Some(&Value::Number(5.0)));
}

#[test]
fn type_mismatch_rejected_at_runtime() {
    let source = r#"
export default function App() {
  let x: number = "hello"
  return <div>{x}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run();
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("type") || err.contains("number") || err.contains("string"),
        "expected type error, got: {}",
        err
    );
}

#[test]
fn typed_arrow_param_parsed() {
    // Arrow function params can have type annotations; validation happens on call.
    let source = r#"
export default function App() {
  const greet = (name: string) => name
  return <div>{greet("world")}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run().unwrap();
    let obj = match &result {
        Value::Object(o) => o,
        _ => panic!("expected object"),
    };
    let children = match obj.get("children") {
        Some(Value::Array(c)) => c,
        _ => panic!("expected children"),
    };
    assert_eq!(
        children.first(),
        Some(&Value::String("world".to_string()))
    );
}
