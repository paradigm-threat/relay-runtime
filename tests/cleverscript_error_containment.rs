//! Phase 3: Automatic error containment.
//! TDD: When a child expression/element throws, sibling and parent still render.

use relay_runtime::{analyze, parse, runtime::Runtime, value_to_vnode};

#[test]
fn sibling_error_contained() {
    // One child throws at runtime (type error: "x" + 1), sibling renders. No crash.
    let source = r#"
export default function App() {
  const bad = "x"
  return (
    <div>
      <span>ok</span>
      <span>{bad + 1}</span>
      <span>also-ok</span>
    </div>
  )
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run();
    assert!(result.is_ok(), "run should not crash despite child error");

    let v = result.unwrap();
    let tree = value_to_vnode(&v).normalize();
    let text = tree.text_content();
    // Should contain "ok" and "also-ok"; bad+1 (type error) is skipped
    assert!(text.contains("ok"), "first sibling should render");
    assert!(text.contains("also-ok"), "third sibling should render");
}

#[test]
fn nested_error_contained() {
    // Inner expression has runtime error (call non-function); outer and siblings still render.
    let source = r#"
export default function App() {
  const bad = 1
  return (
    <div>
      <span>before</span>
      <span>{bad()}</span>
      <span>after</span>
    </div>
  )
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run();
    assert!(result.is_ok(), "run should not crash");

    let v = result.unwrap();
    let tree = value_to_vnode(&v).normalize();
    let text = tree.text_content();
    assert!(text.contains("before"));
    assert!(text.contains("after"));
}
