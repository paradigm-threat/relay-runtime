//! Phase 6: Module system tests

use relay_runtime::{analyze, parse, host::MockHost, runtime::Runtime, value::Value};
use std::collections::HashMap;

#[test]
fn module_import() {
    let source = r#"
export default function App() {
  const mod = import('./plugin/tmdb.mjs')
  return mod
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mock = MockHost::new();
    let module = Value::Object(
        [
            ("default".to_string(), Value::Null),
            (
                "handleQuery".to_string(),
                Value::Native(|_| Ok(Value::Array(vec![]))),
            ),
        ]
        .into_iter()
        .collect(),
    );
    mock.borrow().register_module("./plugin/tmdb.mjs", module);
    let mut runtime = Runtime::new(program, analysis).with_host(mock.clone());

    let result = runtime.run().unwrap();
    assert!(matches!(result, Value::Object(_)));

    let host = mock.borrow();
    let calls = host.load_module_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0], "./plugin/tmdb.mjs");
}

#[test]
fn module_context() {
    let source = r#"
export default function App() {
  return params
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let params = Value::Object(
        [("q".to_string(), Value::String("test".to_string()))]
            .into_iter()
            .collect(),
    );
    let env = Value::Object(HashMap::new());
    let navigate = Value::Native(|_args: &[Value]| Ok(Value::Null));
    let mut runtime = Runtime::new(program, analysis)
        .with_context(params, env, navigate);

    let result = runtime.run().unwrap();
    let obj = match &result {
        Value::Object(o) => o,
        _ => panic!("expected params object"),
    };
    assert_eq!(obj.get("q"), Some(&Value::String("test".to_string())));
}
