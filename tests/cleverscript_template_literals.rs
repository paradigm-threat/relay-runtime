//! Template literals expand to string + concatenation at lex time.

use relay_runtime::host::MockHost;
use relay_runtime::parse_with_path;
use relay_runtime::run_query_script;
use relay_runtime::value::Value;

const TIMELINE_SNIPPET: &str = r#"
import React, { useEffect, useState } from 'react'
export default function Root(props) {
  const path = '/x'
  const [md, setMd] = useState('')
  useEffect(() => {
    setMd(`# Title

**Path:** ${path}
`)
  }, [path])
  return <div>{md}</div>
}
"#;

#[test]
fn template_literal_with_interpolation_parses() {
    let err = parse_with_path(TIMELINE_SNIPPET, None::<String>);
    assert!(err.is_ok(), "{:?}", err);
}

#[test]
fn template_literal_static_only_parses() {
    let s = r#"
import React from 'react'
export default function X() {
  const a = `hello\nworld`
  return <span>{a}</span>
}
"#;
    parse_with_path(s, None::<String>).expect("parse");
}

#[test]
fn template_literal_interpolation_evaluates() {
    let src = r#"
export default function run(ctx) {
  const name = 'world'
  return `hello ${name}`
}
"#;
    let v = run_query_script(src, MockHost::new(), &serde_json::json!({}), None, None)
        .expect("run");
    assert_eq!(v, Value::String("hello world".to_string()));
}

#[test]
fn template_literal_multiple_substitutions() {
    let src = r#"
export default function run(ctx) {
  const a = 1
  const b = 2
  return `${a}+${b}=${a + b}`
}
"#;
    let v = run_query_script(src, MockHost::new(), &serde_json::json!({}), None, None)
        .expect("run");
    assert_eq!(v, Value::String("1+2=3".to_string()));
}
