//! Runtime limits: loop iteration cap, recursion depth, array length, map key requirement.

use relay_runtime::{analyze, parse, runtime::Runtime, value_to_vnode};

#[test]
fn map_without_key_fails_analysis() {
    let source = r#"
export default function App() {
  const items = [1, 2, 3]
  return (
    <div>
      {items.map(x => <span>{x}</span>)}
    </div>
  )
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program);
    assert!(
        result.is_err(),
        "arr.map() returning JSX without key should fail analysis"
    );
    let err = result.unwrap_err();
    assert!(
        format!("{}", err).contains("key"),
        "error should mention key: {}",
        err
    );
}

#[test]
fn map_with_key_passes_analysis() {
    let source = r#"
export default function App() {
  const items = [{ id: 1 }, { id: 2 }]
  return (
    <div>
      {items.map(item => <span key={item.id}>{item.id}</span>)}
    </div>
  )
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program);
    assert!(result.is_ok(), "map with key should pass: {:?}", result);
}

#[test]
fn infinite_loop_hits_limit() {
    let source = r#"
export default function App() {
  let n = 0
  for (let i = 0; true; i++) {
    n = i
  }
  return <div>{n}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run();
    assert!(result.is_err(), "infinite for loop should abort");
    let err = result.unwrap_err();
    assert!(
        format!("{}", err).contains("iterations"),
        "error should mention iterations: {}",
        err
    );
}

#[test]
fn component_error_boundary_does_not_crash() {
    // When a child component throws at runtime, run completes (no crash) and siblings still render.
    // The broken component is replaced by the error boundary fallback.
    let source = r#"
export default function App() {
  const Broken = () => {
    const bad = 1
    return <span>{bad()}</span>
  }
  return (
    <div>
      <span>before</span>
      <Broken />
      <span>after</span>
    </div>
  )
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run();
    assert!(result.is_ok(), "run should not crash: {:?}", result);
    let v = result.unwrap();
    let tree = value_to_vnode(&v).normalize();
    let text = tree.text_content();
    assert!(text.contains("before"), "sibling before should render");
    assert!(text.contains("after"), "sibling after should render");
    // Error boundary replaces Broken with fallback; exact format may vary
}

#[test]
fn while_infinite_hits_limit() {
    let source = r#"
export default function App() {
  let n = 0
  while (true) {
    n = n + 1
  }
  return <div>{n}</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let result = runtime.run();
    assert!(result.is_err(), "infinite while loop should abort");
}

#[test]
fn array_literal_hits_length_limit() {
    use relay_runtime::host::MockHost;
    use relay_runtime::run_query_script;

    // Double via concat until near the limit, then spread past it.
    let src = r#"
export default function run(ctx) {
  let a = [1]
  while (a.length < 30000) {
    a = a.concat(a)
  }
  return [...a, ...a].length
}
"#;
    let result = run_query_script(src, MockHost::new(), &serde_json::json!({}), None, None);
    assert!(result.is_err(), "oversized array spread should fail");
}
