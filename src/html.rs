//! VNode → HTML serialization for SSR (web Platform styles already resolved).

use crate::vdom::{PropValue, VNode};
use std::collections::HashMap;

/// Options for HTML serialization.
#[derive(Debug, Clone)]
pub struct HtmlRenderOptions {
    /// Base URL for resolving relative `src` / `href` (no trailing slash), e.g. `https://host`.
    pub asset_base_url: String,
    /// When true, stamp `data-relay-hid` on elements that have HandlerId props (for hydrate).
    pub stamp_handler_attrs: bool,
}

impl Default for HtmlRenderOptions {
    fn default() -> Self {
        Self {
            asset_base_url: String::new(),
            stamp_handler_attrs: true,
        }
    }
}

/// Serialize a styled VNode tree to an HTML fragment (no document wrapper).
pub fn vnode_to_html(vnode: &VNode, opts: &HtmlRenderOptions) -> String {
    let mut out = String::new();
    write_vnode(&mut out, vnode, opts);
    out
}

fn write_vnode(out: &mut String, vnode: &VNode, opts: &HtmlRenderOptions) {
    match vnode {
        VNode::Empty => {}
        VNode::Text(s) => out.push_str(&escape_html_text(s)),
        VNode::Element {
            tag,
            props,
            children,
            ..
        } => {
            if tag == "fragment" {
                for c in children {
                    write_vnode(out, c, opts);
                }
                return;
            }
            if tag == "relay-hook-renderer" {
                out.push_str(r#"<div class="relay-hook-renderer" data-relay-ssr-stub="hook-renderer">"#);
                for c in children {
                    write_vnode(out, c, opts);
                }
                out.push_str("</div>");
                return;
            }
            if tag == "relay-video-player" {
                write_video_fallback(out, props, opts);
                return;
            }
            let html_tag = tag.as_str();
            out.push('<');
            out.push_str(html_tag);
            write_attrs(out, props, opts);
            if is_void_element(html_tag) {
                out.push_str(" />");
                return;
            }
            out.push('>');
            for c in children {
                write_vnode(out, c, opts);
            }
            out.push_str("</");
            out.push_str(html_tag);
            out.push('>');
        }
    }
}

fn write_video_fallback(
    out: &mut String,
    props: &HashMap<String, PropValue>,
    opts: &HtmlRenderOptions,
) {
    let src = prop_string(props, "src")
        .or_else(|| prop_string(props, "url"))
        .unwrap_or_default();
    let poster = prop_string(props, "poster").unwrap_or_default();
    let resolved_src = resolve_asset_url(&src, &opts.asset_base_url);
    let resolved_poster = resolve_asset_url(&poster, &opts.asset_base_url);
    out.push_str(r#"<div class="relay-video-player" data-relay-ssr-stub="video">"#);
    out.push_str("<video");
    if !resolved_src.is_empty() {
        out.push_str(" src=\"");
        out.push_str(&escape_attr(&resolved_src));
        out.push('"');
    }
    if !resolved_poster.is_empty() {
        out.push_str(" poster=\"");
        out.push_str(&escape_attr(&resolved_poster));
        out.push('"');
    }
    out.push_str(" controls playsinline");
    out.push_str("></video></div>");
}

fn write_attrs(out: &mut String, props: &HashMap<String, PropValue>, opts: &HtmlRenderOptions) {
    let mut handler_ids: Vec<u32> = Vec::new();
    for (key, val) in props {
        match val {
            PropValue::HandlerId(id) => {
                handler_ids.push(*id);
                continue;
            }
            PropValue::Handler => continue,
            PropValue::Style(style) => {
                if style.is_empty() {
                    continue;
                }
                out.push_str(" style=\"");
                out.push_str(&escape_attr(&style_to_css(style)));
                out.push('"');
            }
            PropValue::String(s) => {
                let attr = prop_key_to_attr(key);
                if attr.is_empty() {
                    continue;
                }
                let mut value = s.clone();
                if attr == "src" || attr == "href" || attr == "poster" {
                    value = resolve_asset_url(&value, &opts.asset_base_url);
                }
                out.push(' ');
                out.push_str(&attr);
                out.push_str("=\"");
                out.push_str(&escape_attr(&value));
                out.push('"');
            }
            PropValue::Number(n) => {
                let attr = prop_key_to_attr(key);
                if attr.is_empty() {
                    continue;
                }
                out.push(' ');
                out.push_str(&attr);
                out.push_str("=\"");
                out.push_str(&escape_attr(&n.to_string()));
                out.push('"');
            }
            PropValue::Bool(b) => {
                let attr = prop_key_to_attr(key);
                if attr.is_empty() {
                    continue;
                }
                if *b {
                    out.push(' ');
                    out.push_str(&attr);
                }
            }
        }
    }
    if opts.stamp_handler_attrs && !handler_ids.is_empty() {
        // First handler id is enough for hydrate walk-by-position; stamp all for debug.
        out.push_str(" data-relay-hid=\"");
        out.push_str(
            &handler_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('"');
    }
}

fn prop_key_to_attr(key: &str) -> String {
    if key.starts_with("on") && key.len() > 2 {
        // Event props are not HTML attributes; hydrate attaches listeners.
        return String::new();
    }
    match key {
        "className" => "class".to_string(),
        "htmlFor" => "for".to_string(),
        "tabIndex" => "tabindex".to_string(),
        "readOnly" => "readonly".to_string(),
        "autoPlay" => "autoplay".to_string(),
        "colSpan" => "colspan".to_string(),
        "rowSpan" => "rowspan".to_string(),
        "maxLength" => "maxlength".to_string(),
        "relayControlled" => "data-relay-controlled".to_string(),
        other => {
            // Skip internal / non-DOM props
            if other.starts_with("data-") || other.starts_with("aria-") {
                return other.to_string();
            }
            if other
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return other.to_string();
            }
            String::new()
        }
    }
}

fn style_to_css(style: &HashMap<String, String>) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(style.len());
    let mut keys: Vec<&String> = style.keys().collect();
    keys.sort();
    for k in keys {
        if let Some(v) = style.get(k) {
            if v.is_empty() {
                continue;
            }
            let property = camel_to_kebab(k);
            parts.push(format!(
                "{}:{}",
                property,
                normalize_inline_css_value(&property, v)
            ));
        }
    }
    parts.join(";")
}

fn normalize_inline_css_value(property: &str, value: &str) -> String {
    let trimmed = value.trim();
    let is_numeric = !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        && trimmed.parse::<f64>().is_ok();
    if !is_numeric
        || trimmed.parse::<f64>().ok() == Some(0.0)
        || property.starts_with("--")
        || matches!(
            property,
            "animation-iteration-count"
                | "column-count"
                | "fill-opacity"
                | "flex"
                | "flex-grow"
                | "flex-shrink"
                | "font-weight"
                | "grid-column"
                | "grid-row"
                | "line-height"
                | "opacity"
                | "order"
                | "orphans"
                | "scale"
                | "stroke-opacity"
                | "stroke-width"
                | "tab-size"
                | "widows"
                | "z-index"
                | "zoom"
        )
    {
        trimmed.to_string()
    } else {
        format!("{trimmed}px")
    }
}

fn camel_to_kebab(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn prop_string(props: &HashMap<String, PropValue>, key: &str) -> Option<String> {
    match props.get(key)? {
        PropValue::String(s) => Some(s.clone()),
        PropValue::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn resolve_asset_url(url: &str, base: &str) -> String {
    let u = url.trim();
    if u.is_empty() {
        return String::new();
    }
    if u.starts_with("http://")
        || u.starts_with("https://")
        || u.starts_with("data:")
        || u.starts_with("blob:")
        || u.starts_with("mailto:")
        || u.starts_with("tel:")
        || u.starts_with("//")
    {
        return u.to_string();
    }
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        if u.starts_with('/') {
            return u.to_string();
        }
        return format!("/{}", u.trim_start_matches('/'));
    }
    format!("{}/{}", base, u.trim_start_matches('/'))
}

fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Escape text content for HTML.
pub fn escape_html_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escape `<` in JSON embedded in `<script type="application/json">`.
pub fn escape_json_for_html_script(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for ch in json.chars() {
        if ch == '<' {
            out.push_str("\\u003c");
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn serializes_element_with_style_and_text() {
        let mut style = HashMap::new();
        style.insert("backgroundColor".to_string(), "#fff".to_string());
        style.insert("fontSize".to_string(), "16px".to_string());
        let mut props = HashMap::new();
        props.insert("className".to_string(), PropValue::String("box".to_string()));
        props.insert("style".to_string(), PropValue::Style(style));
        let vnode = VNode::Element {
            tag: "div".to_string(),
            props,
            children: vec![VNode::Text("hi <there>".to_string())],
            key: None,
        };
        let html = vnode_to_html(&vnode, &HtmlRenderOptions::default());
        assert!(html.contains(r#"class="box""#), "{html}");
        assert!(html.contains("background-color:#fff"), "{html}");
        assert!(html.contains("font-size:16px"), "{html}");
        assert!(html.contains("hi &lt;there&gt;"), "{html}");
    }

    #[test]
    fn numeric_inline_styles_follow_react_pixel_semantics() {
        let mut style = HashMap::new();
        style.insert("borderLeftWidth".to_string(), "2".to_string());
        style.insert("opacity".to_string(), "0.5".to_string());
        style.insert("lineHeight".to_string(), "1.6".to_string());
        style.insert("margin".to_string(), "0".to_string());
        let mut props = HashMap::new();
        props.insert("style".to_string(), PropValue::Style(style));
        let vnode = VNode::Element {
            tag: "div".to_string(),
            props,
            children: vec![],
            key: None,
        };
        let html = vnode_to_html(&vnode, &HtmlRenderOptions::default());
        assert!(html.contains("border-left-width:2px"), "{html}");
        assert!(html.contains("opacity:0.5"), "{html}");
        assert!(html.contains("line-height:1.6"), "{html}");
        assert!(html.contains("margin:0"), "{html}");
    }

    #[test]
    fn stamps_handler_ids() {
        let mut props = HashMap::new();
        props.insert("onClick".to_string(), PropValue::HandlerId(3));
        let vnode = VNode::Element {
            tag: "button".to_string(),
            props,
            children: vec![VNode::Text("Go".to_string())],
            key: None,
        };
        let html = vnode_to_html(&vnode, &HtmlRenderOptions::default());
        assert!(html.contains(r#"data-relay-hid="3""#), "{html}");
        assert!(!html.contains("onClick"), "{html}");
    }

    #[test]
    fn video_player_fallback() {
        let mut props = HashMap::new();
        props.insert("src".to_string(), PropValue::String("clip.mp4".to_string()));
        props.insert(
            "poster".to_string(),
            PropValue::String("poster.jpg".to_string()),
        );
        let vnode = VNode::Element {
            tag: "relay-video-player".to_string(),
            props,
            children: vec![],
            key: None,
        };
        let opts = HtmlRenderOptions {
            asset_base_url: "https://cdn.example".to_string(),
            stamp_handler_attrs: false,
        };
        let html = vnode_to_html(&vnode, &opts);
        assert!(html.contains("relay-video-player"), "{html}");
        assert!(html.contains("https://cdn.example/clip.mp4"), "{html}");
        assert!(html.contains("https://cdn.example/poster.jpg"), "{html}");
    }

    #[test]
    fn hook_renderer_stub() {
        let vnode = VNode::Element {
            tag: "relay-hook-renderer".to_string(),
            props: HashMap::new(),
            children: vec![VNode::Text("nested".to_string())],
            key: None,
        };
        let html = vnode_to_html(&vnode, &HtmlRenderOptions::default());
        assert!(html.contains("data-relay-ssr-stub=\"hook-renderer\""), "{html}");
        assert!(html.contains("nested"), "{html}");
    }

    #[test]
    fn resolves_relative_src() {
        let mut props = HashMap::new();
        props.insert("src".to_string(), PropValue::String("img/a.png".to_string()));
        let vnode = VNode::Element {
            tag: "img".to_string(),
            props,
            children: vec![],
            key: None,
        };
        let opts = HtmlRenderOptions {
            asset_base_url: "https://example.com".to_string(),
            stamp_handler_attrs: false,
        };
        let html = vnode_to_html(&vnode, &opts);
        assert!(html.contains(r#"src="https://example.com/img/a.png""#), "{html}");
        assert!(html.contains(" />"), "{html}");
    }

    #[test]
    fn preserves_mailto_and_tel_hrefs() {
        let mut props = HashMap::new();
        props.insert(
            "href".to_string(),
            PropValue::String("mailto:ari@example.com".to_string()),
        );
        let vnode = VNode::Element {
            tag: "a".to_string(),
            props,
            children: vec![],
            key: None,
        };
        let html = vnode_to_html(&vnode, &HtmlRenderOptions::default());
        assert!(html.contains(r#"href="mailto:ari@example.com""#), "{html}");

        let mut props = HashMap::new();
        props.insert(
            "href".to_string(),
            PropValue::String("tel:602-632-6729".to_string()),
        );
        let vnode = VNode::Element {
            tag: "a".to_string(),
            props,
            children: vec![],
            key: None,
        };
        let html = vnode_to_html(&vnode, &HtmlRenderOptions::default());
        assert!(html.contains(r#"href="tel:602-632-6729""#), "{html}");
    }

    #[test]
    fn fragment_unwraps() {
        let vnode = VNode::Element {
            tag: "fragment".to_string(),
            props: HashMap::new(),
            children: vec![
                VNode::Text("a".to_string()),
                VNode::Text("b".to_string()),
            ],
            key: None,
        };
        assert_eq!(vnode_to_html(&vnode, &HtmlRenderOptions::default()), "ab");
    }
}
