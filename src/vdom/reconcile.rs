//! Virtual DOM diff: compare trees and describe changes (no DOM).
//!
//! Hosts can use [`patches`] to apply minimal updates, or [`diff`] for debugging/tests.

use super::{PropValue, VNode};
use std::collections::HashMap;

/// Human- / test-oriented difference between two trees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    KindMismatch {
        path: Vec<usize>,
        /// "empty", "text", "element:div"
        left: String,
        right: String,
    },
    TagChanged {
        path: Vec<usize>,
        from: String,
        to: String,
    },
    KeyChanged {
        path: Vec<usize>,
        from: Option<String>,
        to: Option<String>,
    },
    PropKeyMissing {
        path: Vec<usize>,
        name: String,
    },
    PropKeyExtra {
        path: Vec<usize>,
        name: String,
    },
    PropValueChanged {
        path: Vec<usize>,
        name: String,
    },
    TextChanged {
        path: Vec<usize>,
        from: String,
        to: String,
    },
    ChildCount {
        path: Vec<usize>,
        left: usize,
        right: usize,
    },
}

fn vnode_kind_label(v: &VNode) -> String {
    match v {
        VNode::Empty => "empty".to_string(),
        VNode::Text(_) => "text".to_string(),
        VNode::Element { tag, .. } => format!("element:{tag}"),
    }
}

/// Deep structural + value diff (props compared with `PartialEq`).
pub fn diff(left: &VNode, right: &VNode) -> Vec<DiffOp> {
    let mut out = Vec::new();
    diff_at(left, right, &[], &mut out);
    out
}

/// True if trees are equal (uses `VNode` `PartialEq`).
#[inline]
pub fn vnodes_equal(left: &VNode, right: &VNode) -> bool {
    left == right
}

fn diff_at(left: &VNode, right: &VNode, path: &[usize], out: &mut Vec<DiffOp>) {
    match (left, right) {
        (VNode::Empty, VNode::Empty) => {}
        (VNode::Text(a), VNode::Text(b)) => {
            if a != b {
                out.push(DiffOp::TextChanged {
                    path: path.to_vec(),
                    from: a.clone(),
                    to: b.clone(),
                });
            }
        }
        (VNode::Element { .. }, VNode::Element { .. }) => {
            diff_elements(left, right, path, out);
        }
        _ => {
            out.push(DiffOp::KindMismatch {
                path: path.to_vec(),
                left: vnode_kind_label(left),
                right: vnode_kind_label(right),
            });
        }
    }
}

fn diff_props(
    path: &[usize],
    a: &HashMap<String, PropValue>,
    b: &HashMap<String, PropValue>,
    out: &mut Vec<DiffOp>,
) {
    for k in a.keys() {
        if !b.contains_key(k) {
            out.push(DiffOp::PropKeyMissing {
                path: path.to_vec(),
                name: k.clone(),
            });
        } else if a.get(k) != b.get(k) {
            out.push(DiffOp::PropValueChanged {
                path: path.to_vec(),
                name: k.clone(),
            });
        }
    }
    for k in b.keys() {
        if !a.contains_key(k) {
            out.push(DiffOp::PropKeyExtra {
                path: path.to_vec(),
                name: k.clone(),
            });
        }
    }
}

fn diff_elements(left: &VNode, right: &VNode, path: &[usize], out: &mut Vec<DiffOp>) {
    let (VNode::Element {
        tag: tl,
        props: pl,
        children: cl,
        key: kl,
    }, VNode::Element {
        tag: tr,
        props: pr,
        children: cr,
        key: kr,
    }) = (left, right)
    else {
        return;
    };

    if tl != tr {
        out.push(DiffOp::TagChanged {
            path: path.to_vec(),
            from: tl.clone(),
            to: tr.clone(),
        });
    }
    if kl != kr {
        out.push(DiffOp::KeyChanged {
            path: path.to_vec(),
            from: kl.clone(),
            to: kr.clone(),
        });
    }
    diff_props(path, pl, pr, out);

    if cl.len() != cr.len() {
        out.push(DiffOp::ChildCount {
            path: path.to_vec(),
            left: cl.len(),
            right: cr.len(),
        });
    }
    let n = cl.len().min(cr.len());
    for i in 0..n {
        let mut p = path.to_vec();
        p.push(i);
        diff_at(&cl[i], &cr[i], &p, out);
    }
}

/// Coarse patches: replace whole subtrees where structure diverges.
#[derive(Debug, Clone, PartialEq)]
pub enum Patch {
    /// Replace everything at `path` with `tree` (path `[]` = root).
    ReplaceSubtree {
        path: Vec<usize>,
        tree: VNode,
    },
}

/// Build a minimal list of [`Patch`] instructions (greedy: first mismatch → replace subtree).
pub fn patches(old: &VNode, new: &VNode) -> Vec<Patch> {
    if old == new {
        return Vec::new();
    }
    let mut acc = Vec::new();
    patches_at(old, new, &[], &mut acc);
    acc
}

fn patches_at(old: &VNode, new: &VNode, path: &[usize], acc: &mut Vec<Patch>) {
    if old == new {
        return;
    }
    match (old, new) {
        (VNode::Empty, VNode::Empty) => {}
        (VNode::Text(a), VNode::Text(b)) if a == b => {}
        (
            VNode::Element {
                tag: t1,
                key: k1,
                children: c1,
                ..
            },
            VNode::Element {
                tag: t2,
                key: k2,
                children: c2,
                ..
            },
        ) if t1 == t2 && k1 == k2 && c1.len() == c2.len() => {
            for i in 0..c1.len() {
                let mut p = path.to_vec();
                p.push(i);
                patches_at(&c1[i], &c2[i], &p, acc);
            }
        }
        _ => {
            acc.push(Patch::ReplaceSubtree {
                path: path.to_vec(),
                tree: new.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn el(tag: &str, children: Vec<VNode>) -> VNode {
        VNode::Element {
            tag: tag.to_string(),
            props: HashMap::new(),
            children,
            key: None,
        }
    }

    #[test]
    fn diff_identical_empty() {
        assert!(diff(&VNode::Empty, &VNode::Empty).is_empty());
    }

    #[test]
    fn diff_text_change() {
        let d = diff(&VNode::Text("a".into()), &VNode::Text("b".into()));
        assert_eq!(d.len(), 1);
        assert!(matches!(
            &d[0],
            DiffOp::TextChanged { from, to, .. } if from == "a" && to == "b"
        ));
    }

    #[test]
    fn patches_text_replaces_root() {
        let p = patches(&VNode::Text("a".into()), &VNode::Text("b".into()));
        assert_eq!(p.len(), 1);
        assert!(matches!(
            &p[0],
            Patch::ReplaceSubtree { path, tree } if path.is_empty() && *tree == VNode::Text("b".into())
        ));
    }

    #[test]
    fn patches_nested_child_only() {
        let old = el("div", vec![VNode::Text("x".into())]);
        let new = el("div", vec![VNode::Text("y".into())]);
        let p = patches(&old, &new);
        assert_eq!(p.len(), 1);
        assert!(matches!(
            &p[0],
            Patch::ReplaceSubtree { path, .. } if path == &[0]
        ));
    }
}
