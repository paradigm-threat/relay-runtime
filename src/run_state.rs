//! RunState persistence for re-renders.
//!
//! Stores program, analysis, component instances, and host across
//! run_hook_with_host calls so that useState state persists when re-running.

use crate::analysis::AnalysisResult;
use crate::ast::Program;
use crate::error::RuntimeError;
use crate::host::HostHandle;
use crate::runtime::{ComponentInstance, Runtime};
use crate::value::Value;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Mutex;

/// Serializes concurrent hook runs that share thread-local [`RunState`] (e.g. gateway SSR).
static HOOK_RUN_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static RUN_STATE: RefCell<Option<RunState>> = RefCell::new(None);
    static HANDLER_REGISTRY: RefCell<HashMap<u32, Value>> = RefCell::new(HashMap::new());
    static NEXT_HANDLER_ID: Cell<u32> = Cell::new(1);
    /// Set when a setter runs; host checks via consume_render_requested() to re-run.
    static RENDER_REQUESTED: Cell<bool> = Cell::new(false);
    /// In-memory localStorage for hooks (getItem/setItem).
    static LOCAL_STORAGE: RefCell<std::collections::HashMap<String, String>> = RefCell::new(std::collections::HashMap::new());
    /// Host for lazy() during first run when RunState not yet set.
    pub(crate) static HOST_FOR_LAZY: RefCell<Option<HostHandle>> = RefCell::new(None);
    /// Request-local tip blob GET cache (`@relay/ui` cachedGet).
    static TIP_GET_CACHE: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    /// Parsed `nav-index.txt` rows keyed by text hash (legacy parse helper; park catalog is QUERY).
    static NAV_INDEX_PARSED: RefCell<Option<(u64, std::rc::Rc<Vec<Value>>)>> =
        RefCell::new(None);
    /// Extra keys merged onto CleverScript `globalThis` at [`inject_standard_builtins`]
    /// (e.g. `__RELAY_PORTRAIT_SIBLINGS__` for gateway SSR).
    static CS_GLOBAL_THIS_EXTRAS: RefCell<HashMap<String, Value>> = RefCell::new(HashMap::new());
}

/// Seed a value onto CleverScript `globalThis` for the next hook run(s).
pub fn set_cs_global_this_extra(key: impl Into<String>, value: Value) {
    CS_GLOBAL_THIS_EXTRAS.with(|m| {
        m.borrow_mut().insert(key.into(), value);
    });
}

/// Drop all [`set_cs_global_this_extra`] entries.
pub fn clear_cs_global_this_extras() {
    CS_GLOBAL_THIS_EXTRAS.with(|m| m.borrow_mut().clear());
}

/// Clear tip GET cache (called with [`clear_run_state`]).
pub fn clear_tip_get_cache() {
    TIP_GET_CACHE.with(|c| c.borrow_mut().clear());
    clear_nav_index_parsed_cache();
}

/// Drop warmed nav-index parse (also via [`clear_tip_get_cache`]).
pub fn clear_nav_index_parsed_cache() {
    NAV_INDEX_PARSED.with(|c| *c.borrow_mut() = None);
}

