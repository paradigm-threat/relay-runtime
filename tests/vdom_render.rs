//! Phase 5: Virtual DOM tests (platform-agnostic)

use relay_runtime::{analyze, parse, runtime::Runtime, value_to_vnode, vdom::VNode};

#[test]
fn vdom_simple_div() {
    let source = r#"
export default function App() {
  return <div>hello</div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run().unwrap();
    let vnode = value_to_vnode(&result);
    let vnode = vnode.normalize();

    match &vnode {
        VNode::Element { tag, children, .. } => {
            assert_eq!(tag, "div");
            assert_eq!(children.len(), 1);
            if let VNode::Text(t) = &children[0] {
                assert_eq!(t, "hello");
            } else {
                panic!("expected text child");
            }
        }
        _ => panic!("expected element, got {:?}", vnode),
    }
}

#[test]
fn vdom_nested_structure() {
    let source = r#"
export default function App() {
  return (
    <div className="layout">
      <span>a</span>
      <span>b</span>
    </div>
  )
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run().unwrap();
    let vnode = value_to_vnode(&result).normalize();

    match &vnode {
        VNode::Element { tag, children, props, .. } => {
            assert_eq!(tag, "div");
            assert_eq!(children.len(), 2);
            assert!(props.contains_key("className"));
            if let VNode::Text(t) = &children[0] {
                assert_eq!(t, "a");
            }
            if let VNode::Text(t) = &children[1] {
                assert_eq!(t, "b");
            }
        }
        _ => panic!("expected element"),
    }
}

#[test]
fn vdom_node_count() {
    let source = r#"
export default function App() {
  return <div><span>x</span></div>
}
"#;
    let program = parse(source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);

    let result = runtime.run().unwrap();
    let vnode = value_to_vnode(&result).normalize();

    assert_eq!(vnode.node_count(), 3); // div + span + text
}
