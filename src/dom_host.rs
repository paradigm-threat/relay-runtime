//! Browser `document` / timers for CleverScript (WASM) and SSR stubs.

use crate::value::Value;
use std::cell::Cell;
use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
use crate::value::NativeFnClosure;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

thread_local! {
    static DOM_NEXT_ID: Cell<u32> = Cell::new(1);
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static DOM_NODES: RefCell<HashMap<u32, web_sys::Element>> = RefCell::new(HashMap::new());
    static INTERVALS: RefCell<HashMap<i32, wasm_bindgen::closure::Closure<dyn FnMut()>>> =
        RefCell::new(HashMap::new());
}

/// Injected as CleverScript `document`.
pub fn document_value() -> Value {
    #[cfg(target_arch = "wasm32")]
    {
        wasm_document_value()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        stub_document_value()
    }
}

fn stub_document_value() -> Value {
    let mut doc = HashMap::new();
    doc.insert(
        "getElementById".to_string(),
        Value::Native(|_| Ok(Value::Null)),
    );
    doc.insert(
        "createElement".to_string(),
        Value::Native(|_| Ok(Value::Null)),
    );
    doc.insert(
        "querySelector".to_string(),
        Value::Native(|_| Ok(Value::Null)),
    );
    doc.insert(
        "querySelectorAll".to_string(),
        Value::Native(|_| Ok(Value::Array(vec![]))),
    );
    doc.insert("documentElement".to_string(), Value::Null);
    doc.insert("body".to_string(), Value::Null);
    let mut loc = HashMap::new();
    loc.insert("hash".to_string(), Value::String(String::new()));
    doc.insert("location".to_string(), Value::Object(loc));
    doc.insert(
        "addEventListener".to_string(),
        Value::Native(|_| Ok(Value::Null)),
    );
    doc.insert(
        "removeEventListener".to_string(),
        Value::Native(|_| Ok(Value::Null)),
    );
    Value::Object(doc)
}

#[cfg(target_arch = "wasm32")]
fn wasm_document_value() -> Value {
    use wasm_bindgen::JsCast;

    let mut doc = HashMap::new();
    doc.insert(
        "getElementById".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(|args| {
            let id = args
                .first()
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("document.getElementById requires string")?;
            let document = web_sys::window()
                .and_then(|w| w.document())
                .ok_or("document unavailable")?;
            match document.get_element_by_id(id) {
                Some(el) => Ok(wrap_element(el)),
                None => Ok(Value::Null),
            }
        }))),
    );
    doc.insert(
        "createElement".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(|args| {
            let tag = args
                .first()
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("document.createElement requires string")?;
            let document = web_sys::window()
                .and_then(|w| w.document())
                .ok_or("document unavailable")?;
            let el = document
                .create_element(tag)
                .map_err(|e| format!("{:?}", e))?;
            Ok(wrap_element(el))
        }))),
    );
    doc.insert(
        "querySelector".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(|args| {
            let sel = args
                .first()
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("document.querySelector requires string")?;
            let document = web_sys::window()
                .and_then(|w| w.document())
                .ok_or("document unavailable")?;
            match document.query_selector(sel).map_err(|e| format!("{:?}", e))? {
                Some(el) => Ok(wrap_element(el)),
                None => Ok(Value::Null),
            }
        }))),
    );
    doc.insert(
        "querySelectorAll".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(|args| {
            let sel = args
                .first()
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("document.querySelectorAll requires string")?;
            let document = web_sys::window()
                .and_then(|w| w.document())
                .ok_or("document unavailable")?;
            let list = document
                .query_selector_all(sel)
                .map_err(|e| format!("{:?}", e))?;
            let mut out = Vec::new();
            for i in 0..list.length() {
                if let Some(node) = list.item(i) {
                    if let Ok(el) = node.dyn_into::<web_sys::Element>() {
                        out.push(wrap_element(el));
                    }
                }
            }
            Ok(Value::Array(out))
        }))),
    );
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Some(root) = document.document_element() {
            doc.insert("documentElement".to_string(), wrap_element(root));
        }
        if let Some(body) = document.body() {
            doc.insert("body".to_string(), wrap_element(body.unchecked_into()));
        }
        let mut loc = HashMap::new();
        let href_hash = document
            .location()
            .and_then(|l| l.hash().ok())
            .unwrap_or_default();
        loc.insert("hash".to_string(), Value::String(href_hash));
        doc.insert("location".to_string(), Value::Object(loc));
    }
    doc.insert(
        "addEventListener".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(|_args| Ok(Value::Null)))),
    );
    doc.insert(
        "removeEventListener".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(|_args| Ok(Value::Null)))),
    );
    Value::Object(doc)
}

