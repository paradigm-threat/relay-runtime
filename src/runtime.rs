//! Hooks runtime: interpret AST with useState, useEffect, useCallback semantics.

use crate::analysis::AnalysisResult;
use crate::ast::*;
use crate::error::{RuntimeError, RuntimeErrorDetail};
use crate::run_state::LoadedInstances;
use crate::value::{Env, Mutability, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Max iterations for for/while loops. Prevents infinite loops from hanging the runtime.
pub const MAX_LOOP_ITERATIONS: u64 = 100_000;

/// Max call depth (component/function recursion). Prevents stack overflow from deep recursion.
pub const MAX_RECURSION_DEPTH: usize = 256;

/// Max array length for literals and concat. Prevents memory exhaustion.
pub const MAX_ARRAY_LENGTH: usize = 50_000;

/// Control flow from `eval_stmt` (early `return` / `break` / `continue` inside blocks).
enum EvalControl {
    /// Fall through to the next statement.
    Next,
    Return(Value),
    Break,
    ContinueLoop,
}

/// UTF-16 code-unit slice (JS `String.prototype.slice` indices are UTF-16 units).
fn utf16_slice_by_units(s: &str, start: usize, end: usize) -> String {
    let v: Vec<u16> = s.encode_utf16().collect();
    let end = end.min(v.len());
    let start = start.min(v.len());
    if start >= end {
        return String::new();
    }
    String::from_utf16_lossy(&v[start..end])
}

/// Hook slot state for a component instance.
#[derive(Debug)]
pub enum HookSlot {
    State(Rc<RefCell<Value>>),
    Effect { deps: Vec<Value>, ran: bool },
    Callback(Value),
}

/// Component instance with hook slots.
#[derive(Debug)]
pub struct ComponentInstance {
    pub name: String,
    pub hook_slots: Vec<HookSlot>,
    pub hook_index: usize,
}

impl ComponentInstance {
    fn new(name: String, num_slots: usize) -> Self {
        Self {
            name,
            hook_slots: (0..num_slots)
                .map(|_| HookSlot::State(Rc::new(RefCell::new(Value::Null))))
                .collect(),
            hook_index: 0,
        }
    }

    fn with_hook_calls(name: String, calls: &[crate::analysis::HookCallSite]) -> Self {
        let hook_slots = calls
            .iter()
            .map(|c| match c.hook_name.as_str() {
                "useEffect" => HookSlot::Effect {
                    deps: Vec::new(),
                    ran: false,
                },
                "useCallback" => HookSlot::Callback(Value::Null),
                _ => HookSlot::State(Rc::new(RefCell::new(Value::Null))),
            })
            .collect();
        Self {
            name,
            hook_slots,
            hook_index: 0,
        }
    }
}

/// Runtime state.
pub struct Runtime {
    pub env: Rc<RefCell<Env>>,
    pub analysis: AnalysisResult,
    pub program: Program,
    /// Current component instance being rendered (for hook dispatch).
    pub current_component: Option<Rc<RefCell<ComponentInstance>>>,
    /// Component instance for default export (persists across re-renders).
    pub default_instance: Option<Rc<RefCell<ComponentInstance>>>,
    /// Host for fetch, navigate, createElement (optional).
    pub host: Option<crate::host::HostHandle>,
    pub loaded_instances: Option<LoadedInstances>,
    /// Call depth for recursion limit. Incremented on function/component call, decremented on return.
    pub call_depth: usize,
    /// Call stack for debug (Tier 1). Pushed on component/function entry, popped on success.
    pub call_stack: Vec<String>,
}

impl Runtime {
    /// Build a fallback VNode for error boundary: a div with data-error-boundary and the error message.
    fn error_fallback_vnode(component_name: &str, message: &str) -> Value {
        let mut props = HashMap::new();
        props.insert("className".to_string(), Value::String("relay-error-boundary".to_string()));
        props.insert(
            "data-error-boundary".to_string(),
            Value::String(component_name.to_string()),
        );
        props.insert(
            "data-error-message".to_string(),
            Value::String(message.to_string()),
        );
        props.insert(
            "style".to_string(),
            Value::Object(
                [
                    ("padding".to_string(), Value::String("8px".to_string())),
                    ("backgroundColor".to_string(), Value::String("#fee".to_string())),
                    ("color".to_string(), Value::String("#c00".to_string())),
                    ("fontSize".to_string(), Value::String("12px".to_string())),
                    ("borderRadius".to_string(), Value::String("4px".to_string())),
                    ("border".to_string(), Value::String("1px solid #c00".to_string())),
                ]
                .into_iter()
                .collect(),
            ),
        );
        let children = vec![Value::String(format!("[{}] {}", component_name, message))];
        Value::Object(
            [
                ("type".to_string(), Value::String("element".to_string())),
                ("tag".to_string(), Value::String("div".to_string())),
                ("props".to_string(), Value::Object(props)),
                ("children".to_string(), Value::Array(children)),
            ]
            .into_iter()
            .collect(),
        )
    }

    pub fn new(program: Program, analysis: AnalysisResult) -> Self {
        Self {
            env: Env::new(),
            analysis,
            program,
            current_component: None,
            default_instance: None,
            host: None,
            loaded_instances: None,
            call_depth: 0,
            call_stack: Vec::new(),
        }
    }

    /// Build RuntimeError with call stack and locals for debug UI (Tier 1).
    fn err(&self, message: impl Into<String>) -> RuntimeError {
        RuntimeError::RuntimeDetail(RuntimeErrorDetail {
            message: message.into(),
            call_stack: self.call_stack.clone(),
            locals: self.env.borrow().to_locals_json(),
        })
    }

    /// Convert Runtime(s) to RuntimeDetail; other variants unchanged.
    fn enrich_err(&self, e: RuntimeError) -> RuntimeError {
        match e {
            RuntimeError::Runtime(msg) => self.err(msg),
            other => other,
        }
    }

    /// Attach a host for fetch, navigate, createElement, load_module.
    pub fn with_host(mut self, host: crate::host::HostHandle) -> Self {
        self.host = Some(host);
        self
    }

    /// Attach loaded instances map for useState persistence in dynamically imported modules.
    pub fn with_loaded_instances(mut self, loaded: LoadedInstances) -> Self {
        self.loaded_instances = Some(loaded);
        self
    }

    /// Inject params, env, navigate into the runtime env before run.
    pub fn with_context(
        self,
        params: Value,
        env: Value,
        navigate: Value,
    ) -> Self {
        self.env.borrow_mut().set("params", params);
        self.env.borrow_mut().set("env", env);
        self.env.borrow_mut().set("navigate", navigate);
        self
    }

    /// Evaluate an expression.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Literal(lit) => Ok(match lit {
                Literal::Null => Value::Null,
                Literal::Bool(b) => Value::Bool(*b),
                Literal::Number(n) => Value::Number(*n),
                Literal::String(s) => Value::String(s.clone()),
            }),
            Expr::Unary { op, argument } => {
                let v = self.eval_expr(argument)?;
                Ok(match op {
                    UnaryOp::Not => Value::Bool(!v.is_truthy()),
                    UnaryOp::Neg => {
                        let n = match &v {
                            Value::Number(x) => *x,
                            _ => return Err(RuntimeError::Runtime("- requires number".to_string())),
                        };
                        Value::Number(-n)
                    }
                    UnaryOp::Typeof => Value::String(match &v {
                        Value::Null => "object",
                        Value::Bool(_) => "boolean",
                        Value::Number(_) => "number",
                        Value::String(_) => "string",
                        Value::Array(_) => "object",
                        Value::Object(_) => "object",
                        Value::Function { .. } | Value::Native(_) | Value::NativeFn(_) | Value::Setter(_) | Value::ReactHook(_) => "function",
                    }.to_string()),
                })
            }
            Expr::Identifier { name } => self.env
                .borrow()
                .get(name)
                .ok_or_else(|| RuntimeError::Runtime(format!("Undefined: {}", name))),
            Expr::Call {
                callee,
                arguments,
                optional,
            } => {
                let args: Vec<Value> = arguments
                    .iter()
                    .map(|a| self.eval_expr(a))
                    .collect::<Result<_, _>>()?;
                // console.log / warn / error → host.log (Tier 5)
                if let Expr::Member {
                    object,
                    property,
                    optional: _,
                } = callee.as_ref()
                {
                    if let (Expr::Identifier { name: obj }, Expr::Identifier { name: method }) =
                        (object.as_ref(), property.as_ref())
                    {
                        if obj == "console" {
                            match method.as_str() {
                                "log" | "warn" | "error" | "debug" | "info" => {
                                    if let Some(host) = &self.host {
                                        let msg = format_console_message(&args);
                                        host.borrow().log(method, &msg);
                                    }
                                    return Ok(Value::Null);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                if let Expr::Identifier { name } = callee.as_ref() {
                    if let Some(result) = self.try_dispatch_host(name, &args)? {
                        return Ok(result);
                    }
                }
                if let Expr::Member { object, property, optional: _ } = callee.as_ref() {
                    if let Expr::Identifier { name } = property.as_ref() {
                        let obj = self.eval_expr(object)?;
                        if let Value::Array(arr) = &obj {
                            if let Some(result) = self.array_method(name, arr, &args)? {
                                return Ok(result);
                            }
                        }
                        if let Value::String(s) = &obj {
                            if let Some(result) = self.string_method(name, s, &args)? {
                                return Ok(result);
                            }
                        }
                    }
                }
                let f = self.eval_expr(callee)?;
                if *optional && matches!(&f, Value::Null) {
                    return Ok(Value::Null);
                }
                self.call_value(&f, &args)
            }
            Expr::Binary { left, right, op } => {
                if *op == BinaryOp::NullishCoalesce {
                    let l = self.eval_expr(left)?;
                    if matches!(&l, Value::Null) {
                        self.eval_expr(right)
                    } else {
                        Ok(l)
                    }
                } else if *op == BinaryOp::And {
                    // Short-circuit && (avoid evaluating member access on null/undefined RHS)
                    let l = self.eval_expr(left)?;
                    if !l.is_truthy() {
                        Ok(l)
                    } else {
                        self.eval_expr(right)
                    }
                } else if *op == BinaryOp::Or {
                    let l = self.eval_expr(left)?;
                    if l.is_truthy() {
                        Ok(l)
                    } else {
                        self.eval_expr(right)
                    }
                } else {
                    let l = self.eval_expr(left)?;
                    let r = self.eval_expr(right)?;
                    self.eval_binary(l, *op, r)
                }
            }
            Expr::Array { elements } => {
                let mut vals = Vec::new();
                for el in elements {
                    match el {
                        ArrayElement::Expr(e) => {
                            if vals.len() >= MAX_ARRAY_LENGTH {
                                return Err(RuntimeError::Runtime(format!(
                                    "Array literal exceeds max length {}",
                                    MAX_ARRAY_LENGTH
                                )));
                            }
                            vals.push(self.eval_expr(e)?);
                        }
                        ArrayElement::Spread(e) => match self.eval_expr(e)? {
                            Value::Array(a) => {
                                if vals.len() + a.len() > MAX_ARRAY_LENGTH {
                                    return Err(RuntimeError::Runtime(format!(
                                        "Array literal exceeds max length {}",
                                        MAX_ARRAY_LENGTH
                                    )));
                                }
                                vals.extend(a);
                            }
                            _ => {
                                return Err(RuntimeError::Runtime(
                                    "Array spread requires an array".to_string(),
                                ));
                            }
                        },
                    }
                }
                Ok(Value::Array(vals))
            }
            Expr::Object { properties } => {
                let mut obj = HashMap::new();
                for p in properties {
                    match p {
                        ObjectExprProp::KeyValue { key, value } => {
                            obj.insert(key.clone(), self.eval_expr(value)?);
                        }
                        ObjectExprProp::Spread { argument } => match self.eval_expr(argument)? {
                            Value::Object(o) => {
                                for (k, v) in o {
                                    obj.insert(k, v);
                                }
                            }
                            Value::Null => {}
                            _ => {
                                return Err(RuntimeError::Runtime(
                                    "Object spread requires an object".to_string(),
                                ));
                            }
                        },
                    }
                }
                Ok(Value::Object(obj))
            }
            Expr::Await { argument } => self.eval_expr(argument),
            Expr::Import { source } => {
                let path = match self.eval_expr(source)? {
                    Value::String(s) => s,
                    v => {
                        return Err(RuntimeError::Runtime(format!(
                            "import() requires string, got {:?}",
                            v
                        )))
                    }
                };
                let host = match &self.host {
                    Some(h) => h.clone(),
                    None => {
                        return Err(RuntimeError::Runtime(
                            "import() requires host with load_module".to_string(),
                        ))
                    }
                };
                let module = host.borrow().load_module(&path).map_err(RuntimeError::Host)?;
                Ok(module)
            }
            Expr::Member {
                object,
                property,
                optional,
            } => {
                let obj = self.eval_expr(object)?;
                if *optional && matches!(&obj, Value::Null) {
                    return Ok(Value::Null);
                }
                let key = match property.as_ref() {
                    Expr::Identifier { name } => {
                        if matches!(&obj, Value::Array(_)) {
                            // For arrays: "length" is built-in. For indices like arr[i], evaluate variable.
                            // CleverScript: .push not supported — use styler.arrayConcat(arr, [x])
                            if name == "length" {
                                name.clone()
                            } else if name == "push" {
                                return Err(RuntimeError::Runtime(
                                    "CleverScript: .push not supported, use styler.arrayConcat(arr, [x])".to_string(),
                                ));
                            } else {
                                let v = self.eval_expr(property)?;
                                match v {
                                    Value::Number(n) => (n as usize).to_string(),
                                    _ => {
                                        return Err(RuntimeError::Runtime(
                                            "Array index must be number".to_string(),
                                        ))
                                    }
                                }
                            }
                        } else {
                            name.clone()
                        }
                    }
                    Expr::Literal(Literal::Number(n)) => (*n as usize).to_string(),
                    _ => {
                        let v = self.eval_expr(property)?;
                        match v {
                            Value::Number(n) => (n as usize).to_string(),
                            Value::String(s) => s,
                            _ => {
                                return Err(RuntimeError::Runtime(
                                    "Member property must evaluate to string or number".to_string(),
                                ))
                            }
                        }
                    }
                };
                match &obj {
                    Value::Object(o) => {
                        // CleverScript: objects have string keys only. No synthetic length.
                        o.get(&key)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Runtime(format!("Property '{}' not found", key)))
                    }
                    Value::Array(a) => {
                        if key == "length" {
                            Ok(Value::Number(a.len() as f64))
                        } else {
                            let i: usize = key.parse().map_err(|_| {
                                RuntimeError::Runtime("Array index must be number".to_string())
                            })?;
                            a.get(i)
                                .cloned()
                                .ok_or_else(|| RuntimeError::Runtime(format!("Index {} out of bounds", i)))
                        }
                    }
                    Value::String(s) => {
                        // JS-like string.length (UTF-16 code unit count for BMP-ish parity; surrogate pairs differ from JS)
                        if key == "length" {
                            Ok(Value::Number(s.encode_utf16().count() as f64))
                        } else {
                            Err(RuntimeError::Runtime(format!(
                                "Unknown string property '{}'",
                                key
                            )))
                        }
                    }
                    _ => Err(RuntimeError::Runtime(
                        "Member access on non-object/array".to_string(),
                    )),
                }
            }
            Expr::Conditional {
                test,
                consequent,
                alternate,
            } => {
                let t = self.eval_expr(test)?;
                if t.is_truthy() {
                    self.eval_expr(consequent)
                } else {
                    self.eval_expr(alternate)
                }
            }
            Expr::ArrowFunction { params, body } => {
                let body = match body {
                    ArrowBody::Block(stmts) => stmts.clone(),
                    ArrowBody::Expr(expr) => vec![Statement::Return {
                        argument: Some(expr.clone()),
                    }],
                };
                Ok(Value::Function {
                    params: params.clone(),
                    body,
                    env: self.env.clone(),
                    is_async: false,
                    component_hooks: None,
                    component_module_path: None,
                })
            }
            Expr::JsxElement(el) => self.eval_jsx_element(el),
            Expr::JsxFragment { children } => {
                let mut vals = Vec::new();
                for c in children {
                    self.collect_jsx_children(c, &mut vals);
                }
                Ok(if vals.len() == 1 {
                    vals.into_iter().next().unwrap()
                } else {
                    Value::Array(vals)
                })
            }
            Expr::PostfixIncr { argument } => {
                let old = self.eval_expr(argument)?;
                let n = match &old {
                    Value::Number(x) => *x,
                    _ => return Err(RuntimeError::Runtime("++ requires number".to_string())),
                };
                let new_val = Value::Number(n + 1.0);
                if let Expr::Identifier { name } = argument.as_ref() {
                    self.env.borrow_mut().set_checked(name, new_val)
                        .map_err(RuntimeError::Runtime)?;
                }
                Ok(old)
            }
            Expr::PostfixDecr { argument } => {
                let old = self.eval_expr(argument)?;
                let n = match &old {
                    Value::Number(x) => *x,
                    _ => return Err(RuntimeError::Runtime("-- requires number".to_string())),
                };
                let new_val = Value::Number(n - 1.0);
                if let Expr::Identifier { name } = argument.as_ref() {
                    self.env.borrow_mut().set_checked(name, new_val)
                        .map_err(RuntimeError::Runtime)?;
                }
                Ok(old)
            }
            Expr::PrefixIncr { argument } => {
                let old = self.eval_expr(argument)?;
                let n = match &old {
                    Value::Number(x) => *x,
                    _ => return Err(RuntimeError::Runtime("++ requires number".to_string())),
                };
                let new_val = Value::Number(n + 1.0);
                if let Expr::Identifier { name } = argument.as_ref() {
                    self.env
                        .borrow_mut()
                        .set_checked(name, new_val.clone())
                        .map_err(RuntimeError::Runtime)?;
                }
                Ok(new_val)
            }
            Expr::PrefixDecr { argument } => {
                let old = self.eval_expr(argument)?;
                let n = match &old {
                    Value::Number(x) => *x,
                    _ => return Err(RuntimeError::Runtime("-- requires number".to_string())),
                };
                let new_val = Value::Number(n - 1.0);
                if let Expr::Identifier { name } = argument.as_ref() {
                    self.env
                        .borrow_mut()
                        .set_checked(name, new_val.clone())
                        .map_err(RuntimeError::Runtime)?;
                }
                Ok(new_val)
            }
            Expr::Assign { left, right } => {
                let val = self.eval_expr(right)?;
                match left.as_ref() {
                    Expr::Identifier { name } => {
                        self.env.borrow_mut().set_checked(name, val.clone())
                            .map_err(RuntimeError::Runtime)?;
                    }
                    Expr::Member {
                        object,
                        property,
                        optional,
                    } => {
                        let mut obj = self.eval_expr(object)?;
                        if *optional && matches!(&obj, Value::Null) {
                            return Ok(Value::Null);
                        }
                        let key = match property.as_ref() {
                            Expr::Identifier { name } => name.clone(),
                            Expr::Literal(Literal::Number(n)) => (*n as usize).to_string(),
                            _ => {
                                let v = self.eval_expr(property)?;
                                match v {
                                    Value::Number(n) => (n as usize).to_string(),
                                    Value::String(s) => s,
                                    _ => {
                                        return Err(RuntimeError::Runtime(
                                            "Member key must be string or number".to_string(),
                                        ));
                                    }
                                }
                            }
                        };
                        match &mut obj {
                            Value::Object(o) => {
                                o.insert(key, val.clone());
                            }
                            Value::Array(a) => {
                                let i: usize = key.parse().map_err(|_| {
                                    RuntimeError::Runtime("Array index must be number".to_string())
                                })?;
                                if i >= a.len() {
                                    a.resize(i + 1, Value::Null);
                                }
                                a[i] = val.clone();
                            }
                            _ => {
                                return Err(RuntimeError::Runtime(
                                    "Assignment target must be identifier or member of object/array"
                                        .to_string(),
                                ));
                            }
                        }
                        if let Expr::Identifier { name } = object.as_ref() {
                            self.env.borrow_mut().set_checked(name, obj)
                                .map_err(RuntimeError::Runtime)?;
                        }
                    }
                    _ => {
                        return Err(RuntimeError::Runtime(
                            "Assignment target must be identifier or member".to_string(),
                        ));
                    }
                }
                Ok(val)
            }
            _ => Err(RuntimeError::Runtime(format!(
                "Unsupported expression: {:?}",
                expr
            ))),
        }
    }

    fn collect_jsx_children(&mut self, c: &JsxChild, vals: &mut Vec<Value>) {
        match c {
            JsxChild::Expr(e) => {
                if let Ok(v) = self.eval_expr(e) {
                    vals.push(v);
                }
            }
            JsxChild::Text(s) => vals.push(Value::String(s.clone())),
            JsxChild::Element(ce) => {
                if let Ok(v) = self.eval_jsx_element(ce) {
                    vals.push(v);
                }
            }
            JsxChild::Fragment { children } => {
                for fc in children {
                    self.collect_jsx_children(fc, vals);
                }
            }
        }
    }

    fn eval_jsx_element(&mut self, el: &JsxElement) -> Result<Value, RuntimeError> {
        let props: HashMap<String, Value> = el
            .props
            .iter()
            .filter_map(|p| {
                if let JsxProp::Attribute { name, value } = p {
                    let v = match value {
                        JsxAttrValue::String(s) => Value::String(s.clone()),
                        JsxAttrValue::Expr(e) => self.eval_expr(e).ok()?,
                    };
                    Some((name.clone(), v))
                } else {
                    None
                }
            })
            .collect();
        let mut children: Vec<Value> = Vec::new();
        for c in &el.children {
            match c {
                JsxChild::Expr(e) => {
                    if let Ok(v) = self.eval_expr(e) {
                        children.push(v);
                    }
                }
                JsxChild::Text(s) => {
                    children.push(Value::String(s.clone()));
                }
                JsxChild::Element(child_el) => {
                    if let Ok(v) = self.eval_jsx_element(child_el) {
                        children.push(v);
                    }
                }
                JsxChild::Fragment { children: frag_children } => {
                    for fc in frag_children {
                        match fc {
                            JsxChild::Expr(e) => {
                                if let Ok(v) = self.eval_expr(e) {
                                    children.push(v);
                                }
                            }
                            JsxChild::Text(s) => {
                                children.push(Value::String(s.clone()));
                            }
                            JsxChild::Element(ce) => {
                                if let Ok(v) = self.eval_jsx_element(ce) {
                                    children.push(v);
                                }
                            }
                            JsxChild::Fragment { .. } => {}
                        }
                    }
                }
            }
        }
        // Component invocation: if tag resolves to a function, call it with props (React convention).
        // Per-component error boundary: on error, render fallback UI instead of crashing the whole tree.
        if let JsxTag::Identifier { name } = &el.tag {
            let component = self.env.borrow().get(name);
            if let Some(ref c) = component {
                if matches!(c, Value::Function { .. } | Value::Native(_) | Value::NativeFn(_)) {
                    let mut props_with_children = props.clone();
                    props_with_children.insert("children".to_string(), Value::Array(children));
                    let props_val = Value::Object(props_with_children);
                    return match self.call_value(c, &[props_val]) {
                        Ok(v) => Ok(v),
                        Err(e) => Ok(Self::error_fallback_vnode(name, &e.to_string())),
                    };
                }
            }
        }
        Ok(Value::Object(
            [
                ("type".to_string(), Value::String("element".to_string())),
                (
                    "tag".to_string(),
                    Value::String(match &el.tag {
                        JsxTag::Identifier { name } => name.clone(),
                        JsxTag::Member { object, property } => format!("{}.{}", object, property),
                    }),
                ),
                ("props".to_string(), Value::Object(props)),
                ("children".to_string(), Value::Array(children)),
            ]
            .into_iter()
            .collect(),
        ))
    }

    fn eval_binary(&self, l: Value, op: BinaryOp, r: Value) -> Result<Value, RuntimeError> {
        Ok(match op {
            BinaryOp::Add => match (l, r) {
                (Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                (Value::String(a), Value::String(b)) => Value::String(format!("{}{}", a, b)),
                // JS-style coercion for template-ish concatenation (`'x' + n`, `'' + value`, `'x' + flag`)
                (Value::String(a), Value::Number(b)) => Value::String(format!("{}{}", a, b)),
                (Value::Number(a), Value::String(b)) => Value::String(format!("{}{}", a, b)),
                (Value::String(a), Value::Bool(b)) => Value::String(format!("{}{}", a, b)),
                (Value::Bool(a), Value::String(b)) => Value::String(format!("{}{}", a, b)),
                _ => return Err(RuntimeError::Runtime("Invalid + operands".to_string())),
            },
            BinaryOp::Sub => match (l, r) {
                (Value::Number(a), Value::Number(b)) => Value::Number(a - b),
                _ => return Err(RuntimeError::Runtime("Invalid - operands".to_string())),
            },
            BinaryOp::Mul => match (l, r) {
                (Value::Number(a), Value::Number(b)) => Value::Number(a * b),
                _ => return Err(RuntimeError::Runtime("Invalid * operands".to_string())),
            },
            BinaryOp::Div => match (l, r) {
                (Value::Number(a), Value::Number(b)) => Value::Number(a / b),
                _ => return Err(RuntimeError::Runtime("Invalid / operands".to_string())),
            },
            BinaryOp::Mod => match (l, r) {
                (Value::Number(a), Value::Number(b)) => Value::Number(a % b),
                _ => return Err(RuntimeError::Runtime("Invalid % operands".to_string())),
            },
            BinaryOp::Eq => Value::Bool(values_equal(&l, &r)),
            BinaryOp::StrictEq => Value::Bool(values_equal(&l, &r)),
            BinaryOp::Neq => Value::Bool(!values_equal(&l, &r)),
            BinaryOp::StrictNeq => Value::Bool(!values_equal(&l, &r)),
            BinaryOp::And => Value::Bool(l.is_truthy() && r.is_truthy()),
            BinaryOp::Or => Value::Bool(l.is_truthy() || r.is_truthy()),
            BinaryOp::NullishCoalesce => {
                if matches!(&l, Value::Null) {
                    r
                } else {
                    l
                }
            }
            BinaryOp::Lt => Value::Bool(cmp_values(&l, &r, |a, b| a < b)),
            BinaryOp::Le => Value::Bool(cmp_values(&l, &r, |a, b| a <= b)),
            BinaryOp::Gt => Value::Bool(cmp_values(&l, &r, |a, b| a > b)),
            BinaryOp::Ge => Value::Bool(cmp_values(&l, &r, |a, b| a >= b)),
        })
    }

    /// Call a value as a function (e.g. setter).
    pub fn call_value(&mut self, f: &Value, args: &[Value]) -> Result<Value, RuntimeError> {
        self.call_depth += 1;
        if self.call_depth > MAX_RECURSION_DEPTH {
            self.call_depth -= 1;
            return Err(RuntimeError::Runtime(
                "Maximum recursion depth exceeded (infinite recursion or very deep component tree)".to_string(),
            ));
        }
        let result = self.call_value_inner(f, args);
        self.call_depth -= 1;
        result
    }

    fn call_value_inner(&mut self, f: &Value, args: &[Value]) -> Result<Value, RuntimeError> {
        match f {
            Value::Setter(cell) => {
                let new_val = if let Some(Value::Function { .. } | Value::Native(_) | Value::NativeFn(_)) = args.first() {
                    let f = args.first().unwrap();
                    let current = cell.borrow().clone();
                    self.call_value(f, &[current])?
                } else {
                    args.first().cloned().unwrap_or(Value::Null)
                };
                let current = cell.borrow().clone();
                if current != new_val {
                    *cell.borrow_mut() = new_val;
                    crate::run_state::set_render_requested();
                }
                Ok(Value::Null)
            }
            Value::Native(native) => native(args).map_err(RuntimeError::Runtime),
            Value::NativeFn(f) => f.0(args).map_err(RuntimeError::Runtime),
            Value::ReactHook(name) => self
                .try_dispatch_hook(name, args)?
                .ok_or_else(|| RuntimeError::Runtime(format!("Unknown hook: {}", name))),
            Value::Function {
                params,
                body,
                env,
                component_hooks,
                component_module_path,
                ..
            } => {
                let frame = component_module_path
                    .as_deref()
                    .unwrap_or_else(|| component_hooks.map(|_| "<component>").unwrap_or("function"));
                self.call_stack.push(frame.to_string());
                let child_env = Env::child(env.clone());
                let mut arg_i = 0usize;
                for param in params.iter() {
                    let type_name = param.type_annotation.as_ref().map(|t| t.name.clone());
                    if param.rest {
                        let rest: Vec<Value> = args.get(arg_i..).unwrap_or(&[]).to_vec();
                        if rest.len() > MAX_ARRAY_LENGTH {
                            return Err(RuntimeError::Runtime(format!(
                                "Rest parameter exceeds max array length {}",
                                MAX_ARRAY_LENGTH
                            )));
                        }
                        child_env
                            .borrow_mut()
                            .set_with_mutability(
                                &param.name,
                                Value::Array(rest),
                                Mutability::Var,
                                type_name,
                            )
                            .map_err(RuntimeError::Runtime)?;
                        break;
                    }
                    let raw = args.get(arg_i).cloned().unwrap_or(Value::Null);
                    arg_i += 1;
                    let val = if matches!(&raw, Value::Null) {
                        if let Some(default_expr) = &param.default {
                            let prev = std::mem::replace(&mut self.env, child_env.clone());
                            let d = self.eval_expr(default_expr);
                            self.env = prev;
                            d?
                        } else {
                            Value::Null
                        }
                    } else {
                        raw
                    };
                    child_env
                        .borrow_mut()
                        .set_with_mutability(&param.name, val, Mutability::Var, type_name)
                        .map_err(RuntimeError::Runtime)?;
                }
                let prev_env = std::mem::replace(&mut self.env, child_env);
                let prev_component = if component_hooks.is_some() {
                    std::mem::replace(&mut self.current_component, None)
                } else {
                    None
                };
                if let Some(num_hooks) = *component_hooks {
                    let instance = if let (Some(ref path), Some(ref loaded)) =
                        (component_module_path, &self.loaded_instances)
                    {
                        let mut map = loaded.borrow_mut();
                        map.entry(path.clone())
                            .or_insert_with(|| {
                                Rc::new(RefCell::new(ComponentInstance::new(
                                    "<loaded>".to_string(),
                                    num_hooks,
                                )))
                            })
                            .clone()
                    } else {
                        Rc::new(RefCell::new(ComponentInstance::new(
                            "<loaded>".to_string(),
                            num_hooks,
                        )))
                    };
                    instance.borrow_mut().hook_index = 0;
                    self.current_component = Some(instance);
                }
                let mut result = Value::Null;
                let exec_result = (|| {
                    for stmt in body {
                        match self.eval_stmt(stmt)? {
                            EvalControl::Return(v) => {
                                result = v;
                                break;
                            }
                            EvalControl::Break => {
                                return Err(RuntimeError::Runtime(
                                    "break outside of loop or switch".to_string(),
                                ));
                            }
                            EvalControl::ContinueLoop => {
                                return Err(RuntimeError::Runtime(
                                    "continue outside of loop".to_string(),
                                ));
                            }
                            EvalControl::Next => {}
                        }
                    }
                    Ok(())
                })();
                if component_hooks.is_some() {
                    self.current_component = prev_component;
                }
                self.env = prev_env;
                match exec_result {
                    Ok(()) => {
                        self.call_stack.pop();
                        Ok(result)
                    }
                    Err(e) => Err(self.enrich_err(e)),
                }
            }
            _ => Err(self.err("Not a function")),
        }
    }

    /// Evaluate a statement (internal; used by `run` / component bodies).
    fn eval_stmt(&mut self, stmt: &Statement) -> Result<EvalControl, RuntimeError> {
        match stmt {
            Statement::VariableDecl {
                kind,
                declarations,
            } => {
                let mutability = match kind {
                    VarKind::Const => Mutability::Const,
                    VarKind::Let => Mutability::Let,
                    VarKind::Var => Mutability::Var,
                };
                for decl in declarations {
                    let val = decl
                        .init
                        .as_ref()
                        .map(|e| self.eval_expr(e))
                        .transpose()?
                        .unwrap_or(Value::Null);
                    if let Pattern::Identifier { name, type_annotation } = &decl.id {
                        let type_name = type_annotation.as_ref().map(|t| t.name.clone());
                        self.env
                            .borrow_mut()
                            .set_with_mutability(name, val, mutability, type_name)
                            .map_err(RuntimeError::Runtime)?;
                    } else if let Pattern::Array { elements } = &decl.id {
                        if let Value::Array(arr) = val {
                            for (i, elem) in elements.iter().enumerate() {
                                if let Some(Pattern::Identifier { name, .. }) = elem.as_ref() {
                                    let v = arr.get(i).cloned().unwrap_or(Value::Null);
                                    self.env
                                        .borrow_mut()
                                        .set_with_mutability(name, v, mutability, None)
                                        .map_err(RuntimeError::Runtime)?;
                                }
                            }
                        }
                    } else if let Pattern::Object { properties } = &decl.id {
                        let obj = match &val {
                            Value::Object(o) => Some(o),
                            _ => None,
                        };
                        for prop in properties {
                            let bind_name = match &prop.value {
                                Some(p) => match p.as_ref() {
                                    Pattern::Identifier { name, .. } => name.as_str(),
                                    _ => {
                                        return Err(RuntimeError::Runtime(
                                            "CleverScript: nested object destructuring not supported"
                                                .to_string(),
                                        ));
                                    }
                                },
                                None => prop.key.as_str(),
                            };
                            let v = obj
                                .and_then(|o| o.get(&prop.key).cloned())
                                .unwrap_or(Value::Null);
                            self.env
                                .borrow_mut()
                                .set_with_mutability(bind_name, v, mutability, None)
                                .map_err(RuntimeError::Runtime)?;
                        }
                    }
                }
            }
            Statement::Return { argument } => {
                let val = argument
                    .as_ref()
                    .map(|e| self.eval_expr(e))
                    .transpose()?
                    .unwrap_or(Value::Null);
                return Ok(EvalControl::Return(val));
            }
            Statement::Break => return Ok(EvalControl::Break),
            Statement::Continue => return Ok(EvalControl::ContinueLoop),
            Statement::Expr { expression } => {
                self.eval_expr(expression)?;
            }
            Statement::If {
                test,
                consequent,
                alternate,
            } => {
                let t = self.eval_expr(test)?;
                let stmts = if t.is_truthy() {
                    consequent
                } else if let Some(alt) = alternate {
                    alt
                } else {
                    &[][..]
                };
                let block_env = Env::child(self.env.clone());
                let prev_env = std::mem::replace(&mut self.env, block_env);
                for s in stmts {
                    match self.eval_stmt(s)? {
                        EvalControl::Return(v) => {
                            self.env = prev_env;
                            return Ok(EvalControl::Return(v));
                        }
                        EvalControl::Break => {
                            self.env = prev_env;
                            return Ok(EvalControl::Break);
                        }
                        EvalControl::ContinueLoop => {
                            self.env = prev_env;
                            return Ok(EvalControl::ContinueLoop);
                        }
                        EvalControl::Next => {}
                    }
                }
                self.env = prev_env;
            }
            Statement::For {
                init,
                test,
                update,
                body,
            } => {
                let block_env = Env::child(self.env.clone());
                let prev_env = std::mem::replace(&mut self.env, block_env);
                if let Some(i) = init {
                    match self.eval_stmt(i)? {
                        EvalControl::Return(v) => {
                            self.env = prev_env;
                            return Ok(EvalControl::Return(v));
                        }
                        EvalControl::Break | EvalControl::ContinueLoop => {
                            self.env = prev_env;
                            return Err(RuntimeError::Runtime(
                                "break/continue in for-init not allowed".to_string(),
                            ));
                        }
                        EvalControl::Next => {}
                    }
                }
                let mut iterations: u64 = 0;
                loop {
                    iterations += 1;
                    if iterations > MAX_LOOP_ITERATIONS {
                        self.env = prev_env;
                        return Err(RuntimeError::Runtime(format!(
                            "Loop exceeded {} iterations (possible infinite loop)",
                            MAX_LOOP_ITERATIONS
                        )));
                    }
                    if let Some(t) = test {
                        let cond = self.eval_expr(t)?;
                        if !cond.is_truthy() {
                            break;
                        }
                    }
                    let mut broken = false;
                    for s in body {
                        match self.eval_stmt(s)? {
                            EvalControl::Return(v) => {
                                self.env = prev_env;
                                return Ok(EvalControl::Return(v));
                            }
                            EvalControl::Break => {
                                broken = true;
                                break;
                            }
                            EvalControl::ContinueLoop => break,
                            EvalControl::Next => {}
                        }
                    }
                    if broken {
                        break;
                    }
                    if let Some(u) = update {
                        self.eval_expr(u)?;
                    }
                }
                self.env = prev_env;
            }
            Statement::ForOf {
                kind,
                id,
                right,
                body,
            } => {
                let iterable = self.eval_expr(right)?;
                let arr = match iterable {
                    Value::Array(a) => a,
                    _ => {
                        return Err(RuntimeError::Runtime(
                            "for...of requires an array".to_string(),
                        ));
                    }
                };
                let mutability = match kind {
                    VarKind::Const => Mutability::Const,
                    VarKind::Let => Mutability::Let,
                    VarKind::Var => Mutability::Var,
                };
                let mut iterations: u64 = 0;
                for item in arr {
                    iterations += 1;
                    if iterations > MAX_LOOP_ITERATIONS {
                        return Err(RuntimeError::Runtime(format!(
                            "Loop exceeded {} iterations (possible infinite loop)",
                            MAX_LOOP_ITERATIONS
                        )));
                    }
                    let block_env = Env::child(self.env.clone());
                    block_env
                        .borrow_mut()
                        .set_with_mutability(id, item, mutability, None)
                        .map_err(RuntimeError::Runtime)?;
                    let prev_env = std::mem::replace(&mut self.env, block_env);
                    let mut broken = false;
                    for s in body {
                        match self.eval_stmt(s)? {
                            EvalControl::Return(v) => {
                                self.env = prev_env;
                                return Ok(EvalControl::Return(v));
                            }
                            EvalControl::Break => {
                                broken = true;
                                break;
                            }
                            EvalControl::ContinueLoop => break,
                            EvalControl::Next => {}
                        }
                    }
                    self.env = prev_env;
                    if broken {
                        break;
                    }
                }
            }
            Statement::Switch {
                discriminant,
                cases,
            } => {
                let disc = self.eval_expr(discriminant)?;
                let mut matched_default: Option<&[Statement]> = None;
                let mut ran = false;
                for case in cases {
                    match &case.test {
                        Some(t) => {
                            let tv = self.eval_expr(t)?;
                            if values_equal(&disc, &tv) {
                                let block_env = Env::child(self.env.clone());
                                let prev_env = std::mem::replace(&mut self.env, block_env);
                                let mut early = false;
                                for s in &case.consequent {
                                    match self.eval_stmt(s)? {
                                        EvalControl::Return(v) => {
                                            self.env = prev_env.clone();
                                            return Ok(EvalControl::Return(v));
                                        }
                                        EvalControl::Break => {
                                            early = true;
                                            break;
                                        }
                                        EvalControl::ContinueLoop => {
                                            self.env = prev_env.clone();
                                            return Ok(EvalControl::ContinueLoop);
                                        }
                                        EvalControl::Next => {}
                                    }
                                }
                                self.env = prev_env;
                                ran = true;
                                let _ = early;
                                break;
                            }
                        }
                        None => {
                            matched_default = Some(&case.consequent);
                        }
                    }
                    if ran {
                        break;
                    }
                }
                if !ran {
                    if let Some(body) = matched_default {
                        let block_env = Env::child(self.env.clone());
                        let prev_env = std::mem::replace(&mut self.env, block_env);
                        for s in body {
                            match self.eval_stmt(s)? {
                                EvalControl::Return(v) => {
                                    self.env = prev_env.clone();
                                    return Ok(EvalControl::Return(v));
                                }
                                EvalControl::Break => break,
                                EvalControl::ContinueLoop => {
                                    self.env = prev_env.clone();
                                    return Ok(EvalControl::ContinueLoop);
                                }
                                EvalControl::Next => {}
                            }
                        }
                        self.env = prev_env;
                    }
                }
            }
            Statement::Try {
                block,
                handler,
                finalizer,
            } => {
                let block_env = Env::child(self.env.clone());
                let prev_env = std::mem::replace(&mut self.env, block_env);
                let mut control = EvalControl::Next;
                let try_result: Result<(), RuntimeError> = (|| {
                    for s in block {
                        match self.eval_stmt(s)? {
                            EvalControl::Return(v) => {
                                control = EvalControl::Return(v);
                                return Ok(());
                            }
                            EvalControl::Break => {
                                control = EvalControl::Break;
                                return Ok(());
                            }
                            EvalControl::ContinueLoop => {
                                control = EvalControl::ContinueLoop;
                                return Ok(());
                            }
                            EvalControl::Next => {}
                        }
                    }
                    Ok(())
                })();
                self.env = prev_env;

                if let Err(err) = try_result {
                    if let Some(h) = handler {
                        let catch_env = Env::child(self.env.clone());
                        if let Some(param) = &h.param {
                            catch_env
                                .borrow_mut()
                                .set_with_mutability(
                                    param,
                                    Value::String(err.to_string()),
                                    Mutability::Const,
                                    None,
                                )
                                .map_err(RuntimeError::Runtime)?;
                        }
                        let prev = std::mem::replace(&mut self.env, catch_env);
                        for s in &h.body {
                            match self.eval_stmt(s)? {
                                EvalControl::Return(v) => {
                                    control = EvalControl::Return(v);
                                    break;
                                }
                                EvalControl::Break => {
                                    control = EvalControl::Break;
                                    break;
                                }
                                EvalControl::ContinueLoop => {
                                    control = EvalControl::ContinueLoop;
                                    break;
                                }
                                EvalControl::Next => {}
                            }
                        }
                        self.env = prev;
                    } else if finalizer.is_none() {
                        return Err(err);
                    }
                }

                if let Some(fin) = finalizer {
                    let fin_env = Env::child(self.env.clone());
                    let prev = std::mem::replace(&mut self.env, fin_env);
                    for s in fin {
                        match self.eval_stmt(s)? {
                            EvalControl::Return(v) => {
                                self.env = prev;
                                return Ok(EvalControl::Return(v));
                            }
                            EvalControl::Break => {
                                control = EvalControl::Break;
                                break;
                            }
                            EvalControl::ContinueLoop => {
                                control = EvalControl::ContinueLoop;
                                break;
                            }
                            EvalControl::Next => {}
                        }
                    }
                    self.env = prev;
                }

                return Ok(control);
            }
            Statement::While { test, body } => {
                let mut iterations: u64 = 0;
                while self.eval_expr(test)?.is_truthy() {
                    iterations += 1;
                    if iterations > MAX_LOOP_ITERATIONS {
                        return Err(RuntimeError::Runtime(format!(
                            "Loop exceeded {} iterations (possible infinite loop)",
                            MAX_LOOP_ITERATIONS
                        )));
                    }
                    let block_env = Env::child(self.env.clone());
                    let prev_env = std::mem::replace(&mut self.env, block_env);
                    let mut broken = false;
                    for s in body {
                        match self.eval_stmt(s)? {
                            EvalControl::Return(v) => {
                                self.env = prev_env;
                                return Ok(EvalControl::Return(v));
                            }
                            EvalControl::Break => {
                                broken = true;
                                break;
                            }
                            EvalControl::ContinueLoop => break,
                            EvalControl::Next => {}
                        }
                    }
                    self.env = prev_env;
                    if broken {
                        break;
                    }
                }
            }
            _ => {}
        }
        Ok(EvalControl::Next)
    }

    /// Dispatch hook calls to component's hook slots.
    fn try_dispatch_hook(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        if !["useState", "useEffect", "useCallback", "useRef"].contains(&name) {
            return Ok(None);
        }
        let comp = match &self.current_component {
            Some(c) => c.clone(),
            None => return Ok(None),
        };
        let mut comp = comp.borrow_mut();
        let slot_idx = comp.hook_index;
        if slot_idx >= comp.hook_slots.len() {
            return Err(RuntimeError::Runtime(
                "Too many hooks (more than analysis)".to_string(),
            ));
        }
        comp.hook_index += 1;

        let result = match name {
            "useState" => {
                let initial = args.first().cloned().unwrap_or(Value::Null);
                let slot = &comp.hook_slots[slot_idx];
                let cell = match slot {
                    HookSlot::State(c) => {
                        if matches!(&*c.borrow(), Value::Null) {
                            *c.borrow_mut() = initial.clone();
                        }
                        c.clone()
                    }
                    _ => return Err(RuntimeError::Runtime("Hook slot type mismatch".to_string())),
                };
                let current = cell.borrow().clone();
                let setter = Value::Setter(cell);
                Value::Array(vec![current, setter])
            }
            "useEffect" => {
                let callback = args.first().cloned().unwrap_or(Value::Null);
                let deps = args.get(1).cloned().unwrap_or(Value::Array(vec![]));
                let deps_vec = match deps {
                    Value::Array(a) => a,
                    _ => vec![],
                };
                if !matches!(
                    comp.hook_slots[slot_idx],
                    HookSlot::Effect { .. }
                ) {
                    comp.hook_slots[slot_idx] = HookSlot::Effect {
                        deps: Vec::new(),
                        ran: false,
                    };
                }
                let slot = &mut comp.hook_slots[slot_idx];
                let HookSlot::Effect { deps: stored, ran } = slot else {
                    return Err(RuntimeError::Runtime("Hook slot type mismatch".to_string()));
                };
                let should_run = !*ran || stored.as_slice() != deps_vec.as_slice();
                if should_run {
                    *stored = deps_vec;
                    *ran = true;
                    if let Value::Function { .. } = callback {
                        let _ = self.call_value(&callback, &[]);
                    }
                }
                Value::Null
            }
            "useCallback" => {
                let callback = args.first().cloned().unwrap_or(Value::Null);
                callback
            }
            "useRef" => {
                let initial = args.first().cloned().unwrap_or(Value::Null);
                let slot = &comp.hook_slots[slot_idx];
                let cell = match slot {
                    HookSlot::State(c) => {
                        if matches!(&*c.borrow(), Value::Null) {
                            let mut obj = HashMap::new();
                            obj.insert("current".to_string(), initial.clone());
                            *c.borrow_mut() = Value::Object(obj);
                        }
                        c.clone()
                    }
                    _ => return Err(RuntimeError::Runtime("Hook slot type mismatch for useRef".to_string())),
                };
                let v = cell.borrow().clone();
                v
            }
            _ => return Ok(None),
        };
        drop(comp);
        Ok(Some(result))
    }

    /// Dispatch host calls: fetch, navigate, createElement.
    fn try_dispatch_host(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        let host = match &self.host {
            Some(h) => h.clone(),
            None => return Ok(None),
        };
        let result = match name {
            "fetch" => {
                let url = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    _ => return Err(RuntimeError::Host("fetch requires url string".to_string())),
                };
                let options = args.get(1).cloned();
                Some(host.borrow().fetch(&url, options.as_ref()).map_err(RuntimeError::Host)?)
            }
            "navigate" => {
                let path = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    _ => return Err(RuntimeError::Host("navigate requires path string".to_string())),
                };
                host.borrow().navigate(&path).map_err(RuntimeError::Host)?;
                Some(Value::Null)
            }
            "createElement" => {
                let tag = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    _ => return Err(RuntimeError::Host("createElement requires tag".to_string())),
                };
                let props = args.get(1).cloned().unwrap_or(Value::Object(Default::default()));
                let children = match args.get(2) {
                    Some(Value::Array(c)) => c.clone(),
                    Some(v) => vec![v.clone()],
                    None => vec![],
                };
                Some(
                    host.borrow()
                        .create_element(&tag, &props, &children)
                        .map_err(RuntimeError::Host)?,
                )
            }
            _ => return Ok(None),
        };
        Ok(result)
    }

    /// Array instance methods: arr.map(fn), arr.filter(fn), arr.find(fn), arr.concat(arr2)
    fn array_method(&mut self, name: &str, arr: &[Value], args: &[Value]) -> Result<Option<Value>, RuntimeError> {
        let cb = match name {
            "map" | "filter" | "find" => args.first().cloned(),
            "concat" => None,
            _ => return Ok(None),
        };
        let result = match name {
            "map" => {
                if arr.len() > MAX_ARRAY_LENGTH {
                    return Err(RuntimeError::Runtime(format!(
                        "Array length {} exceeds max {}",
                        arr.len(),
                        MAX_ARRAY_LENGTH
                    )));
                }
                let f = cb.ok_or_else(|| RuntimeError::Runtime("map requires callback".to_string()))?;
                let mut out = Vec::with_capacity(arr.len());
                for (i, v) in arr.iter().enumerate() {
                    let r = self.call_value(&f, &[v.clone(), Value::Number(i as f64)])?;
                    out.push(r);
                }
                Value::Array(out)
            }
            "filter" => {
                if arr.len() > MAX_ARRAY_LENGTH {
                    return Err(RuntimeError::Runtime(format!(
                        "Array length {} exceeds max {}",
                        arr.len(),
                        MAX_ARRAY_LENGTH
                    )));
                }
                let f = cb.ok_or_else(|| RuntimeError::Runtime("filter requires callback".to_string()))?;
                let mut out = Vec::new();
                for (i, v) in arr.iter().enumerate() {
                    let r = self.call_value(&f, &[v.clone(), Value::Number(i as f64)])?;
                    if r.is_truthy() {
                        out.push(v.clone());
                    }
                }
                Value::Array(out)
            }
            "find" => {
                if arr.len() > MAX_ARRAY_LENGTH {
                    return Err(RuntimeError::Runtime(format!(
                        "Array length {} exceeds max {}",
                        arr.len(),
                        MAX_ARRAY_LENGTH
                    )));
                }
                let f = cb.ok_or_else(|| RuntimeError::Runtime("find requires callback".to_string()))?;
                for (i, v) in arr.iter().enumerate() {
                    let r = self.call_value(&f, &[v.clone(), Value::Number(i as f64)])?;
                    if r.is_truthy() {
                        return Ok(Some(v.clone()));
                    }
                }
                Value::Null
            }
            "concat" => {
                let mut out = arr.to_vec();
                for a in args {
                    if let Value::Array(b) = a {
                        if out.len() + b.len() > MAX_ARRAY_LENGTH {
                            return Err(RuntimeError::Runtime(format!(
                                "concat would exceed max array length {}",
                                MAX_ARRAY_LENGTH
                            )));
                        }
                        out.extend(b.iter().cloned());
                    } else {
                        if out.len() >= MAX_ARRAY_LENGTH {
                            return Err(RuntimeError::Runtime(format!(
                                "concat would exceed max array length {}",
                                MAX_ARRAY_LENGTH
                            )));
                        }
                        out.push(a.clone());
                    }
                }
                Value::Array(out)
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// String instance methods (JS subset for hooks).
    fn string_method(&mut self, name: &str, s: &str, _args: &[Value]) -> Result<Option<Value>, RuntimeError> {
        let result = match name {
            "split" => {
                let sep = _args
                    .first()
                    .and_then(|v| if let Value::String(x) = v { Some(x.as_str()) } else { None })
                    .unwrap_or("");
                let parts: Vec<Value> = if sep.is_empty() {
                    s.chars().map(|c| Value::String(c.to_string())).collect()
                } else {
                    s.split(sep).map(|p| Value::String(p.to_string())).collect()
                };
                Value::Array(parts)
            }
            "trim" => Value::String(s.trim().to_string()),
            "startsWith" => {
                let prefix = _args
                    .first()
                    .and_then(|v| if let Value::String(x) = v { Some(x.as_str()) } else { None })
                    .unwrap_or("");
                Value::Bool(s.starts_with(prefix))
            }
            "indexOf" => {
                let sub = _args
                    .first()
                    .and_then(|v| if let Value::String(x) = v { Some(x.as_str()) } else { None })
                    .unwrap_or("");
                let start = _args
                    .get(1)
                    .and_then(|v| if let Value::Number(n) = v { Some(*n as isize) } else { None })
                    .unwrap_or(0);
                let cu = s.encode_utf16().count() as isize;
                let idx = if sub.is_empty() {
                    let mut fi = start;
                    if fi < 0 {
                        fi = cu + fi;
                    }
                    fi.clamp(0, cu) as f64
                } else if start <= 0 {
                    s.find(sub).map(|i| i as f64).unwrap_or(-1.0)
                } else {
                    s.get(start.max(0) as usize..)
                        .and_then(|tail| tail.find(sub))
                        .map(|i| (start.max(0) as usize + i) as f64)
                        .unwrap_or(-1.0)
                };
                Value::Number(idx)
            }
            "lastIndexOf" => {
                let sub = _args
                    .first()
                    .and_then(|v| if let Value::String(x) = v { Some(x.as_str()) } else { None })
                    .unwrap_or("");
                if sub.is_empty() {
                    Value::Number(s.encode_utf16().count() as f64)
                } else if let Some(i) = s.rfind(sub) {
                    Value::Number(i as f64)
                } else {
                    Value::Number(-1.0)
                }
            }
            "slice" => {
                let len_i = s.encode_utf16().count() as isize;
                let start_raw = _args
                    .first()
                    .and_then(|v| if let Value::Number(n) = v { Some(*n as isize) } else { None })
                    .unwrap_or(0);
                let end_raw = _args
                    .get(1)
                    .and_then(|v| if let Value::Number(n) = v { Some(*n as isize) } else { None });
                let mut start = start_raw;
                let mut end = end_raw.unwrap_or(len_i);
                if start < 0 {
                    start = len_i + start;
                }
                if end < 0 {
                    end = len_i + end;
                }
                start = start.clamp(0, len_i);
                end = end.clamp(0, len_i);
                if start > end {
                    Value::String(String::new())
                } else {
                    let out = utf16_slice_by_units(s, start as usize, end as usize);
                    Value::String(out)
                }
            }
            "charAt" => {
                let pos = _args
                    .first()
                    .and_then(|v| if let Value::Number(n) = v { Some(*n as isize) } else { None })
                    .unwrap_or(0);
                let units: Vec<u16> = s.encode_utf16().collect();
                let len = units.len() as isize;
                let mut i = pos;
                if i < 0 {
                    i = len + i;
                }
                let ui = i as usize;
                let out = if i < 0 || i >= len {
                    String::new()
                } else if ui + 1 < units.len()
                    && (0xD800..=0xDBFF).contains(&units[ui])
                    && (0xDC00..=0xDFFF).contains(&units[ui + 1])
                {
                    String::from_utf16_lossy(&units[ui..ui + 2])
                } else if ui < units.len() {
                    String::from_utf16_lossy(&units[ui..ui + 1])
                } else {
                    String::new()
                };
                Value::String(out)
            }
            "toLowerCase" => Value::String(s.to_lowercase()),
            "toUpperCase" => Value::String(s.to_uppercase()),
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// Process top-level import declarations and populate env.
    fn process_imports(&mut self) -> Result<(), RuntimeError> {
        for stmt in &self.program.body {
            if let Statement::ImportDecl {
                default_binding,
                named_bindings,
                source,
            } = stmt
            {
                let host = self.host.as_ref().ok_or_else(|| {
                    RuntimeError::Runtime("import requires host with load_module".to_string())
                })?;
                let module = host
                    .borrow()
                    .load_module(source)
                    .map_err(|e| RuntimeError::Host(e))?;
                if let Some(name) = default_binding {
                    // Default import: `module.default` when present and non-null; else whole module
                    // (JSON hooks manifests often omit `default`; `"default": null` must not win over the object.)
                    let bound = match &module {
                        Value::Object(o) => match o.get("default") {
                            Some(v) if !matches!(v, Value::Null) => v.clone(),
                            _ => module.clone(),
                        },
                        _ => module.clone(),
                    };
                    self.env.borrow_mut().set(name, bound);
                }
                for (local, imported) in named_bindings {
                    let val = match &module {
                        Value::Object(o) => o.get(imported).cloned(),
                        _ => None,
                    };
                    if let Some(v) = val {
                        self.env.borrow_mut().set(local, v);
                    }
                }
            }
        }
        Ok(())
    }

    /// Bind top-level `function name() {}` and `export function name() {}` into module env so hooks can use sibling helpers.
    fn process_top_level_functions(&mut self) {
        for stmt in &self.program.body {
            match stmt {
                Statement::FunctionDecl {
                    id: Some(name),
                    params,
                    body,
                    r#async,
                } => {
                    let f = Value::Function {
                        params: params.clone(),
                        body: body.clone(),
                        env: self.env.clone(),
                        is_async: *r#async,
                        component_hooks: None,
                        component_module_path: None,
                    };
                    self.env.borrow_mut().set(name, f);
                }
                Statement::ExportNamed {
                    declaration: ExportNamedDecl::Function { id, params, body },
                } => {
                    let f = Value::Function {
                        params: params.clone(),
                        body: body.clone(),
                        env: self.env.clone(),
                        is_async: false,
                        component_hooks: None,
                        component_module_path: None,
                    };
                    self.env.borrow_mut().set(id, f);
                }
                _ => {}
            }
        }
    }

    fn run_default_export_body(&mut self, body: &[Statement]) -> Result<Value, RuntimeError> {
        let mut result = Value::Null;
        for s in body {
            match self.eval_stmt(s)? {
                EvalControl::Return(v) => {
                    result = v;
                    break;
                }
                EvalControl::Break => {
                    return Err(RuntimeError::Runtime(
                        "break outside of loop or switch".to_string(),
                    ));
                }
                EvalControl::ContinueLoop => {
                    return Err(RuntimeError::Runtime(
                        "continue outside of loop".to_string(),
                    ));
                }
                EvalControl::Next => {}
            }
        }
        Ok(result)
    }

    /// Run the default export and return the result.
    pub fn run(&mut self) -> Result<Value, RuntimeError> {
        self.process_imports()?;
        self.process_top_level_functions();
        let default_name = self
            .analysis
            .default_export
            .clone()
            .ok_or_else(|| RuntimeError::Runtime("No default export".to_string()))?;
        let hook_calls = self.analysis.hook_calls.get(&default_name);
        let num_hooks = hook_calls.map(|v| v.len()).unwrap_or(0);

        if self.default_instance.is_none() {
            let instance = if let Some(calls) = hook_calls {
                ComponentInstance::with_hook_calls(default_name.clone(), calls)
            } else {
                ComponentInstance::new(default_name.clone(), num_hooks)
            };
            self.default_instance = Some(Rc::new(RefCell::new(instance)));
        }
        let instance = self.default_instance.as_ref().unwrap().clone();
        instance.borrow_mut().hook_index = 0;
        self.current_component = Some(instance);

        let body = self.program.body.iter().find_map(|stmt| {
            if let Statement::ExportDefault { declaration } = stmt {
                if let ExportDefaultDecl::Function { body, .. } = declaration.as_ref() {
                    return Some(body.clone());
                }
            }
            None
        });
        if let Some(body) = body {
            self.call_stack.push(default_name.clone());
            let run_result = self.run_default_export_body(&body);
            self.current_component = None;
            return match run_result {
                Ok(v) => {
                    self.call_stack.pop();
                    Ok(v)
                }
                Err(e) => Err(self.enrich_err(e)),
            };
        }
        self.current_component = None;
        Err(self.err("Default export not found"))
    }

    /// Get module exports for a given path. Sets component_module_path so instances persist across re-renders.
    pub fn get_module_exports_for_path(&mut self, path: &str) -> Result<Value, RuntimeError> {
        let mut exports = self.get_module_exports()?;
        if let Value::Object(ref mut o) = exports {
            for value in o.values_mut() {
                if let Value::Function {
                    params,
                    body,
                    env,
                    is_async,
                    component_hooks,
                    ..
                } = value
                {
                    *value = Value::Function {
                        params: params.clone(),
                        body: body.clone(),
                        env: env.clone(),
                        is_async: *is_async,
                        component_hooks: *component_hooks,
                        component_module_path: Some(path.to_string()),
                    };
                }
            }
        }
        Ok(exports)
    }

    /// Get module exports (default + named export functions) without running the component.
    /// Used by load_module when a hook does import(path).
    pub fn get_module_exports(&mut self) -> Result<Value, RuntimeError> {
        self.process_imports()?;
        self.process_top_level_functions();
        let mut exports = HashMap::new();

        for stmt in &self.program.body {
            if let Statement::ExportNamed {
                declaration: ExportNamedDecl::Function { id, params, body },
            } = stmt
            {
                let num_hooks = self
                    .analysis
                    .hook_calls
                    .get(id)
                    .map(|v| v.len())
                    .unwrap_or(0);
                exports.insert(
                    id.clone(),
                    Value::Function {
                        params: params.clone(),
                        body: body.clone(),
                        env: self.env.clone(),
                        is_async: false,
                        component_hooks: if num_hooks > 0 { Some(num_hooks) } else { None },
                        component_module_path: None,
                    },
                );
            }
        }

        if let Some((params, body, is_async)) = self.program.body.iter().find_map(|stmt| {
            if let Statement::ExportDefault { declaration } = stmt {
                if let ExportDefaultDecl::Function {
                    params,
                    body,
                    r#async,
                    ..
                } = declaration.as_ref()
                {
                    return Some((params.clone(), body.clone(), *r#async));
                }
            }
            None
        }) {
            let default_name = self
                .analysis
                .default_export
                .clone()
                .unwrap_or_else(|| "<default>".to_string());
            let num_hooks = self
                .analysis
                .hook_calls
                .get(&default_name)
                .map(|v| v.len())
                .unwrap_or(0);
            exports.insert(
                "default".to_string(),
                Value::Function {
                    params,
                    body,
                    env: self.env.clone(),
                    is_async,
                    component_hooks: Some(num_hooks),
                    component_module_path: None,
                },
            );
        }

        if exports.is_empty() {
            return Err(RuntimeError::Runtime(
                "Module has no default or named exports".to_string(),
            ));
        }
        Ok(Value::Object(exports))
    }
}

/// Serialize console.log arguments similarly to browser consoles (space-separated).
fn format_console_message(args: &[Value]) -> String {
    args.iter()
        .map(|v| match v {
            Value::String(s) => s.clone(),
            _ => serde_json::to_string(&v.to_json()).unwrap_or_else(|_| format!("{:?}", v)),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        _ => false,
    }
}

fn cmp_values<F>(a: &Value, b: &Value, cmp: F) -> bool
where
    F: FnOnce(f64, f64) -> bool,
{
    let (x, y) = match (a, b) {
        (Value::Number(a), Value::Number(b)) => (*a, *b),
        (Value::Number(a), Value::String(b)) => (*a, b.len() as f64),
        (Value::String(a), Value::Number(b)) => (a.len() as f64, *b),
        (Value::String(a), Value::String(b)) => (a.len() as f64, b.len() as f64),
        (Value::Array(a), Value::Number(b)) => (a.len() as f64, *b),
        (Value::Number(a), Value::Array(b)) => (*a, b.len() as f64),
        _ => return false,
    };
    cmp(x, y)
}
