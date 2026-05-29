//! `&&` / `||` must short-circuit so patterns like `resp && resp.ok` do not read members on null.

use relay_runtime::{analyze, parse, Runtime};

#[test]
fn and_short_circuits_before_member_access_on_null() {
    let src = r#"
export default function Root() {
  return null && null.x
}
"#;
    let program = parse(src).expect("parse");
    let analysis = analyze(&program).expect("analyze");
    let mut rt = Runtime::new(program, analysis);
    let v = rt.run().expect("run must not evaluate RHS of &&");
    assert!(matches!(v, relay_runtime::value::Value::Null));
}

#[test]
fn and_returns_rhs_when_lhs_truthy() {
    let src = r#"
export default function Root() {
  return 1 && 2
}
"#;
    let program = parse(src).expect("parse");
    let analysis = analyze(&program).expect("analyze");
    let mut rt = Runtime::new(program, analysis);
    let v = rt.run().expect("run");
    assert_eq!(v, relay_runtime::value::Value::Number(2.0));
}

#[test]
fn or_short_circuits_when_lhs_truthy() {
    let src = r#"
export default function Root() {
  return 1 || null.x
}
"#;
    let program = parse(src).expect("parse");
    let analysis = analyze(&program).expect("analyze");
    let mut rt = Runtime::new(program, analysis);
    let v = rt.run().expect("run");
    assert_eq!(v, relay_runtime::value::Value::Number(1.0));
}