fn hash_tip_text(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Cached tip `fetch(url)` text body (null→empty). Used by `@relay/*` catalog/ui helpers.
pub fn tip_cached_get(url: &str) -> Option<String> {
    let key = url.to_string();
    if let Some(hit) = TIP_GET_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Some(hit);
    }
    let host = get_host_for_lazy()?;
    let resp = host.borrow().fetch(url, None).ok()?;
    let text = match resp {
        Value::Object(o) => match o.get("text") {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    };
    TIP_GET_CACHE.with(|c| {
        c.borrow_mut().insert(key, text.clone());
    });
    Some(text)
}

/// Parse `/nav-index.txt` once per distinct tip blob text within a RunState session.
pub fn tip_cached_nav_index_rows(text: &str) -> std::rc::Rc<Vec<Value>> {
    let h = hash_tip_text(text);
    NAV_INDEX_PARSED.with(|c| {
        if let Some((prev, rows)) = c.borrow().as_ref() {
            if *prev == h {
                return rows.clone();
            }
        }
        let parsed = std::rc::Rc::new(crate::relay_modules::parse_nav_index_txt(text));
        *c.borrow_mut() = Some((h, parsed.clone()));
        parsed
    })
}

/// Inject Array / JSON / Object / Date / Math / Number / parseInt / localStorage / undefined /
/// document / setInterval.
/// Entry hooks get this via [`run_hook_persistent`]; imported modules must call it too.
///
/// Also mirrors JSON/Array/Object onto `globalThis` so `globalThis.JSON.parse` works
/// (tip code previously noted bare `globalThis.JSON` as unreliable).
pub fn inject_standard_builtins(runtime: &Runtime) {
    let mut env = runtime.env.borrow_mut();
    env.set("undefined", Value::Null);
    env.set("localStorage", localStorage_value());
    let array = array_value();
    let json = json_value();
    let object = object_value();
    env.set("Array", array.clone());
    env.set("JSON", json.clone());
    env.set("Object", object.clone());
    env.set("Date", date_value());
    env.set("Math", math_value());
    env.set("Number", number_value());
    env.set("parseInt", Value::Native(parse_int_native));
    env.set("parseFloat", Value::Native(parse_float_native));
    env.set("document", crate::dom_host::document_value());
    env.set("setInterval", crate::dom_host::set_interval_value());
    env.set("clearInterval", crate::dom_host::clear_interval_value());

    let mut gt = match env.get("globalThis") {
        Some(Value::Object(o)) => o,
        _ => std::collections::HashMap::new(),
    };
    gt.insert("JSON".to_string(), json);
    gt.insert("Array".to_string(), array);
    gt.insert("Object".to_string(), object);
    CS_GLOBAL_THIS_EXTRAS.with(|m| {
        for (k, v) in m.borrow().iter() {
            gt.insert(k.clone(), v.clone());
        }
    });
    #[cfg(target_arch = "wasm32")]
    {
        seed_portrait_siblings_from_browser(&mut gt);
    }
    env.set("globalThis", Value::Object(gt));
}

/// Copy browser tip portrait index into CS `globalThis` (sandbox ≠ window.globalThis).
#[cfg(target_arch = "wasm32")]
fn seed_portrait_siblings_from_browser(gt: &mut HashMap<String, Value>) {
    const KEY: &str = "__RELAY_PORTRAIT_SIBLINGS__";
    if gt.contains_key(KEY) {
        return;
    }
    let json = (|| {
        let global = js_sys::global();
        if let Ok(v) = js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str(KEY)) {
            if !v.is_undefined() && !v.is_null() {
                if let Ok(s) = js_sys::JSON::stringify(&v) {
                    if let Some(s) = s.as_string() {
                        if !s.is_empty() && s != "null" {
                            return Some(s);
                        }
                    }
                }
            }
        }
        let window = web_sys::window()?;
        let document = window.document()?;
        let el = document.get_element_by_id("relay-portrait-siblings")?;
        el.text_content()
    })();
    let Some(raw) = json.filter(|s| !s.is_empty()) else {
        return;
    };
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
        gt.insert(KEY.to_string(), Value::from_json(&parsed));
    }
}

/// Build Array object with isArray for injection into runtime env.
pub fn array_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "isArray".to_string(),
        Value::Native(|args: &[Value]| {
            let v = args.first();
            Ok(Value::Bool(matches!(v, Some(Value::Array(_)))))
        }),
    );
    Value::Object(obj)
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

