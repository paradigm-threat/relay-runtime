//! try/catch statement execution in CleverScript runtime.

use relay_runtime::run_state::{clear_run_state, run_hook_persistent};
use relay_runtime::value::Value;
use relay_runtime::vdom::value_to_vnode;
use relay_runtime::HostHandle;
use std::cell::RefCell;
use std::rc::Rc;

struct NullHost;

impl relay_runtime::Host for NullHost {
    fn fetch(&self, _url: &str, _options: Option<&Value>) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn navigate(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }
    fn create_element(
        &self,
        _tag: &str,
        _props: &Value,
        _children: &[Value],
    ) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn load_module(&self, _path: &str) -> Result<Value, String> {
        Err("no modules".into())
    }
}

fn null_host() -> HostHandle {
    Rc::new(RefCell::new(NullHost))
}

#[test]
fn try_block_runs_and_catch_swallows_errors() {
    clear_run_state();
    let src = r#"
export default function T() {
  let n = 0
  try {
    n = 1
  } catch (e) {
    n = 2
  }
  return <span>{n}</span>
}
"#;
    let result = run_hook_persistent(src, null_host()).expect("try runs");
    let text = value_to_vnode(&result).normalize().text_content();
    assert_eq!(text, "1");
}

#[test]
fn catch_runs_on_runtime_error() {
    clear_run_state();
    let src = r#"
export default function T() {
  let n = 0
  try {
    const bad = 1
    bad()
    n = 1
  } catch (e) {
    n = 2
  }
  return <span>{n}</span>
}
"#;
    let result = run_hook_persistent(src, null_host()).expect("catch runs");
    let text = value_to_vnode(&result).normalize().text_content();
    assert_eq!(text, "2");
}

#[test]
fn finally_runs_after_try() {
    clear_run_state();
    let src = r#"
export default function T() {
  let n = 0
  try {
    n = 1
  } finally {
    n = n + 10
  }
  return <span>{n}</span>
}
"#;
    let result = run_hook_persistent(src, null_host()).expect("finally runs");
    let text = value_to_vnode(&result).normalize().text_content();
    assert_eq!(text, "11");
}
