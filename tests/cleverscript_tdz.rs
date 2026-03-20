//! Phase 1.3: TDZ (Temporal Dead Zone).
//! TDD: Accessing a variable before its declaration/initialization throws.

use relay_runtime::{analyze, parse};

#[test]
fn tdz_const_before_init() {
    // Using y before declaration - analysis or runtime catches it
    let source = r#"
export default function App() {
  const x = y
  const y = 1
  return <div>{x}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis_result = analyze(&program);
    assert!(analysis_result.is_err(), "accessing y before init should fail (TDZ or undefined)");
}

#[test]
fn tdz_let_before_init() {
    // Using b before declaration - analysis or runtime catches it
    let source = r#"
export default function App() {
  const a = b
  let b = 2
  return <div>{a}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis_result = analyze(&program);
    assert!(analysis_result.is_err(), "accessing b before init should fail (TDZ or undefined)");
}
