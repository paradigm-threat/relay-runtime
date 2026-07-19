//! Virtual `@relay/util|path|date|catalog|ui` modules for CleverScript tips.
//! Pure helpers register on every Host; catalog IO uses `fetch` (SSR maps to git).

use crate::run_state::tip_cached_get;
use crate::value::Value;
use std::collections::HashMap;

pub const RELAY_UTIL: &str = "@relay/util";
pub const RELAY_PATH: &str = "@relay/path";
pub const RELAY_DATE: &str = "@relay/date";
pub const RELAY_CATALOG: &str = "@relay/catalog";
pub const RELAY_UI: &str = "@relay/ui";

/// Resolve pure `@relay/*` modules. Returns `None` for unknown specs.
pub fn try_load_relay_module(path: &str) -> Option<Value> {
    match path {
        RELAY_UTIL | "@relay/util.js" => Some(util_module()),
        RELAY_PATH | "@relay/path.js" => Some(path_module()),
        RELAY_DATE | "@relay/date.js" => Some(date_module()),
        RELAY_CATALOG | "@relay/catalog.js" => Some(catalog_module()),
        RELAY_UI | "@relay/ui.js" => Some(ui_module()),
        _ => None,
    }
}

fn named_exports(map: HashMap<String, Value>) -> Value {
    let mut o = map;
    // Named-import friendly: also expose identity as default for `import x from`
    if !o.contains_key("default") {
        o.insert("default".to_string(), Value::Object(o.clone()));
    }
    Value::Object(o)
}

fn arg_str(args: &[Value], i: usize) -> Option<String> {
    args.get(i).and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
    })
}

fn arg_num(args: &[Value], i: usize) -> Option<f64> {
    args.get(i).and_then(|v| match v {
        Value::Number(n) => Some(*n),
        _ => None,
    })
}

fn arg_bool(args: &[Value], i: usize) -> bool {
    args.get(i).map(|v| v.is_truthy()).unwrap_or(false)
}

fn arg_arr(args: &[Value], i: usize) -> Option<Vec<Value>> {
    args.get(i).and_then(|v| match v {
        Value::Array(a) => Some(a.clone()),
        _ => None,
    })
}

fn arg_obj(args: &[Value], i: usize) -> Option<HashMap<String, Value>> {
    args.get(i).and_then(|v| match v {
        Value::Object(o) => Some(o.clone()),
        _ => None,
    })
}

// --- path ---

pub fn path_basename(p: &str) -> String {
    let p = p.trim_end_matches('/');
    p.rsplit('/').next().unwrap_or(p).to_string()
}

pub fn path_dirname(p: &str) -> String {
    let p = p.trim_end_matches('/');
    match p.rfind('/') {
        Some(i) if i > 0 => p[..i].to_string(),
        Some(_) => "/".to_string(),
        None => ".".to_string(),
    }
}

pub fn path_extname(p: &str) -> String {
    let base = path_basename(p);
    match base.rfind('.') {
        Some(i) if i > 0 => base[i..].to_string(),
        _ => String::new(),
    }
}