/// Parse a subset of ISO-8601 / epoch millis into UTC epoch millis. Invalid → None.
pub fn date_parse_ms(input: &str) -> Option<f64> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<f64>() {
        if n.is_finite() {
            return Some(n);
        }
    }
    // YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS(.sss)?Z?
    if s.len() < 10 || s.as_bytes().get(4) != Some(&b'-') || s.as_bytes().get(7) != Some(&b'-') {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    let (hh, mm, ss) = if s.len() >= 19 && s.as_bytes().get(10) == Some(&b'T') {
        let hh: u32 = s.get(11..13)?.parse().ok()?;
        let mm: u32 = s.get(14..16)?.parse().ok()?;
        let ss: u32 = s.get(17..19)?.parse().ok()?;
        (hh, mm, ss)
    } else {
        (0, 0, 0)
    };
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    Some(utc_ymd_hms_to_ms(y, mo, d, hh, mm, ss)?)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> Option<i64> {
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m as i64 + if m > 2 { -3 } else { 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

fn utc_ymd_hms_to_ms(y: i64, mo: u32, d: u32, hh: u32, mm: u32, ss: u32) -> Option<f64> {
    let days = days_from_civil(y, mo, d)?;
    let secs = days * 86400 + hh as i64 * 3600 + mm as i64 * 60 + ss as i64;
    Some((secs as f64) * 1000.0)
}

/// UTC civil components from epoch millis (Hinnant algorithm).
pub fn ms_to_utc_parts(ms: f64) -> (i64, u32, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000.0).floor() as i64;
    let z = secs.div_euclid(86400) + 719468;
    let tod = secs.rem_euclid(86400) as u32;
    let hh = tod / 3600;
    let mm = (tod % 3600) / 60;
    let ss = tod % 60;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (mp + if mp < 10 { 3 } else { -9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    let weekday = ((secs.div_euclid(86400) + 4).rem_euclid(7)) as u32; // 0=Sun
    (y, m, d, hh, mm, ss, weekday)
}

pub fn date_object_from_ms(ms: f64) -> Value {
    let mut o = HashMap::new();
    o.insert("__type".to_string(), Value::String("Date".to_string()));
    o.insert("__ms".to_string(), Value::Number(ms));
    Value::Object(o)
}

fn coerce_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) if n.is_finite() => Some(*n),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                return Some(0.0);
            }
            t.parse::<f64>().ok().filter(|n| n.is_finite())
        }
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Null => Some(0.0),
        _ => None,
    }
}

fn parse_int_native(args: &[Value]) -> Result<Value, String> {
    let s = match args.first() {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => format!("{}", *n as i64),
        Some(v) => match coerce_number(v) {
            Some(n) => format!("{}", n as i64),
            None => return Ok(Value::Null),
        },
        None => return Ok(Value::Null),
    };
    let radix = match args.get(1) {
        Some(Value::Number(n)) => *n as u32,
        _ => 10,
    };
    if !(2..=36).contains(&radix) && radix != 0 {
        return Ok(Value::Null);
    }
    let radix = if radix == 0 { 10 } else { radix };
    let digits: String = s
        .chars()
        .enumerate()
        .take_while(|(i, c)| {
            if *i == 0 && (*c == '+' || *c == '-') {
                return true;
            }
            c.is_digit(radix)
        })
        .map(|(_, c)| c)
        .collect();
    if digits.is_empty() || digits == "+" || digits == "-" {
        return Ok(Value::Null);
    }
    match i64::from_str_radix(&digits, radix) {
        Ok(n) => Ok(Value::Number(n as f64)),
        Err(_) => Ok(Value::Null),
    }
}

fn parse_float_native(args: &[Value]) -> Result<Value, String> {
    let s = match args.first() {
        Some(Value::String(s)) => s.trim(),
        Some(Value::Number(n)) => return Ok(Value::Number(*n)),
        Some(v) => {
            return Ok(coerce_number(v).map(Value::Number).unwrap_or(Value::Null));
        }
        None => return Ok(Value::Null),
    };
    // parse leading float
    let mut end = 0usize;
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b'+') || bytes.first() == Some(&b'-') {
        end = 1;
    }
    let mut seen_dot = false;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_digit() {
            end += 1;
        } else if b == b'.' && !seen_dot {
            seen_dot = true;
            end += 1;
        } else {
            break;
        }
    }
    if end == 0 || (end == 1 && (bytes[0] == b'+' || bytes[0] == b'-')) {
        return Ok(Value::Null);
    }
    match s[..end].parse::<f64>() {
        Ok(n) if n.is_finite() => Ok(Value::Number(n)),
        _ => Ok(Value::Null),
    }
}

