//! Virtual DOM: platform-agnostic tree representation.
//!
//! No DOM dependency. The runtime produces Value::Object element trees;
//! we convert to VNode and optionally reconcile. The host commits the
//! tree to actual DOM/native views when ready.

pub mod reconcile;

use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Virtual DOM node. Platform-agnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VNode {
    /// Element: tag, props, children
    Element {
        tag: String,
        props: HashMap<String, PropValue>,
        children: Vec<VNode>,
        key: Option<String>,
    },
    /// Text node
    Text(String),
    /// Null/empty (e.g. {x && <Y />} when x is falsy)
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum PropValue {
    String(String),
    Number(f64),
    Bool(bool),
    /// Event handler or callback ref (host resolves)
    Handler,
    /// Handler registered by ID; host calls invoke_handler(id). Serializes as {"type": "HandlerId", "value": id}.
    HandlerId(u32),
    /// Style object
    Style(HashMap<String, String>),
}

/// Convert runtime Value (JSX output) to VNode.
pub fn value_to_vnode(v: &Value) -> VNode {
    value_to_vnode_with_handlers(v, &mut |_| 0)
}

/// Convert runtime Value to VNode, registering handlers and replacing with HandlerId.
/// `register` is called for each Function/Setter prop; return value becomes the handler ID.
pub fn value_to_vnode_with_handlers<F>(v: &Value, register: &mut F) -> VNode
where
    F: FnMut(&Value) -> u32,
{
    match v {
        Value::Object(obj) => {
            let type_ = obj.get("type").and_then(|t| {
                if let Value::String(s) = t {
                    Some(s.as_str())
                } else {
                    None
                }
            });
            if type_.as_deref() == Some("element") {
                let tag = obj
                    .get("tag")
                    .and_then(|t| {
                        if let Value::String(s) = t {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "div".to_string());
                let default_props = Value::Object(Default::default());
                let props_val = obj.get("props").unwrap_or(&default_props);
                let props = value_props_to_map_with_handlers(props_val, register);
                let children = obj
                    .get("children")
                    .and_then(|c| {
                        if let Value::Array(arr) = c {
                            Some(arr.iter().map(|c| value_to_vnode_with_handlers(c, register)).collect())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                let key = props.get("key").and_then(|p| {
                    if let PropValue::String(s) = p {
                        Some(s.clone())
                    } else {
                        None
                    }
                });
                let mut props = props;
                props.remove("key");
                VNode::Element {
                    tag,
                    props,
                    children,
                    key,
                }
            } else {
                VNode::Empty
            }
        }
        Value::String(s) => VNode::Text(s.clone()),
        Value::Array(arr) => {
            if arr.is_empty() {
                VNode::Empty
            } else if arr.len() == 1 {
                value_to_vnode_with_handlers(&arr[0], register)
            } else {
                VNode::Element {
                    tag: "fragment".to_string(),
                    props: HashMap::new(),
                    children: arr.iter().map(|c| value_to_vnode_with_handlers(c, register)).collect(),
                    key: None,
                }
            }
        }
        Value::Null => VNode::Empty,
        Value::Bool(b) => VNode::Text(if *b { "true" } else { "false" }.to_string()),
        Value::Number(n) => VNode::Text(n.to_string()),
        _ => VNode::Empty,
    }
}

fn value_props_to_map_with_handlers<F>(v: &Value, register: &mut F) -> HashMap<String, PropValue>
where
    F: FnMut(&Value) -> u32,
{
    let mut out = HashMap::new();
    if let Value::Object(obj) = v {
        for (k, v) in obj {
            if let Some(pv) = value_to_prop_with_handlers(k, v, register) {
                out.insert(k.clone(), pv);
            }
        }
    }
    out
}

fn object_to_style_map(obj: &HashMap<String, Value>) -> HashMap<String, String> {
    obj.iter()
        .filter_map(|(k, v)| {
            let s = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => return None,
            };
            Some((k.clone(), s))
        })
        .collect()
}

fn value_to_prop_with_handlers<F>(key: &str, v: &Value, register: &mut F) -> Option<PropValue>
where
    F: FnMut(&Value) -> u32,
{
    match v {
        Value::String(s) => Some(PropValue::String(s.clone())),
        Value::Number(n) => Some(PropValue::Number(*n)),
        Value::Bool(b) => Some(PropValue::Bool(*b)),
        Value::Object(obj) => {
            if obj.get("type").and_then(|t| {
                if let Value::String(s) = t {
                    Some(s.as_str())
                } else {
                    None
                }
            }) == Some("element")
            {
                None
            } else if key == "style" {
                // Always treat `style={{...}}` as a style map — including layout-only
                // keys (display, gridTemplateColumns, gap, …) that the old heuristic dropped.
                Some(PropValue::Style(object_to_style_map(obj)))
            } else if obj.keys().any(|k| {
                [
                    "color",
                    "backgroundColor",
                    "fontSize",
                    "fontWeight",
                    "padding",
                    "paddingTop",
                    "paddingBottom",
                    "paddingLeft",
                    "paddingRight",
                    "margin",
                    "marginTop",
                    "marginBottom",
                    "marginLeft",
                    "marginRight",
                    "display",
                    "flexDirection",
                    "gridTemplateColumns",
                    "gap",
                    "alignItems",
                    "justifyContent",
                ]
                .contains(&k.as_str())
            }) {
                Some(PropValue::Style(object_to_style_map(obj)))
            } else {
                None
            }
        }
        Value::Function { .. } | Value::Native(_) | Value::NativeFn(_) | Value::Setter(_) => {
            let id = register(v);
            if id == 0 {
                Some(PropValue::Handler)
            } else {
                Some(PropValue::HandlerId(id))
            }
        }
        _ => None,
    }
}

impl VNode {
    /// Flatten empty nodes and merge text where possible.
    pub fn normalize(self) -> VNode {
        match self {
            VNode::Element {
                tag,
                props,
                children,
                key,
            } => {
                let children: Vec<VNode> = children
                    .into_iter()
                    .map(VNode::normalize)
                    .filter(|c| !matches!(c, VNode::Empty))
                    .collect();
                VNode::Element {
                    tag,
                    props,
                    children,
                    key,
                }
            }
            VNode::Text(s) if s.is_empty() => VNode::Empty,
            other => other,
        }
    }

    /// Count nodes in tree (for tests).
    pub fn node_count(&self) -> usize {
        match self {
            VNode::Element { children, .. } => 1 + children.iter().map(VNode::node_count).sum::<usize>(),
            VNode::Text(_) => 1,
            VNode::Empty => 0,
        }
    }

    /// Collect all text content (for tests). Concatenates Text nodes in tree order.
    pub fn text_content(&self) -> String {
        match self {
            VNode::Element { children, .. } => {
                children.iter().map(VNode::text_content).collect::<Vec<_>>().join("")
            }
            VNode::Text(s) => s.clone(),
            VNode::Empty => String::new(),
        }
    }
}

/// Collect used tags, classes, and tag+class pairs from a vnode JSON string.
/// Returns JSON object: {tags: string[], classes: string[], tag_classes: string[]}
pub fn collect_usage_from_vnode_json(vnode_json: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(vnode_json).map_err(|e| e.to_string())?;
    let mut tags = std::collections::HashSet::new();
    let mut classes = std::collections::HashSet::new();
    let mut tag_classes = std::collections::HashSet::new();
    collect_usage_rec(&v, &mut tags, &mut classes, &mut tag_classes);
    let out = serde_json::json!({
        "tags": tags.into_iter().collect::<Vec<_>>(),
        "classes": classes.into_iter().collect::<Vec<_>>(),
        "tag_classes": tag_classes.into_iter().collect::<Vec<_>>(),
    });
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

fn collect_usage_rec(
    v: &serde_json::Value,
    tags: &mut std::collections::HashSet<String>,
    classes: &mut std::collections::HashSet<String>,
    tag_classes: &mut std::collections::HashSet<String>,
) {
    if v.get("Text").is_some() {
        return;
    }
    // Value format: { type: "element", tag, props, children }
    let el = v.get("Element").or_else(|| {
        if v.get("type").and_then(|t| t.as_str()) == Some("element") {
            Some(v)
        } else {
            None
        }
    });
    if let Some(el) = el {
        let tag = el
            .get("tag")
            .and_then(|t| t.as_str())
            .unwrap_or("div");
        let t = if tag == "fragment" { "div" } else { tag };
        if !t.is_empty() {
            tags.insert(t.to_string());
        }
        let mut class_name = String::new();
        if let Some(props) = el.get("props") {
            if let Some(cv) = props.get("className") {
                if let Some(s) = cv.get("value").and_then(|x| x.as_str()) {
                    class_name = s.to_string();
                } else if let Some(s) = cv.get("String").and_then(|x| x.as_str()) {
                    class_name = s.to_string();
                } else if let Some(s) = cv.as_str() {
                    class_name = s.to_string();
                }
            }
        }
        if !class_name.is_empty() {
            for c in class_name.trim().split_whitespace().filter(|s| !s.is_empty()) {
                classes.insert(c.to_string());
                if !t.is_empty() {
                    tag_classes.insert(format!("{}|{}", t, c));
                }
            }
        }
        if let Some(children) = el.get("children") {
            if let Some(arr) = children.as_array() {
                for c in arr {
                    collect_usage_rec(c, tags, classes, tag_classes);
                }
            }
        }
        return;
    }
    if let Some(arr) = v.as_array() {
        for item in arr {
            collect_usage_rec(item, tags, classes, tag_classes);
        }
    }
}
