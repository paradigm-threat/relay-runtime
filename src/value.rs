//! Runtime value types.

use crate::ast::{Param, Statement};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Runtime value.
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    /// Function reference: (params, body, closure env)
    /// component_hooks: when Some(n), this is a component from another module; use its own hook slots.
    Function {
        params: Vec<Param>,
        body: Vec<Statement>,
        env: Rc<RefCell<Env>>,
        is_async: bool,
        component_hooks: Option<usize>,
    },
    /// Native/host function
    Native(fn(&[Value]) -> Result<Value, String>),
    /// useState setter - shared ref to state cell
    Setter(Rc<RefCell<Value>>),
    /// React hook from import('react') — name is "useState", "useEffect", etc.
    ReactHook(String),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            (Value::Setter(a), Value::Setter(b)) => Rc::ptr_eq(a, b),
            (Value::ReactHook(a), Value::ReactHook(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    /// Convert from serde_json::Value (for theme config, etc.)
    pub fn from_json(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(arr) => Value::Array(arr.iter().map(Self::from_json).collect()),
            serde_json::Value::Object(obj) => Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), Self::from_json(v)))
                    .collect(),
            ),
        }
    }

    /// Convert to serde_json::Value for JSON serialization.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Number(n) => serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::to_json).collect())
            }
            Value::Object(obj) => serde_json::Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect(),
            ),
            _ => serde_json::Value::Null,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null | Value::Bool(false) => false,
            Value::Number(n) => *n != 0.0 && !n.is_nan(),
            Value::String(s) => !s.is_empty(),
            _ => true,
        }
    }

    /// Invoke a setter (Value::Setter) with the given args. Used when the host
    /// needs to call back into the runtime (e.g. after user click) without a full Runtime.
    pub fn invoke_setter(&self, args: &[Value]) -> Result<Value, crate::error::RuntimeError> {
        match self {
            Value::Setter(cell) => {
                let new_val = args.first().cloned().unwrap_or(Value::Null);
                *cell.borrow_mut() = new_val;
                crate::run_state::set_render_requested();
                Ok(Value::Null)
            }
            _ => Err(crate::error::RuntimeError::Runtime("Not a setter".to_string())),
        }
    }
}

/// Environment (scope) for variable lookup.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub bindings: HashMap<String, Value>,
    pub parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::default()))
    }

    pub fn child(parent: Rc<RefCell<Env>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            bindings: HashMap::new(),
            parent: Some(parent),
        }))
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.bindings.get(name).cloned().or_else(|| {
            self.parent
                .as_ref()
                .and_then(|p| p.borrow().get(name))
        })
    }

    pub fn set(&mut self, name: &str, value: Value) {
        self.bindings.insert(name.to_string(), value);
    }
}