/// Build Number helpers. CleverScript has no callable-with-properties, so use `Number.from(x)`.
pub fn number_value() -> Value {
    let mut obj = HashMap::new();
    obj.insert(
        "from".to_string(),
        Value::Native(|args: &[Value]| {
            Ok(args
                .first()
                .and_then(coerce_number)
                .map(Value::Number)
                .unwrap_or(Value::Null))
        }),
    );
    obj.insert(
        "isFinite".to_string(),
        Value::Native(|args: &[Value]| {
            Ok(Value::Bool(matches!(
                args.first(),
                Some(Value::Number(n)) if n.is_finite()
            )))
        }),
    );
    obj.insert(
        "isInteger".to_string(),
        Value::Native(|args: &[Value]| {
            Ok(Value::Bool(matches!(
                args.first(),
                Some(Value::Number(n)) if n.is_finite() && n.fract() == 0.0
            )))
        }),
    );
    Value::Object(obj)
}

/// Math.min / max / abs / floor / ceil / round.
pub fn math_value() -> Value {
    let mut obj = HashMap::new();
    obj.insert(
        "min".to_string(),
        Value::Native(|args: &[Value]| {
            let mut best: Option<f64> = None;
            for a in args {
                let Some(n) = coerce_number(a) else {
                    continue;
                };
                best = Some(match best {
                    Some(b) => b.min(n),
                    None => n,
                });
            }
            Ok(best.map(Value::Number).unwrap_or(Value::Null))
        }),
    );
    obj.insert(
        "max".to_string(),
        Value::Native(|args: &[Value]| {
            let mut best: Option<f64> = None;
            for a in args {
                let Some(n) = coerce_number(a) else {
                    continue;
                };
                best = Some(match best {
                    Some(b) => b.max(n),
                    None => n,
                });
            }
            Ok(best.map(Value::Number).unwrap_or(Value::Null))
        }),
    );
    obj.insert(
        "abs".to_string(),
        Value::Native(|args: &[Value]| {
            Ok(args
                .first()
                .and_then(coerce_number)
                .map(|n| Value::Number(n.abs()))
                .unwrap_or(Value::Null))
        }),
    );
    obj.insert(
        "floor".to_string(),
        Value::Native(|args: &[Value]| {
            Ok(args
                .first()
                .and_then(coerce_number)
                .map(|n| Value::Number(n.floor()))
                .unwrap_or(Value::Null))
        }),
    );
    obj.insert(
        "ceil".to_string(),
        Value::Native(|args: &[Value]| {
            Ok(args
                .first()
                .and_then(coerce_number)
                .map(|n| Value::Number(n.ceil()))
                .unwrap_or(Value::Null))
        }),
    );
    obj.insert(
        "round".to_string(),
        Value::Native(|args: &[Value]| {
            Ok(args
                .first()
                .and_then(coerce_number)
                .map(|n| Value::Number(n.round()))
                .unwrap_or(Value::Null))
        }),
    );
    Value::Object(obj)
}

/// Build Date object: `now`, `parse`, `from` (CS has no `new Date` yet).
/// Instances are `{ __type: "Date", __ms }` with methods dispatched in the runtime.
pub fn date_value() -> Value {
    let mut obj = HashMap::new();
    obj.insert(
        "now".to_string(),
        Value::Native(|_args: &[Value]| Ok(Value::Number(now_ms()))),
    );
    obj.insert(
        "parse".to_string(),
        Value::Native(|args: &[Value]| {
            let s = match args.first() {
                Some(Value::String(s)) => s.as_str(),
                Some(Value::Number(n)) => return Ok(Value::Number(*n)),
                _ => return Ok(Value::Null),
            };
            Ok(date_parse_ms(s).map(Value::Number).unwrap_or(Value::Null))
        }),
    );
    obj.insert(
        "from".to_string(),
        Value::Native(|args: &[Value]| {
            let ms = match args.first() {
                Some(Value::Number(n)) if n.is_finite() => *n,
                Some(Value::String(s)) => match date_parse_ms(s) {
                    Some(n) => n,
                    None => return Ok(Value::Null),
                },
                None => now_ms(),
                _ => return Ok(Value::Null),
            };
            Ok(date_object_from_ms(ms))
        }),
    );
    Value::Object(obj)
}

