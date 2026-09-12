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
pub const RELAY_TTS: &str = "@relay/tts";

/// Resolve pure `@relay/*` modules. Returns `None` for unknown specs.
pub fn try_load_relay_module(path: &str) -> Option<Value> {
    match path {
        RELAY_UTIL | "@relay/util.js" => Some(util_module()),
        RELAY_PATH | "@relay/path.js" => Some(path_module()),
        RELAY_DATE | "@relay/date.js" => Some(date_module()),
        RELAY_CATALOG | "@relay/catalog.js" => Some(catalog_module()),
        RELAY_UI | "@relay/ui.js" => Some(ui_module()),
        RELAY_TTS | "@relay/tts.js" => Some(tts_module()),
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

/// Strip leading slashes; empty → `README.md` (tip article default).
pub fn path_normalize_rel(path: &str) -> String {
    let mut t = path.trim().to_string();
    while t.starts_with('/') {
        t = t[1..].to_string();
    }
    if t.is_empty() {
        "README.md".into()
    } else {
        t
    }
}

/// Resolve a markdown/href against the current article path (`a/b.md` + `../c.png` → `a/c.png`).
/// Optional `base_url`: if `href` is absolute under that host, strip to a tip-relative path.
pub fn path_resolve_rel(href: &str, current_rel: &str, base_url: &str) -> String {
    let h = href.trim();
    if h.is_empty() {
        return path_normalize_rel(current_rel);
    }
    if h.starts_with("mailto:") || h.starts_with("tel:") || h.starts_with('#') {
        return path_normalize_rel(current_rel);
    }
    if h.contains("://") {
        let base = base_url.trim_end_matches('/');
        if !base.is_empty() && h.starts_with(base) {
            let mut rest = h[base.len()..].to_string();
            while rest.starts_with('/') {
                rest = rest[1..].to_string();
            }
            return path_normalize_rel(&rest);
        }
        return path_normalize_rel(h);
    }
    if h.starts_with('/') {
        return path_normalize_rel(h);
    }
    let mut rel = h.to_string();
    if rel.starts_with("./") {
        rel = rel[2..].to_string();
    }
    let mut dir = path_normalize_rel(current_rel);
    if let Some(i) = dir.rfind('/') {
        dir = dir[..i].to_string();
    } else {
        dir.clear();
    }
    let mut segs: Vec<String> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
    };
    for p in rel.split('/') {
        if p.is_empty() || p == "." {
            continue;
        }
        if p == ".." {
            segs.pop();
        } else {
            segs.push(p.to_string());
        }
    }
    path_normalize_rel(&path_join(&segs))
}

pub fn path_parent_dir(rel: &str) -> String {
    let p = path_normalize_rel(rel);
    match p.rfind('/') {
        Some(i) if i > 0 => path_normalize_rel(&p[..i]),
        _ => "README.md".into(),
    }
}

/// Join park base URL with a site path (`base` + `/` + path). Absolute/`mailto`/`#` pass through.
pub fn path_site_href(base: &str, path: &str) -> String {
    let mut p = path.to_string();
    if p.is_empty() {
        p = "/".into();
    }
    if p.starts_with('#') || p.starts_with("mailto:") || p.starts_with("tel:") {
        return p;
    }
    if p.contains("://") {
        return p;
    }
    while p.starts_with('/') {
        p = p[1..].to_string();
    }
    let b = base.trim_end_matches('/').to_string();
    if b.is_empty() {
        return format!("/{p}");
    }
    if p.is_empty() {
        return format!("{b}/");
    }
    format!("{b}/{p}")
}

fn is_asset_ext(path: &str) -> bool {
    let lower = path.to_lowercase();
    let ext = path_extname(&lower);
    matches!(
        ext.as_str(),
        ".png"
            | ".jpg"
            | ".jpeg"
            | ".gif"
            | ".webp"
            | ".svg"
            | ".pdf"
            | ".json"
            | ".txt"
            | ".csv"
            | ".mp4"
            | ".webm"
            | ".wav"
            | ".mp3"
    )
}

/// Resolve embed/img `src` to a tip-relative path (no host). Absolute http(s) returned unchanged.
pub fn path_resolve_embed_src(src: &str, article_rel: &str) -> String {
    let s = src.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("data:") {
        return s.to_string();
    }
    let (path_part, query) = match s.split_once('?') {
        Some((p, q)) => (p.to_string(), format!("?{q}")),
        None => (s.to_string(), String::new()),
    };
    if path_part.starts_with('/') {
        return format!("{}{}", path_normalize_rel(&path_part), query);
    }
    format!("{}{}", path_resolve_rel(&path_part, article_rel, ""), query)
}

/// Absolute URL for a binary/raster asset (GitLab Pages / mirror). Relative → mirror + resolved path.
pub fn path_resolve_asset_url(src: &str, article_rel: &str, mirror_base: &str) -> String {
    let s = src.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("data:") {
        return s.to_string();
    }
    let rel = path_resolve_embed_src(s, article_rel);
    if rel.is_empty() {
        return String::new();
    }
    if rel.contains("://") {
        return rel;
    }
    let mirror = mirror_base.trim_end_matches('/');
    if mirror.is_empty() {
        return format!("/{rel}");
    }
    format!("{mirror}/{rel}")
}

/// Landscape `foo.ext` → sibling `foo-portrait.ext`, preserving origin + query.
/// Empty string when SVG, already `-portrait`, or unparseable.
pub fn path_portrait_sibling_src(src: &str) -> String {
    if src.is_empty() {
        return String::new();
    }
    let (path_and_host, query) = match src.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (src, None),
    };
    let lower = path_and_host.to_ascii_lowercase();
    if lower.ends_with(".svg") {
        return String::new();
    }
    let (prefix, file) = match path_and_host.rfind('/') {
        Some(i) => (&path_and_host[..=i], &path_and_host[i + 1..]),
        None => ("", path_and_host),
    };
    let Some(dot) = file.rfind('.') else {
        return String::new();
    };
    if dot == 0 {
        return String::new();
    }
    let stem = &file[..dot];
    let ext = &file[dot..];
    if stem.to_ascii_lowercase().ends_with("-portrait") {
        return String::new();
    }
    let mut out = String::with_capacity(src.len() + 9);
    out.push_str(prefix);
    out.push_str(stem);
    out.push_str("-portrait");
    out.push_str(ext);
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    out
}