pub fn path_split(p: &str) -> Vec<String> {
    p.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

pub fn path_join(parts: &[String]) -> String {
    let mut out = String::new();
    for part in parts {
        let part = part.trim_matches('/');
        if part.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    out
}

fn path_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "basename".into(),
        Value::Native(|args| Ok(Value::String(path_basename(&arg_str(args, 0).unwrap_or_default())))),
    );
    m.insert(
        "dirname".into(),
        Value::Native(|args| Ok(Value::String(path_dirname(&arg_str(args, 0).unwrap_or_default())))),
    );
    m.insert(
        "extname".into(),
        Value::Native(|args| Ok(Value::String(path_extname(&arg_str(args, 0).unwrap_or_default())))),
    );
    m.insert(
        "split".into(),
        Value::Native(|args| {
            let parts = path_split(&arg_str(args, 0).unwrap_or_default());
            Ok(Value::Array(
                parts.into_iter().map(Value::String).collect(),
            ))
        }),
    );
    m.insert(
        "join".into(),
        Value::Native(|args| {
            let mut parts = Vec::new();
            for a in args {
                match a {
                    Value::String(s) => parts.push(s.clone()),
                    Value::Array(arr) => {
                        for v in arr {
                            if let Value::String(s) = v {
                                parts.push(s.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Value::String(path_join(&parts)))
        }),
    );
    named_exports(m)
}

// --- date formatting (park ChangeLog) ---

fn parse_iso_ymd(iso: &str) -> Option<(u32, u32, u32, u32, u32)> {
    if iso.len() < 10 || iso.as_bytes().get(4) != Some(&b'-') {
        return None;
    }
    let y: u32 = iso.get(0..4)?.parse().ok()?;
    let mo: u32 = iso.get(5..7)?.parse().ok()?;
    let d: u32 = iso.get(8..10)?.parse().ok()?;
    let (hh, mm) = if iso.len() >= 16 && iso.as_bytes().get(10) == Some(&b'T') {
        let h: u32 = iso.get(11..13)?.parse().ok()?;
        let m: u32 = iso.get(14..16)?.parse().ok()?;
        (h, m)
    } else {
        (0, 0)
    };
    Some((y, mo, d, hh, mm))
}

fn month_short(mo: u32) -> &'static str {
    match mo {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

fn month_long(mo: u32) -> &'static str {
    match mo {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "???",
    }
}

pub fn format_tiny(iso: &str) -> String {
    let Some((y, mo, d, _, _)) = parse_iso_ymd(iso) else {
        return String::new();
    };
    format!("{} {}, {}", month_short(mo), d, y)
}

pub fn format_long(iso: &str) -> String {
    let Some((y, mo, d, _, _)) = parse_iso_ymd(iso) else {
        return String::new();
    };
    format!("{} {}, {}", month_long(mo), d, y)
}

pub fn format_feed(iso: &str) -> String {
    let Some((y, mo, d, hh, mm)) = parse_iso_ymd(iso) else {
        return format_tiny(iso);
    };
    let y2 = y % 100;
    let (h12, ampm) = if hh == 0 {
        (12, "AM")
    } else if hh < 12 {
        (hh, "AM")
    } else if hh == 12 {
        (12, "PM")
    } else {
        (hh - 12, "PM")
    };
    format!("{}/{}/{:02}, {}:{} {}", mo, d, y2, h12, format!("{:02}", mm), ampm)
}

pub fn same_day(a: &str, b: &str) -> bool {
    a.len() >= 10 && b.len() >= 10 && a[..10] == b[..10]
}

fn date_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "formatTiny".into(),
        Value::Native(|args| Ok(Value::String(format_tiny(&arg_str(args, 0).unwrap_or_default())))),
    );
    m.insert(
        "formatLong".into(),
        Value::Native(|args| Ok(Value::String(format_long(&arg_str(args, 0).unwrap_or_default())))),
    );
    m.insert(
        "formatFeed".into(),
        Value::Native(|args| Ok(Value::String(format_feed(&arg_str(args, 0).unwrap_or_default())))),
    );
    m.insert(
        "sameDay".into(),
        Value::Native(|args| {
            Ok(Value::Bool(same_day(
                &arg_str(args, 0).unwrap_or_default(),
                &arg_str(args, 1).unwrap_or_default(),
            )))
        }),
    );
    named_exports(m)
}

// --- util ---

fn util_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "take".into(),
        Value::Native(|args| {
            let arr = arg_arr(args, 0).unwrap_or_default();
            let n = arg_num(args, 1).unwrap_or(0.0).max(0.0) as usize;
            Ok(Value::Array(arr.into_iter().take(n).collect()))
        }),
    );
    m.insert(
        "prop".into(),
        Value::Native(|args| {
            let obj = arg_obj(args, 0).unwrap_or_default();
            let key = arg_str(args, 1).unwrap_or_default();
            let fallback = args.get(2).cloned().unwrap_or(Value::Null);
            Ok(obj.get(&key).cloned().unwrap_or(fallback))
        }),
    );
    m.insert(
        "omitUndefined".into(),
        Value::Native(|args| {
            let obj = arg_obj(args, 0).unwrap_or_default();
            let mut out = HashMap::new();
            for (k, v) in obj {
                if !matches!(v, Value::Null) {
                    out.insert(k, v);
                }
            }
            Ok(Value::Object(out))
        }),
    );
    m.insert(
        "sortBy".into(),
        Value::Native(|args| {
            let mut arr = arg_arr(args, 0).unwrap_or_default();
            let key = arg_str(args, 1).unwrap_or_default();
            let desc = arg_bool(args, 2);
            arr.sort_by(|a, b| {
                let ka = match a {
                    Value::Object(o) => o.get(&key).cloned().unwrap_or(Value::Null),
                    _ => Value::Null,
                };
                let kb = match b {
                    Value::Object(o) => o.get(&key).cloned().unwrap_or(Value::Null),
                    _ => Value::Null,
                };
                let ord = match (&ka, &kb) {
                    (Value::String(x), Value::String(y)) => x.cmp(y),
                    (Value::Number(x), Value::Number(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                    _ => format!("{:?}", ka).cmp(&format!("{:?}", kb)),
                };
                if desc {
                    ord.reverse()
                } else {
                    ord
                }
            });
            Ok(Value::Array(arr))
        }),
    );
    named_exports(m)
}

// --- catalog ---

pub fn parse_nav_index_txt(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let mut cols = line.splitn(3, '\t');
        let path = cols.next().unwrap_or("").to_string();
        if path.len() < 4 || !path.to_lowercase().ends_with(".md") {
            continue;
        }
        let title = cols.next().unwrap_or(&path).to_string();
        let updated = cols.next().unwrap_or("").to_string();
        let mut row = HashMap::new();
        row.insert("path".into(), Value::String(path));
        row.insert("title".into(), Value::String(title));
        row.insert("content_kind".into(), Value::String("article".into()));
        if !updated.is_empty() {
            row.insert("_updated".into(), Value::String(updated));
        }
        out.push(Value::Object(row));
    }
    out
}

pub fn content_kind(path: &str) -> &'static str {
    if path.len() < 4 || !path.to_lowercase().ends_with(".md") {
        return "exclude";
    }
    let base = path_basename(path);
    let bl = base.to_lowercase();
    if bl == "page.md" || bl == "readme.md" {
        return "exclude";
    }
    if path.starts_with("docs/") {
        return "exclude";
    }
    if path.starts_with("blog/") {
        return "blog";
    }
    if path.contains("/investigations/") || bl.contains("investigation") {
        return "investigation";
    }
    "article"
}

pub fn published_from_blog_path(path: &str) -> String {
    if !path.starts_with("blog/") {
        return String::new();
    }
    let base = path_basename(path);
    if base.len() < 10 || base.as_bytes().get(4) != Some(&b'-') || base.as_bytes().get(7) != Some(&b'-')
    {
        return String::new();
    }
    let y = &base[0..4];
    let m = &base[5..7];
    let d = &base[8..10];
    if y < "1990" {
        return String::new();
    }
    format!("{y}-{m}-{d}T12:00:00Z")
}

/// Build nested folder tree as arrays (CS-friendly, no computed keys in tip).
pub fn build_dir_tree(rows: &[Value]) -> Value {
    #[derive(Default)]
    struct Node {
        files: Vec<Value>,
        folders: HashMap<String, Node>,
    }
    fn insert(node: &mut Node, parts: &[&str], row: &Value) {
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            node.files.push(row.clone());
            return;
        }
        let child = node.folders.entry(parts[0].to_string()).or_default();
        insert(child, &parts[1..], row);
    }
    fn to_value(name: &str, node: Node) -> Value {
        let mut folder_list: Vec<(String, Node)> = node.folders.into_iter().collect();
        folder_list.sort_by(|a, b| a.0.cmp(&b.0));
        let folders: Vec<Value> = folder_list
            .into_iter()
            .map(|(n, child)| to_value(&n, child))
            .collect();
        let mut o = HashMap::new();
        o.insert("name".into(), Value::String(name.into()));
        o.insert("files".into(), Value::Array(node.files));
        o.insert("folders".into(), Value::Array(folders));
        Value::Object(o)
    }
    let mut root = Node::default();
    for row in rows {
        let path = match row {
            Value::Object(o) => o
                .get("path")
                .and_then(|v| match v {
                    Value::String(s) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or(""),
            _ => "",
        };
        if path.is_empty() {
            continue;
        }
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        insert(&mut root, &parts, row);
    }
    to_value("", root)
}

fn catalog_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "parseNavIndexTxt".into(),
        Value::Native(|args| {
            Ok(Value::Array(parse_nav_index_txt(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "contentKind".into(),
        Value::Native(|args| {
            Ok(Value::String(
                content_kind(&arg_str(args, 0).unwrap_or_default()).into(),
            ))
        }),
    );
    m.insert(
        "publishedFromBlogPath".into(),
        Value::Native(|args| {
            Ok(Value::String(published_from_blog_path(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "buildDirTree".into(),
        Value::Native(|args| {
            let rows = arg_arr(args, 0).unwrap_or_default();
            Ok(build_dir_tree(&rows))
        }),
    );
    m.insert(
        "loadNavIndex".into(),
        Value::Native(|args| {
            let limit = arg_num(args, 0).unwrap_or(0.0);
            // Relative tip blob — SSR RepoFetchHost maps to git; CSR uses network.
            let text = tip_cached_get("/nav-index.txt").unwrap_or_default();
            let mut rows = parse_nav_index_txt(&text);
            if limit > 0.0 {
                let n = limit as usize;
                if rows.len() > n {
                    rows.truncate(n);
                }
            }
            Ok(Value::Array(rows))
        }),
    );
    named_exports(m)
}

// --- ui ---

fn el(tag: &str, props: HashMap<String, Value>, children: Vec<Value>) -> Value {
    let mut o = HashMap::new();
    o.insert("type".into(), Value::String("element".into()));
    o.insert("tag".into(), Value::String(tag.into()));
    o.insert("props".into(), Value::Object(props));
    o.insert("children".into(), Value::Array(children));
    Value::Object(o)
}

fn text_node(s: &str) -> Value {
    let mut o = HashMap::new();
    o.insert("type".into(), Value::String("text".into()));
    o.insert("text".into(), Value::String(s.into()));
    Value::Object(o)
}

fn ui_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "omitUndefined".into(),
        Value::Native(|args| {
            let obj = arg_obj(args, 0).unwrap_or_default();
            let mut out = HashMap::new();
            for (k, v) in obj {
                if !matches!(v, Value::Null) {
                    out.insert(k, v);
                }
            }
            Ok(Value::Object(out))
        }),
    );
    m.insert(
        "safeProps".into(),
        Value::Native(|args| {
            let obj = arg_obj(args, 0).unwrap_or_default();
            let mut out = HashMap::new();
            for (k, v) in obj {
                if !matches!(v, Value::Null) {
                    out.insert(k, v);
                }
            }
            Ok(Value::Object(out))
        }),
    );
    m.insert(
        "cachedGet".into(),
        Value::Native(|args| {
            let url = arg_str(args, 0).unwrap_or_default();
            Ok(Value::String(tip_cached_get(&url).unwrap_or_default()))
        }),
    );
    m.insert(
        "Link".into(),
        Value::Native(|args| {
            let props = arg_obj(args, 0).unwrap_or_default();
            let href = props
                .get("href")
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "#".into());
            let external = props.get("external").map(|v| v.is_truthy()).unwrap_or(false);
            let class_name = props
                .get("className")
                .cloned()
                .unwrap_or(Value::String(String::new()));
            let children = props
                .get("children")
                .cloned()
                .unwrap_or(Value::Array(vec![]));
            let child_list = match children {
                Value::Array(a) => a,
                Value::String(s) => vec![text_node(&s)],
                other => vec![other],
            };
            let mut p = HashMap::new();
            p.insert("href".into(), Value::String(href));
            if let Value::String(c) = &class_name {
                if !c.is_empty() {
                    p.insert("className".into(), class_name);
                }
            }
            if external {
                p.insert("target".into(), Value::String("_blank".into()));
                p.insert("rel".into(), Value::String("noopener noreferrer".into()));
            }
            Ok(el("a", p, child_list))
        }),
    );
    m.insert(
        "RadioTabs".into(),
        Value::Native(|args| {
            let props = arg_obj(args, 0).unwrap_or_default();
            let name = props
                .get("name")
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "pt-tabs".into());
            let tabs = match props.get("tabs") {
                Some(Value::Array(a)) => a.clone(),
                _ => vec![],
            };
            let panels = match props.get("panels") {
                Some(Value::Array(a)) => a.clone(),
                _ => vec![],
            };
            let mut tab_els = Vec::new();
            let mut panel_els = Vec::new();
            let mut style_bits = String::from(
                ".pt-relay-tabs-panel{display:none;}.pt-relay-tabs-radio{position:absolute;opacity:0;pointer-events:none;width:0;height:0;}",
            );
            for (i, tab) in tabs.iter().enumerate() {
                let (id, label) = match tab {
                    Value::Object(o) => {
                        let id = o
                            .get("id")
                            .and_then(|v| match v {
                                Value::String(s) => Some(s.clone()),
                                _ => None,
                            })
                            .unwrap_or_else(|| format!("tab{i}"));
                        let label = o
                            .get("label")
                            .and_then(|v| match v {
                                Value::String(s) => Some(s.clone()),
                                _ => None,
                            })
                            .unwrap_or_else(|| id.clone());
                        (id, label)
                    }
                    _ => (format!("tab{i}"), format!("Tab {i}")),
                };
                let radio_id = format!("{}-{}", name, id);
                style_bits.push_str(&format!(
                    ".pt-relay-tabs:has(#{rid}:checked) .pt-relay-tabs-panel-{id}{{display:block;}}",
                    rid = radio_id,
                    id = id
                ));
                let mut radio_props = HashMap::new();
                radio_props.insert("type".into(), Value::String("radio".into()));
                radio_props.insert("name".into(), Value::String(name.clone()));
                radio_props.insert("id".into(), Value::String(radio_id));
                radio_props.insert("className".into(), Value::String("pt-relay-tabs-radio".into()));
                if i == 0 {
                    radio_props.insert("checked".into(), Value::Bool(true));
                }
                let mut label_props = HashMap::new();
                label_props.insert(
                    "className".into(),
                    Value::String(format!("pt-relay-tabs-tab pt-relay-tabs-tab-{id}")),
                );
                tab_els.push(el(
                    "label",
                    label_props,
                    vec![el("input", radio_props, vec![]), text_node(&label)],
                ));
                let panel_body = panels.get(i).cloned().unwrap_or(Value::Null);
                let mut panel_props = HashMap::new();
                panel_props.insert(
                    "className".into(),
                    Value::String(format!("pt-relay-tabs-panel pt-relay-tabs-panel-{id}")),
                );
                let kids = match panel_body {
                    Value::Null => vec![],
                    Value::Array(a) => a,
                    other => vec![other],
                };
                panel_els.push(el("div", panel_props, kids));
            }
            let style_props = HashMap::new();
            let style_el = el("style", style_props, vec![text_node(&style_bits)]);
            let mut toolbar_props = HashMap::new();
            toolbar_props.insert("className".into(), Value::String("pt-relay-tabs-toolbar".into()));
            toolbar_props.insert("role".into(), Value::String("tablist".into()));
            let toolbar = el("div", toolbar_props, tab_els);
            let mut root_props = HashMap::new();
            root_props.insert("className".into(), Value::String("pt-relay-tabs".into()));
            let mut kids = vec![style_el, toolbar];
            kids.extend(panel_els);
            Ok(el("section", root_props, kids))
        }),
    );
    named_exports(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_and_nav_parse() {
        assert_eq!(path_basename("a/b/c.md"), "c.md");
        let rows = parse_nav_index_txt("blog/x.md\tHello\t2026-01-01T00:00:00Z\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(content_kind("blog/x.md"), "blog");
        assert!(!format_tiny("2026-07-18T12:00:00Z").is_empty());
    }
}