/// Build Object with keys, entries for injection into runtime env.
pub fn object_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "keys".to_string(),
        Value::Native(|args: &[Value]| {
            let o = args.first().and_then(|v| if let Value::Object(x) = v { Some(x) } else { None }).ok_or("Object.keys requires object")?;
            let keys: Vec<Value> = o.keys().map(|k| Value::String(k.clone())).collect();
            Ok(Value::Array(keys))
        }),
    );
    obj.insert(
        "entries".to_string(),
        Value::Native(|args: &[Value]| {
            let o = args.first().and_then(|v| if let Value::Object(x) = v { Some(x) } else { None }).ok_or("Object.entries requires object")?;
            let entries: Vec<Value> = o.iter().map(|(k, v)| Value::Array(vec![Value::String(k.clone()), v.clone()])).collect();
            Ok(Value::Array(entries))
        }),
    );
    Value::Object(obj)
}

/// Build JSON object with parse/stringify for injection into runtime env.
///
/// First-class CleverScript builtin (Rust `serde_json`). Supports nested
/// objects/arrays of strings/numbers/bools/null — used by tip DOM buses
/// (`JSON.stringify` gallery image lists into `data-*` attributes).
pub fn json_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "parse".to_string(),
        Value::Native(|args: &[Value]| {
            let s = args
                .first()
                .and_then(|v| if let Value::String(x) = v { Some(x.as_str()) } else { None })
                .ok_or("JSON.parse requires string")?;
            let v: serde_json::Value =
                serde_json::from_str(s).map_err(|e| format!("JSON.parse: {}", e))?;
            Ok(Value::from_json(&v))
        }),
    );
    obj.insert(
        "stringify".to_string(),
        Value::Native(|args: &[Value]| {
            // ES: JSON.stringify() with no args → undefined (we return Null → "null").
            let v = args.first().cloned().unwrap_or(Value::Null);
            Ok(Value::String(
                serde_json::to_string(&v.to_json()).unwrap_or_else(|_| "null".to_string()),
            ))
        }),
    );
    Value::Object(obj)
}

/// Build localStorage object for injection into runtime env.
#[allow(non_snake_case)]
pub fn localStorage_value() -> Value {
    use crate::value::Value;
    let mut obj = std::collections::HashMap::new();
    obj.insert(
        "getItem".to_string(),
        Value::Native(|args: &[Value]| {
            let key = args
                .first()
                .and_then(|v| if let Value::String(s) = v { Some(s.as_str()) } else { None })
                .unwrap_or("");
            let val = LOCAL_STORAGE.with(|m| m.borrow().get(key).cloned());
            Ok(Value::String(val.unwrap_or_default()))
        }),
    );
    obj.insert(
        "setItem".to_string(),
        Value::Native(|args: &[Value]| {
            let key = args
                .get(0)
                .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                .unwrap_or_default();
            let val = args
                .get(1)
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => format!("{}", n),
                    Value::Bool(b) => format!("{}", b),
                    Value::Null => "null".to_string(),
                    _ => "".to_string(),
                })
                .unwrap_or_default();
            LOCAL_STORAGE.with(|m| m.borrow_mut().insert(key, val));
            Ok(Value::Null)
        }),
    );
    Value::Object(obj)
}

/// Shared map of module path -> component instance for useState persistence in loaded hooks.
pub type LoadedInstances = Rc<RefCell<HashMap<String, Rc<RefCell<ComponentInstance>>>>>;

struct RunState {
    program: Program,
    analysis: AnalysisResult,
    default_instance: Rc<RefCell<ComponentInstance>>,
    loaded_instances: LoadedInstances,
    host: HostHandle,
    /// Source string; when it changes, we reset.
    source: String,
}

/// Clear RunState (e.g. for tests or when loading a new app).
pub fn clear_run_state() {
    RUN_STATE.with(|r| *r.borrow_mut() = None);
    HANDLER_REGISTRY.with(|h| h.borrow_mut().clear());
    NEXT_HANDLER_ID.with(|n| n.set(1));
    RENDER_REQUESTED.with(|r| r.set(false));
    LOCAL_STORAGE.with(|m| m.borrow_mut().clear());
    HOST_FOR_LAZY.with(|h| *h.borrow_mut() = None);
    clear_tip_get_cache();
    // Intentionally keep CS_GLOBAL_THIS_EXTRAS: gateway SSR sets them around
    // `with_isolated_hook_run`, which clears RunState before inject_standard_builtins.
    crate::session_theme::clear_session_theme();
}

