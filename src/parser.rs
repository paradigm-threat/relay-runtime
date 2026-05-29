//! Recursive descent parser for the client hook language.
//!
//! Parses Program with Statements, JSX, variable decls with array patterns,
//! export default async function, and expressions.

use crate::ast::*;
use crate::error::{line_col_for_offset, get_line_at, ParseErrorDetail, RuntimeError, SourceLoc};
use crate::lexer::{Keyword, Lexer, Token};

/// Token stream with next/peek for recursive descent. Stores (token, start_offset).
struct TokenStream<'a> {
    source: &'a str,
    file: Option<String>,
    tokens: Vec<(Token, usize)>,
    pos: usize,
}

impl<'a> TokenStream<'a> {
    fn new(source: &'a str, file: Option<String>) -> Result<Self, RuntimeError> {
        let mut lexer = Lexer::new(source);
        let mut tokens = Vec::new();
        loop {
            let (t, start) = lexer.next_token().map_err(|(msg, offset)| {
                let (line, col) = line_col_for_offset(source, offset);
                let source_line = get_line_at(source, line);
                RuntimeError::ParseDetail(ParseErrorDetail {
                    message: msg,
                    loc: SourceLoc::new(file.clone(), line, col, offset),
                    source_line,
                })
            })?;
            let is_eof = matches!(&t, Token::Eof);
            tokens.push((t, start));
            if is_eof {
                break;
            }
        }
        Ok(Self { source, file, tokens, pos: 0 })
    }

    fn next(&mut self) -> (Token, usize) {
        let (t, start) = self.tokens.get(self.pos).cloned().unwrap_or((Token::Eof, self.source.chars().count()));
        self.pos += 1;
        (t, start)
    }

    fn peek(&self) -> &Token {
        &self.tokens.get(self.pos).map(|(t, _)| t).unwrap_or(&Token::Eof)
    }

    fn peek_start(&self) -> usize {
        self.tokens.get(self.pos).map(|(_, s)| *s).unwrap_or(self.source.chars().count())
    }

