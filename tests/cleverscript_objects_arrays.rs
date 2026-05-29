//! Phase 2: Object/array strict separation.
//! TDD: Objects are objects (string keys only, no synthetic length).
//!      Arrays are arrays (numeric indices and length only).

use relay_runtime::{analyze, parse, runtime::Runtime, value_to_vnode};

fn run_and_text(source: &str) -> String {
    let program = parse(source).expect("parse");
    let analysis = analyze(&program).expect("analyze");
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run().expect("run");
    let vnode = value_to_vnode(&result).normalize();
    vnode.text_content()
}

#[test]
fn object_has_no_synthetic_length() {
    // Plain object without .length property: obj.length should fail (no synthetic length)
    let source = r#"
export default function App() {
  const obj = { a: 1, b: 2 }
  const len = obj.length
  return <div>{len}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run();
    // obj.length on plain object: no synthetic length - should be undefined/error
    assert!(result.is_err(), "obj.length on plain object should fail");
}

#[test]
fn object_explicit_length_property_works() {
    let source = r#"
export default function App() {
  const obj = { length: 5 }
  return <div>{obj.length}</div>
}
"#;
    let text = run_and_text(source);
    assert_eq!(text, "5");
}

#[test]
fn array_length_works() {
    let source = r#"
export default function App() {
  const arr = [1, 2, 3]
  return <div>{arr.length}</div>
}
"#;
    let text = run_and_text(source);
    assert_eq!(text, "3");
}

#[test]
fn array_string_key_rejected() {
    // arr["foo"] on array should fail (arrays: numeric indices only)
    let source = r#"
export default function App() {
  const arr = [1, 2, 3]
  const x = arr["foo"]
  return <div>{x}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run();
    assert!(result.is_err(), "arr[\"foo\"] on array should fail");
}

#[test]
fn array_numeric_index_works() {
    let source = r#"
export default function App() {
  const arr = [10, 20, 30]
  return <div>{arr[1]}</div>
}
"#;
    let text = run_and_text(source);
    assert_eq!(text, "20");
}

#[test]
fn string_to_lowercase_and_to_uppercase() {
    let source = r#"
export default function App() {
  return <div>{'aBc'.toLowerCase() + '|' + 'xYz'.toUpperCase()}</div>
}
"#;
    let text = run_and_text(source);
    assert_eq!(text, "abc|XYZ");
}
