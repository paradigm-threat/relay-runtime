//! Virtual `@clevertree/markdown` host module: **`MarkdownRenderer`** via **md2ast** → CleverScript VDOM Values.
//!
//! Default anchor behavior follows the browser (`href` only). Repos opt into SPA routing by passing
//! **`anchorProps`** (shallow merge onto each `<a>`) and/or **`anchorOnClick`** (sets `onClick` + **`relayControlled`** per link so hosts can `preventDefault` and pass `href` into the callback).

use crate::value::{NativeFnClosure, Value};
use md2ast::{parse as md2ast_parse, Node, TranspileOptions};
use serde_json;
use std::collections::HashMap;
use std::rc::Rc;

/// Matches `relay-client-web` Test tab / WASM default HTML allowlist for `markdown_to_ast_json`.
pub fn default_allowed_html_tags() -> Vec<String> {
    [
        "span", "div", "math", "annotation", "semantics", "mrow", "mi", "mo", "mn",
        "msup", "msub", "mfrac", "mtext",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn transpile_options_from_allowed_json(allowed_html_tags_json: &str) -> TranspileOptions {
    let allowed_tags: Vec<String> =
        serde_json::from_str(allowed_html_tags_json).unwrap_or_default();
    TranspileOptions { allowed_tags }
}

pub fn markdown_to_ast_json(
    markdown: &str,
    options: &TranspileOptions,
) -> Result<String, String> {
    let ast = md2ast_parse(markdown, options);
    serde_json::to_string(&ast).map_err(|e| e.to_string())
}

pub fn markdown_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "MarkdownRenderer".to_string(),
        Value::Native(markdown_renderer_native),
    );
    Value::Object(m)
}

/// Renders markdown to a hook **Value** (`element`/`text` trees or an `Array` of roots) using the same
/// md2ast options as `MarkdownRenderer` with default math-friendly HTML allowlist.
pub fn render_markdown_to_value(
    markdown: &str,
    anchor_props: Option<&HashMap<String, Value>>,
    anchor_on_click: Option<&Value>,
) -> Value {
    let options = TranspileOptions {
        allowed_tags: default_allowed_html_tags(),
    };
    let nodes = md2ast_parse(markdown, &options);
    render_nodes_to_value(&nodes, anchor_props, anchor_on_click)
}

fn element_value(tag: &str, props: HashMap<String, Value>, children: Vec<Value>) -> Value {
    let mut obj = HashMap::new();
    obj.insert("type".to_string(), Value::String("element".to_string()));
    obj.insert(
        "tag".to_string(),
        Value::String(tag.to_string()),
    );
    obj.insert(
        "props".to_string(),
        Value::Object(props),
    );
    obj.insert(
        "children".to_string(),
        Value::Array(children),
    );
    Value::Object(obj)
}

