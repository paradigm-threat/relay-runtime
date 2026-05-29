//! Early `return` inside control-flow blocks in default-export `run`.

use relay_runtime::host::MockHost;
use relay_runtime::run_query_script;
use relay_runtime::value::Value;

#[test]
fn return_inside_if_in_default_export() {
    let src = r#"
export default function run(ctx) {
  if (ctx.flag) {
    return 42;
  }
  return 0;
}
"#;
    let host = MockHost::new();
    let v = run_query_script(
        src,
        host.clone(),
        &serde_json::json!({ "flag": true }),
        None,
        None,
    )
    .expect("run");
    assert_eq!(v, Value::Number(42.0));

    let v = run_query_script(
        src,
        host,
        &serde_json::json!({ "flag": false }),
        None,
        None,
    )
    .expect("run");
    assert_eq!(v, Value::Number(0.0));
}

#[test]
fn return_inside_while_loop_in_default_export() {
    let src = r#"
export default function run(ctx) {
  var i = 0;
  while (i < 5) {
    if (i === 3) {
      return i;
    }
    i = i + 1;
  }
  return 99;
}
"#;
    let host = MockHost::new();
    let v = run_query_script(src, host, &serde_json::json!({}), None, None).expect("run");
    assert_eq!(v, Value::Number(3.0));
}
