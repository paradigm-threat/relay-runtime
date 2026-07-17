//! Runtime value types.

use crate::ast::{Param, Statement};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Closures used for host callbacks (e.g. `Relay.git.readFile`) where a plain function pointer cannot capture state.
#[derive(Clone)]
pub struct NativeFnClosure(pub Rc<dyn Fn(&[Value]) -> Result<Value, String>>);

impl PartialEq for NativeFnClosure {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for NativeFnClosure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeFnClosure(..)")
    }
}

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
    /// component_module_path: when Some(path), instance is looked up by path for state persistence.
    Function {
        params: Vec<Param>,
        body: Vec<Statement>,
        env: Rc<RefCell<Env>>,
        is_async: bool,
        component_hooks: Option<usize>,
        component_module_path: Option<String>,
    },
    /// Native/host function (function pointer — no environment)
    Native(fn(&[Value]) -> Result<Value, String>),
    /// Host closure (e.g. `Relay.git.readFile` with captured repo path). Prefer over static `Native` when state is required.
    NativeFn(NativeFnClosure),
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
            (Value::NativeFn(a), Value::NativeFn(b)) => a == b,
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

    /// Convert to JSON for debug context (locals snapshot). Non-serializable values become placeholders.
    pub fn to_debug_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Number(n) => serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::to_debug_json).collect())
            }
            Value::Object(obj) => serde_json::Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.to_debug_json()))
                    .collect(),
            ),
            Value::Function { .. } => serde_json::Value::String("[Function]".to_string()),
            Value::Native(_) => serde_json::Value::String("[Native]".to_string()),
            Value::NativeFn(_) => serde_json::Value::String("[NativeFn]".to_string()),
            Value::Setter(_) => serde_json::Value::String("[Setter]".to_string()),
            Value::ReactHook(name) => {
                serde_json::Value::String(format!("[Hook: {}]", name))
            }
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
                let current = cell.borrow().clone();
                if current != new_val {
                    *cell.borrow_mut() = new_val;
                    crate::run_state::set_render_requested();
                }
                Ok(Value::Null)
            }
            _ => Err(crate::error::RuntimeError::Runtime("Not a setter".to_string())),
        }
    }
}

/// Mutability of a variable binding (CleverScript: const vs let/var).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    Const,
    Let,
    Var,
}

/// Optional type name for runtime validation ("number", "string", etc.).
pub type TypeName = String;

/// Environment (scope) for variable lookup.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub bindings: HashMap<String, (Value, Mutability, Option<TypeName>)>,
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
        self.bindings
            .get(name)
            .map(|(v, _, _)| v.clone())
            .or_else(|| self.parent.as_ref().and_then(|p| p.borrow().get(name)))
    }

    /// Snapshot current scope bindings for debug (error context). Uses Value::to_debug_json.
    pub fn to_locals_json(&self) -> serde_json::Map<String, serde_json::Value> {
        self.bindings
            .iter()
            .map(|(k, (v, _, _))| (k.clone(), v.to_debug_json()))
            .collect()
    }

    pub fn get_mutability(&self, name: &str) -> Option<Mutability> {
        self.bindings
            .get(name)
            .map(|(_, m, _)| *m)
            .or_else(|| {
                self.parent
                    .as_ref()
                    .and_then(|p| p.borrow().get_mutability(name))
            })
    }

    /// Set a variable (host-injected: params, imports, etc.). Always uses Var mutability.
    pub fn set(&mut self, name: &str, value: Value) {
        self.bindings
            .insert(name.to_string(), (value, Mutability::Var, None));
    }

    /// Declare a variable with explicit mutability and optional type.
    /// Returns Err if type annotation is present and value does not match.
    pub fn set_with_mutability(
        &mut self,
        name: &str,
        value: Value,
        mutability: Mutability,
        type_name: Option<String>,
    ) -> Result<(), String> {
        if let Some(ref tn) = type_name {
            if !value_matches_type(&value, tn) {
                return Err(format!(
                    "Type mismatch: {} expects {}, got {}",
                    name,
                    tn,
                    value_type_name(&value)
                ));
            }
        }
        self.bindings
            .insert(name.to_string(), (value, mutability, type_name));
        Ok(())
    }

    /// Assign to an existing variable. Returns Err if variable is const or type mismatch.
    pub fn set_checked(&mut self, name: &str, value: Value) -> Result<(), String> {
        if let Some((_, mutability, type_name)) = self.bindings.get(name) {
            if *mutability == Mutability::Const {
                return Err(format!("Cannot reassign const variable: {}", name));
            }
            if let Some(tn) = type_name {
                if !value_matches_type(&value, tn) {
                    return Err(format!(
                        "Type mismatch: {} expects {}, got {}",
                        name,
                        tn,
                        value_type_name(&value)
                    ));
                }
            }
            self.bindings
                .insert(name.to_string(), (value, *mutability, type_name.clone()));
            return Ok(());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow_mut().set_checked(name, value);
        }
        self.bindings
            .insert(name.to_string(), (value, Mutability::Var, None));
        Ok(())
    }
}

fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        _ => "other",
    }
}

fn value_matches_type(v: &Value, type_name: &str) -> bool {
    match type_name {
        "number" => matches!(v, Value::Number(_)),
        "string" => matches!(v, Value::String(_)),
        "boolean" => matches!(v, Value::Bool(_)),
        "object" => matches!(v, Value::Object(_)),
        "array" => matches!(v, Value::Array(_)),
        "null" => matches!(v, Value::Null),
        "undefined" => matches!(v, Value::Null),
        _ => true,
    }
}