fn md_node_to_value(
    node: &Node,
    anchor_props: Option<&HashMap<String, Value>>,
    anchor_on_click: Option<&Value>,
) -> Value {
    match node {
        Node::Text { content } => Value::String(content.clone()),
        Node::Element {
            tag,
            props,
            children,
        } => {
            let mut props_vm = HashMap::new();
            for (k, v) in props {
                props_vm.insert(k.clone(), Value::from_json(v));
            }
            if tag.as_str() == "a" {
                if let Some(ap) = anchor_props {
                    for (k, v) in ap {
                        props_vm.insert(k.clone(), v.clone());
                    }
                }
                if let Some(h) = anchor_on_click {
                    let href = props_vm
                        .get("href")
                        .and_then(|v| {
                            if let Value::String(s) = v {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();
                    let inner = h.clone();
                    let href_cap = href.clone();
                    let wrap = Value::NativeFn(NativeFnClosure(Rc::new(move |_invoke_args: &[Value]| {
                        crate::run_state::invoke_handler_value(
                            &inner,
                            &[Value::String(href_cap.clone())],
                        )
                        .map_err(|e| e.to_string())
                    })));
                    props_vm.insert("onClick".to_string(), wrap);
                    props_vm.insert("relayControlled".to_string(), Value::Bool(true));
                }
            }
            let kid_vals = children
                .iter()
                .map(|c| md_node_to_value(c, anchor_props, anchor_on_click))
                .collect::<Vec<_>>();
            element_value(tag, props_vm, kid_vals)
        }
    }
}

fn render_nodes_to_value(
    nodes: &[Node],
    anchor_props: Option<&HashMap<String, Value>>,
    anchor_on_click: Option<&Value>,
) -> Value {
    let vals: Vec<Value> = nodes
        .iter()
        .map(|n| md_node_to_value(n, anchor_props, anchor_on_click))
        .collect();
    if vals.len() == 1 {
        vals.into_iter().next().unwrap_or(Value::Null)
    } else {
        Value::Array(vals)
    }
}

fn object_props_only<'a>(
    props: &'a HashMap<String, Value>,
    key: &str,
) -> Option<&'a HashMap<String, Value>> {
    props.get(key).and_then(|v| {
        if let Value::Object(map) = v {
            Some(map)
        } else {
            None
        }
    })
}

fn markdown_renderer_native(args: &[Value]) -> Result<Value, String> {
    let props = args
        .first()
        .ok_or_else(|| "MarkdownRenderer: missing props".to_string())?;
    let Value::Object(props_map) = props else {
        return Err("MarkdownRenderer: props must be an object".to_string());
    };
    let content = props_map
        .get("content")
        .and_then(|v| {
            if let Value::String(s) = v {
                Some(s.as_str())
            } else {
                None
            }
        })
        .unwrap_or("")
        .to_string();
    let anchor_props_full = object_props_only(props_map, "anchorProps");
    let anchor_on_click = props_map.get("anchorOnClick");
    Ok(render_markdown_to_value(
        &content,
        anchor_props_full,
        anchor_on_click,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vdom::{value_to_vnode_with_handlers, PropValue, VNode};

    fn walk_vnode_each<F>(v: &VNode, f: &mut F)
    where
        F: FnMut(&VNode),
    {
        if let VNode::Element { tag, children, .. } = v {
            if tag == "a" {
                f(v);
            }
            for c in children {
                walk_vnode_each(c, f);
            }
        }
    }

    #[test]
    fn link_default_no_click_handler_in_props() {
        let v = render_markdown_to_value(
            "[hello](http://example.com/page)",
            None,
            None,
        );
        let mut registrations = 0u32;
        let vn = value_to_vnode_with_handlers(&v, &mut |_hv| {
            registrations += 1;
            registrations
        });
        assert!(matches!(&vn, VNode::Element { tag: t, .. } if t == "p")
            || matches!(&vn, VNode::Element { tag: t, .. } if t == "a"));
        let mut saw_a = false;
        walk_vnode_each(&vn, &mut |a| {
            saw_a = true;
            if let VNode::Element { props, .. } = a {
                assert!(
                    !props.contains_key("onClick"),
                    "must not synthesize onClick by default"
                );
            }
        });
        assert!(saw_a, "expected an anchor vnode");
        assert_eq!(
            registrations, 0,
            "no handler registration without anchorOnClick"
        );
    }

    #[test]
    fn link_with_anchor_on_click_registers_handler() {
        let click = Value::Native(|_: &[Value]| Ok(Value::Null));
        let v = render_markdown_to_value("[x](https://example.com/foo)", None, Some(&click));
        let mut id = 0u32;
        let vn = value_to_vnode_with_handlers(&v, &mut |_v| {
            id += 1;
            id
        });
        let mut found_on_click = false;
        walk_vnode_each(&vn, &mut |a| {
            if let VNode::Element { props, .. } = a {
                if let Some(PropValue::HandlerId(_)) = props.get("onClick") {
                    found_on_click = true;
                }
                assert_eq!(
                    props.get("relayControlled"),
                    Some(&PropValue::Bool(true)),
                    "anchorOnClick implies relayControlled for host preventDefault policy"
                );
            }
        });
        assert!(found_on_click, "expected onClick handler on <a>");
        assert!(id > 0);
    }

    #[test]
    fn two_md_links_get_distinct_onclick_handler_ids() {
        let click = Value::Native(|_: &[Value]| Ok(Value::Null));
        let v = render_markdown_to_value(
            "[a](https://example.com/a) [b](https://example.com/b)",
            None,
            Some(&click),
        );
        crate::run_state::clear_handler_registry_for_vnode_pass();
        let vn = value_to_vnode_with_handlers(&v, &mut |v| crate::run_state::register_handler(v));
        let mut onclick_ids = Vec::new();
        walk_vnode_each(&vn, &mut |a| {
            if let VNode::Element { props, .. } = a {
                if let Some(PropValue::HandlerId(h)) = props.get("onClick") {
                    onclick_ids.push(*h);
                }
            }
        });
        assert_eq!(onclick_ids.len(), 2, "expected two anchors with onClick");
        assert_ne!(onclick_ids[0], onclick_ids[1], "per-link wrappers must register distinct IDs");
    }

    #[test]
    fn anchor_props_classname_merge() {
        let mut overlay = HashMap::new();
        overlay.insert(
            "className".to_string(),
            Value::String("relay-md-link".to_string()),
        );
        let v = render_markdown_to_value("[x](y)", Some(&overlay), None);
        let vn = value_to_vnode_with_handlers(&v, &mut |_| 0);
        let mut found = false;
        walk_vnode_each(&vn, &mut |a| {
            if let VNode::Element { props, .. } = a {
                if let Some(PropValue::String(s)) = props.get("className") {
                    if s == "relay-md-link" {
                        found = true;
                    }
                }
            }
        });
        assert!(found, "expected merged className on anchor");
    }
}
