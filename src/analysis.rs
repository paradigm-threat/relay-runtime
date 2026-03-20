//! Semantic analysis: component detection, hook call order, exports, undefined variables.
//!
//! Walks AST to produce metadata for the interpreter. No separate IR;
//! we annotate the program with analysis results.

use crate::ast::*;
use crate::error::RuntimeError;
use std::collections::{HashMap, HashSet};

/// Result of semantic analysis.
#[derive(Debug, Clone, Default)]
pub struct AnalysisResult {
    /// Default export (hook entry point): function name or None for anonymous.
    pub default_export: Option<String>,
    /// Named exports: name -> (is_async, is_component).
    pub named_exports: HashMap<String, ExportInfo>,
    /// Functions that are components (return JSX). Used for hook rules.
    pub components: Vec<String>,
    /// Hook call order per component: component_name -> [slot_index, hook_name].
    pub hook_calls: HashMap<String, Vec<HookCallSite>>,
}

#[derive(Debug, Clone)]
pub struct ExportInfo {
    pub is_async: bool,
    pub is_component: bool,
}

#[derive(Debug, Clone)]
pub struct HookCallSite {
    pub slot: usize,
    pub hook_name: String,
}

const HOOK_NAMES: &[&str] = &["useState", "useEffect", "useCallback"];

/// Analyze program and produce metadata.
pub fn analyze(program: &Program) -> Result<AnalysisResult, RuntimeError> {
    check_undefined_variables(program)?;
    let mut result = AnalysisResult::default();
    let mut component_names = Vec::new();

    for stmt in &program.body {
        match stmt {
            Statement::ExportDefault { declaration } => {
                match declaration.as_ref() {
                    ExportDefaultDecl::Function {
                        id,
                        body,
                        r#async,
                        ..
                    } => {
                        let name = id.clone().unwrap_or_else(|| "<default>".to_string());
                        result.default_export = Some(name.clone());
                        let is_component = function_returns_jsx(body);
                        if is_component {
                            component_names.push(name.clone());
                        }
                        result.named_exports.insert(
                            name.clone(),
                            ExportInfo {
                                is_async: *r#async,
                                is_component,
                            },
                        );
                        if is_component {
                            let hooks = collect_hook_calls(body);
                            result.hook_calls.insert(name.clone(), hooks);
                        }
                    }
                    ExportDefaultDecl::Expr { .. } => {
                        result.default_export = Some("<default>".to_string());
                    }
                }
            }
            Statement::ExportNamed { declaration } => {
                let ExportNamedDecl::Function { id, body, .. } = declaration;
                let is_component = function_returns_jsx(body);
                if is_component {
                    component_names.push(id.clone());
                }
                result.named_exports.insert(
                    id.clone(),
                    ExportInfo {
                        is_async: false,
                        is_component,
                    },
                );
                if is_component {
                    let hooks = collect_hook_calls(body);
                    result.hook_calls.insert(id.clone(), hooks);
                }
            }
            Statement::FunctionDecl {
                id,
                body,
                r#async: _,
                ..
            } => {
                if let Some(name) = id {
                    let is_component = function_returns_jsx(body);
                    if is_component {
                        component_names.push(name.clone());
                        let hooks = collect_hook_calls(body);
                        result.hook_calls.insert(name.clone(), hooks);
                    }
                }
            }
            _ => {}
        }
    }

    result.components = component_names;
    Ok(result)
}

fn function_returns_jsx(body: &[Statement]) -> bool {
    for stmt in body {
        if stmt_returns_jsx(stmt) {
            return true;
        }
    }
    false
}

fn stmt_returns_jsx(stmt: &Statement) -> bool {
    match stmt {
        Statement::Return { argument } => {
            if let Some(expr) = argument {
                return expr_returns_jsx(expr);
            }
            false
        }
        Statement::If {
            consequent,
            alternate,
            ..
        } => {
            consequent.iter().any(stmt_returns_jsx)
                || alternate
                    .as_ref()
                    .map(|a| a.iter().any(stmt_returns_jsx))
                    .unwrap_or(false)
        }
        Statement::Try { block, handler, .. } => {
            block.iter().any(stmt_returns_jsx)
                || handler
                    .as_ref()
                    .map(|h| h.body.iter().any(stmt_returns_jsx))
                    .unwrap_or(false)
        }
        Statement::Expr { expression } => expr_returns_jsx(expression),
        _ => false,
    }
}

