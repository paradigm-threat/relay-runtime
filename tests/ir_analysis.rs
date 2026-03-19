//! Phase 2: IR / semantic analysis tests

use relay_runtime::{analyze, parse};

#[test]
fn ir_simple_component() {
    let source = r#"
function Foo() {
  return <div>hi</div>
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program).unwrap();
    assert!(result.components.contains(&"Foo".to_string()));
    assert_eq!(result.hook_calls.get("Foo").map(|v| v.len()), Some(0));
}

#[test]
fn ir_hook_order() {
    let source = r#"
function Bar() {
  const [a, setA] = useState(0)
  const [b, setB] = useState(1)
  return <div>{a}</div>
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program).unwrap();
    assert!(result.components.contains(&"Bar".to_string()));
    let hooks = result.hook_calls.get("Bar").unwrap();
    assert_eq!(hooks.len(), 2);
    assert_eq!(hooks[0].hook_name, "useState");
    assert_eq!(hooks[0].slot, 0);
    assert_eq!(hooks[1].hook_name, "useState");
    assert_eq!(hooks[1].slot, 1);
}

#[test]
fn ir_nested_components() {
    let source = r#"
function Layout(props) {
  const { children } = props || {}
  return <div className="layout">{children}</div>
}
export default function App() {
  return <Layout><span>hi</span></Layout>
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program).unwrap();
    assert!(result.components.contains(&"Layout".to_string()));
    assert!(result.components.contains(&"App".to_string()));
    assert_eq!(result.default_export, Some("App".to_string()));
}

#[test]
fn ir_export_default_async() {
    let source = r#"
export default async function queryHook(params) {
  return <div>done</div>
}
"#;
    let program = parse(source).unwrap();
    let result = analyze(&program).unwrap();
    assert_eq!(result.default_export, Some("queryHook".to_string()));
    assert!(result.components.contains(&"queryHook".to_string()));
    let info = result.named_exports.get("queryHook").unwrap();
    assert!(info.is_async);
    assert!(info.is_component);
}
