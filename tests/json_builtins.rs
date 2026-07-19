//! JSON.parse / JSON.stringify CleverScript builtins (Rust serde_json).

use relay_runtime::host::MockHost;
use relay_runtime::run_state::inject_standard_builtins;
use relay_runtime::{analyze, parse, runtime::Runtime, Value};

fn run_with_builtins(source: &str) -> Value {
    let program = parse(source).expect("parse");
    let analysis = analyze(&program).expect("analyze");
    let mock = MockHost::new();
    let mut runtime = Runtime::new(program, analysis).with_host(mock);
    inject_standard_builtins(&runtime);
    runtime.run().expect("run")
}

#[test]
fn json_stringify_parse_roundtrip_gallery_list() {
    let source = r#"
export default function App() {
  const list = [
    { src: "/a.jpg?w=360", alt: "alpha", highResSrc: "" },
    { src: "/b.jpg", alt: "", highResSrc: "/b-hi.jpg" },
  ]
  const raw = JSON.stringify(list)
  const back = JSON.parse(raw)
  return <div>{back.length}:{back[0].src}:{back[1].highResSrc}</div>
}
"#;
    let result = run_with_builtins(source);
    let vnode = relay_runtime::value_to_vnode(&result).normalize();
    let text = vnode.text_content().split_whitespace().collect::<String>();
    assert_eq!(
        text, "2:/a.jpg?w=360:/b-hi.jpg",
        "gallery-shaped array must survive stringify→parse"
    );
}

#[test]
fn json_parse_invalid_throws() {
    let source = r#"
export default function App() {
  try {
    JSON.parse("{")
    return <div>ok</div>
  } catch (e) {
    return <div>err</div>
  }
}
"#;
    let result = run_with_builtins(source);
    let vnode = relay_runtime::value_to_vnode(&result).normalize();
    assert_eq!(vnode.text_content(), "err");
}

#[test]
fn global_this_json_mirrors_builtin() {
    let source = r#"
export default function App() {
  const raw = globalThis.JSON.stringify({ n: 1 })
  const d = globalThis.JSON.parse(raw)
  return <div>{d.n}</div>
}
"#;
    let result = run_with_builtins(source);
    let vnode = relay_runtime::value_to_vnode(&result).normalize();
    assert_eq!(vnode.text_content(), "1");
}

#[test]
fn json_value_unit_roundtrip() {
    use relay_runtime::run_state::json_value;
    let Value::Object(json) = json_value() else {
        panic!("JSON must be object");
    };
    let stringify = json.get("stringify").expect("stringify");
    let parse = json.get("parse").expect("parse");
    let Value::Native(stringify_fn) = stringify else {
        panic!("stringify native");
    };
    let Value::Native(parse_fn) = parse else {
        panic!("parse native");
    };
    let mut row = std::collections::HashMap::new();
    row.insert("src".to_string(), Value::String("/x.png".into()));
    row.insert("alt".to_string(), Value::String("x".into()));
    let list = Value::Array(vec![Value::Object(row)]);
    let s = stringify_fn(&[list]).expect("stringify");
    let Value::String(raw) = s else {
        panic!("stringify string");
    };
    assert!(raw.contains("/x.png"));
    let back = parse_fn(&[Value::String(raw)]).expect("parse");
    let Value::Array(arr) = back else {
        panic!("parse array");
    };
    assert_eq!(arr.len(), 1);
}
