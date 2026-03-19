//! parse_export: export default async function hook(params) { ... }

use relay_runtime::{parse, ast::*};

#[test]
fn test_export_default_async_function() {
    let source = r#"
export default async function hook(params) {
  return <div>hi</div>
}
"#;
    let program = parse(source).expect("parse");
    assert_eq!(program.body.len(), 1);
    let stmt = &program.body[0];
    if let Statement::ExportDefault { declaration } = stmt {
        if let ExportDefaultDecl::Function { r#async, params, .. } = declaration.as_ref() {
            assert!(r#async);
            assert_eq!(params.len(), 1);
            return;
        }
    }
    panic!("expected export default async function");
}
