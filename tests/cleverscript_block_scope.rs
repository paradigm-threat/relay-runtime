//! Phase 1.2: Block scoping for let/const.
//! TDD: Variables in if/for/while blocks don't leak to outer scope.

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
fn if_block_variable_does_not_leak() {
    // Variable declared inside if should not be visible after.
    // Analysis or runtime should catch undefined x.
    let source = r#"
export default function App() {
  if (true) {
    const x = 1
  }
  const y = x
  return <div>{y}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis_result = analyze(&program);
    if let Ok(analysis) = analysis_result {
        let mut runtime = Runtime::new(program, analysis);
        let result = runtime.run();
        assert!(result.is_err(), "accessing x outside block should fail");
    } else {
        // Analysis caught it (undefined variable)
        assert!(analysis_result.is_err());
    }
}

#[test]
fn for_loop_variable_does_not_leak() {
    // Loop variable should not leak (when we have block scoping).
    let source = r#"
export default function App() {
  for (let i = 0; i < 1; i++) {}
  const z = i
  return <div>{z}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis_result = analyze(&program);
    if let Ok(analysis) = analysis_result {
        let mut runtime = Runtime::new(program, analysis);
        let result = runtime.run();
        assert!(result.is_err(), "accessing i outside loop should fail");
    } else {
        assert!(analysis_result.is_err());
    }
}

#[test]
fn while_block_variable_does_not_leak() {
    // y declared in while body - not visible outside.
    let source = r#"
export default function App() {
  let n = 0
  while (n < 1) {
    const y = 99
    n = 1
  }
  const z = y
  return <div>{z}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis_result = analyze(&program);
    if let Ok(analysis) = analysis_result {
        let mut runtime = Runtime::new(program, analysis);
        let result = runtime.run();
        assert!(result.is_err(), "accessing y outside while should fail");
    } else {
        assert!(analysis_result.is_err());
    }
}

#[test]
fn if_block_variable_visible_inside_block() {
    let source = r#"
export default function App() {
  let out = 0
  if (true) {
    const x = 42
    out = x
  }
  return <div>{out}</div>
}
"#;
    let text = run_and_text(source);
    assert_eq!(text, "42");
}