#[cfg(target_arch = "wasm32")]
fn wrap_element(el: web_sys::Element) -> Value {
    let text = el.text_content().unwrap_or_default();
    let tag = el.tag_name();
    let id_attr = el.id();
    let id = DOM_NEXT_ID.with(|c| {
        let n = c.get();
        c.set(n + 1);
        n
    });
    DOM_NODES.with(|m| {
        m.borrow_mut().insert(id, el);
    });

    let mut o = HashMap::new();
    o.insert("__type".to_string(), Value::String("DomNode".into()));
    o.insert("__id".to_string(), Value::Number(id as f64));
    o.insert("textContent".to_string(), Value::String(text));
    o.insert("tagName".to_string(), Value::String(tag));
    o.insert("id".to_string(), Value::String(id_attr));

    let id_set = id;
    o.insert(
        "setAttribute".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(move |args| {
            let name = args
                .get(0)
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .ok_or("setAttribute requires name")?;
            let val = args
                .get(1)
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => format!("{}", n),
                    Value::Bool(b) => format!("{}", b),
                    Value::Null => String::new(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            DOM_NODES.with(|m| {
                let map = m.borrow();
                let el = map.get(&id_set).ok_or("DomNode gone")?;
                el.set_attribute(&name, &val)
                    .map_err(|e| format!("{:?}", e))?;
                Ok(Value::Null)
            })
        }))),
    );

    let id_get = id;
    o.insert(
        "getAttribute".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(move |args| {
            let name = args
                .first()
                .and_then(|v| {
                    if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .ok_or("getAttribute requires name")?;
            DOM_NODES.with(|m| {
                let map = m.borrow();
                let el = map.get(&id_get).ok_or("DomNode gone")?;
                Ok(match el.get_attribute(name) {
                    Some(s) => Value::String(s),
                    None => Value::Null,
                })
            })
        }))),
    );

    let id_append = id;
    o.insert(
        "appendChild".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(move |args| {
            let child = args.first().ok_or("appendChild requires node")?;
            let child_id = match child {
                Value::Object(o) => match o.get("__id") {
                    Some(Value::Number(n)) => *n as u32,
                    _ => return Err("appendChild arg must be DomNode".into()),
                },
                _ => return Err("appendChild arg must be DomNode".into()),
            };
            DOM_NODES.with(|m| {
                let map = m.borrow();
                let parent = map.get(&id_append).ok_or("parent DomNode gone")?;
                let child_el = map.get(&child_id).ok_or("child DomNode gone")?;
                parent
                    .append_child(child_el)
                    .map_err(|e| format!("{:?}", e))?;
                Ok(Value::Null)
            })
        }))),
    );

    let mut style_bag = HashMap::new();
    style_bag.insert("overflow".to_string(), Value::String(String::new()));
    o.insert("style".to_string(), Value::Object(style_bag));

    let id_scroll = id;
    o.insert(
        "scrollIntoView".to_string(),
        Value::NativeFn(NativeFnClosure(Rc::new(move |_args| {
            DOM_NODES.with(|m| {
                let map = m.borrow();
                let el = map.get(&id_scroll).ok_or("DomNode gone")?;
                el.scroll_into_view();
                Ok(Value::Null)
            })
        }))),
    );

    Value::Object(o)
}

pub fn set_interval_value() -> Value {
    #[cfg(target_arch = "wasm32")]
    {
        wasm_set_interval()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Value::Native(|_| Ok(Value::Number(0.0)))
    }
}

pub fn clear_interval_value() -> Value {
    #[cfg(target_arch = "wasm32")]
    {
        wasm_clear_interval()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Value::Native(|_| Ok(Value::Null))
    }
}

#[cfg(target_arch = "wasm32")]
fn wasm_set_interval() -> Value {
    use crate::run_state::invoke_handler_value;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    Value::NativeFn(NativeFnClosure(Rc::new(|args| {
        let cb = args.first().cloned().ok_or("setInterval requires callback")?;
        let ms = args
            .get(1)
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i32)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let window = web_sys::window().ok_or("window unavailable")?;
        let closure = Closure::wrap(Box::new(move || {
            let _ = invoke_handler_value(&cb, &[]);
        }) as Box<dyn FnMut()>);
        let id = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                ms,
            )
            .map_err(|e| format!("{:?}", e))?;
        INTERVALS.with(|m| {
            m.borrow_mut().insert(id, closure);
        });
        Ok(Value::Number(id as f64))
    })))
}

#[cfg(target_arch = "wasm32")]
fn wasm_clear_interval() -> Value {
    Value::NativeFn(NativeFnClosure(Rc::new(|args| {
        let id = args
            .first()
            .and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n as i32)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        if let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(id);
        }
        INTERVALS.with(|m| {
            m.borrow_mut().remove(&id);
        });
        Ok(Value::Null)
    })))
}