/// Strip scheme/host and query → repo-relative path (`governance/war/foo.png`).
pub fn path_repo_rel_from_media_src(src: &str) -> String {
    let no_query = src.split('?').next().unwrap_or(src);
    let path = if no_query.starts_with("http://") || no_query.starts_with("https://") {
        let rest = no_query
            .split_once("://")
            .map(|(_, r)| r)
            .unwrap_or(no_query);
        rest.find('/').map(|i| &rest[i..]).unwrap_or("/")
    } else {
        no_query
    };
    path.trim_start_matches('/').to_string()
}

/// DOM id for gallery anchors: `img-` + alnum, other chars → `-`.
pub fn path_safe_img_dom_id(src: &str) -> String {
    if src.is_empty() {
        return "img-unknown".into();
    }
    let mut out = String::from("img-");
    for c in src.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push('-');
        }
    }
    out
}

/// Parse `location.hash` (`#img=…` or `img=…`) → image src (may be absolute URL).
pub fn path_read_hash_img_src(hash: &str) -> String {
    let h = hash.trim();
    if let Some(rest) = h.strip_prefix("#img=") {
        return rest.to_string();
    }
    if let Some(rest) = h.strip_prefix("img=") {
        return rest.to_string();
    }
    String::new()
}

/// Value for `location.hash = …` (no leading `#`; browser adds it).
pub fn path_gallery_img_hash(src: &str) -> String {
    if src.is_empty() {
        return String::new();
    }
    format!("img={src}")
}

/// Prefer `highResSrc` when present; else `src`.
pub fn path_gallery_display_src(src: &str, high_res_src: &str) -> String {
    if !high_res_src.is_empty() {
        return high_res_src.to_string();
    }
    src.to_string()
}

/// Invent portrait sibling URL only when `map[repoRel] === true`.
pub fn path_portrait_sibling_if_present(src: &str, map: &Value) -> String {
    let cand = path_portrait_sibling_src(src);
    if cand.is_empty() {
        return String::new();
    }
    let rel = path_repo_rel_from_media_src(&cand);
    if rel.is_empty() {
        return String::new();
    }
    let Value::Object(o) = map else {
        return String::new();
    };
    match o.get(&rel) {
        Some(Value::Bool(true)) => cand,
        Some(Value::Number(n)) if *n == 1.0 => cand,
        Some(Value::String(s)) if s == "true" || s == "1" => cand,
        _ => String::new(),
    }
}

/// Canonical http(s) href for dead-URL maps. Must match paradigm-threat-files
/// `scripts/lib/dead-urls.mjs` `normalizeHttpHref` (and the Node checker tests).
///
/// Trim; non-http(s) returned unchanged. Lowercase scheme + host. Drop `#fragment`.
/// Drop default ports `:80` / `:443`. Drop a trailing `/` on the path except
/// origin-only `https://host/`. Keep query. Do not rewrite `http`↔`https` or strip
/// `www`. Parse failure → original string.
pub fn path_normalize_http_href(href: &str) -> String {
    let trimmed = href.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_ascii_lowercase();
    let (scheme, rest) = if lower.starts_with("https://") {
        ("https", &trimmed[8..])
    } else if lower.starts_with("http://") {
        ("http", &trimmed[7..])
    } else {
        return trimmed.to_string();
    };

    let rest = match rest.find('#') {
        Some(i) => &rest[..i],
        None => rest,
    };
    let (hier, query) = match rest.find('?') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if hier.is_empty() {
        return trimmed.to_string();
    }

    let hier = match hier.rfind('@') {
        Some(i) => &hier[i + 1..],
        None => hier,
    };
    if hier.is_empty() {
        return trimmed.to_string();
    }

    let Some((authority, path_raw)) = split_http_authority_path(hier) else {
        return trimmed.to_string();
    };
    if authority.is_empty() {
        return trimmed.to_string();
    }

    let Some((host_raw, port_opt)) = split_http_host_port(authority) else {
        return trimmed.to_string();
    };
    if host_raw.is_empty() {
        return trimmed.to_string();
    }
    let host = host_raw.to_ascii_lowercase();

    let drop_port = match port_opt {
        Some("443") if scheme == "https" => true,
        Some("80") if scheme == "http" => true,
        None => true,
        Some(_) => false,
    };
    let port_part = match port_opt {
        Some(p) if !drop_port => format!(":{p}"),
        _ => String::new(),
    };

    let mut path = if path_raw.is_empty() {
        "/".to_string()
    } else {
        path_raw.to_string()
    };
    if path.len() > 1 && path.ends_with('/') {
        path.pop();
    }

    let origin = format!("{scheme}://{host}{port_part}");
    if path == "/" && query.is_empty() {
        format!("{origin}/")
    } else {
        format!("{origin}{path}{query}")
    }
}

fn split_http_authority_path(hier: &str) -> Option<(&str, &str)> {
    if hier.starts_with('[') {
        let end = hier.find(']')?;
        let mut i = end + 1;
        if hier.as_bytes().get(i) == Some(&b':') {
            i += 1;
            let port_start = i;
            while i < hier.len() && hier.as_bytes()[i].is_ascii_digit() {
                i += 1;
            }
            if i == port_start {
                return None;
            }
        }
        if i == hier.len() {
            return Some((hier, ""));
        }
        if hier.as_bytes().get(i) == Some(&b'/') {
            return Some((&hier[..i], &hier[i..]));
        }
        return None;
    }
    match hier.find('/') {
        Some(i) => Some((&hier[..i], &hier[i..])),
        None => Some((hier, "")),
    }
}

fn split_http_host_port(authority: &str) -> Option<(&str, Option<&str>)> {
    if authority.starts_with('[') {
        let end = authority.find(']')?;
        let host = &authority[..=end];
        let after = &authority[end + 1..];
        if after.is_empty() {
            return Some((host, None));
        }
        let port = after.strip_prefix(':')?;
        if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        return Some((host, Some(port)));
    }
    match authority.rfind(':') {
        Some(i) => {
            let port = &authority[i + 1..];
            if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            Some((&authority[..i], Some(port)))
        }
        None => Some((authority, None)),
    }
}

/// Look up `href` in a dead-URL map: exact key, then [`path_normalize_http_href`].
/// Missing / non-object map → `null` (CleverScript must not throw).
pub fn path_lookup_http_href(map: &Value, href: &str) -> Value {
    let Value::Object(o) = map else {
        return Value::Null;
    };
    if let Some(v) = o.get(href) {
        if matches!(v, Value::Object(_)) {
            return v.clone();
        }
    }
    let n = path_normalize_http_href(href);
    if let Some(v) = o.get(&n) {
        if matches!(v, Value::Object(_)) {
            return v.clone();
        }
    }
    Value::Null
}

