//! Phase 7: integration-style tests against copied relay-template fixtures.
//!
//! - **Layout.jsx**: parse/analyze when the parser supports it (currently may fail — tracks gap).
//! - **mini_ok.jsx**: minimal default export the runtime can execute end-to-end.

use relay_runtime::{analyze, parse, runtime::Runtime, value_to_vnode, vdom::reconcile};
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/client");
    p.push(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e))
}

/// relay-template `Layout.jsx` — enable when parser supports default params, optional chain, etc.
#[test]
#[ignore = "parser/runtime gap: Layout uses default params, optional chaining, complex JSX"]
fn fixture_layout_parses_and_analyzes() {
    let source = fixture("components/Layout.jsx");
    let program = parse(&source).expect("Layout.jsx parse");
    let analysis = analyze(&program).expect("analysis");
    assert!(
        !analysis.components.is_empty() || analysis.default_export.is_some(),
        "expected component or default export metadata"
    );
}

#[test]
fn fixture_mini_ok_parse_run_vdom_reconcile() {
    let source = fixture("mini_ok.jsx");
    let program = parse(&source).expect("mini_ok.jsx parse");
    let analysis = analyze(&program).expect("analysis");
    let mut r1 = Runtime::new(program, analysis);
    let v1 = r1.run().expect("run");
    let tree1 = value_to_vnode(&v1).normalize();

    let program2 = parse(&source).expect("parse2");
    let analysis2 = analyze(&program2).expect("analyze2");
    let mut r2 = Runtime::new(program2, analysis2);
    let v2 = r2.run().expect("run2");
    let tree2 = value_to_vnode(&v2).normalize();

    assert!(tree1.node_count() >= 1);
    assert!(reconcile::vnodes_equal(&tree1, &tree2));
    assert!(reconcile::diff(&tree1, &tree2).is_empty());
    assert!(reconcile::patches(&tree1, &tree2).is_empty());
}

#[test]
fn fixture_mini_ok_has_layout_class() {
    let source = fixture("mini_ok.jsx");
    let program = parse(&source).unwrap();
    let analysis = analyze(&program).unwrap();
    let mut runtime = Runtime::new(program, analysis);
    let v = runtime.run().unwrap();
    let tree = value_to_vnode(&v).normalize();
    match &tree {
        relay_runtime::VNode::Element { tag, props, .. } => {
            assert_eq!(tag, "div");
            assert_eq!(
                props.get("className"),
                Some(&relay_runtime::PropValue::String("fixture-layout".into()))
            );
        }
        _ => panic!("expected element root"),
    }
}
