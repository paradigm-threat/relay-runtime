//! Lexer for the client hook language.

use std::collections::VecDeque;
use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug)]
enum TemplatePiece {
    Static(String),
    Expr(String),
}

fn slice_chars(source: &str, start_char: usize, end_exclusive_char: usize) -> String {
    source
        .chars()
        .skip(start_char)
        .take(end_exclusive_char.saturating_sub(start_char))
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    String(String),
    TemplatePart(String),
    Number(f64),
    True,
    False,
    Null,

    // Identifiers and keywords
    Ident(String),
    Keyword(Keyword),
    Typeof,

    // Punctuation
    LCurly,
    RCurly,
    LParen,
    RParen,
    LBrack,
    RBrack,
    LAngle, // < for JSX and comparisons
    RAngle, // > for JSX and comparisons
    Slash,
    Comma,
    Semicolon,
    Colon,
    Dot,
    Question,      // ? (ternary)
    QuestionDot,   // ?.
    QuestionQuestion, // ??
    Eq,
    EqEq,
    EqEqEq,
    Bang,  // ! (logical NOT)
    Neq,
    NeqEq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Arrow,
    Spread,
    Star,
    Add,
    Sub,
    Incr,  // ++
    Decr,  // --

    // End
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Const,
    Let,
    Var,
    Function,
    Return,
    If,
    Else,
    For,
    While,
    Try,
    Catch,
    Finally,
    Await,
    Async,
    Export,
    Default,
    Import,
    From,
    As,
}