/// Split text on `.` `!` `?` into trimmed sentences (TTS chunking).
pub fn util_split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        cur.push(c);
        if c == '.' || c == '!' || c == '?' {
            let t = cur.trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
            cur.clear();
        }
    }
    let tail = cur.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

/// Pack sentences from `start` until `max_chars` (char count soft bound, tip TTS dock).
/// Returns `(chunk_text, next_index)`.
pub fn util_take_sentence_chunk(sentences: &[String], start: usize, max_chars: usize) -> (String, usize) {
    let mut out = String::new();
    let mut i = start;
    while i < sentences.len() {
        let next = &sentences[i];
        let out_len = out.chars().count();
        let next_len = next.chars().count();
        if !out.is_empty() && out_len + next_len + 1 > max_chars {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(next);
        i += 1;
        if out.chars().count() >= max_chars {
            break;
        }
    }
    (out, i)
}

/// Rewrite relative asset URLs in markdown `](...)` and `src="..."` / `src='...'`.
pub fn path_rewrite_markdown_asset_urls(md: &str, article_rel: &str, mirror_base: &str) -> String {
    if md.is_empty() || mirror_base.trim().is_empty() {
        return md.to_string();
    }
    let bytes = md.as_bytes();
    let mut out = String::with_capacity(md.len() + 64);
    let mut i = 0usize;
    while i < bytes.len() {
        // ](url)
        if bytes[i] == b']' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
            out.push(']');
            out.push('(');
            i += 2;
            let start = i;
            while i < bytes.len() && bytes[i] != b')' && bytes[i] != b'\n' {
                i += 1;
            }
            let url = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
            let rewritten = if should_rewrite_asset_url(url) {
                path_resolve_asset_url(url, article_rel, mirror_base)
            } else {
                url.to_string()
            };
            out.push_str(&rewritten);
            // `i` still at closing `)` / newline — next loop emits it.
            continue;
        }
        // src="..." or src='...'
        if i + 5 <= bytes.len()
            && (bytes[i] == b's' || bytes[i] == b'S')
            && std::str::from_utf8(&bytes[i..i + 4])
                .map(|s| s.eq_ignore_ascii_case("src="))
                .unwrap_or(false)
        {
            out.push_str(std::str::from_utf8(&bytes[i..i + 4]).unwrap_or("src="));
            i += 4;
            let quote = bytes.get(i).copied().unwrap_or(0);
            if quote == b'"' || quote == b'\'' {
                out.push(quote as char);
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != quote && bytes[i] != b'\n' {
                    i += 1;
                }
                let url = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                let rewritten = if should_rewrite_asset_url(url) {
                    path_resolve_asset_url(url, article_rel, mirror_base)
                } else {
                    url.to_string()
                };
                out.push_str(&rewritten);
                // `i` still at closing quote — next loop emits it.
                continue;
            }
            // Unquoted src= — keep scanning from current `i`.
        }
        // Emit one UTF-8 char (never slice mid-codepoint).
        // Defensive: if a prior byte-scan left `i` mid-sequence, skip to next boundary.
        if i < md.len() && !md.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let Some(ch) = md[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn should_rewrite_asset_url(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() || u.starts_with('#') || u.starts_with("mailto:") || u.starts_with("tel:") {
        return false;
    }
    if u.starts_with("http://") || u.starts_with("https://") || u.starts_with("data:") {
        return false;
    }
    let path = u.split('?').next().unwrap_or(u);
    is_asset_ext(path)
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
    m.insert(
        "normalizeRel".into(),
        Value::Native(|args| {
            Ok(Value::String(path_normalize_rel(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "resolveRel".into(),
        Value::Native(|args| {
            Ok(Value::String(path_resolve_rel(
                &arg_str(args, 0).unwrap_or_default(),
                &arg_str(args, 1).unwrap_or_default(),
                &arg_str(args, 2).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "parentDir".into(),
        Value::Native(|args| {
            Ok(Value::String(path_parent_dir(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "siteHref".into(),
        Value::Native(|args| {
            Ok(Value::String(path_site_href(
                &arg_str(args, 0).unwrap_or_default(),
                &arg_str(args, 1).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "resolveEmbedSrc".into(),
        Value::Native(|args| {
            Ok(Value::String(path_resolve_embed_src(
                &arg_str(args, 0).unwrap_or_default(),
                &arg_str(args, 1).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "resolveAssetUrl".into(),
        Value::Native(|args| {
            Ok(Value::String(path_resolve_asset_url(
                &arg_str(args, 0).unwrap_or_default(),
                &arg_str(args, 1).unwrap_or_default(),
                &arg_str(args, 2).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "rewriteMarkdownAssetUrls".into(),
        Value::Native(|args| {
            Ok(Value::String(path_rewrite_markdown_asset_urls(
                &arg_str(args, 0).unwrap_or_default(),
                &arg_str(args, 1).unwrap_or_default(),
                &arg_str(args, 2).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "portraitSiblingSrc".into(),
        Value::Native(|args| {
            Ok(Value::String(path_portrait_sibling_src(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "repoRelFromMediaSrc".into(),
        Value::Native(|args| {
            Ok(Value::String(path_repo_rel_from_media_src(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "safeImgDomId".into(),
        Value::Native(|args| {
            Ok(Value::String(path_safe_img_dom_id(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "readHashImgSrc".into(),
        Value::Native(|args| {
            Ok(Value::String(path_read_hash_img_src(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "galleryImgHash".into(),
        Value::Native(|args| {
            Ok(Value::String(path_gallery_img_hash(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "galleryDisplaySrc".into(),
        Value::Native(|args| {
            Ok(Value::String(path_gallery_display_src(
                &arg_str(args, 0).unwrap_or_default(),
                &arg_str(args, 1).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "portraitSiblingIfPresent".into(),
        Value::Native(|args| {
            let src = arg_str(args, 0).unwrap_or_default();
            let map = args.get(1).cloned().unwrap_or(Value::Null);
            Ok(Value::String(path_portrait_sibling_if_present(&src, &map)))
        }),
    );
    m.insert(
        "normalizeHttpHref".into(),
        Value::Native(|args| {
            Ok(Value::String(path_normalize_http_href(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "lookupHttpHref".into(),
        Value::Native(|args| {
            let map = args.get(0).cloned().unwrap_or(Value::Null);
            let href = arg_str(args, 1).unwrap_or_default();
            Ok(path_lookup_http_href(&map, &href))
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
    match (a.get(..10), b.get(..10)) {
        (Some(da), Some(db)) => da == db,
        _ => false,
    }
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

/// TOC / heading id: lowercase, spaces→`_`, keep `[a-z0-9_-]`.
pub fn util_slugify(title: &str) -> String {
    let s = title.to_lowercase();
    let mut out = String::new();
    for c in s.chars() {
        if c.is_whitespace() {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
        } else if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "section".into()
    } else {
        out
    }
}

pub fn util_includes_str(arr: &[Value], item: &str) -> bool {
    for v in arr {
        if let Value::String(s) = v {
            if s == item {
                return true;
            }
        }
    }
    false
}

pub fn util_unique_id(base: &str, used: &[Value]) -> String {
    let mut id = base.to_string();
    let mut n = 1u32;
    while util_includes_str(used, &id) {
        id = format!("{base}_{n}");
        n += 1;
    }
    id
}

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
    m.insert(
        "slugify".into(),
        Value::Native(|args| {
            Ok(Value::String(util_slugify(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "includes".into(),
        Value::Native(|args| {
            let arr = arg_arr(args, 0).unwrap_or_default();
            let item = arg_str(args, 1).unwrap_or_default();
            Ok(Value::Bool(util_includes_str(&arr, &item)))
        }),
    );
    m.insert(
        "uniqueId".into(),
        Value::Native(|args| {
            let base = arg_str(args, 0).unwrap_or_default();
            let used = arg_arr(args, 1).unwrap_or_default();
            Ok(Value::String(util_unique_id(&base, &used)))
        }),
    );
    m.insert(
        "splitSentences".into(),
        Value::Native(|args| {
            let parts = util_split_sentences(&arg_str(args, 0).unwrap_or_default());
            Ok(Value::Array(
                parts.into_iter().map(Value::String).collect(),
            ))
        }),
    );
    m.insert(
        "takeSentenceChunk".into(),
        Value::Native(|args| {
            let raw = arg_arr(args, 0).unwrap_or_default();
            let sentences: Vec<String> = raw
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    _ => String::new(),
                })
                .collect();
            let start = arg_num(args, 1).unwrap_or(0.0).max(0.0) as usize;
            let max_chars = arg_num(args, 2).unwrap_or(0.0).max(0.0) as usize;
            let (chunk, next) = util_take_sentence_chunk(&sentences, start, max_chars);
            let mut out = HashMap::new();
            out.insert("text".into(), Value::String(chunk));
            // Tip TTS dock reads `.next` (sentence index after this chunk).
            out.insert("next".into(), Value::Number(next as f64));
            Ok(Value::Object(out))
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
/// Each folder has `name`, `path`, `count` (descendant files), `files`, `folders`.
pub fn build_dir_tree(rows: &[Value]) -> Value {
    #[derive(Default)]
    struct Node {
        files: Vec<Value>,
        folders: HashMap<String, Node>,
    }
    fn skip_dir_leaf(name: &str) -> bool {
        if name == "page.md" || name == "page.mdx" {
            return true;
        }
        if name.starts_with('_') {
            return true;
        }
        name.ends_with(".auto.md")
    }
    fn insert(node: &mut Node, parts: &[&str], row: &Value) {
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            if !skip_dir_leaf(parts[0]) {
                node.files.push(row.clone());
            }
            return;
        }
        let child = node.folders.entry(parts[0].to_string()).or_default();
        insert(child, &parts[1..], row);
    }
    fn to_value(name: &str, path: &str, node: Node) -> (Value, usize) {
        let mut folder_list: Vec<(String, Node)> = node.folders.into_iter().collect();
        folder_list.sort_by(|a, b| a.0.cmp(&b.0));
        let mut folders: Vec<Value> = Vec::new();
        let mut count = node.files.len();
        for (n, child) in folder_list {
            let child_path = if path.is_empty() {
                n.clone()
            } else {
                format!("{path}/{n}")
            };
            let (val, child_count) = to_value(&n, &child_path, child);
            count += child_count;
            folders.push(val);
        }
        let mut files = node.files;
        files.sort_by(|a, b| row_path_str(a).cmp(&row_path_str(b)));
        let mut o = HashMap::new();
        o.insert("name".into(), Value::String(name.into()));
        o.insert("path".into(), Value::String(path.into()));
        o.insert("count".into(), Value::Number(count as f64));
        o.insert("files".into(), Value::Array(files));
        o.insert("folders".into(), Value::Array(folders));
        (Value::Object(o), count)
    }
    let mut root = Node::default();
    for row in rows {
        let path = row_path_str(row);
        if path.is_empty() {
            continue;
        }
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        insert(&mut root, &parts, row);
    }
    to_value("", "", root).0
}

/// Ancestor folder paths for Directory expand-to-current (no filename leaf).
/// Strips leading slashes only — does not map `/` to `README.md`.
pub fn dir_expand_paths(rel: &str) -> Value {
    let mut p = rel.trim().to_string();
    while p.starts_with('/') {
        p = p[1..].to_string();
    }
    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Value::Array(vec![]);
    }
    let last = parts[parts.len() - 1];
    let end = if last.contains('.') {
        parts.len() - 1
    } else {
        parts.len()
    };
    let mut out: Vec<Value> = Vec::new();
    let mut acc = String::new();
    for part in parts.iter().take(end) {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(part);
        out.push(Value::String(acc.clone()));
    }
    Value::Array(out)
}

struct TocNestNode {
    id: String,
    title: String,
    level: u32,
    children: Vec<TocNestNode>,
}

fn toc_nest_to_value(node: TocNestNode) -> Value {
    let children: Vec<Value> = node.children.into_iter().map(toc_nest_to_value).collect();
    let mut o = HashMap::new();
    o.insert("id".into(), Value::String(node.id));
    o.insert("title".into(), Value::String(node.title));
    o.insert("level".into(), Value::Number(node.level as f64));
    o.insert("children".into(), Value::Array(children));
    Value::Object(o)
}

/// Nest flat `{id,title,level}` TOC rows into a heading tree (old-site DynamicIndex).
pub fn nest_toc_items(items: &[Value]) -> Value {
    fn close_until(stack: &mut Vec<TocNestNode>, roots: &mut Vec<TocNestNode>, level: u32) {
        while let Some(top) = stack.last() {
            if top.level < level {
                break;
            }
            let finished = stack.pop().unwrap();
            if let Some(parent) = stack.last_mut() {
                parent.children.push(finished);
            } else {
                roots.push(finished);
            }
        }
    }

    let mut roots: Vec<TocNestNode> = Vec::new();
    let mut stack: Vec<TocNestNode> = Vec::new();
    for item in items {
        let (id, title, level) = match item {
            Value::Object(o) => {
                let id = match o.get("id") {
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                };
                let title = match o.get("title") {
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                };
                let mut level = match o.get("level") {
                    Some(Value::Number(n)) => *n as u32,
                    _ => 1,
                };
                if level < 1 {
                    level = 1;
                }
                if level > 6 {
                    level = 6;
                }
                (id, title, level)
            }
            _ => continue,
        };
        if id.is_empty() {
            continue;
        }
        close_until(&mut stack, &mut roots, level);
        stack.push(TocNestNode {
            id,
            title,
            level,
            children: Vec::new(),
        });
    }
    close_until(&mut stack, &mut roots, 0);
    Value::Array(roots.into_iter().map(toc_nest_to_value).collect())
}

fn row_path_str(row: &Value) -> String {
    match row {
        Value::Object(o) => o
            .get("path")
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

pub fn catalog_neighbor_paths(current_rel: &str, rows: &[Value]) -> Value {
    let cur = path_normalize_rel(current_rel);
    let mut idx: i32 = -1;
    for (i, row) in rows.iter().enumerate() {
        if row_path_str(row) == cur {
            idx = i as i32;
            break;
        }
    }
    let mut prev = String::new();
    let mut next = String::new();
    if idx > 0 {
        prev = row_path_str(&rows[(idx - 1) as usize]);
    }
    if idx >= 0 {
        let n = (idx + 1) as usize;
        if n < rows.len() {
            next = row_path_str(&rows[n]);
        }
    }
    let mut o = HashMap::new();
    o.insert("prev".into(), Value::String(prev));
    o.insert("next".into(), Value::String(next));
    Value::Object(o)
}

pub fn catalog_is_feeling_lucky_candidate(path: &str) -> bool {
    if path.len() < 4 {
        return false;
    }
    let lower = path.to_lowercase();
    if !lower.ends_with(".md") {
        return false;
    }
    if lower.contains(".auto.md") {
        return false;
    }
    if path == "README.md" || path == "page.md" {
        return false;
    }
    true
}

pub fn catalog_pick_feeling_lucky_path(rows: &[Value], seed: f64) -> String {
    let mut candidates: Vec<String> = Vec::new();
    for row in rows {
        let p = row_path_str(row);
        if catalog_is_feeling_lucky_candidate(&p) {
            candidates.push(p);
        }
    }
    if candidates.is_empty() {
        return String::new();
    }
    let mut idx = seed.abs() as usize % candidates.len();
    if idx >= candidates.len() {
        idx = 0;
    }
    candidates[idx].clone()
}

/// ATX heading extract for TOC (host/OS-agnostic). Mirrors tip `extractMarkdownHeadings.js`.
pub fn extract_markdown_headings(md: &str) -> Value {
    const MAX_TOC: usize = 80;
    let empty = || {
        let mut o = HashMap::new();
        o.insert("tocItems".into(), Value::Array(vec![]));
        o.insert("idQueue".into(), Value::Array(vec![]));
        Value::Object(o)
    };
    if md.is_empty() {
        return empty();
    }

    fn skip_indent(line: &str) -> usize {
        let mut i = 0usize;
        let b = line.as_bytes();
        while i < b.len() && i < 3 && b[i] == b' ' {
            i += 1;
        }
        i
    }

    fn fence_marker(line: &str, start: usize) -> (char, usize) {
        let b = line.as_bytes();
        if start >= b.len() {
            return ('\0', 0);
        }
        let ch = b[start] as char;
        if ch != '`' && ch != '~' {
            return ('\0', 0);
        }
        let mut count = 0usize;
        let mut i = start;
        while i < b.len() && b[i] as char == ch {
            count += 1;
            i += 1;
        }
        if count < 3 {
            ('\0', 0)
        } else {
            (ch, count)
        }
    }

    fn is_fence_open(line: &str) -> bool {
        let start = skip_indent(line);
        fence_marker(line, start).1 >= 3
    }

    fn closes_fence(line: &str, open_ch: char, open_count: usize) -> bool {
        let start = skip_indent(line);
        let (ch, count) = fence_marker(line, start);
        if ch != open_ch || count < open_count {
            return false;
        }
        let rest = &line[start + count..];
        rest.chars().all(|c| c == ' ' || c == '\t')
    }

    fn strip_trailing_atx(title: &str) -> String {
        let t = title.trim_end_matches([' ', '\t']);
        let bytes = t.as_bytes();
        let mut end = bytes.len();
        while end > 0 && bytes[end - 1] == b'#' {
            end -= 1;
        }
        if end < bytes.len() {
            if end == 0 {
                return String::new();
            }
            let prev = bytes[end - 1];
            if prev == b' ' || prev == b'\t' {
                return t[..end].trim_end_matches([' ', '\t']).to_string();
            }
        }
        t.to_string()
    }

    fn parse_atx(line: &str) -> Option<(u32, String)> {
        let start = skip_indent(line);
        let b = line.as_bytes();
        let mut i = start;
        let mut level = 0u32;
        while i < b.len() && level < 6 && b[i] == b'#' {
            level += 1;
            i += 1;
        }
        if level < 1 || i >= b.len() {
            return None;
        }
        let after = b[i];
        if after != b' ' && after != b'\t' {
            return None;
        }
        while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        let title = strip_trailing_atx(line[i..].trim_start_matches([' ', '\t']));
        let title = title.trim().to_string();
        if title.is_empty() {
            None
        } else {
            Some((level, title))
        }
    }

    fn line_has_toc_ignore(line: &str) -> bool {
        line.contains("data-toc-ignore") || line.contains("no-index")
    }

    fn line_html_close(line: &str) -> bool {
        let t = line.trim_start_matches([' ', '\t']);
        t.starts_with("</")
    }

    fn line_html_open_ignore(line: &str) -> bool {
        let t = line.trim_start_matches([' ', '\t']);
        if t.len() < 2 || !t.starts_with('<') || t.starts_with("</") {
            return false;
        }
        line_has_toc_ignore(line)
    }

    fn heading_value(id: &str, title: &str, level: u32) -> Value {
        let mut o = HashMap::new();
        o.insert("id".into(), Value::String(id.into()));
        o.insert("title".into(), Value::String(title.into()));
        o.insert("level".into(), Value::Number(level as f64));
        Value::Object(o)
    }

    let mut used: Vec<Value> = Vec::new();
    let mut all: Vec<(String, String, u32, bool)> = Vec::new();
    let mut in_fence = false;
    let mut fence_ch = '\0';
    let mut fence_count = 0usize;
    let mut ignore_depth = 0i32;

    for line in md.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if in_fence {
            if closes_fence(line, fence_ch, fence_count) {
                in_fence = false;
                fence_ch = '\0';
                fence_count = 0;
            }
            continue;
        }
        if is_fence_open(line) {
            let start = skip_indent(line);
            let (ch, count) = fence_marker(line, start);
            in_fence = true;
            fence_ch = ch;
            fence_count = count;
            continue;
        }

        let mut skip_toc_region = false;
        if ignore_depth > 0 {
            skip_toc_region = true;
            if line_html_close(line) {
                ignore_depth -= 1;
                if ignore_depth < 0 {
                    ignore_depth = 0;
                }
            }
        } else if line_html_open_ignore(line) {
            ignore_depth += 1;
            skip_toc_region = true;
        }

        if let Some((level, title)) = parse_atx(line) {
            let base = util_slugify(&title);
            let id = util_unique_id(&base, &used);
            used.push(Value::String(id.clone()));
            let skip_toc = skip_toc_region || line_has_toc_ignore(line);
            all.push((id, title, level, skip_toc));
        }
    }

    let mut start_idx = 0usize;
    if all.len() > 1 && all[0].2 == 1 {
        start_idx = 1;
    }
    let mut toc_items = Vec::new();
    let mut n = 0usize;
    for entry in all.iter().skip(start_idx) {
        if n >= MAX_TOC {
            break;
        }
        if entry.3 {
            continue;
        }
        toc_items.push(heading_value(&entry.0, &entry.1, entry.2));
        n += 1;
    }
    let id_queue: Vec<Value> = all
        .iter()
        .map(|e| heading_value(&e.0, &e.1, e.2))
        .collect();

    let mut out = HashMap::new();
    out.insert("tocItems".into(), Value::Array(toc_items));
    out.insert("idQueue".into(), Value::Array(id_queue));
    Value::Object(out)
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
        "extractMarkdownHeadings".into(),
        Value::Native(|args| {
            Ok(extract_markdown_headings(
                &arg_str(args, 0).unwrap_or_default(),
            ))
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
        "dirExpandPaths".into(),
        Value::Native(|args| {
            Ok(dir_expand_paths(&arg_str(args, 0).unwrap_or_default()))
        }),
    );
    m.insert(
        "nestTocItems".into(),
        Value::Native(|args| {
            let items = arg_arr(args, 0).unwrap_or_default();
            Ok(nest_toc_items(&items))
        }),
    );
    m.insert(
        "neighborPaths".into(),
        Value::Native(|args| {
            let cur = arg_str(args, 0).unwrap_or_default();
            let rows = arg_arr(args, 1).unwrap_or_default();
            Ok(catalog_neighbor_paths(&cur, &rows))
        }),
    );
    m.insert(
        "isFeelingLuckyCandidate".into(),
        Value::Native(|args| {
            Ok(Value::Bool(catalog_is_feeling_lucky_candidate(
                &arg_str(args, 0).unwrap_or_default(),
            )))
        }),
    );
    m.insert(
        "pickFeelingLuckyPath".into(),
        Value::Native(|args| {
            let rows = arg_arr(args, 0).unwrap_or_default();
            let seed = arg_num(args, 1).unwrap_or(0.0);
            Ok(Value::String(catalog_pick_feeling_lucky_path(&rows, seed)))
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

// --- tts ---

pub const AUDIO_PLAYER_TAG: &str = "relay-audio-player";

fn tts_module() -> Value {
    let mut m = HashMap::new();
    m.insert(
        "AudioPlayer".into(),
        Value::Native(|args| {
            let props = arg_obj(args, 0).unwrap_or_default();
            let src = match props.get("src") {
                Some(Value::String(s)) if !s.is_empty() => s.clone(),
                _ => return Err("AudioPlayer: src is required".into()),
            };
            let mut out = HashMap::new();
            out.insert("src".into(), Value::String(src));
            if let Some(Value::Bool(b)) = props.get("playing") {
                out.insert("playing".into(), Value::Bool(*b));
            }
            if let Some(Value::Bool(b)) = props.get("controls") {
                out.insert("controls".into(), Value::Bool(*b));
            }
            if let Some(Value::Number(n)) = props.get("playbackRate") {
                out.insert("playbackRate".into(), Value::Number(*n));
            }
            if let Some(Value::Number(n)) = props.get("command") {
                out.insert("command".into(), Value::Number(*n));
            }
            if let Some(Value::Number(n)) = props.get("commandSeq") {
                out.insert("commandSeq".into(), Value::Number(*n));
            }
            if let Some(Value::Number(n)) = props.get("commandArg") {
                out.insert("commandArg".into(), Value::Number(*n));
            }
            if let Some(Value::String(s)) = props.get("className") {
                out.insert("className".into(), Value::String(s.clone()));
            }
            if let Some(Value::String(s)) = props.get("playerId") {
                out.insert("playerId".into(), Value::String(s.clone()));
            }
            for key in ["onPlay", "onPause", "onEnded", "onError"] {
                if let Some(v) = props.get(key) {
                    match v {
                        Value::Function { .. }
                        | Value::Native(_)
                        | Value::NativeFn(_)
                        | Value::Setter(_) => {
                            out.insert(key.into(), v.clone());
                        }
                        _ => {}
                    }
                }
            }
            Ok(el(AUDIO_PLAYER_TAG, out, vec![]))
        }),
    );
    m.insert(
        "synthesize".into(),
        Value::Native(|args| {
            let opts = arg_obj(args, 0).unwrap_or_default();
            let text = match opts.get("text") {
                Some(Value::String(s)) => s.trim().to_string(),
                _ => String::new(),
            };
            if text.is_empty() {
                return Err("synthesize: text is required".into());
            }
            if text.len() > 8000 {
                return Err("synthesize: text exceeds 8000 chars".into());
            }
            let mut bases: Vec<String> = Vec::new();
            if let Some(Value::String(s)) = opts.get("baseUrl") {
                if !s.is_empty() {
                    bases.push(s.trim_end_matches('/').to_string());
                }
            }
            if let Some(Value::Array(arr)) = opts.get("endpoints") {
                for v in arr {
                    if let Value::String(s) = v {
                        let t = s.trim_end_matches('/');
                        if !t.is_empty() {
                            bases.push(t.to_string());
                        }
                    }
                }
            }
            if bases.is_empty() {
                bases.push("https://iad1.relaygateway.net".into());
                bases.push("https://sjc1.relaygateway.net".into());
                bases.push("https://hkg1.relaygateway.net".into());
            }
            let voice_id = match opts.get("voiceId") {
                Some(Value::String(s)) if !s.is_empty() => s.clone(),
                _ => "en_US-lessac-medium".to_string(),
            };
            let host = crate::run_state::get_host_for_lazy()
                .ok_or_else(|| "synthesize: no host".to_string())?;
            let mut body_obj = serde_json::Map::new();
            body_obj.insert("text".into(), serde_json::Value::String(text));
            body_obj.insert("voiceId".into(), serde_json::Value::String(voice_id));
            let body = serde_json::Value::Object(body_obj).to_string();
            let mut last_err = String::from("synthesize: all endpoints failed");
            for base in bases {
                let url = format!("{base}/tts");
                let mut headers = HashMap::new();
                headers.insert(
                    "Content-Type".into(),
                    Value::String("application/json".into()),
                );
                let mut fo = HashMap::new();
                fo.insert("method".into(), Value::String("POST".into()));
                fo.insert("body".into(), Value::String(body.clone()));
                fo.insert("responseType".into(), Value::String("base64".into()));
                fo.insert("headers".into(), Value::Object(headers));
                let resp = match host.borrow().fetch(&url, Some(&Value::Object(fo))) {
                    Ok(v) => v,
                    Err(e) => {
                        last_err = e;
                        continue;
                    }
                };
                let Value::Object(map) = resp else {
                    last_err = "synthesize: bad response".into();
                    continue;
                };
                let ok = matches!(map.get("ok"), Some(Value::Bool(true)));
                let status = match map.get("status") {
                    Some(Value::Number(n)) => *n,
                    _ => 0.0,
                };
                if !ok || status < 200.0 || status >= 300.0 {
                    last_err = format!("synthesize: HTTP {status} from {base}");
                    continue;
                }
                let b64 = match map.get("base64") {
                    Some(Value::String(s)) if !s.is_empty() => s.clone(),
                    _ => {
                        last_err = "synthesize: empty audio".into();
                        continue;
                    }
                };
                let ctype = match map.get("contentType") {
                    Some(Value::String(s)) if !s.is_empty() => s.clone(),
                    _ => "audio/wav".to_string(),
                };
                let data_url = format!("data:{ctype};base64,{b64}");
                return Ok(Value::String(data_url));
            }
            Err(last_err)
        }),
    );
    named_exports(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn path_and_nav_parse() {
        assert_eq!(path_basename("a/b/c.md"), "c.md");
        let rows = parse_nav_index_txt("blog/x.md\tHello\t2026-01-01T00:00:00Z\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(content_kind("blog/x.md"), "blog");
        assert!(!format_tiny("2026-07-18T12:00:00Z").is_empty());
    }

    #[test]
    fn dir_tree_count_path_and_expand() {
        let rows = parse_nav_index_txt(
            "history/a.md\tA\nhistory/chronology/b.md\tB\nhistory/page.md\tHub\ninfluence/c.md\tC\n",
        );
        let tree = build_dir_tree(&rows);
        let obj = match tree {
            Value::Object(o) => o,
            _ => panic!("tree"),
        };
        assert_eq!(obj.get("count"), Some(&Value::Number(3.0)));
        let folders = match obj.get("folders") {
            Some(Value::Array(a)) => a,
            _ => panic!("folders"),
        };
        assert_eq!(folders.len(), 2);
        let hist = match &folders[0] {
            Value::Object(o) => o,
            _ => panic!("history"),
        };
        assert_eq!(hist.get("name"), Some(&Value::String("history".into())));
        assert_eq!(hist.get("path"), Some(&Value::String("history".into())));
        assert_eq!(hist.get("count"), Some(&Value::Number(2.0)));
        let hist_files = match hist.get("files") {
            Some(Value::Array(a)) => a,
            _ => panic!("history files"),
        };
        assert_eq!(hist_files.len(), 1);
        assert_eq!(
            dir_expand_paths("/history/chronology/b.md"),
            dir_expand_paths("history/chronology/b.md")
        );
        assert_eq!(dir_expand_paths("/"), Value::Array(vec![]));
        let expand = dir_expand_paths("history/chronology/b.md");
        assert_eq!(
            expand,
            Value::Array(vec![
                Value::String("history".into()),
                Value::String("history/chronology".into()),
            ])
        );
        assert_eq!(dir_expand_paths("page.md"), Value::Array(vec![]));
    }

    #[test]
    fn nest_toc_items_builds_heading_tree() {
        let items = vec![
            heading_value_for_test("welcome", "Welcome", 1),
            heading_value_for_test("proj", "Project", 2),
            heading_value_for_test("ta", "TA", 3),
            heading_value_for_test("cats", "Categories", 2),
        ];
        let tree = nest_toc_items(&items);
        let roots = match tree {
            Value::Array(a) => a,
            _ => panic!("roots"),
        };
        assert_eq!(roots.len(), 1);
        let welcome = match &roots[0] {
            Value::Object(o) => o,
            _ => panic!("welcome"),
        };
        let kids = match welcome.get("children") {
            Some(Value::Array(a)) => a,
            _ => panic!("children"),
        };
        assert_eq!(kids.len(), 2);
        let proj = match &kids[0] {
            Value::Object(o) => o,
            _ => panic!("proj"),
        };
        let nested = match proj.get("children") {
            Some(Value::Array(a)) => a,
            _ => panic!("nested"),
        };
        assert_eq!(nested.len(), 1);
    }

    fn heading_value_for_test(id: &str, title: &str, level: u32) -> Value {
        let mut o = HashMap::new();
        o.insert("id".into(), Value::String(id.into()));
        o.insert("title".into(), Value::String(title.into()));
        o.insert("level".into(), Value::Number(level as f64));
        Value::Object(o)
    }

    #[test]
    fn extract_headings_skips_fence_and_first_h1() {
        let md = "# Title\n\n## Section One\n\n```\n# not\n```\n\n### Nested\n\n<div data-toc-ignore>\n\n## Hidden\n\n</div>\n\n## After\n";
        let v = extract_markdown_headings(md);
        let obj = match v {
            Value::Object(o) => o,
            _ => panic!("expected object"),
        };
        let toc = match obj.get("tocItems") {
            Some(Value::Array(a)) => a,
            _ => panic!("tocItems"),
        };
        let queue = match obj.get("idQueue") {
            Some(Value::Array(a)) => a,
            _ => panic!("idQueue"),
        };
        let toc_ids: Vec<String> = toc
            .iter()
            .filter_map(|h| match h {
                Value::Object(o) => o.get("id").and_then(|x| match x {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                }),
                _ => None,
            })
            .collect();
        let queue_ids: Vec<String> = queue
            .iter()
            .filter_map(|h| match h {
                Value::Object(o) => o.get("id").and_then(|x| match x {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                }),
                _ => None,
            })
            .collect();
        assert_eq!(
            toc_ids,
            vec![
                "section_one".to_string(),
                "nested".to_string(),
                "after".to_string()
            ]
        );
        assert_eq!(
            queue_ids,
            vec![
                "title".to_string(),
                "section_one".to_string(),
                "nested".to_string(),
                "hidden".to_string(),
                "after".to_string()
            ]
        );
    }

    #[test]
    fn rewrite_markdown_preserves_utf8_around_links() {
        let md = "See [note — detail](./pic.png) and café src=\"x.jpg\" done";
        let out = path_rewrite_markdown_asset_urls(md, "blog/post.md", "https://mirror.example");
        assert!(out.contains('—'), "em dash must survive rewrite: {out}");
        assert!(out.contains("café"), "utf8 text must survive: {out}");
        assert!(out.contains("https://mirror.example/blog/pic.png"), "{out}");
        assert!(same_day("2026-08-02T12:00:00Z", "2026-08-02T99:99:99Z"));
        assert!(!same_day("短", "短abcdefghij")); // short / non-ascii prefix must not panic
    }

    #[test]
    fn gallery_path_and_portrait_index_helpers() {
        assert_eq!(
            path_repo_rel_from_media_src("https://ex.ample/governance/war/a.png?w=1"),
            "governance/war/a.png"
        );
        assert_eq!(
            path_repo_rel_from_media_src("/governance/war/a.png"),
            "governance/war/a.png"
        );
        assert_eq!(path_safe_img_dom_id(""), "img-unknown");
        assert_eq!(
            path_safe_img_dom_id("/a/b.png"),
            "img--a-b-png"
        );
        assert_eq!(path_read_hash_img_src("#img=/x.png"), "/x.png");
        assert_eq!(path_read_hash_img_src("img=/x.png"), "/x.png");
        assert_eq!(path_gallery_img_hash("/x.png"), "img=/x.png");
        assert_eq!(
            path_gallery_display_src("/lo.png", "/hi.png"),
            "/hi.png"
        );
        assert_eq!(path_gallery_display_src("/lo.png", ""), "/lo.png");

        let mut map = HashMap::new();
        map.insert(
            "governance/war/a-portrait.png".into(),
            Value::Bool(true),
        );
        let present = path_portrait_sibling_if_present(
            "/governance/war/a.png",
            &Value::Object(map),
        );
        assert_eq!(present, "/governance/war/a-portrait.png");
        assert_eq!(
            path_portrait_sibling_if_present("/governance/war/a.png", &Value::Null),
            ""
        );
        assert_eq!(
            path_portrait_sibling_if_present("/governance/war/a-portrait.png", &Value::Object(HashMap::new())),
            ""
        );
    }

    #[test]
    fn util_sentence_chunking_matches_tip_tts() {
        let parts = util_split_sentences("One. Two! Three? Tail");
        assert_eq!(parts, vec!["One.", "Two!", "Three?", "Tail"]);
        let (chunk, next) = util_take_sentence_chunk(&parts, 0, 12);
        assert_eq!(chunk, "One. Two!");
        assert_eq!(next, 2);
        let (chunk2, next2) = util_take_sentence_chunk(&parts, next, 700);
        assert_eq!(chunk2, "Three? Tail");
        assert_eq!(next2, 4);
    }

    #[test]
    fn normalize_http_href_matches_node_checker() {
        assert_eq!(
            path_normalize_http_href("https://Example.COM/Path#frag"),
            "https://example.com/Path"
        );
        assert_eq!(
            path_normalize_http_href("https://example.com/foo/"),
            "https://example.com/foo"
        );
        assert_eq!(
            path_normalize_http_href("https://example.com"),
            "https://example.com/"
        );
        assert_eq!(
            path_normalize_http_href("https://example.com/"),
            "https://example.com/"
        );
        assert_eq!(
            path_normalize_http_href("https://example.com:443/a?x=1"),
            "https://example.com/a?x=1"
        );
        assert_eq!(
            path_normalize_http_href("http://example.com:80/a"),
            "http://example.com/a"
        );
        assert_eq!(
            path_normalize_http_href("http://www.example.com/a"),
            "http://www.example.com/a"
        );
        assert_eq!(path_normalize_http_href("/foo"), "/foo");
        assert_eq!(path_normalize_http_href("mailto:a@b.c"), "mailto:a@b.c");
        assert_eq!(path_normalize_http_href("https://"), "https://");
        assert_eq!(
            path_normalize_http_href("https://example.com/?q=1"),
            "https://example.com/?q=1"
        );
        assert_eq!(
            path_normalize_http_href("http://example.com:8080/a"),
            "http://example.com:8080/a"
        );
    }

    #[test]
    fn lookup_http_href_exact_then_normalize_and_miss() {
        let mut inner = HashMap::new();
        inner.insert("reason".into(), Value::String("http_404".into()));
        inner.insert("status".into(), Value::Number(404.0));
        let mut map = HashMap::new();
        map.insert(
            "https://example.com/gone".into(),
            Value::Object(inner),
        );
        let map_v = Value::Object(map);

        let hit = path_lookup_http_href(&map_v, "https://Example.COM/gone#x");
        match hit {
            Value::Object(o) => match o.get("reason") {
                Some(Value::String(s)) => assert_eq!(s, "http_404"),
                other => panic!("reason {other:?}"),
            },
            other => panic!("{other:?}"),
        }

        let exact = path_lookup_http_href(&map_v, "https://example.com/gone");
        assert!(matches!(exact, Value::Object(_)));

        assert!(matches!(
            path_lookup_http_href(&Value::Null, "https://example.com/gone"),
            Value::Null
        ));
        assert!(matches!(
            path_lookup_http_href(&map_v, "https://example.com/live"),
            Value::Null
        ));
        assert!(matches!(
            path_lookup_http_href(&Value::Array(vec![]), "https://example.com/gone"),
            Value::Null
        ));
    }
}