/// Run `f` while holding the process-wide hook lock and with TLS [`RunState`] cleared
/// before and after. Use for SSR / multi-threaded hosts so concurrent requests cannot
/// corrupt thread-local hook state.
pub fn with_isolated_hook_run<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = HOOK_RUN_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    clear_run_state();
    let out = f();
    clear_run_state();
    out
}

/// Called when a setter runs; signals the host to re-run and re-render.
pub fn set_render_requested() {
    RENDER_REQUESTED.with(|r| r.set(true));
}

/// Returns true if a setter ran since last run, and clears the flag.
pub fn consume_render_requested() -> bool {
    RENDER_REQUESTED.with(|r| r.replace(false))
}

/// Host for lazy() imports; prefer `HOST_FOR_LAZY` (set before hook run) so fetch during
/// `run_hook_persistent` does not re-borrow `RUN_STATE` while it is mutably held.
pub fn get_host_for_lazy() -> Option<HostHandle> {
    HOST_FOR_LAZY
        .with(|h| h.borrow().clone())
        .or_else(|| {
            RUN_STATE.with(|r| {
                r.try_borrow()
                    .ok()
                    .and_then(|state| state.as_ref().map(|rs| rs.host.clone()))
            })
        })
}

/// Install host used by lazy imports and tip GET cache (tests / SSR preflight).
pub fn set_host_for_lazy(host: Option<HostHandle>) {
    HOST_FOR_LAZY.with(|h| *h.borrow_mut() = host);
}

/// Clear handler IDs accumulated during the previous VNode walk so only the **current** tree’s
/// handlers remain registered. Call immediately before each `value_to_vnode_with_handlers` pass
/// after `run_hook_persistent` returns (same hook source, re-render).
pub fn clear_handler_registry_for_vnode_pass() {
    HANDLER_REGISTRY.with(|h| h.borrow_mut().clear());
    NEXT_HANDLER_ID.with(|n| n.set(1));
}

/// Register a handler (Function or Setter) and return its ID for use in VNode props.
pub fn register_handler(v: &Value) -> u32 {
    let id = NEXT_HANDLER_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    HANDLER_REGISTRY.with(|h| h.borrow_mut().insert(id, v.clone()));
    id
}

/// Invoke a handler by ID. Uses RunState's Runtime to call the handler.
pub fn invoke_handler(id: u32) -> Result<Value, RuntimeError> {
    let handler = HANDLER_REGISTRY
        .with(|h| h.borrow().get(&id).cloned())
        .ok_or_else(|| RuntimeError::Runtime(format!("Handler {} not found", id)))?;

    RUN_STATE.with(|r| {
        let state = r.borrow();
        let rs = state.as_ref().ok_or_else(|| {
            RuntimeError::Runtime("No RunState (invoke_handler called before run)".to_string())
        })?;
        let mut runtime = Runtime::new(rs.program.clone(), rs.analysis.clone())
            .with_host(rs.host.clone())
            .with_loaded_instances(rs.loaded_instances.clone());
        runtime.default_instance = Some(rs.default_instance.clone());
        runtime.call_value(&handler, &[])
    })
}

