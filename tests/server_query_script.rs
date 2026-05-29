//! QUERY-style one-shot CleverScript (no RUN_STATE).

use relay_runtime::host::MockHost;
use relay_runtime::run_query_script;
use relay_runtime::value::{NativeFnClosure, Value};
use std::collections::HashMap;
use std::rc::Rc;

#[test]
fn query_script_returns_rows_object() {
    let src = r#"
export default function run(ctx) {
  return { rows: [{ a: 1, title: "Hi" }] };
}
"#;
    let host = MockHost::new();
    let ctx = serde_json::json!({ "collection": "articles" });
    let v = run_query_script(src, host, &ctx, None, None).expect("run");
    match v {
        Value::Object(o) => {
            let rows = o.get("rows").expect("rows");
            match rows {
                Value::Array(arr) => {
                    assert_eq!(arr.len(), 1);
                }
                _ => panic!("rows not array"),
            }
        }
        _ => panic!("expected object"),
    }
}

#[test]
fn query_script_virtual_git_readfile_via_host() {
    let src = r#"
import { readFile } from '@relay/git';
export default function run(ctx) {
  const t = readFile("hello.txt");
  return { rows: [{ path: "hello.txt", body: t }] };
}
"#;
    let read = Rc::new(|args: &[Value]| {
        let path = args
            .first()
            .and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .ok_or_else(|| "path".to_string())?;
        assert_eq!(path, "hello.txt");
        Ok(Value::String("file-content".into()))
    });
    let mut git = HashMap::new();
    git.insert(
        "readFile".to_string(),
        Value::NativeFn(NativeFnClosure(read)),
    );
    let module = Value::Object(git);
    let host = MockHost::new();
    host
        .borrow()
        .register_module("@relay/git", module);
    let ctx = serde_json::json!({});
    let v = run_query_script(src, host, &ctx, None, None).expect("run");
    let Value::Object(o) = v else {
        panic!("expected object");
    };
    let Value::Array(rows) = o.get("rows").expect("rows") else {
        panic!("rows");
    };
    let Value::Object(row) = &rows[0] else {
        panic!("row");
    };
    assert_eq!(
        row.get("body"),
        Some(&Value::String("file-content".into()))
    );
}

#[test]
fn query_script_relay_global_still_works_when_passed() {
    let src = r#"
export default function run(ctx) {
  const t = Relay.git.readFile("hello.txt");
  return { rows: [{ path: "hello.txt", body: t }] };
}
"#;
    let read = Rc::new(|args: &[Value]| {
        let path = args
            .first()
            .and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .ok_or_else(|| "path".to_string())?;
        assert_eq!(path, "hello.txt");
        Ok(Value::String("file-content".into()))
    });
    let mut git = HashMap::new();
    git.insert(
        "readFile".to_string(),
        Value::NativeFn(NativeFnClosure(read)),
    );
    let mut relay = HashMap::new();
    relay.insert("git".to_string(), Value::Object(git));
    let relay_val = Value::Object(relay);

    let host = MockHost::new();
    let ctx = serde_json::json!({});
    let v = run_query_script(src, host, &ctx, Some(relay_val), None).expect("run");
    let Value::Object(o) = v else {
        panic!("expected object");
    };
    let Value::Array(rows) = o.get("rows").expect("rows") else {
        panic!("rows");
    };
    let Value::Object(row) = &rows[0] else {
        panic!("row");
    };
    assert_eq!(
        row.get("body"),
        Some(&Value::String("file-content".into()))
    );
}

#[test]
fn query_script_next_callback_binding() {
    let next = Value::NativeFn(NativeFnClosure(Rc::new(|args| {
        assert!(args.is_empty());
        Ok(Value::Number(42.0))
    })));
    let src = r#"
export default function run(ctx, next) {
  return next();
}
"#;
    let host = MockHost::new();
    let ctx = serde_json::json!({});
    let v = run_query_script(src, host, &ctx, None, Some(next)).expect("run");
    assert_eq!(v, Value::Number(42.0));
}
