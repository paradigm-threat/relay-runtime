//! export const / export let named exports

use relay_runtime::host::ModuleMapHost;
use relay_runtime::{analyze, parse, Host, Value};
use std::collections::HashMap;

#[test]
fn parse_export_const_and_let() {
    let src = r#"
export const answer = 42
export let tally = 0
export function bump() { tally = tally + 1; return tally }
"#;
    let program = parse(src).expect("parse");
    assert_eq!(program.body.len(), 3);
    analyze(&program).expect("analyze");
}

#[test]
fn export_const_value_available_via_import() {
    let host = ModuleMapHost::new(HashMap::from([(
        "nums.jsx".to_string(),
        r#"
export const answer = 41 + 1
export let flag = true
"#
        .to_string(),
    )]));

    let module = host.borrow().load_module("nums.jsx").expect("load");
    let Value::Object(exports) = module else {
        panic!("expected object exports");
    };
    assert_eq!(exports.get("answer"), Some(&Value::Number(42.0)));
    assert_eq!(exports.get("flag"), Some(&Value::Bool(true)));
}

#[test]
fn export_const_closed_over_api_object() {
    let host = ModuleMapHost::new(HashMap::from([(
        "store.jsx".to_string(),
        r#"
function createApi() {
  const s = { n: 0 }
  function get() { return s.n }
  function inc() { s.n = s.n + 1; return s.n }
  return { get: get, inc: inc }
}
export const api = createApi()
"#
        .to_string(),
    )]));

    let module = host.borrow().load_module("store.jsx").expect("load");
    let Value::Object(exports) = module else {
        panic!("expected object");
    };
    let api = exports.get("api").expect("api export").clone();
    let Value::Object(api_obj) = api else {
        panic!("api must be object");
    };
    assert!(matches!(api_obj.get("get"), Some(Value::Function { .. })));
    assert!(matches!(api_obj.get("inc"), Some(Value::Function { .. })));
}