fn expr_returns_jsx(expr: &Expr) -> bool {
    match expr {
        Expr::JsxElement(_) | Expr::JsxFragment { .. } => true,
        Expr::Conditional {
            consequent,
            alternate,
            ..
        } => expr_returns_jsx(consequent) || expr_returns_jsx(alternate),
        Expr::Call { callee, arguments, .. } => {
            if let Expr::Identifier { name } = callee.as_ref() {
                if name == "wrap" || name == "React.createElement" {
                    return arguments.first().map_or(false, expr_returns_jsx);
                }
            }
            false
        }
        _ => false,
    }
}

fn collect_hook_calls(body: &[Statement]) -> Vec<HookCallSite> {
    let mut calls = Vec::new();
    let mut slot = 0;
    collect_hook_calls_in_stmts(body, &mut calls, &mut slot);
    calls
}

fn collect_hook_calls_in_stmts(
    stmts: &[Statement],
    out: &mut Vec<HookCallSite>,
    slot: &mut usize,
) {
    for stmt in stmts {
        collect_hook_calls_in_stmt(stmt, out, slot);
    }
}

fn collect_hook_calls_in_stmt(
    stmt: &Statement,
    out: &mut Vec<HookCallSite>,
    slot: &mut usize,
) {
    match stmt {
        Statement::VariableDecl { declarations, .. } => {
            for decl in declarations {
                if let Some(init) = &decl.init {
                    collect_hook_calls_in_expr(init, out, slot);
                }
            }
        }
        Statement::Return { argument } => {
            if let Some(expr) = argument {
                collect_hook_calls_in_expr(expr, out, slot);
            }
        }
        Statement::Expr { expression } => {
            collect_hook_calls_in_expr(expression, out, slot);
        }
        Statement::If {
            test,
            consequent,
            alternate,
            ..
        } => {
            collect_hook_calls_in_expr(test, out, slot);
            for s in consequent {
                collect_hook_calls_in_stmt(s, out, slot);
            }
            if let Some(alt) = alternate {
                for s in alt {
                    collect_hook_calls_in_stmt(s, out, slot);
                }
            }
        }
        Statement::Try {
            block,
            handler,
            finalizer,
            ..
        } => {
            for s in block {
                collect_hook_calls_in_stmt(s, out, slot);
            }
            if let Some(h) = handler {
                for s in &h.body {
                    collect_hook_calls_in_stmt(s, out, slot);
                }
            }
            if let Some(f) = finalizer {
                for s in f {
                    collect_hook_calls_in_stmt(s, out, slot);
                }
            }
        }
        _ => {}
    }
}

fn hook_name_from_callee(callee: &Expr) -> Option<String> {
    match callee {
        Expr::Identifier { name } if HOOK_NAMES.contains(&name.as_str()) => Some(name.clone()),
        Expr::Member { property, .. } => {
            if let Expr::Identifier { name } = property.as_ref() {
                if HOOK_NAMES.contains(&name.as_str()) {
                    return Some(name.clone());
                }
            }
            None
        }
        _ => None,
    }
}

