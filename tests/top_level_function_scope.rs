//! Regression: sibling top-level function declarations must resolve mutually (analysis hoist).

use relay_runtime::{analyze, parse};

#[test]
fn sibling_functions_call_each_other() {
    let src = r#"
import React from 'react'
function a() { return b() }
function b() { return 1 }
export default function Root() {
  return <div>{a()}</div>
}
"#;
    let program = parse(src).expect("parse");
    analyze(&program).expect("analyze should accept sibling helper calls");
}