    fn peek_ahead(&self, offset: usize) -> &Token {
        &self.tokens.get(self.pos + offset).map(|(t, _)| t).unwrap_or(&Token::Eof)
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    fn save_pos(&self) -> usize {
        self.pos
    }

    fn restore_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    fn parse_error(&self, message: impl Into<String>) -> RuntimeError {
        let offset = self.peek_start();
        let (line, col) = line_col_for_offset(self.source, offset);
        let source_line = get_line_at(self.source, line);
        RuntimeError::ParseDetail(ParseErrorDetail {
            message: message.into(),
            loc: SourceLoc::new(self.file.clone(), line, col, offset),
            source_line,
        })
    }
}

fn expect_token(stream: &mut TokenStream, expected: Token) -> Result<(), RuntimeError> {
    if stream.peek() == &expected {
        stream.next();
        Ok(())
    } else {
        Err(stream.parse_error(format!(
            "Expected {:?}, got {:?}",
            expected,
            stream.peek()
        )))
    }
}

fn eat_token(stream: &mut TokenStream, expected: Token) -> bool {
    if stream.peek() == &expected {
        stream.next();
        true
    } else {
        false
    }
}

fn expect_ident(stream: &mut TokenStream) -> Result<String, RuntimeError> {
    match stream.peek() {
        Token::Ident(name) => {
            let name = name.clone();
            stream.next();
            Ok(name)
        }
        _ => Err(stream.parse_error(format!(
            "Expected identifier, got {:?}",
            stream.peek()
        ))),
    }
}

fn expect_semicolon_optional(stream: &mut TokenStream) {
    if matches!(stream.peek(), Token::Semicolon) {
        stream.next();
    }
}

fn is_expression_start(stream: &TokenStream) -> bool {
    matches!(
        stream.peek(),
        Token::LAngle
            | Token::Number(_)
            | Token::String(_)
            | Token::True
            | Token::False
            | Token::Null
            | Token::Ident(_)
            | Token::LParen
            | Token::LBrack
            | Token::LCurly
            | Token::Keyword(Keyword::Import)
            | Token::Typeof
    )
}

/// Parse source into a Program.
pub fn parse(source: &str) -> Result<Program, RuntimeError> {
    parse_with_path(source, None::<String>)
}

/// Parse source with optional filename for verbose error messages.
pub fn parse_with_path(source: &str, file: Option<impl Into<String>>) -> Result<Program, RuntimeError> {
    let file = file.map(|f| f.into());
    let mut stream = TokenStream::new(source, file)?;
    let body = parse_program(&mut stream)?;
    Ok(Program { body })
}

fn parse_program(stream: &mut TokenStream) -> Result<Vec<Statement>, RuntimeError> {
    let mut body = Vec::new();
    while !stream.is_eof() {
        body.push(parse_statement(stream)?);
    }
    Ok(body)
}

fn parse_statement(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    match stream.peek() {
        Token::Keyword(Keyword::Import) => {
            if stream.peek_ahead(1) == &Token::LParen {
                let expr = parse_expression(stream)?;
                expect_semicolon_optional(stream);
                Ok(Statement::Expr {
                    expression: Box::new(expr),
                })
            } else {
                stream.next();
                parse_import_decl_rest(stream)
            }
        }
        Token::Keyword(Keyword::Export) => {
            stream.next();
            expect_token(stream, Token::Keyword(Keyword::Default))?;
            parse_export_default(stream)
        }
        Token::Keyword(Keyword::Const) | Token::Keyword(Keyword::Let) | Token::Keyword(Keyword::Var) => {
            parse_variable_decl(stream)
        }
        Token::Keyword(Keyword::Return) => {
            stream.next();
            let argument = if is_expression_start(stream) {
                Some(Box::new(parse_expression(stream)?))
            } else {
                None
            };
            expect_semicolon_optional(stream);
            Ok(Statement::Return { argument })
        }
        Token::Keyword(Keyword::Function) => parse_function_decl(stream),
        Token::Keyword(Keyword::Async) => {
            stream.next();
            expect_token(stream, Token::Keyword(Keyword::Function))?;
            parse_function_decl_inner(stream, true)
        }
        Token::Keyword(Keyword::If) => parse_if(stream),
        Token::Keyword(Keyword::For) => parse_for(stream),
        Token::Keyword(Keyword::While) => parse_while(stream),
        Token::Keyword(Keyword::Try) => parse_try(stream),
        _ if is_expression_start(stream) => {
            let expr = parse_expression(stream)?;
            expect_semicolon_optional(stream);
            Ok(Statement::Expr {
                expression: Box::new(expr),
            })
        }
        _ => Err(stream.parse_error(format!(
            "Unexpected token at statement start: {:?}",
            stream.peek()
        ))),
    }
}

fn parse_import_decl_rest(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    let mut default_binding = None;
    let mut named_bindings = Vec::new();
    match stream.peek() {
        Token::Ident(_) => {
            default_binding = Some(expect_ident(stream)?);
            if stream.peek() == &Token::Comma {
                stream.next();
                expect_token(stream, Token::LCurly)?;
                named_bindings = parse_named_imports(stream)?;
            }
            expect_token(stream, Token::Keyword(Keyword::From))?;
        }
        Token::LCurly => {
            stream.next();
            named_bindings = parse_named_imports(stream)?;
            expect_token(stream, Token::Keyword(Keyword::From))?;
        }
        Token::Star => {
            stream.next();
            expect_token(stream, Token::Keyword(Keyword::As))?;
            let name = expect_ident(stream)?;
            default_binding = Some(name);
            expect_token(stream, Token::Keyword(Keyword::From))?;
        }
        _ => return Err(stream.parse_error(format!(
            "Expected identifier, {{, or * after import, got {:?}",
            stream.peek()
        ))),
    }
    let (tok, _) = stream.next();
    let source = match tok {
        Token::String(s) => s,
        t => return Err(stream.parse_error(format!("Expected string in import from, got {:?}", t))),
    };
    expect_semicolon_optional(stream);
    Ok(Statement::ImportDecl {
        default_binding,
        named_bindings,
        source,
    })
}

fn parse_named_imports(stream: &mut TokenStream) -> Result<Vec<(String, String)>, RuntimeError> {
    let mut bindings = Vec::new();
    loop {
        let imported = if eat_token(stream, Token::Keyword(Keyword::Default)) {
            "default".to_string()
        } else {
            expect_ident(stream)?
        };
        let local = if eat_token(stream, Token::Keyword(Keyword::As)) {
            expect_ident(stream)?
        } else {
            imported.clone()
        };
        bindings.push((local, imported));
        if !eat_token(stream, Token::Comma) {
            break;
        }
        if stream.peek() == &Token::RCurly {
            break;
        }
    }
    expect_token(stream, Token::RCurly)?;
    Ok(bindings)
}

fn parse_export_default(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    match stream.peek() {
        Token::Keyword(Keyword::Async) => {
            stream.next();
            expect_token(stream, Token::Keyword(Keyword::Function))?;
            let decl = parse_export_default_function(stream, true)?;
            Ok(Statement::ExportDefault {
                declaration: Box::new(decl),
            })
        }
        Token::Keyword(Keyword::Function) => {
            stream.next();
            let decl = parse_export_default_function(stream, false)?;
            Ok(Statement::ExportDefault {
                declaration: Box::new(decl),
            })
        }
        _ if is_expression_start(stream) => {
            let expr = parse_expression(stream)?;
            expect_semicolon_optional(stream);
            Ok(Statement::ExportDefault {
                declaration: Box::new(ExportDefaultDecl::Expr {
                    expression: Box::new(expr),
                }),
            })
        }
        _ => Err(stream.parse_error(format!(
            "Expected async function, function, or expression after export default: {:?}",
            stream.peek()
        ))),
    }
}

fn parse_export_default_function(
    stream: &mut TokenStream,
    r#async: bool,
) -> Result<ExportDefaultDecl, RuntimeError> {
    let id = match stream.peek() {
        Token::Ident(_) => Some(expect_ident(stream)?),
        _ => None,
    };
    expect_token(stream, Token::LParen)?;
    let params = parse_params(stream)?;
    expect_token(stream, Token::RParen)?;
    expect_token(stream, Token::LCurly)?;
    let body = parse_block(stream)?;
    expect_token(stream, Token::RCurly)?;
    Ok(ExportDefaultDecl::Function {
        id,
        params,
        body,
        r#async,
    })
}

fn parse_variable_decl(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    let (tok, _) = stream.next();
    let kind = match tok {
        Token::Keyword(Keyword::Const) => VarKind::Const,
        Token::Keyword(Keyword::Let) => VarKind::Let,
        Token::Keyword(Keyword::Var) => VarKind::Var,
        t => return Err(stream.parse_error(format!("Expected const/let/var, got {:?}", t))),
    };
    let mut declarations = Vec::new();
    loop {
        let id = parse_pattern(stream)?;
        let init = if eat_token(stream, Token::Eq) {
            Some(Box::new(parse_expression(stream)?))
        } else {
            None
        };
        declarations.push(VariableDeclarator { id, init });
        if !eat_token(stream, Token::Comma) {
            break;
        }
    }
    expect_semicolon_optional(stream);
    Ok(Statement::VariableDecl {
        kind,
        declarations,
    })
}

fn is_type_name(s: &str) -> bool {
    matches!(
        s,
        "number" | "string" | "boolean" | "object" | "array" | "null" | "undefined"
    )
}

fn parse_pattern(stream: &mut TokenStream) -> Result<Pattern, RuntimeError> {
    match stream.peek() {
        Token::Ident(_) => {
            let name = expect_ident(stream)?;
            let type_annotation = if stream.peek() == &Token::Colon {
                if let Token::Ident(ref type_name) = stream.peek_ahead(1) {
                    if is_type_name(type_name) {
                        stream.next(); // consume colon
                        let name = expect_ident(stream)?;
                        Some(crate::ast::TypeAnnotation { name })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            Ok(Pattern::Identifier { name, type_annotation })
        }
        Token::LBrack => {
            stream.next();
            let mut elements = Vec::new();
            while !eat_token(stream, Token::RBrack) {
                if eat_token(stream, Token::Comma) {
                    elements.push(None);
                    continue;
                }
                elements.push(Some(parse_pattern(stream)?));
                eat_token(stream, Token::Comma);
            }
            Ok(Pattern::Array { elements })
        }
        Token::LCurly => {
            stream.next();
            let mut properties = Vec::new();
            while !eat_token(stream, Token::RCurly) {
                let key = expect_ident(stream)?;
                let value = if eat_token(stream, Token::Colon) {
                    Some(Box::new(parse_pattern(stream)?))
                } else {
                    None
                };
                properties.push(ObjectProp { key, value });
                eat_token(stream, Token::Comma);
            }
            Ok(Pattern::Object { properties })
        }
        _ => Err(stream.parse_error(format!(
            "Expected pattern: {:?}",
            stream.peek()
        ))),
    }
}

fn parse_function_decl(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    expect_token(stream, Token::Keyword(Keyword::Function))?;
    parse_function_decl_inner(stream, false)
}

fn parse_function_decl_inner(
    stream: &mut TokenStream,
    r#async: bool,
) -> Result<Statement, RuntimeError> {
    let id = match stream.peek() {
        Token::Ident(_) => Some(expect_ident(stream)?),
        _ => None,
    };
    expect_token(stream, Token::LParen)?;
    let params = parse_params(stream)?;
    expect_token(stream, Token::RParen)?;
    expect_token(stream, Token::LCurly)?;
    let body = parse_block(stream)?;
    expect_token(stream, Token::RCurly)?;
    Ok(Statement::FunctionDecl {
        id,
        params,
        body,
        r#async,
    })
}

fn parse_params(stream: &mut TokenStream) -> Result<Vec<Param>, RuntimeError> {
    let mut params = Vec::new();
    while !matches!(stream.peek(), Token::RParen) {
        let rest = eat_token(stream, Token::Spread);
        let name = expect_ident(stream)?;
        let type_annotation = if stream.peek() == &Token::Colon {
            if let Token::Ident(ref type_name) = stream.peek_ahead(1) {
                if is_type_name(type_name) {
                    stream.next(); // consume colon
                    let name = expect_ident(stream)?;
                    Some(crate::ast::TypeAnnotation { name })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        params.push(Param { name, rest, type_annotation });
        if !eat_token(stream, Token::Comma) {
            break;
        }
    }
    Ok(params)
}

/// Parse arrow function params: ident (',' ident)* ')' or single ident '=>'.
fn parse_arrow_params(stream: &mut TokenStream) -> Result<Vec<Param>, RuntimeError> {
    let mut params = Vec::new();
    loop {
        let name = expect_ident(stream)?;
        let type_annotation = if stream.peek() == &Token::Colon {
            if let Token::Ident(ref type_name) = stream.peek_ahead(1) {
                if is_type_name(type_name) {
                    stream.next(); // consume colon
                    let name = expect_ident(stream)?;
                    Some(crate::ast::TypeAnnotation { name })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        params.push(Param { name, rest: false, type_annotation });
        if eat_token(stream, Token::RParen) || stream.peek() == &Token::Arrow {
            break;
        }
        expect_token(stream, Token::Comma)?;
    }
    Ok(params)
}

fn parse_arrow_body(stream: &mut TokenStream) -> Result<ArrowBody, RuntimeError> {
    if eat_token(stream, Token::LCurly) {
        let body = parse_block(stream)?;
        expect_token(stream, Token::RCurly)?;
        Ok(ArrowBody::Block(body))
    } else {
        let expr = parse_expression(stream)?;
        Ok(ArrowBody::Expr(Box::new(expr)))
    }
}

fn parse_block(stream: &mut TokenStream) -> Result<Vec<Statement>, RuntimeError> {
    let mut body = Vec::new();
    while !matches!(stream.peek(), Token::RCurly | Token::Eof) {
        body.push(parse_statement(stream)?);
    }
    Ok(body)
}

fn parse_if(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    expect_token(stream, Token::Keyword(Keyword::If))?;
    expect_token(stream, Token::LParen)?;
    let test = Box::new(parse_expression(stream)?);
    expect_token(stream, Token::RParen)?;
    let consequent = if eat_token(stream, Token::LCurly) {
        let body = parse_block(stream)?;
        expect_token(stream, Token::RCurly)?;
        body
    } else {
        vec![parse_statement(stream)?]
    };
    let alternate = if eat_token(stream, Token::Keyword(Keyword::Else)) {
        if eat_token(stream, Token::LCurly) {
            let alt = parse_block(stream)?;
            expect_token(stream, Token::RCurly)?;
            Some(alt)
        } else {
            Some(vec![parse_statement(stream)?])
        }
    } else {
        None
    };
    Ok(Statement::If {
        test,
        consequent,
        alternate,
    })
}

fn parse_for(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    expect_token(stream, Token::Keyword(Keyword::For))?;
    expect_token(stream, Token::LParen)?;
    let init = if is_expression_start(stream) || matches!(stream.peek(), Token::Keyword(Keyword::Const) | Token::Keyword(Keyword::Let) | Token::Keyword(Keyword::Var)) {
        Some(Box::new(parse_statement(stream)?))
    } else {
        None
    };
    let test = if is_expression_start(stream) {
        Some(Box::new(parse_expression(stream)?))
    } else {
        None
    };
    expect_token(stream, Token::Semicolon)?;
    let update = if is_expression_start(stream) {
        Some(Box::new(parse_expression(stream)?))
    } else {
        None
    };
    expect_token(stream, Token::RParen)?;
    expect_token(stream, Token::LCurly)?;
    let body = parse_block(stream)?;
    expect_token(stream, Token::RCurly)?;
    Ok(Statement::For {
        init,
        test,
        update,
        body,
    })
}

fn parse_while(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    expect_token(stream, Token::Keyword(Keyword::While))?;
    expect_token(stream, Token::LParen)?;
    let test = Box::new(parse_expression(stream)?);
    expect_token(stream, Token::RParen)?;
    expect_token(stream, Token::LCurly)?;
    let body = parse_block(stream)?;
    expect_token(stream, Token::RCurly)?;
    Ok(Statement::While { test, body })
}

fn parse_try(stream: &mut TokenStream) -> Result<Statement, RuntimeError> {
    expect_token(stream, Token::Keyword(Keyword::Try))?;
    expect_token(stream, Token::LCurly)?;
    let block = parse_block(stream)?;
    expect_token(stream, Token::RCurly)?;
    let handler = if eat_token(stream, Token::Keyword(Keyword::Catch)) {
        expect_token(stream, Token::LParen)?;
        let param = expect_ident(stream)?;
        expect_token(stream, Token::RParen)?;
        expect_token(stream, Token::LCurly)?;
        let body = parse_block(stream)?;
        expect_token(stream, Token::RCurly)?;
        Some(CatchClause {
            param: Some(param),
            body,
        })
    } else {
        None
    };
    let finalizer = if eat_token(stream, Token::Keyword(Keyword::Finally)) {
        expect_token(stream, Token::LCurly)?;
        let body = parse_block(stream)?;
        expect_token(stream, Token::RCurly)?;
        Some(body)
    } else {
        None
    };
    Ok(Statement::Try {
        block,
        handler,
        finalizer,
    })
}

fn parse_expression(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    parse_assignment(stream)
}

fn parse_assignment(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let left = parse_conditional(stream)?;
    if eat_token(stream, Token::Eq) {
        let right = parse_assignment(stream)?;
        Ok(Expr::Assign {
            left: Box::new(left),
            right: Box::new(right),
        })
    } else {
        Ok(left)
    }
}

fn parse_conditional(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let test = parse_binary_or(stream)?;
    if eat_token(stream, Token::Question) {
        let consequent = Box::new(parse_assignment(stream)?);
        expect_token(stream, Token::Colon)?;
        let alternate = Box::new(parse_assignment(stream)?);
        Ok(Expr::Conditional {
            test: Box::new(test),
            consequent,
            alternate,
        })
    } else {
        Ok(test)
    }
}

fn parse_binary_or(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let mut left = parse_binary_nullish(stream)?;
    while eat_token(stream, Token::Or) {
        let right = parse_binary_nullish(stream)?;
        left = Expr::Binary {
            left: Box::new(left),
            op: BinaryOp::Or,
            right: Box::new(right),
        };
    }
    Ok(left)
}

fn parse_binary_nullish(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let mut left = parse_binary_and(stream)?;
    while eat_token(stream, Token::QuestionQuestion) {
        let right = parse_binary_and(stream)?;
        left = Expr::Binary {
            left: Box::new(left),
            op: BinaryOp::NullishCoalesce,
            right: Box::new(right),
        };
    }
    Ok(left)
}

fn parse_binary_and(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let mut left = parse_binary_equality(stream)?;
    while eat_token(stream, Token::And) {
        let right = parse_binary_equality(stream)?;
        left = Expr::Binary {
            left: Box::new(left),
            op: BinaryOp::And,
            right: Box::new(right),
        };
    }
    Ok(left)
}

fn parse_binary_equality(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let mut left = parse_binary_compare(stream)?;
    loop {
        let op = if eat_token(stream, Token::EqEqEq) {
            BinaryOp::StrictEq
        } else if stream.peek() == &Token::EqEq {
            return Err(stream.parse_error("CleverScript: use === instead of =="));
        } else if eat_token(stream, Token::NeqEq) {
            BinaryOp::StrictNeq
        } else if stream.peek() == &Token::Neq {
            return Err(stream.parse_error("CleverScript: use !== instead of !="));
        } else {
            break;
        };
        let right = parse_binary_compare(stream)?;
        left = Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        };
    }
    Ok(left)
}

fn parse_binary_compare(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let mut left = parse_binary_additive(stream)?;
    loop {
        let op = if eat_token(stream, Token::Lt) {
            BinaryOp::Lt
        } else if eat_token(stream, Token::LAngle) {
            BinaryOp::Lt
        } else if eat_token(stream, Token::Le) {
            BinaryOp::Le
        } else if eat_token(stream, Token::Gt) {
            BinaryOp::Gt
        } else if eat_token(stream, Token::RAngle) {
            BinaryOp::Gt
        } else if eat_token(stream, Token::Ge) {
            BinaryOp::Ge
        } else {
            break;
        };
        let right = parse_binary_additive(stream)?;
        left = Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        };
    }
    Ok(left)
}

fn parse_binary_additive(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let mut left = parse_binary_multiplicative(stream)?;
    loop {
        let op = if eat_token(stream, Token::Add) {
            BinaryOp::Add
        } else if eat_token(stream, Token::Sub) {
            BinaryOp::Sub
        } else {
            break;
        };
        let right = parse_binary_multiplicative(stream)?;
        left = Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        };
    }
    Ok(left)
}

fn parse_binary_multiplicative(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let mut left = parse_unary(stream)?;
    loop {
        let op = if eat_token(stream, Token::Star) {
            BinaryOp::Mul
        } else if eat_token(stream, Token::Slash) {
            BinaryOp::Div
        } else {
            break;
        };
        let right = parse_unary(stream)?;
        left = Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        };
    }
    Ok(left)
}

fn parse_unary(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    if eat_token(stream, Token::Typeof) {
        let arg = parse_unary(stream)?;
        return Ok(Expr::Unary {
            op: UnaryOp::Typeof,
            argument: Box::new(arg),
        });
    }
    if eat_token(stream, Token::Bang) {
        let arg = parse_unary(stream)?;
        return Ok(Expr::Unary {
            op: UnaryOp::Not,
            argument: Box::new(arg),
        });
    }
    if eat_token(stream, Token::Sub) {
        let arg = parse_unary(stream)?;
        return Ok(Expr::Unary {
            op: UnaryOp::Neg,
            argument: Box::new(arg),
        });
    }
    if eat_token(stream, Token::Keyword(Keyword::Await)) {
        let arg = parse_unary(stream)?;
        return Ok(Expr::Await {
            argument: Box::new(arg),
        });
    }
    parse_postfix(stream)
}

fn parse_postfix(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let mut expr = parse_primary(stream)?;
    loop {
        expr = match stream.peek() {
            Token::LParen => {
                stream.next();
                let mut args = Vec::new();
                while !eat_token(stream, Token::RParen) {
                    args.push(parse_expression(stream)?);
                    if !eat_token(stream, Token::Comma) {
                        expect_token(stream, Token::RParen)?;
                        break;
                    }
                }
                Expr::Call {
                    callee: Box::new(expr),
                    arguments: args,
                }
            }
            Token::Dot => {
                stream.next();
                let name = if eat_token(stream, Token::Keyword(Keyword::Default)) {
                    "default".to_string()
                } else if eat_token(stream, Token::Keyword(Keyword::Catch)) {
                    "catch".to_string()
                } else if eat_token(stream, Token::Keyword(Keyword::Finally)) {
                    "finally".to_string()
                } else if eat_token(stream, Token::Keyword(Keyword::From)) {
                    "from".to_string()
                } else {
                    expect_ident(stream)?
                };
                Expr::Member {
                    object: Box::new(expr),
                    property: Box::new(Expr::Identifier {
                        name: name.clone(),
                    }),
                    optional: false,
                }
            }
            Token::QuestionDot => {
                stream.next();
                let property = if stream.peek() == &Token::LBrack {
                    stream.next();
                    let index = parse_expression(stream)?;
                    expect_token(stream, Token::RBrack)?;
                    Box::new(index)
                } else {
                    let name = if eat_token(stream, Token::Keyword(Keyword::Default)) {
                        "default".to_string()
                    } else {
                        expect_ident(stream)?
                    };
                    Box::new(Expr::Identifier { name })
                };
                Expr::Member {
                    object: Box::new(expr),
                    property,
                    optional: true,
                }
            }
            Token::LBrack => {
                stream.next();
                let index = parse_expression(stream)?;
                expect_token(stream, Token::RBrack)?;
                Expr::Member {
                    object: Box::new(expr),
                    property: Box::new(index),
                    optional: false,
                }
            }
            Token::Incr => {
                stream.next();
                Expr::PostfixIncr {
                    argument: Box::new(expr),
                }
            }
            Token::Decr => {
                stream.next();
                Expr::PostfixDecr {
                    argument: Box::new(expr),
                }
            }
            _ => break,
        };
    }
    Ok(expr)
}

fn parse_primary(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    match stream.peek() {
        Token::LAngle => parse_jsx_element(stream),
        Token::Number(n) => {
            let n = *n;
            stream.next();
            Ok(Expr::Literal(Literal::Number(n)))
        }
        Token::String(ref s) => {
            let s = s.clone();
            stream.next();
            Ok(Expr::Literal(Literal::String(s)))
        }
        Token::True => {
            stream.next();
            Ok(Expr::Literal(Literal::Bool(true)))
        }
        Token::False => {
            stream.next();
            Ok(Expr::Literal(Literal::Bool(false)))
        }
        Token::Null => {
            stream.next();
            Ok(Expr::Literal(Literal::Null))
        }
        Token::Ident(ref name) => {
            let name = name.clone();
            stream.next();
            if stream.peek() == &Token::Arrow {
                stream.next(); // consume =>
                let body = parse_arrow_body(stream)?;
                return Ok(Expr::ArrowFunction {
                    params: vec![Param { name, rest: false, type_annotation: None }],
                    body,
                });
            }
            Ok(Expr::Identifier { name })
        }
        Token::LParen => {
            stream.next();
            // Try arrow function: () => or (params) =>
            if stream.peek() == &Token::RParen {
                stream.next();
                if stream.peek() == &Token::Arrow {
                    stream.next();
                    let body = parse_arrow_body(stream)?;
                    return Ok(Expr::ArrowFunction {
                        params: Vec::new(),
                        body,
                    });
                }
                return Err(stream.parse_error("Empty () not valid here; use () => expr for arrow function".to_string()));
            }
            if matches!(stream.peek(), Token::Ident(_)) {
                let saved = stream.save_pos();
                if let Ok(params) = parse_arrow_params(stream) {
                    if stream.peek() == &Token::Arrow {
                        stream.next();
                        let body = parse_arrow_body(stream)?;
                        return Ok(Expr::ArrowFunction { params, body });
                    }
                }
                stream.restore_pos(saved);
            }
            let expr = parse_expression(stream)?;
            expect_token(stream, Token::RParen)?;
            Ok(expr)
        }
        Token::LBrack => {
            stream.next();
            let mut elements = Vec::new();
            while !eat_token(stream, Token::RBrack) {
                if eat_token(stream, Token::Spread) {
                    return Err(stream.parse_error("CleverScript: array spread [...a] not supported"));
                }
                elements.push(parse_expression(stream)?);
                eat_token(stream, Token::Comma);
            }
            Ok(Expr::Array { elements })
        }
        Token::LCurly => parse_object(stream),
        Token::Keyword(Keyword::Import) => {
            stream.next();
            expect_token(stream, Token::LParen)?;
            let source = parse_expression(stream)?;
            expect_token(stream, Token::RParen)?;
            Ok(Expr::Import {
                source: Box::new(source),
            })
        }
        _ => Err(stream.parse_error(format!(
            "Unexpected token in expression: {:?}",
            stream.peek()
        ))),
    }
}

fn parse_object(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    expect_token(stream, Token::LCurly)?;
    let mut properties = Vec::new();
    while !eat_token(stream, Token::RCurly) {
        if eat_token(stream, Token::Spread) {
            return Err(stream.parse_error("CleverScript: object spread not supported"));
        } else {
            let key = expect_ident(stream)?;
            let value = if eat_token(stream, Token::Colon) {
                Box::new(parse_expression(stream)?)
            } else {
                Box::new(Expr::Identifier { name: key.clone() })
            };
            properties.push(ObjectExprProp::KeyValue { key, value });
        }
        eat_token(stream, Token::Comma);
    }
    Ok(Expr::Object { properties })
}

// ── JSX ──────────────────────────────────────────────────────

fn parse_jsx_element(stream: &mut TokenStream) -> Result<Expr, RuntimeError> {
    let el = parse_jsx_tag(stream)?;
    Ok(Expr::JsxElement(Box::new(el)))
}

fn parse_jsx_tag(stream: &mut TokenStream) -> Result<JsxElement, RuntimeError> {
    expect_token(stream, Token::LAngle)?;

    // Closing tag </name>
    if eat_token(stream, Token::Slash) {
        let _name = expect_ident(stream)?;
        expect_token(stream, Token::RAngle)?;
        return Err(stream.parse_error("Unexpected closing JSX tag".to_string()));
    }

    let tag = parse_jsx_tag_name(stream)?;
    let (props, self_closing) = parse_jsx_element_tail(stream)?;

    if self_closing {
        return Ok(JsxElement {
            tag,
            props,
            children: Vec::new(),
            self_closing: true,
        });
    }

    let expected_name = jsx_tag_name(&tag);
    let children = parse_jsx_children(stream, &expected_name)?;
    parse_jsx_closing_tag(stream, &expected_name)?;

    Ok(JsxElement {
        tag,
        props,
        children,
        self_closing: false,
    })
}

fn parse_jsx_tag_name(stream: &mut TokenStream) -> Result<JsxTag, RuntimeError> {
    let first = expect_ident(stream)?;
    if eat_token(stream, Token::Dot) {
        let property = expect_ident(stream)?;
        Ok(JsxTag::Member {
            object: first,
            property,
        })
    } else {
        Ok(JsxTag::Identifier { name: first })
    }
}

fn parse_jsx_element_tail(
    stream: &mut TokenStream,
) -> Result<(Vec<JsxProp>, bool), RuntimeError> {
    let mut props = Vec::new();

    loop {
        match stream.peek() {
            Token::RAngle => {
                stream.next();
                return Ok((props, false));
            }
            Token::Slash => {
                stream.next();
                expect_token(stream, Token::RAngle)?;
                return Ok((props, true));
            }
            Token::Ident(ref name) => {
                let name = name.clone();
                stream.next();
                if eat_token(stream, Token::Eq) {
                    let value = parse_jsx_attr_value(stream)?;
                    props.push(JsxProp::Attribute { name, value });
                } else {
                    props.push(JsxProp::Attribute {
                        name,
                        value: JsxAttrValue::String("true".to_string()),
                    });
                }
            }
            Token::LCurly => {
                stream.next();
                if eat_token(stream, Token::Spread) {
                    return Err(stream.parse_error("CleverScript: JSX spread {...props} not supported"));
                } else {
                    return Err(stream.parse_error("Expected attribute name or closing > in JSX".to_string()));
                }
            }
            _ => break,
        }
    }

    expect_token(stream, Token::RAngle)?;
    Ok((props, false))
}

fn parse_jsx_attr_value(stream: &mut TokenStream) -> Result<JsxAttrValue, RuntimeError> {
    match stream.peek() {
        Token::String(ref s) => {
            let s = s.clone();
            stream.next();
            Ok(JsxAttrValue::String(s))
        }
        Token::LCurly => {
            stream.next();
            let expr = parse_expression(stream)?;
            expect_token(stream, Token::RCurly)?;
            Ok(JsxAttrValue::Expr(Box::new(expr)))
        }
        _ => Err(stream.parse_error(format!(
            "Expected string or {{ expr }} in JSX attribute: {:?}",
            stream.peek()
        ))),
    }
}

fn jsx_tag_name(tag: &JsxTag) -> String {
    match tag {
        JsxTag::Identifier { name } => name.clone(),
        JsxTag::Member { object, property } => format!("{}.{}", object, property),
    }
}

fn parse_jsx_children(
    stream: &mut TokenStream,
    _expected_closing: &str,
) -> Result<Vec<JsxChild>, RuntimeError> {
    let mut children = Vec::new();

    loop {
        match stream.peek() {
            Token::LAngle => {
                if matches!(stream.peek_ahead(1), Token::Slash) {
                    break;
                }
                let el = parse_jsx_tag(stream)?;
                children.push(JsxChild::Element(Box::new(el)));
            }
            Token::LCurly => {
                stream.next();
                if stream.peek() == &Token::RCurly {
                    stream.next();
                } else {
                    let expr = parse_expression(stream)?;
                    expect_token(stream, Token::RCurly)?;
                    children.push(JsxChild::Expr(Box::new(expr)));
                }
            }
            Token::Eof => break,
            Token::Ident(ref s) => {
                let s = s.clone();
                stream.next();
                let space = matches!(stream.peek(), Token::LCurly | Token::Ident(_));
                children.push(JsxChild::Text(if space { format!("{} ", s) } else { s }));
            }
            Token::Colon => {
                stream.next();
                let space = matches!(stream.peek(), Token::LCurly | Token::Ident(_));
                children.push(JsxChild::Text(if space { ": ".to_string() } else { ":".to_string() }));
            }
            Token::Eq => {
                stream.next();
                children.push(JsxChild::Text("=".to_string()));
            }
            Token::Comma => {
                stream.next();
                let space = matches!(stream.peek(), Token::LCurly | Token::Ident(_) | Token::Number(_));
                children.push(JsxChild::Text(if space { ", " } else { "," }.to_string()));
            }
            Token::QuestionQuestion => {
                stream.next();
                let space = matches!(stream.peek(), Token::Ident(_) | Token::LCurly);
                children.push(JsxChild::Text(if space { "?? " } else { "??" }.to_string()));
            }
            Token::QuestionDot => {
                stream.next();
                let space = matches!(stream.peek(), Token::Ident(_) | Token::LCurly);
                children.push(JsxChild::Text(if space { "?. " } else { "?." }.to_string()));
            }
            Token::Dot => {
                stream.next();
                children.push(JsxChild::Text(".".to_string()));
            }
            Token::LParen | Token::RParen => {
                let s = if matches!(stream.peek(), Token::LParen) { "(" } else { ")" };
                stream.next();
                children.push(JsxChild::Text(s.to_string()));
            }
            Token::Sub => {
                stream.next();
                children.push(JsxChild::Text("-".to_string()));
            }
            Token::Keyword(kw) => {
                let s = match kw {
                    Keyword::Const => "const",
                    Keyword::Let => "let",
                    Keyword::Var => "var",
                    Keyword::Function => "function",
                    Keyword::Return => "return",
                    Keyword::If => "if",
                    Keyword::Else => "else",
                    Keyword::For => "for",
                    Keyword::While => "while",
                    Keyword::Try => "try",
                    Keyword::Catch => "catch",
                    Keyword::Finally => "finally",
                    Keyword::Await => "await",
                    Keyword::Async => "async",
                    Keyword::Export => "export",
                    Keyword::Default => "default",
                    Keyword::Import => "import",
                    Keyword::From => "from",
                    Keyword::As => "as",
                };
                stream.next();
                let space = matches!(stream.peek(), Token::LCurly | Token::Ident(_));
                children.push(JsxChild::Text(if space { format!("{} ", s) } else { s.to_string() }));
            }
            Token::String(ref s) => {
                let s = s.clone();
                stream.next();
                children.push(JsxChild::Text(s));
            }
            Token::Number(n) => {
                let s = n.to_string();
                stream.next();
                children.push(JsxChild::Text(s));
            }
            _ => {
                stream.next();
            }
        }
    }

    Ok(children)
}

fn parse_jsx_closing_tag(stream: &mut TokenStream, expected: &str) -> Result<(), RuntimeError> {
    expect_token(stream, Token::LAngle)?;
    expect_token(stream, Token::Slash)?;
    let name = expect_ident(stream)?;
    if name != expected {
        return Err(stream.parse_error(format!(
            "Expected closing tag </{}>, got </{}>",
            expected, name
        )));
    }
    expect_token(stream, Token::RAngle)?;
    Ok(())
}