impl Keyword {
    fn from_str(s: &str) -> Option<Keyword> {
        match s {
            "const" => Some(Keyword::Const),
            "let" => Some(Keyword::Let),
            "var" => Some(Keyword::Var),
            "function" => Some(Keyword::Function),
            "return" => Some(Keyword::Return),
            "if" => Some(Keyword::If),
            "else" => Some(Keyword::Else),
            "for" => Some(Keyword::For),
            "while" => Some(Keyword::While),
            "try" => Some(Keyword::Try),
            "catch" => Some(Keyword::Catch),
            "finally" => Some(Keyword::Finally),
            "await" => Some(Keyword::Await),
            "async" => Some(Keyword::Async),
            "export" => Some(Keyword::Export),
            "default" => Some(Keyword::Default),
            "import" => Some(Keyword::Import),
            "from" => Some(Keyword::From),
            "as" => Some(Keyword::As),
            "typeof" => None, // Handled as Token::Typeof
            "true" => None, // Handled as Token::True
            "false" => None,
            "null" => None,
            _ => None,
        }
    }
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: Peekable<Chars<'a>>,
    pos: usize,
    /// Tokens expanded from a single template literal (`+`-chained strings and sub-expressions).
    pending: VecDeque<(Token, usize)>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().peekable(),
            pos: 0,
            pending: VecDeque::new(),
        }
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.next();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    fn skip_whitespace(&mut self) {
        while let Some(&c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.advance() {
            if c == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), String> {
        let mut prev = ' ';
        while let Some(c) = self.advance() {
            if prev == '*' && c == '/' {
                return Ok(());
            }
            prev = c;
        }
        Err("Unterminated block comment".to_string())
    }

    fn read_string(&mut self, quote: char) -> Result<Token, String> {
        let mut s = String::new();
        while let Some(c) = self.advance() {
            if c == '\\' {
                if let Some(esc) = self.advance() {
                    let ch = match esc {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        _ => esc,
                    };
                    s.push(ch);
                }
            } else if c == quote {
                return Ok(Token::String(s));
            } else {
                s.push(c);
            }
        }
        Err("Unterminated string".to_string())
    }

    /// Expand `` `…${expr}…` `` into `String` / `+` / tokens for `expr`.
    /// `${…}` scanning uses brace depth only (no awareness of `"` / `'` / nested templates).
    fn expand_template_literal(&mut self, template_open_start: usize) -> Result<(), String> {
        let mut pieces: Vec<TemplatePiece> = Vec::new();
        let mut buf = String::new();

        loop {
            let c = match self.peek().copied() {
                None => return Err("Unterminated template literal".to_string()),
                Some(c) => c,
            };
            match c {
                '`' => {
                    self.advance();
                    if !buf.is_empty() {
                        pieces.push(TemplatePiece::Static(buf));
                    }
                    break;
                }
                '\\' => {
                    self.advance();
                    let esc = self
                        .advance()
                        .ok_or_else(|| "Unterminated escape in template literal".to_string())?;
                    buf.push(match esc {
                        '`' => '`',
                        '$' => '$',
                        '\\' => '\\',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        _ => esc,
                    });
                }
                '$' => {
                    if self.peek_second() == Some('{') {
                        self.advance(); // $
                        self.advance(); // {
                        if !buf.is_empty() {
                            pieces.push(TemplatePiece::Static(buf));
                            buf = String::new();
                        }
                        let expr_src = self.scan_template_substitution()?;
                        pieces.push(TemplatePiece::Expr(expr_src));
                    } else {
                        self.advance();
                        buf.push('$');
                    }
                }
                _ => {
                    self.advance();
                    buf.push(c);
                }
            }
        }

        self.enqueue_template_pieces(pieces, template_open_start)?;
        Ok(())
    }

    fn peek_second(&self) -> Option<char> {
        let mut it = self.chars.clone();
        it.next()?;
        it.peek().copied()
    }

    /// After `${`, scan until the matching `}` using brace depth only (MVP).
    /// Does not treat `{` / `}` inside JS strings or nested `` ` `` templates specially.
    fn scan_template_substitution(&mut self) -> Result<String, String> {
        let start = self.pos;
        let mut depth = 1usize;
        while depth > 0 {
            let c = self.peek().copied().ok_or_else(|| "Unterminated ${ ... }".to_string())?;
            match c {
                '{' => {
                    self.advance();
                    depth += 1;
                }
                '}' => {
                    self.advance();
                    depth -= 1;
                    if depth == 0 {
                        let end_exclusive = self.pos - 1;
                        return Ok(slice_chars(self.source, start, end_exclusive));
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
        Err("Unclosed ${ ... }".to_string())
    }

    fn enqueue_template_pieces(
        &mut self,
        pieces: Vec<TemplatePiece>,
        offset_hint: usize,
    ) -> Result<(), String> {
        let mut segments: Vec<Vec<(Token, usize)>> = Vec::new();

        for p in pieces {
            match p {
                TemplatePiece::Static(s) => {
                    if !s.is_empty() {
                        segments.push(vec![(Token::String(s), offset_hint)]);
                    }
                }
                TemplatePiece::Expr(src) => {
                    let mut sub = Lexer::new(src.trim());
                    let mut inner: Vec<(Token, usize)> = Vec::new();
                    loop {
                        let (t, st) = sub
                            .next_token()
                            .map_err(|e| format!("in template interpolation: {}", e.0))?;
                        if matches!(t, Token::Eof) {
                            break;
                        }
                        inner.push((t, st));
                    }
                    if inner.is_empty() {
                        segments.push(vec![(Token::String(String::new()), offset_hint)]);
                    } else {
                        segments.push(inner);
                    }
                }
            }
        }

        if segments.is_empty() {
            self.pending
                .push_back((Token::String(String::new()), offset_hint));
            return Ok(());
        }

        let mut out = segments.remove(0);
        for seg in segments {
            out.push((Token::Add, offset_hint));
            out.extend(seg);
        }
        for pair in out {
            self.pending.push_back(pair);
        }
        Ok(())
    }

    fn read_number(&mut self, first: char) -> Token {
        let mut s = first.to_string();
        while let Some(&c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        Token::Number(s.parse().unwrap_or(0.0))
    }

    fn read_ident_or_keyword(&mut self, first: char) -> Token {
        let mut s = first.to_string();
        while let Some(&c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '$' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        match s.as_str() {
            "typeof" => Token::Typeof,
            "true" => Token::True,
            "false" => Token::False,
            "null" => Token::Null,
            _ => {
                if let Some(kw) = Keyword::from_str(&s) {
                    Token::Keyword(kw)
                } else {
                    Token::Ident(s)
                }
            }
        }
    }

    /// Current character offset (number of chars consumed).
    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn next_token(&mut self) -> Result<(Token, usize), (String, usize)> {
        if let Some((tok, off)) = self.pending.pop_front() {
            return Ok((tok, off));
        }
        self.skip_whitespace();
        let start = self.pos;

        let c = match self.advance() {
            Some(ch) => ch,
            None => return Ok((Token::Eof, start)),
        };

        if c == '`' {
            self.expand_template_literal(start)
                .map_err(|e| (e, start))?;
            return self.next_token();
        }

        Ok((match c {
            '"' | '\'' => self.read_string(c).map_err(|e| (e, start))?,
            '/' => {
                if self.peek() == Some(&'/') {
                    self.advance();
                    self.skip_line_comment();
                    return self.next_token();
                } else if self.peek() == Some(&'*') {
                    self.advance();
                    self.skip_block_comment().map_err(|e| (e, start))?;
                    return self.next_token();
                } else {
                    Token::Slash
                }
            }
            '{' => Token::LCurly,
            '}' => Token::RCurly,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '[' => Token::LBrack,
            ']' => Token::RBrack,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            ':' => Token::Colon,
            '.' => {
                if self.peek() == Some(&'.') {
                    self.advance();
                    if self.peek() == Some(&'.') {
                        self.advance();
                        Token::Spread
                    } else {
                        Token::Dot
                    }
                } else {
                    Token::Dot
                }
            }
            '?' => {
                if self.peek() == Some(&'?') {
                    self.advance();
                    Token::QuestionQuestion
                } else if self.peek() == Some(&'.') {
                    self.advance();
                    Token::QuestionDot
                } else {
                    Token::Question
                }
            }
            '=' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    if self.peek() == Some(&'=') {
                        self.advance();
                        Token::EqEqEq
                    } else {
                        Token::EqEq
                    }
                } else if self.peek() == Some(&'>') {
                    self.advance();
                    Token::Arrow
                } else {
                    Token::Eq
                }
            }
            '!' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    if self.peek() == Some(&'=') {
                        self.advance();
                        Token::NeqEq
                    } else {
                        Token::Neq
                    }
                } else {
                    Token::Bang
                }
            }
            '<' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    Token::Le
                } else {
                    Token::LAngle
                }
            }
            '>' => {
                if self.peek() == Some(&'=') {
                    self.advance();
                    Token::Ge
                } else {
                    Token::RAngle
                }
            }
            '&' => {
                if self.peek() == Some(&'&') {
                    self.advance();
                    Token::And
                } else {
                    Token::Ident("&".to_string())
                }
            }
            '|' => {
                if self.peek() == Some(&'|') {
                    self.advance();
                    Token::Or
                } else {
                    return Err(("Unexpected '|'".to_string(), start));
                }
            }
            '-' => {
                if self.peek() == Some(&'-') {
                    self.advance();
                    Token::Decr
                } else {
                    Token::Sub
                }
            }
            '*' => Token::Star,
            '$' => self.read_ident_or_keyword('$'),
            '+' => {
                if self.peek() == Some(&'+') {
                    self.advance();
                    Token::Incr
                } else {
                    Token::Add
                }
            }
            _ if c.is_ascii_digit() => self.read_number(c),
            _ if c.is_alphabetic() || c == '_' || c == '$' => self.read_ident_or_keyword(c),
            _ if !c.is_ascii_control() && !"[]{}()<>.,;:?=!&|^~`@#%\\".contains(c) => {
                Token::Ident(c.to_string())
            }
            _ => return Err((format!("Unexpected character: {:?}", c), start)),
        }, start))
    }
}