/// Invoke a CleverScript/native function value with explicit arguments (e.g. per-anchor `href`).
/// Used by markdown `anchorOnClick` wrappers and **`MarkdownRenderer` `overrides`** during render.
///
/// When [`run_hook_persistent`] holds a mutable `RUN_STATE` borrow (first-pass / re-render),
/// falls back to [`get_host_for_lazy`] so Function overrides can still run. Function values
/// carry their own env; an empty program is enough for `call_value`.
pub fn invoke_handler_value(f: &Value, args: &[Value]) -> Result<Value, RuntimeError> {
    let from_state = RUN_STATE.with(|r| -> Option<Result<Value, RuntimeError>> {
        let state = r.try_borrow().ok()?;
        let rs = state.as_ref()?;
        let mut runtime = Runtime::new(rs.program.clone(), rs.analysis.clone())
            .with_host(rs.host.clone())
            .with_loaded_instances(rs.loaded_instances.clone());
        runtime.default_instance = Some(rs.default_instance.clone());
        Some(runtime.call_value(f, args))
    });
    if let Some(result) = from_state {
        return result;
    }

    let host = get_host_for_lazy().ok_or_else(|| {
        RuntimeError::Runtime(
            "No RunState/host (invoke_handler_value called outside hook run)".to_string(),
        )
    })?;
    let loaded = RUN_STATE.with(|r| {
        r.try_borrow()
            .ok()
            .and_then(|s| s.as_ref().map(|rs| rs.loaded_instances.clone()))
    });
    let mut runtime = Runtime::new(
        crate::ast::Program { body: vec![] },
        crate::analysis::AnalysisResult::default(),
    )
    .with_host(host);
    if let Some(loaded) = loaded {
        runtime = runtime.with_loaded_instances(loaded);
    }
    inject_standard_builtins(&runtime);
    runtime.call_value(f, args)
}

/// Run hook with persistent state. First call creates RunState; subsequent
/// calls with the same source reuse it so useState state persists.
///
/// Does **not** hold a mutable `RUN_STATE` borrow across `runtime.run()`, so mid-render
/// calls (markdown overrides, lazy host) can `try_borrow` / use [`HOST_FOR_LAZY`].
pub fn run_hook_persistent(
    source: &str,
    host: HostHandle,
) -> Result<crate::value::Value, RuntimeError> {
    let program = crate::parse(source)?;
    let analysis = crate::analyze(&program).map_err(|e| RuntimeError::Runtime(e.to_string()))?;

    HOST_FOR_LAZY.with(|h| *h.borrow_mut() = Some(host.clone()));

    let reset_source = RUN_STATE.with(|r| {
        let mut state = r.borrow_mut();
        if let Some(ref rs) = *state {
            if rs.source != source {
                *state = None;
                HANDLER_REGISTRY.with(|h| h.borrow_mut().clear());
                NEXT_HANDLER_ID.with(|n| n.set(1));
                return true;
            }
        }
        false
    });
    let _ = reset_source;

    let is_first = RUN_STATE.with(|r| r.borrow().is_none());
    if is_first {
        HANDLER_REGISTRY.with(|h| h.borrow_mut().clear());
        NEXT_HANDLER_ID.with(|n| n.set(1));
        let loaded = Rc::new(RefCell::new(HashMap::new()));
        let mut runtime = Runtime::new(program.clone(), analysis.clone())
            .with_host(host.clone())
            .with_loaded_instances(loaded.clone());
        inject_standard_builtins(&runtime);
        let result = runtime.run()?;
        let default_instance = runtime
            .default_instance
            .clone()
            .expect("run creates default_instance");
        RUN_STATE.with(|r| {
            *r.borrow_mut() = Some(RunState {
                program,
                analysis,
                default_instance,
                loaded_instances: loaded,
                host,
                source: source.to_string(),
            });
        });
        return Ok(result);
    }

    let (program_r, analysis_r, host_r, loaded_r, default_instance) = RUN_STATE.with(|r| {
        let state = r.borrow();
        let rs = state.as_ref().expect("RunState present after is_first check");
        (
            rs.program.clone(),
            rs.analysis.clone(),
            rs.host.clone(),
            rs.loaded_instances.clone(),
            rs.default_instance.clone(),
        )
    });
    let mut runtime = Runtime::new(program_r, analysis_r)
        .with_host(host_r)
        .with_loaded_instances(loaded_r);
    runtime.default_instance = Some(default_instance);
    inject_standard_builtins(&runtime);
    runtime.run()
}

#[cfg(test)]
mod handler_registry_tests {
    use super::*;
    use crate::value::Value;

    #[test]
    fn clear_handler_registry_for_vnode_pass_resets_ids() {
        clear_handler_registry_for_vnode_pass();
        let a = register_handler(&Value::Native(|_| Ok(Value::Null)));
        let b = register_handler(&Value::Native(|_| Ok(Value::Null)));
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        clear_handler_registry_for_vnode_pass();
        let c = register_handler(&Value::Native(|_| Ok(Value::Null)));
        assert_eq!(c, 1);
    }
}

