//! Semantic analysis: component detection, hook call order, exports.
//!
//! Walks AST to produce metadata for the interpreter. No separate IR;
//! we annotate the program with analysis results.

use crate::ast::*;
use crate::error::RuntimeError;
use std::collections::HashMap;

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
