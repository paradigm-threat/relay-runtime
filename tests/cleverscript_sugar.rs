//! Quick-win CleverScript sugar: fragments, destructuring, export function, rest/defaults,
//! compound assign, %, prefix ++/--, spreads, optional call, switch, for-of, break/continue.

use relay_runtime::host::MockHost;
use relay_runtime::parse;
use relay_runtime::run_query_script;
use relay_runtime::value::Value;

fn run(src: &str) -> Value {
    run_query_script(src, MockHost::new(), &serde_json::json!({}), None, None).expect("run")
}

#[test]
fn object_destructuring_binds_keys() {
    let v = run(
        r#"
export default function run(ctx) {
  const obj = { a: 1, b: 2 }
  const { a, b: c } = obj
  return a + c
}
"#,
    );
    assert_eq!(v, Value::Number(3.0));
}

#[test]
fn object_destructuring_missing_is_null() {
    let v = run(
        r#"
export default function run(ctx) {
  const { missing } = { a: 1 }
  return missing === null
}
"#,
    );
    assert_eq!(v, Value::Bool(true));
}

#[test]
fn export_function_callable_from_default() {
    let v = run(
        r#"
export function add(a, b) {
  return a + b
}
export default function run(ctx) {
  return add(2, 3)
}
"#,
    );
    assert_eq!(v, Value::Number(5.0));
}

#[test]
fn rest_params_collect_remaining_args() {
    let v = run(
        r#"
export default function run(ctx) {
  const f = (a, ...rest) => rest.length + a
  return f(1, 2, 3, 4)
}
"#,
    );
    assert_eq!(v, Value::Number(4.0));
}

#[test]
fn default_params_nullish() {
    let v = run(
        r#"
export default function run(ctx) {
  const f = (a = 7) => a
  return f() + f(null) + f(1)
}
"#,
    );
    assert_eq!(v, Value::Number(15.0));
}

#[test]
fn compound_assign_and_modulo() {
    let v = run(
        r#"
export default function run(ctx) {
  let n = 10
  n += 5
  n -= 2
  n *= 2
  n /= 4
  return n % 3
}
"#,
    );
    assert_eq!(v, Value::Number(0.5)); // 6.5 % 3 = 0.5
}

#[test]
fn prefix_increment_returns_new_value() {
    let v = run(
        r#"
export default function run(ctx) {
  let n = 1
  const a = ++n
  const b = n++
  return a + b + n
}
"#,
    );
    // a=2, b=2 (postfix returns old), n=3 → 7
    assert_eq!(v, Value::Number(7.0));
}

#[test]
fn array_and_object_spread() {
    let v = run(
        r#"
export default function run(ctx) {
  const a = [1, 2]
  const b = [...a, 3]
  const o = { x: 1 }
  const p = { ...o, y: 2, x: 9 }
  return b.length + p.x + p.y
}
"#,
    );
    assert_eq!(v, Value::Number(14.0)); // 3 + 9 + 2
}

#[test]
fn optional_call_null_callee() {
    let v = run(
        r#"
export default function run(ctx) {
  const f = null
  return f?.(1) === null
}
"#,
    );
    assert_eq!(v, Value::Bool(true));
}

#[test]
fn switch_exclusive_cases() {
    let v = run(
        r#"
export default function run(ctx) {
  let n = 0
  switch (2) {
    case 1:
      n = 10
      break
    case 2:
      n = 20
      break
    default:
      n = 30
  }
  return n
}
"#,
    );
    assert_eq!(v, Value::Number(20.0));
}

#[test]
fn for_of_arrays_and_break_continue() {
    let v = run(
        r#"
export default function run(ctx) {
  let sum = 0
  for (const x of [1, 2, 3, 4, 5]) {
    if (x === 2) {
      continue
    }
    if (x === 4) {
      break
    }
    sum = sum + x
  }
  return sum
}
"#,
    );
    assert_eq!(v, Value::Number(4.0)); // 1 + 3
}

#[test]
fn jsx_fragment_parses() {
    let src = r#"
export default function App() {
  return <>
    <span>a</span>
    <span>b</span>
  </>
}
"#;
    parse(src).expect("fragment parses");
}
