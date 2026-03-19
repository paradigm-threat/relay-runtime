//! Phase 4: Host bindings tests

use relay_runtime::{analyze, parse, host::MockHost, runtime::Runtime};

#[test]
fn host_create_element() {
    let source = r#"
export default function App() {
  const el = createElement('div', {}, [])
  return el
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mock = MockHost::new();
    let mut runtime = Runtime::new(program, analysis).with_host(mock.clone());

    runtime.run().unwrap();

    let host = mock.borrow();
    let calls = host.create_element_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "div");
}

#[test]
fn host_fetch() {
    let source = r#"
export default function App() {
  const resp = fetch('/api/data')
  return resp
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mock = MockHost::new();
    let mut runtime = Runtime::new(program, analysis).with_host(mock.clone());

    let result = runtime.run().unwrap();
    assert!(matches!(result, relay_runtime::Value::Object(_)));

    let host = mock.borrow();
    let calls = host.fetch_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "/api/data");
}

#[test]
fn host_navigate() {
    let source = r#"
export default function App() {
  navigate('/search/foo')
  return null
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mock = MockHost::new();
    let mut runtime = Runtime::new(program, analysis).with_host(mock.clone());

    runtime.run().unwrap();

    let host = mock.borrow();
    let calls = host.navigate_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], "/search/foo");
}
