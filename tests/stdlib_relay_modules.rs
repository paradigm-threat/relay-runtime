//! ES6 builtins + @relay/* smoke tests for CleverScript host helpers.

use relay_runtime::host::{host_fetch_object, Host, HostHandle, MockHost};
use relay_runtime::run_state::{
    clear_tip_get_cache, inject_standard_builtins, set_host_for_lazy, tip_cached_get,
};
use relay_runtime::value::Value;
use relay_runtime::{analyze, parse, Runtime};
use std::cell::RefCell;
use std::rc::Rc;

fn eval_expr(src: &str) -> Value {
    let wrapped = format!("export default function App() {{ return ({src}); }}");
    let program = parse(&wrapped).expect("parse");
    let analysis = analyze(&program).expect("analyze");
    let mut rt = Runtime::new(program, analysis).with_host(MockHost::new());
    inject_standard_builtins(&rt);
    let exports = rt.get_module_exports().expect("exports");
    let f = match exports {
        Value::Object(map) => map.get("default").cloned().expect("default"),
        _ => panic!("expected object exports"),
    };
    rt.call_value(&f, &[]).expect("call")
}

#[test]
fn parse_int_math_number_sort_replace_all() {
    assert_eq!(eval_expr(r#"parseInt("42", 10)"#), Value::Number(42.0));
    assert_eq!(eval_expr(r#"Math.max(1, 9, 3)"#), Value::Number(9.0));
    assert_eq!(eval_expr(r#"Number.from("3.5")"#), Value::Number(3.5));
    assert_eq!(
        eval_expr(r#"[3,1,2].sort((a,b) => a - b)"#),
        Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0)
        ])
    );
    assert_eq!(
        eval_expr(r#""aa-bb-aa".replaceAll("aa", "x")"#),
        Value::String("x-bb-x".into())
    );
    assert!(matches!(eval_expr("undefined"), Value::Null));
}

#[test]
fn date_from_and_iso() {
    let v = eval_expr(r#"Date.from("2020-01-02T03:04:05Z").toISOString()"#);
    match v {
        Value::String(s) => assert!(s.starts_with("2020-01-02T03:04:05"), "{s}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn relay_path_util_modules() {
    let src = r#"
import { basename, join, normalizeRel, resolveRel, parentDir, siteHref, resolveAssetUrl, rewriteMarkdownAssetUrls } from '@relay/path';
import { take, sortBy, slugify, uniqueId, includes } from '@relay/util';
import { formatTiny } from '@relay/date';
import { neighborPaths, isFeelingLuckyCandidate, pickFeelingLuckyPath } from '@relay/catalog';
export default function App() {
  const rows = [{ path: 'b.md', _updated: '2020-01-01' }, { path: 'a.md', _updated: '2021-01-01' }];
  const sorted = sortBy(rows, '_updated', true);
  const used = ['intro'];
  const uid = uniqueId('intro', used);
  const nb = neighborPaths('b.md', [{ path: 'a.md' }, { path: 'b.md' }, { path: 'c.md' }]);
  const lucky = pickFeelingLuckyPath([{ path: 'page.md' }, { path: 'blog/x.md' }], 1);
  const md = rewriteMarkdownAssetUrls('![](./pic.png)', 'dir/a.md', 'https://mirror.example');
  return basename(join(['docs', 'x.md']))
    + '|' + take(sorted, 1)[0].path
    + '|' + formatTiny('2026-07-18T00:00:00Z')
    + '|' + slugify('Hello World')
    + '|' + uid
    + '|' + (includes(used, 'intro') ? 'y' : 'n')
    + '|' + normalizeRel('/foo/bar.md')
    + '|' + resolveRel('../z.md', 'dir/a.md', '')
    + '|' + parentDir('dir/a.md')
    + '|' + siteHref('https://park.example', '/x.md')
    + '|' + resolveAssetUrl('./pic.png', 'dir/a.md', 'https://mirror.example')
    + '|' + nb.prev + '>' + nb.next
    + '|' + (isFeelingLuckyCandidate('blog/x.md') ? 'L' : 'n')
    + '|' + lucky
    + '|' + md;
}
"#;
    let program = parse(src).expect("parse");
    let analysis = analyze(&program).expect("analyze");
    struct RelayHost;
    impl Host for RelayHost {
        fn fetch(&self, url: &str, _: Option<&Value>) -> Result<Value, String> {
            Ok(host_fetch_object(true, 200.0, format!("body:{url}"), None))
        }
        fn navigate(&self, _: &str) -> Result<(), String> {
            Ok(())
        }
        fn create_element(
            &self,
            tag: &str,
            props: &Value,
            children: &[Value],
        ) -> Result<Value, String> {
            Ok(Value::Object(
                [
                    ("type".into(), Value::String("element".into())),
                    ("tag".into(), Value::String(tag.into())),
                    ("props".into(), props.clone()),
                    ("children".into(), Value::Array(children.to_vec())),
                ]
                .into_iter()
                .collect(),
            ))
        }
        fn load_module(&self, path: &str) -> Result<Value, String> {
            relay_runtime::relay_modules::try_load_relay_module(path)
                .ok_or_else(|| format!("missing {path}"))
        }
        fn log(&self, _: &str, _: &str) {}
    }
    let host: HostHandle = Rc::new(RefCell::new(RelayHost));
    let mut rt = Runtime::new(program, analysis).with_host(host);
    inject_standard_builtins(&rt);
    let exports = rt.get_module_exports().expect("exports");
    let f = match exports {
        Value::Object(o) => o.get("default").cloned().unwrap(),
        _ => panic!(),
    };
    let out = rt.call_value(&f, &[]).expect("call");
    match out {
        Value::String(s) => {
            assert!(s.starts_with("x.md|a.md|"), "{s}");
            assert!(s.contains("Jul"), "{s}");
            assert!(s.contains("|hello_world|"), "{s}");
            assert!(s.contains("|intro_1|"), "{s}");
            assert!(s.contains("|y|"), "{s}");
            assert!(s.contains("|foo/bar.md|"), "{s}");
            assert!(s.contains("|z.md|") || s.contains("|dir/z.md|"), "{s}");
            assert!(s.contains("|dir|"), "{s}");
            assert!(s.contains("https://park.example/x.md"), "{s}");
            assert!(s.contains("https://mirror.example/dir/pic.png"), "{s}");
            assert!(s.contains("|a.md>c.md|"), "{s}");
            assert!(s.contains("|L|"), "{s}");
            assert!(s.contains("|blog/x.md|"), "{s}");
            assert!(s.contains("![](https://mirror.example/dir/pic.png)"), "{s}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn tip_cached_get_dedupes_fetch() {
    clear_tip_get_cache();
    let host = MockHost::new();
    set_host_for_lazy(Some(host.clone()));
    let a = tip_cached_get("/nav-index.txt").unwrap();
    let b = tip_cached_get("/nav-index.txt").unwrap();
    assert_eq!(a, b);
    let calls = host.borrow().fetch_calls.borrow().len();
    assert_eq!(calls, 1, "expected single fetch, got {calls}");
    set_host_for_lazy(None);
    clear_tip_get_cache();
}

#[test]
fn tip_cached_nav_index_rows_warms_parse() {
    use relay_runtime::tip_cached_nav_index_rows;
    clear_tip_get_cache();
    let text = "blog/a.md\tHello\t2026-01-01T00:00:00Z\nblog/b.md\tWorld\t2026-01-02T00:00:00Z\n";
    let a = tip_cached_nav_index_rows(text);
    let b = tip_cached_nav_index_rows(text);
    assert!(std::rc::Rc::ptr_eq(&a, &b), "expected same warmed Rc");
    assert_eq!(a.len(), 2);
    clear_tip_get_cache();
    let c = tip_cached_nav_index_rows(text);
    assert!(!std::rc::Rc::ptr_eq(&a, &c), "cache clear should re-parse");
    assert_eq!(c.len(), 2);
}
