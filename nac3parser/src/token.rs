//! Different token definitions.
//! Loosely based on token.h from CPython source:
use std::fmt::{self, Write};

use crate::ast;

/// Python source code can be tokenized in a sequence of these tokens.
#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Name { name: ast::StrRef },
    Int { value: i128 },
    Float { value: f64 },
    Complex { real: f64, imag: f64 },
    String { value: String, is_fstring: bool },
    Bytes { value: Vec<u8> },
    ConfigComment { content: ast::StrRef },
    Newline,
    Indent,
    Dedent,
    StartModule,
    StartInteractive,
    StartExpression,
    EndOfFile,
    Lpar,
    Rpar,
    Lsqb,
    Rsqb,
    Colon,
    Comma,
    Semi,
    Plus,
    Minus,
    Star,
    Slash,
    Vbar,  // '|'
    Amper, // '&'
    Less,
    Greater,
    Equal,
    Dot,
    Percent,
    Lbrace,
    Rbrace,
    EqEqual,
    NotEqual,
    LessEqual,
    GreaterEqual,
    Tilde,
    CircumFlex,
    LeftShift,
    RightShift,
    DoubleStar,
    DoubleStarEqual, // '**='
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    AmperEqual, // '&='
    VbarEqual,
    CircumflexEqual, // '^='
    LeftShiftEqual,
    RightShiftEqual,
    DoubleSlash, // '//'
    DoubleSlashEqual,
    ColonEqual,
    At,
    AtEqual,
    Rarrow,
    Ellipsis,

    // Keywords (alphabetically):
    False,
    None,
    True,

    And,
    As,
    Assert,
    Async,
    Await,
    Break,
    Class,
    Continue,
    Def,
    Del,
    Elif,
    Else,
    Except,
    Finally,
    For,
    From,
    Global,
    If,
    Import,
    In,
    Is,
    Lambda,
    Nonlocal,
    Not,
    Or,
    Pass,
    Raise,
    Return,
    Try,
    While,
    With,
    Yield,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Name { name } => {
                write!(f, "'{}'", ast::get_str_from_ref(&ast::get_str_ref_lock(), *name))
            }
            Tok::Int { value } => {
                if *value == i128::MAX {
                    write!(f, "'#OFL#'")
                } else {
                    write!(f, "'{value}'")
                }
            }
            Tok::Float { value } => write!(f, "'{value}'"),
            Tok::Complex { real, imag } => write!(f, "{real}j{imag}"),
            Tok::String { value, is_fstring } => {
                if *is_fstring {
                    write!(f, "f")?;
                }
                write!(f, "{value:?}")
            }
            Tok::Bytes { value } => {
                write!(f, "b\"")?;
                for i in value {
                    match i {
                        9 => f.write_str("\\t")?,
                        10 => f.write_str("\\n")?,
                        13 => f.write_str("\\r")?,
                        32..=126 => f.write_char(*i as char)?,
                        _ => write!(f, "\\x{i:02x}")?,
                    }
                }
                f.write_str("\"")
            }
            Tok::ConfigComment { content } => write!(
                f,
                "ConfigComment: '{}'",
                ast::get_str_from_ref(&ast::get_str_ref_lock(), *content)
            ),
            Tok::Newline => f.write_str("Newline"),
            Tok::Indent => f.write_str("Indent"),
            Tok::Dedent => f.write_str("Dedent"),
            Tok::StartModule => f.write_str("StartProgram"),
            Tok::StartInteractive => f.write_str("StartInteractive"),
            Tok::StartExpression => f.write_str("StartExpression"),
            Tok::EndOfFile => f.write_str("EOF"),
            Tok::Lpar => f.write_str("'('"),
            Tok::Rpar => f.write_str("')'"),
            Tok::Lsqb => f.write_str("'['"),
            Tok::Rsqb => f.write_str("']'"),
            Tok::Colon => f.write_str("':'"),
            Tok::Comma => f.write_str("','"),
            Tok::Semi => f.write_str("';'"),
            Tok::Plus => f.write_str("'+'"),
            Tok::Minus => f.write_str("'-'"),
            Tok::Star => f.write_str("'*'"),
            Tok::Slash => f.write_str("'/'"),
            Tok::Vbar => f.write_str("'|'"),
            Tok::Amper => f.write_str("'&'"),
            Tok::Less => f.write_str("'<'"),
            Tok::Greater => f.write_str("'>'"),
            Tok::Equal => f.write_str("'='"),
            Tok::Dot => f.write_str("'.'"),
            Tok::Percent => f.write_str("'%'"),
            Tok::Lbrace => f.write_str("'{'"),
            Tok::Rbrace => f.write_str("'}'"),
            Tok::EqEqual => f.write_str("'=='"),
            Tok::NotEqual => f.write_str("'!='"),
            Tok::LessEqual => f.write_str("'<='"),
            Tok::GreaterEqual => f.write_str("'>='"),
            Tok::Tilde => f.write_str("'~'"),
            Tok::CircumFlex => f.write_str("'^'"),
            Tok::LeftShift => f.write_str("'<<'"),
            Tok::RightShift => f.write_str("'>>'"),
            Tok::DoubleStar => f.write_str("'**'"),
            Tok::DoubleStarEqual => f.write_str("'**='"),
            Tok::PlusEqual => f.write_str("'+='"),
            Tok::MinusEqual => f.write_str("'-='"),
            Tok::StarEqual => f.write_str("'*='"),
            Tok::SlashEqual => f.write_str("'/='"),
            Tok::PercentEqual => f.write_str("'%='"),
            Tok::AmperEqual => f.write_str("'&='"),
            Tok::VbarEqual => f.write_str("'|='"),
            Tok::CircumflexEqual => f.write_str("'^='"),
            Tok::LeftShiftEqual => f.write_str("'<<='"),
            Tok::RightShiftEqual => f.write_str("'>>='"),
            Tok::DoubleSlash => f.write_str("'//'"),
            Tok::DoubleSlashEqual => f.write_str("'//='"),
            Tok::At => f.write_str("'@'"),
            Tok::AtEqual => f.write_str("'@='"),
            Tok::Rarrow => f.write_str("'->'"),
            Tok::Ellipsis => f.write_str("'...'"),
            Tok::False => f.write_str("'False'"),
            Tok::None => f.write_str("'None'"),
            Tok::True => f.write_str("'True'"),
            Tok::And => f.write_str("'and'"),
            Tok::As => f.write_str("'as'"),
            Tok::Assert => f.write_str("'assert'"),
            Tok::Async => f.write_str("'async'"),
            Tok::Await => f.write_str("'await'"),
            Tok::Break => f.write_str("'break'"),
            Tok::Class => f.write_str("'class'"),
            Tok::Continue => f.write_str("'continue'"),
            Tok::Def => f.write_str("'def'"),
            Tok::Del => f.write_str("'del'"),
            Tok::Elif => f.write_str("'elif'"),
            Tok::Else => f.write_str("'else'"),
            Tok::Except => f.write_str("'except'"),
            Tok::Finally => f.write_str("'finally'"),
            Tok::For => f.write_str("'for'"),
            Tok::From => f.write_str("'from'"),
            Tok::Global => f.write_str("'global'"),
            Tok::If => f.write_str("'if'"),
            Tok::Import => f.write_str("'import'"),
            Tok::In => f.write_str("'in'"),
            Tok::Is => f.write_str("'is'"),
            Tok::Lambda => f.write_str("'lambda'"),
            Tok::Nonlocal => f.write_str("'nonlocal'"),
            Tok::Not => f.write_str("'not'"),
            Tok::Or => f.write_str("'or'"),
            Tok::Pass => f.write_str("'pass'"),
            Tok::Raise => f.write_str("'raise'"),
            Tok::Return => f.write_str("'return'"),
            Tok::Try => f.write_str("'try'"),
            Tok::While => f.write_str("'while'"),
            Tok::With => f.write_str("'with'"),
            Tok::Yield => f.write_str("'yield'"),
            Tok::ColonEqual => f.write_str("':='"),
        }
    }
}
