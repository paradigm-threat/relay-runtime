//! Hooks runtime: interpret AST with useState, useEffect, useCallback semantics.

use crate::analysis::AnalysisResult;
use crate::ast::*;
use crate::error::RuntimeError;
use crate::value::{Env, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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
}

impl Runtime {
    pub fn new(program: Program, analysis: AnalysisResult) -> Self {
        Self {
            env: Env::new(),
            analysis,
            program,
            current_component: None,
            default_instance: None,
            host: None,
        }
    }

    /// Attach a host for fetch, navigate, createElement, load_module.
    pub fn with_host(mut self, host: crate::host::HostHandle) -> Self {
        self.host = Some(host);
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
            Expr::Identifier { name } => self.env
                .borrow()
                .get(name)
                .ok_or_else(|| RuntimeError::Runtime(format!("Undefined: {}", name))),
            Expr::Call { callee, arguments } => {
                let args: Vec<Value> = arguments
                    .iter()
                    .map(|a| self.eval_expr(a))
                    .collect::<Result<_, _>>()?;
                if let Expr::Identifier { name } = callee.as_ref() {
                    if let Some(result) = self.try_dispatch_host(name, &args)? {
                        return Ok(result);
                    }
                }
                let f = self.eval_expr(callee)?;
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
                } else {
                    let l = self.eval_expr(left)?;
                    let r = self.eval_expr(right)?;
                    self.eval_binary(l, *op, r)
                }
            }
            Expr::Array { elements } => {
                let vals: Vec<Value> = elements
                    .iter()
                    .map(|e| self.eval_expr(e))
                    .collect::<Result<_, _>>()?;
                Ok(Value::Array(vals))
            }
            Expr::Object { properties } => {
                let mut obj = HashMap::new();
                for p in properties {
                    match p {
                        ObjectExprProp::KeyValue { key, value } => {
                            obj.insert(key.clone(), self.eval_expr(value)?);
                        }
                        ObjectExprProp::Spread { .. } => {
                            return Err(RuntimeError::Runtime(
                                "CleverScript: object spread not supported".to_string(),
                            ));
                        }
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
                        if key == "length" {
                            // Array-like objects (e.g. from JSON): synthetic length from numeric keys
                            let numeric_keys: Vec<usize> = o.keys().filter_map(|k| k.parse().ok()).collect();
                            let len = if numeric_keys.is_empty() {
                                0
                            } else {
                                (*numeric_keys.iter().max().unwrap() + 1).max(numeric_keys.len())
                            };
                            Ok(o.get(&key)
                                .cloned()
                                .unwrap_or_else(|| Value::Number(len as f64)))
                        } else {
                            o.get(&key)
                                .cloned()
                                .ok_or_else(|| RuntimeError::Runtime(format!("Property '{}' not found", key)))
                        }
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
                    self.env.borrow_mut().set(name, new_val);
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
                    self.env.borrow_mut().set(name, new_val);
                }
                Ok(old)
            }
            Expr::Assign { left, right } => {
                let val = self.eval_expr(right)?;
                match left.as_ref() {
                    Expr::Identifier { name } => {
                        self.env.borrow_mut().set(name, val.clone());
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
                            self.env.borrow_mut().set(name, obj);
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
            _ => return Err(RuntimeError::Runtime("Unsupported binary op".to_string())),
        })
    }

    /// Call a value as a function (e.g. setter).
    pub fn call_value(&mut self, f: &Value, args: &[Value]) -> Result<Value, RuntimeError> {
        match f {
            Value::Setter(cell) => {
                let new_val = args.first().cloned().unwrap_or(Value::Null);
                *cell.borrow_mut() = new_val;
                Ok(Value::Null)
            }
            Value::Native(native) => native(args).map_err(RuntimeError::Runtime),
            Value::ReactHook(name) => self
                .try_dispatch_hook(name, args)?
                .ok_or_else(|| RuntimeError::Runtime(format!("Unknown hook: {}", name))),
            Value::Function {
                params,
                body,
                env,
                component_hooks,
                ..
            } => {
                let child_env = Env::child(env.clone());
                for (i, param) in params.iter().enumerate() {
                    let val = args.get(i).cloned().unwrap_or(Value::Null);
                    child_env.borrow_mut().set(&param.name, val);
                }
                let prev_env = std::mem::replace(&mut self.env, child_env);
                let prev_component = if component_hooks.is_some() {
                    std::mem::replace(&mut self.current_component, None)
                } else {
                    None
                };
                if let Some(num_hooks) = *component_hooks {
                    let instance = Rc::new(RefCell::new(ComponentInstance::new(
                        "<loaded>".to_string(),
                        num_hooks,
                    )));
                    self.current_component = Some(instance);
                }
                let mut result = Value::Null;
                for stmt in body {
                    if let Statement::Return { argument } = stmt {
                        result = argument
                            .as_ref()
                            .map(|e| self.eval_expr(e))
                            .transpose()?
                            .unwrap_or(Value::Null);
                        break;
                    }
                    self.eval_stmt(stmt)?;
                }
                if component_hooks.is_some() {
                    self.current_component = prev_component;
                }
                self.env = prev_env;
                Ok(result)
            }
            _ => Err(RuntimeError::Runtime("Not a function".to_string())),
        }
    }

    /// Evaluate a statement.
    pub fn eval_stmt(&mut self, stmt: &Statement) -> Result<(), RuntimeError> {
        match stmt {
            Statement::VariableDecl {
                kind: _,
                declarations,
            } => {
                for decl in declarations {
                    let val = decl
                        .init
                        .as_ref()
                        .map(|e| self.eval_expr(e))
                        .transpose()?
                        .unwrap_or(Value::Null);
                    if let Pattern::Identifier { name } = &decl.id {
                        self.env.borrow_mut().set(name, val);
                    } else if let Pattern::Array { elements } = &decl.id {
                        if let Value::Array(arr) = val {
                            for (i, elem) in elements.iter().enumerate() {
                                if let Some(Pattern::Identifier { name }) = elem.as_ref() {
                                    let v = arr.get(i).cloned().unwrap_or(Value::Null);
                                    self.env.borrow_mut().set(name, v);
                                }
                            }
                        }
                    }
                }
            }
            Statement::Return { .. } => {}
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
                for s in stmts {
                    self.eval_stmt(s)?;
                }
            }
            Statement::For {
                init,
                test,
                update,
                body,
            } => {
                if let Some(i) = init {
                    self.eval_stmt(i)?;
                }
                loop {
                    if let Some(t) = test {
                        let cond = self.eval_expr(t)?;
                        if !cond.is_truthy() {
                            break;
                        }
                    }
                    for s in body {
                        self.eval_stmt(s)?;
                    }
                    if let Some(u) = update {
                        self.eval_expr(u)?;
                    }
                }
            }
            Statement::While { test, body } => {
                while self.eval_expr(test)?.is_truthy() {
                    for s in body {
                        self.eval_stmt(s)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Dispatch hook calls to component's hook slots.
    fn try_dispatch_hook(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        if !["useState", "useEffect", "useCallback"].contains(&name) {
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
            "useEffect" | "useCallback" => {
                return Err(RuntimeError::Runtime(format!(
                    "{} not yet implemented",
                    name
                )));
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
                    self.env.borrow_mut().set(name, module.clone());
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

    /// Run the default export and return the result.
    pub fn run(&mut self) -> Result<Value, RuntimeError> {
        self.process_imports()?;
        let default_name = self
            .analysis
            .default_export
            .clone()
            .ok_or_else(|| RuntimeError::Runtime("No default export".to_string()))?;
        let num_hooks = self.analysis.hook_calls.get(&default_name).map(|v| v.len()).unwrap_or(0);

        if self.default_instance.is_none() {
            self.default_instance = Some(Rc::new(RefCell::new(ComponentInstance::new(
                default_name.clone(),
                num_hooks,
            ))));
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
            let mut result = Value::Null;
            for s in &body {
                if let Statement::Return { argument } = s {
                    result = argument
                        .as_ref()
                        .map(|e| self.eval_expr(e))
                        .transpose()?
                        .unwrap_or(Value::Null);
                    break;
                }
                self.eval_stmt(s)?;
            }
            self.current_component = None;
            return Ok(result);
        }
        self.current_component = None;
        Err(RuntimeError::Runtime("Default export not found".to_string()))
    }

    /// Get module exports (default export function) without running the component.
    /// Used by load_module when a hook does import(path).
    pub fn get_module_exports(&mut self) -> Result<Value, RuntimeError> {
        self.process_imports()?;
        let (params, body, is_async) = match self.program.body.iter().find_map(|stmt| {
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
            Some(x) => x,
            None => return Err(RuntimeError::Runtime("Module has no default export".to_string())),
        };
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
        let default_fn = Value::Function {
            params,
            body,
            env: self.env.clone(),
            is_async,
            component_hooks: Some(num_hooks),
        };
        let mut exports = HashMap::new();
        exports.insert("default".to_string(), default_fn);
        Ok(Value::Object(exports))
    }
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
