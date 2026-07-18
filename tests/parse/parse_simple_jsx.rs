//! parse_simple_jsx: <div>hello</div> → AST with element, tag, text child

use relay_runtime::{parse, ast::*};

#[test]
fn test_simple_div() {
    let source = r#"<div>hello</div>"#;
    let program = parse(source).expect("parse");
    assert_eq!(program.body.len(), 1);
    let stmt = &program.body[0];
    if let Statement::Expr { expression } = stmt {
        if let Expr::JsxElement(el) = expression.as_ref() {
            assert!(matches!(el.tag, JsxTag::Identifier { ref name } if name == "div"));
            assert_eq!(el.props.len(), 0);
            assert_eq!(el.children.len(), 1);
            if let JsxChild::Text(ref t) = el.children[0] {
                assert_eq!(t, "hello");
            } else {
                panic!("expected text child");
            }
            return;
        }
    }
    panic!("expected JSX element");
}

#[test]
fn test_div_with_classname() {
    let source = r#"<div className="foo">x</div>"#;
    let program = parse(source).expect("parse");
    assert_eq!(program.body.len(), 1);
    let stmt = &program.body[0];
    if let Statement::Expr { expression } = stmt {
        if let Expr::JsxElement(el) = expression.as_ref() {
            assert_eq!(el.props.len(), 1);
            if let JsxProp::Attribute { name, value } = &el.props[0] {
                assert_eq!(name, "className");
                if let JsxAttrValue::String(s) = value {
                    assert_eq!(s, "foo");
                } else {
                    panic!("expected string value");
                }
            }
            return;
        }
    }
    panic!("expected JSX element with props");
}

#[test]
fn test_hyphenated_jsx_attrs() {
    let source = r#"<div data-theme="dark" aria-label="Home" data-relay-ssr={1} />"#;
    let program = parse(source).expect("hyphenated attrs should parse");
    let Statement::Expr { expression } = &program.body[0] else {
        panic!("expected expr");
    };
    let Expr::JsxElement(el) = expression.as_ref() else {
        panic!("expected jsx");
    };
    assert_eq!(el.props.len(), 3);
    let names: Vec<&str> = el
        .props
        .iter()
        .map(|p| match p {
            JsxProp::Attribute { name, .. } => name.as_str(),
            _ => panic!("expected attribute"),
        })
        .collect();
    assert_eq!(names, ["data-theme", "aria-label", "data-relay-ssr"]);
}