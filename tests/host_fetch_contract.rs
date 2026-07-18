//! Contract tests for CleverScript `fetch()` / [`relay_runtime::host::Host::fetch`].

use relay_runtime::host::{fetch_options_json, host_fetch_object, Host, MockHost};
use relay_runtime::value::Value;

#[test]
fn fetch_options_json_default_get() {
    assert_eq!(fetch_options_json(None), r#"{"method":"GET"}"#);
}

#[test]
fn fetch_options_json_post() {
    let mut m = std::collections::HashMap::new();
    m.insert("method".to_string(), Value::String("POST".to_string()));
    let opts = Value::Object(m);
    assert_eq!(
        fetch_options_json(Some(&opts)),
        r#"{"method":"POST"}"#
    );
}

#[test]
fn fetch_options_json_includes_body() {
    let mut m = std::collections::HashMap::new();
    m.insert("method".to_string(), Value::String("QUERY".to_string()));
    m.insert(
        "body".to_string(),
        Value::String(r#"{"collection":"index"}"#.to_string()),
    );
    let opts = Value::Object(m);
    let s = fetch_options_json(Some(&opts));
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["method"], "QUERY");
    assert_eq!(v["body"], r#"{"collection":"index"}"#);
}

#[test]
fn host_fetch_object_has_stable_keys() {
    let v = host_fetch_object(true, 200.0, "hello", None);
    let Value::Object(o) = v else {
        panic!("expected object");
    };
    assert_eq!(o.get("ok"), Some(&Value::Bool(true)));
    assert_eq!(o.get("status"), Some(&Value::Number(200.0)));
    assert_eq!(o.get("text"), Some(&Value::String("hello".into())));
    assert!(o.get("error").is_none());
}

#[test]
fn host_fetch_object_includes_error_when_set() {
    let v = host_fetch_object(false, 0.0, "", Some("nope".into()));
    let Value::Object(o) = v else {
        panic!("expected object");
    };
    assert_eq!(o.get("error"), Some(&Value::String("nope".into())));
}

#[test]
fn mock_host_fetch_returns_contract_shape() {
    let h = MockHost::new();
    let v = Host::fetch(&*h.borrow(), "https://x/y.md", None).expect("fetch");
    let Value::Object(o) = v else {
        panic!("expected object");
    };
    assert!(o.contains_key("ok"));
    assert!(o.contains_key("status"));
    assert!(o.contains_key("text"));
    let text = o.get("text").and_then(|t| match t {
        Value::String(s) => Some(s.as_str()),
        _ => None,
    });
    assert!(text.is_some_and(|s| s.contains("mock-body:https://x/y.md")));
}
