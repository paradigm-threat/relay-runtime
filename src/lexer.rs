//! Lexer for the client hook language.

use std::iter::Peekable;
use std::str::Chars;

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
            "true" => None, // Handled as Token::True
            "false" => None,
            "null" => None,
            _ => None,
        }
    }
}

pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().peekable(),
            pos: 0,
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

    fn read_template_literal(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        while let Some(c) = self.advance() {
            if c == '\\' {
                if let Some(esc) = self.advance() {
                    s.push(match esc {
                        '`' => '`',
                        '$' => '$',
                        '\\' => '\\',
                        'n' => '\n',
                        't' => '\t',
                        _ => esc,
                    });
                }
            } else if c == '$' && self.peek() == Some(&'{') {
                return Err("CleverScript: template literals with ${} not supported".to_string());
            } else if c == '`' {
                return Ok(Token::TemplatePart(s));
            } else {
                s.push(c);
            }
        }
        Err("Unterminated template literal".to_string())
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
        self.skip_whitespace();
        let start = self.pos;

        let c = match self.advance() {
            Some(ch) => ch,
            None => return Ok((Token::Eof, start)),
        };

        Ok((match c {
            '"' | '\'' => self.read_string(c).map_err(|e| (e, start))?,
            '`' => self.read_template_literal().map_err(|e| (e, start))?,
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
                    return Err(("Unexpected '!'".to_string(), start));
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
                    return Err(("Unexpected '&'".to_string(), start));
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
                if self.peek() == Some(&'>') {
                    self.advance();
                    Token::Arrow
                } else if self.peek() == Some(&'-') {
                    self.advance();
                    Token::Decr
                } else {
                    Token::Sub
                }
            }
            '*' => Token::Star,
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
            _ => return Err((format!("Unexpected character: {:?}", c), start)),
        }, start))
    }
}

