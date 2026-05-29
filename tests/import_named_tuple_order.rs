//! Confirm ImportDecl named_bindings tuple matches process_imports (local, imported).

use relay_runtime::ast::Statement;
use relay_runtime::parse;

#[test]
fn named_import_binding_tuple_is_local_then_imported() {
    let src = r#"
import { relayClientBaseUrl as rcu } from '@hooks'
export default function X() { return <div /> }
"#;
    let program = parse(src).expect("parse");
    let Statement::ImportDecl { named_bindings, .. } = &program.body[0] else {
        panic!("expected import");
    };
    assert_eq!(named_bindings.len(), 1);
    // local binding first, exported name second (see parser parse_named_imports)
    assert_eq!(named_bindings[0].0, "rcu");
    assert_eq!(named_bindings[0].1, "relayClientBaseUrl");
}