fn collect_hook_calls_in_expr(expr: &Expr, out: &mut Vec<HookCallSite>, slot: &mut usize) {
    match expr {
        Expr::Call { callee, arguments, .. } => {
            if let Some(hook_name) = hook_name_from_callee(callee.as_ref()) {
                out.push(HookCallSite {
                    slot: *slot,
                    hook_name,
                });
                *slot += 1;
                // Don't recurse into hook args (e.g. useCallback's callback)
            } else {
                for arg in arguments {
                    collect_hook_calls_in_expr(arg, out, slot);
                }
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_hook_calls_in_expr(left, out, slot);
            collect_hook_calls_in_expr(right, out, slot);
        }
        Expr::Conditional {
            test,
            consequent,
            alternate,
            ..
        } => {
            collect_hook_calls_in_expr(test, out, slot);
            collect_hook_calls_in_expr(consequent, out, slot);
            collect_hook_calls_in_expr(alternate, out, slot);
        }
        Expr::Array { elements } => {
            for e in elements {
                collect_hook_calls_in_expr(e, out, slot);
            }
        }
        Expr::Object { properties } => {
            for p in properties {
                match p {
                    ObjectExprProp::KeyValue { value, .. } => {
                        collect_hook_calls_in_expr(value, out, slot);
                    }
                    ObjectExprProp::Spread { argument } => {
                        collect_hook_calls_in_expr(argument, out, slot);
                    }
                }
            }
        }
        Expr::JsxElement(el) => {
            for prop in &el.props {
                match prop {
                    JsxProp::Attribute {
                        value: JsxAttrValue::Expr(e),
                        ..
                    } => collect_hook_calls_in_expr(e, out, slot),
                    JsxProp::Spread { argument } => {
                        collect_hook_calls_in_expr(argument, out, slot);
                    }
                    _ => {}
                }
            }
            for child in &el.children {
                match child {
                    JsxChild::Expr(e) => collect_hook_calls_in_expr(e, out, slot),
                    JsxChild::Element(inner) => {
                        for prop in &inner.props {
                            if let JsxProp::Attribute {
                                value: JsxAttrValue::Expr(e),
                                ..
                            } = prop
                            {
                                collect_hook_calls_in_expr(e, out, slot);
                            }
                        }
                        for c in &inner.children {
                            if let JsxChild::Expr(e) = c {
                                collect_hook_calls_in_expr(e, out, slot);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Expr::ArrowFunction { .. } => {
            // Don't recurse into nested functions for hook collection
        }
        _ => {}
    }
}

/// Check for undefined variable references. Returns Err on first undefined.
fn check_undefined_variables(program: &Program) -> Result<(), RuntimeError> {
    let mut scope: Vec<HashSet<String>> = vec![HashSet::new()];
    // Host-injected bindings - provided by host at runtime
    for name in ["params", "env", "navigate", "createElement", "fetch", "useState", "useEffect", "useCallback"] {
        scope.last_mut().unwrap().insert(name.to_string());
    }

    for stmt in &program.body {
        match stmt {
            Statement::ImportDecl {
                default_binding,
                named_bindings,
                ..
            } => {
                if let Some(name) = default_binding {
                    scope.last_mut().unwrap().insert(name.clone());
                }
                for (local, _) in named_bindings {
                    scope.last_mut().unwrap().insert(local.clone());
                }
            }
            Statement::ExportDefault { declaration } => {
                if let ExportDefaultDecl::Function { params, body, .. } = declaration.as_ref() {
                    check_function_scope(params, body, &mut scope)?;
                }
            }
            Statement::ExportNamed { declaration } => {
                let ExportNamedDecl::Function { params, body, .. } = declaration;
                check_function_scope(params, body, &mut scope)?;
            }
            Statement::FunctionDecl { params, body, .. } => {
                check_function_scope(params, body, &mut scope)?;
            }
            Statement::VariableDecl { declarations, .. } => {
                for decl in declarations {
                    check_expr_refs(decl.init.as_deref(), &mut scope)?;
                    add_pattern_names(&decl.id, scope.last_mut().unwrap());
                }
            }
            Statement::Return { argument } => check_expr_refs(argument.as_deref(), &mut scope)?,
            Statement::Expr { expression } => check_expr_refs(Some(expression), &mut scope)?,
            Statement::If {
                test,
                consequent,
                alternate,
                ..
            } => {
                check_expr_refs(Some(test), &mut scope)?;
                scope.push(HashSet::new());
                for s in consequent {
                    check_stmt_refs(s, &mut scope)?;
                }
                scope.pop();
                if let Some(alt) = alternate {
                    scope.push(HashSet::new());
                    for s in alt {
                        check_stmt_refs(s, &mut scope)?;
                    }
                    scope.pop();
                }
            }
            Statement::For {
                init,
                test,
                update,
                body,
                ..
            } => {
                scope.push(HashSet::new());
                if let Some(i) = init {
                    check_stmt_refs(i, &mut scope)?;
                }
                check_expr_refs(test.as_deref(), &mut scope)?;
                for s in body {
                    check_stmt_refs(s, &mut scope)?;
                }
                check_expr_refs(update.as_deref(), &mut scope)?;
                scope.pop();
            }
            Statement::While { test, body } => {
                check_expr_refs(Some(test.as_ref()), &mut scope)?;
                scope.push(HashSet::new());
                for s in body {
                    check_stmt_refs(s, &mut scope)?;
                }
                scope.pop();
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_function_scope(
    params: &[Param],
    body: &[Statement],
    scope: &mut Vec<HashSet<String>>,
) -> Result<(), RuntimeError> {
    scope.push(HashSet::new());
    for p in params {
        scope.last_mut().unwrap().insert(p.name.clone());
    }
    for s in body {
        check_stmt_refs(s, scope)?;
    }
    scope.pop();
    Ok(())
}

fn check_stmt_refs(stmt: &Statement, scope: &mut Vec<HashSet<String>>) -> Result<(), RuntimeError> {
    match stmt {
        Statement::VariableDecl { declarations, .. } => {
            for decl in declarations {
                check_expr_refs(decl.init.as_deref(), scope)?;
                add_pattern_names(&decl.id, scope.last_mut().unwrap());
            }
        }
        Statement::Return { argument } => check_expr_refs(argument.as_deref(), scope)?,
        Statement::Expr { expression } => check_expr_refs(Some(expression.as_ref()), scope)?,
        Statement::If {
            test,
            consequent,
            alternate,
            ..
        } => {
            check_expr_refs(Some(test.as_ref()), scope)?;
            scope.push(HashSet::new());
            for s in consequent {
                check_stmt_refs(s, scope)?;
            }
            scope.pop();
            if let Some(alt) = alternate {
                scope.push(HashSet::new());
                for s in alt {
                    check_stmt_refs(s, scope)?;
                }
                scope.pop();
            }
        }
        Statement::For {
            init,
            test,
            update,
            body,
            ..
        } => {
            scope.push(HashSet::new());
            if let Some(i) = init {
                check_stmt_refs(i, scope)?;
            }
            check_expr_refs(test.as_deref(), scope)?;
            for s in body {
                check_stmt_refs(s, scope)?;
            }
            check_expr_refs(update.as_deref(), scope)?;
            scope.pop();
        }
        Statement::While { test, body } => {
            check_expr_refs(Some(test.as_ref()), scope)?;
            scope.push(HashSet::new());
            for s in body {
                check_stmt_refs(s, scope)?;
            }
            scope.pop();
        }
        Statement::Try {
            block,
            handler,
            finalizer,
            ..
        } => {
            scope.push(HashSet::new());
            for s in block {
                check_stmt_refs(s, scope)?;
            }
            if let Some(h) = handler {
                if let Some(param) = &h.param {
                    scope.last_mut().unwrap().insert(param.clone());
                }
                for s in &h.body {
                    check_stmt_refs(s, scope)?;
                }
            }
            scope.pop();
            if let Some(f) = finalizer {
                for s in f {
                    check_stmt_refs(s, scope)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_expr_refs(
    expr: Option<&Expr>,
    scope: &mut Vec<HashSet<String>>,
) -> Result<(), RuntimeError> {
    let Some(expr) = expr else { return Ok(()) };
    match expr {
        Expr::Identifier { name } => {
            for s in scope.iter().rev() {
                if s.contains(name) {
                    return Ok(());
                }
            }
            return Err(RuntimeError::Ir(format!("Undefined variable: {}", name)));
        }
        Expr::Binary { left, right, .. } => {
            check_expr_refs(Some(left), scope)?;
            check_expr_refs(Some(right), scope)?;
        }
        Expr::Unary { argument, .. } => check_expr_refs(Some(argument), scope)?,
        Expr::Call { callee, arguments } => {
            check_expr_refs(Some(callee), scope)?;
            for a in arguments {
                check_expr_refs(Some(a), scope)?;
            }
        }
        Expr::Member { object, property, .. } => {
            check_expr_refs(Some(object), scope)?;
            // Property Identifier is used as key (obj.prop, arr.length), not as variable - skip
            if !matches!(property.as_ref(), Expr::Identifier { .. }) {
                check_expr_refs(Some(property), scope)?;
            }
        }
        Expr::Conditional {
            test,
            consequent,
            alternate,
            ..
        } => {
            check_expr_refs(Some(test), scope)?;
            check_expr_refs(Some(consequent), scope)?;
            check_expr_refs(Some(alternate), scope)?;
        }
        Expr::Array { elements } => {
            for e in elements {
                check_expr_refs(Some(e), scope)?;
            }
        }
        Expr::Object { properties } => {
            for p in properties {
                match p {
                    ObjectExprProp::KeyValue { value, .. } => check_expr_refs(Some(value), scope)?,
                    ObjectExprProp::Spread { argument } => check_expr_refs(Some(argument), scope)?,
                }
            }
        }
        Expr::Assign { left, right } => {
            check_expr_refs(Some(left), scope)?;
            check_expr_refs(Some(right), scope)?;
        }
        Expr::JsxElement(el) => {
            for prop in &el.props {
                if let JsxProp::Attribute {
                    value: JsxAttrValue::Expr(e),
                    ..
                } = prop
                {
                    check_expr_refs(Some(e), scope)?;
                }
            }
            for child in &el.children {
                match child {
                    JsxChild::Expr(e) => check_expr_refs(Some(e), scope)?,
                    JsxChild::Element(inner) => {
                        for prop in &inner.props {
                            if let JsxProp::Attribute {
                                value: JsxAttrValue::Expr(e),
                                ..
                            } = prop
                            {
                                check_expr_refs(Some(e), scope)?;
                            }
                        }
                        for c in &inner.children {
                            if let JsxChild::Expr(e) = c {
                                check_expr_refs(Some(e), scope)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Expr::JsxFragment { children } => {
            for c in children {
                if let JsxChild::Expr(e) = c {
                    check_expr_refs(Some(e), scope)?;
                }
            }
        }
        Expr::PostfixIncr { argument } | Expr::PostfixDecr { argument } => {
            check_expr_refs(Some(argument), scope)?;
        }
        Expr::TemplateLiteral { expressions, .. } => {
            for e in expressions {
                check_expr_refs(Some(e), scope)?;
            }
        }
        Expr::Import { source } => check_expr_refs(Some(source), scope)?,
        Expr::Await { argument } => check_expr_refs(Some(argument), scope)?,
        Expr::ArrowFunction { params, body } => {
            scope.push(HashSet::new());
            for p in params {
                scope.last_mut().unwrap().insert(p.name.clone());
            }
            match body {
                ArrowBody::Block(stmts) => {
                    for s in stmts {
                        check_stmt_refs(s, scope)?;
                    }
                }
                ArrowBody::Expr(e) => check_expr_refs(Some(e), scope)?,
            }
            scope.pop();
        }
        _ => {}
    }
    Ok(())
}

fn add_pattern_names(pattern: &Pattern, scope: &mut HashSet<String>) {
    match pattern {
        Pattern::Identifier { name, .. } => {
            scope.insert(name.clone());
        }
        Pattern::Array { elements } => {
            for elem in elements {
                if let Some(Pattern::Identifier { name, .. }) = elem {
                    scope.insert(name.clone());
                }
            }
        }
        Pattern::Object { properties } => {
            for p in properties {
                if let Some(inner) = &p.value {
                    add_pattern_names(inner, scope);
                } else {
                    // Shorthand: { children } binds variable "children"
                    scope.insert(p.key.clone());
                }
            }
        }
    }
}
