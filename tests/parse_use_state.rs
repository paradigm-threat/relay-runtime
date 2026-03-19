//! parse_use_state: const [a, setA] = useState(0) → variable decl, array pattern, call

use relay_runtime::{parse, ast::*};

#[test]
fn test_use_state() {
    let source = r#"const [a, setA] = useState(0)"#;
    let program = parse(source).expect("parse");
    assert_eq!(program.body.len(), 1);
    let stmt = &program.body[0];
    if let Statement::VariableDecl { kind: VarKind::Const, declarations } = stmt {
        assert_eq!(declarations.len(), 1);
        let decl = &declarations[0];
        if let Pattern::Array { elements } = &decl.id {
            assert_eq!(elements.len(), 2);
        } else {
            panic!("expected array pattern");
        }
        let init = decl.init.as_ref().expect("init");
        if let Expr::Call { callee, arguments } = init.as_ref() {
            if let Expr::Identifier { name } = callee.as_ref() {
                assert_eq!(name, "useState");
            }
            assert_eq!(arguments.len(), 1);
            if let Expr::Literal(Literal::Number(n)) = &arguments[0] {
                assert_eq!(*n, 0.0);
            }
        }
        return;
    }
    panic!("expected variable decl");
}
