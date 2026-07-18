//! CleverScript string literal + replaceAll hardening (park tip lessons).
//!
//! Lone `'"'` / `"'" ` forms must round-trip without corrupting nearby text
//! (SSR previously showed `ban`→`ba)` when tip hooks used those literals).

use relay_runtime::host::MockHost;
use relay_runtime::parse;
use relay_runtime::run_query_script;
use relay_runtime::value::Value;

fn run(src: &str) -> Value {
    run_query_script(src, MockHost::new(), &serde_json::json!({}), None, None).expect("run")
}

#[test]
fn lone_double_quote_string_literal_roundtrips() {
    let v = run(
        r#"
export default function run(ctx) {
  const dq = '"'
  const sq = "'"
  return dq + sq + dq.length + sq.length
}
"#,
    );
    assert_eq!(v, Value::String("\"'11".to_string()));
}

#[test]
fn replace_all_double_quote_does_not_corrupt_ban() {
    // Mirrors thin-park softenImageAltQuotes / replaceAllLiteral pattern.
    let v = run(
        r#"
function replaceAllLiteral(text, needle, replacement) {
  if (!text || !needle || needle.length === 0) return text
  let out = ''
  let pos = 0
  const fl = needle.length
  while (pos < text.length) {
    const idx = text.indexOf(needle, pos)
    if (idx < 0) {
      out = out + text.slice(pos)
      break
    }
    out = out + text.slice(pos, idx)
    out = out + replacement
    pos = idx + fl
  }
  return out
}
export default function run(ctx) {
  const sample = 'they can never ban us. sort=name-asc automatic directory /influence/conspiracy/'
  const dq = '"'
  const sq = "'"
  let s = replaceAllLiteral(sample, dq, sq)
  return s
}
"#,
    );
    let Value::String(s) = v else {
        panic!("expected string: {v:?}");
    };
    assert!(s.contains("never ban us"), "corrupted ban: {s}");
    assert!(!s.contains("ba)"), "n→) corruption: {s}");
    assert!(s.contains("name-asc"), "corrupted asc: {s}");
    assert!(!s.contains("name-as)"), "c→) corruption: {s}");
    assert!(s.contains("/influence/conspiracy/"), "path mangled: {s}");
    assert!(!s.contains("/ce/conspiracy"), "influen stripped: {s}");
    assert!(s.contains("automatic directory"), "space→) corruption: {s}");
}

#[test]
fn large_markdown_replace_all_preserves_body() {
    let v = run(
        r#"
function replaceAllLiteral(text, needle, replacement) {
  if (!text || !needle || needle.length === 0) return text
  let out = ''
  let pos = 0
  const fl = needle.length
  while (pos < text.length) {
    const idx = text.indexOf(needle, pos)
    if (idx < 0) {
      out = out + text.slice(pos)
      break
    }
    out = out + text.slice(pos, idx)
    out = out + replacement
    pos = idx + fl
  }
  return out
}
export default function run(ctx) {
  let chunk = 'never ban us. '
  let md = ''
  let i = 0
  while (i < 2000) {
    md = md + chunk
    i = i + 1
  }
  md = md + '<ChangeLog />'
  md = replaceAllLiteral(md, '<ChangeLog />', '')
  const ok = md.indexOf('never ban us') >= 0 && md.indexOf('ba)') < 0 && md.indexOf('ChangeLog') < 0
  return ok && md.length > 20000
}
"#,
    );
    assert_eq!(v, Value::Bool(true));
}

#[test]
fn reserved_from_binding_has_clear_error() {
    let err = {
        let src = r#"
export default function run(ctx) {
  const from = '](/x/'
  return from
}
"#;
        match parse(src) {
            Ok(_) => panic!("expected parse error for `const from`"),
            Err(e) => format!("{e}"),
        }
    };
    assert!(
        err.contains("reserved keyword") && err.contains("from"),
        "expected reserved-keyword diagnostic, got: {err}"
    );
}

#[test]
fn while_without_braces_has_clear_error() {
    let err = {
        let src = r#"
export default function run(ctx) {
  let t = '/x'
  while (t.length > 0 && t.charAt(0) === '/') t = t.slice(1)
  return t
}
"#;
        match parse(src) {
            Ok(_) => panic!("expected parse error for unbraced while"),
            Err(e) => format!("{e}"),
        }
    };
    assert!(
        err.contains("braces") || err.contains("`{`"),
        "expected braces diagnostic, got: {err}"
    );
}

#[test]
fn while_with_braces_ok() {
    let v = run(
        r#"
export default function run(ctx) {
  let t = '/x/y'
  while (t.length > 0 && t.charAt(0) === '/') {
    t = t.slice(1)
  }
  return t
}
"#,
    );
    assert_eq!(v, Value::String("x/y".to_string()));
}
