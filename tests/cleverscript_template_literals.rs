//! Template literals expand to string + concatenation at lex time.

use relay_runtime::parse_with_path;

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
