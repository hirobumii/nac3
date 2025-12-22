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
            Self::Name { name } => {
                write!(f, "'{}'", ast::get_str_from_ref(&ast::get_str_ref_lock(), *name))
            }
            Self::Int { value } => {
                if *value == i128::MAX {
                    write!(f, "'#OFL#'")
                } else {
                    write!(f, "'{value}'")
                }
            }
            Self::Float { value } => write!(f, "'{value}'"),
            Self::Complex { real, imag } => write!(f, "{real}j{imag}"),
            Self::String { value, is_fstring } => {
                if *is_fstring {
                    write!(f, "f")?;
                }
                write!(f, "{value:?}")
            }
            Self::Bytes { value } => {
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
            Self::ConfigComment { content } => write!(
                f,
                "ConfigComment: '{}'",
                ast::get_str_from_ref(&ast::get_str_ref_lock(), *content)
            ),
            Self::Newline => f.write_str("Newline"),
            Self::Indent => f.write_str("Indent"),
            Self::Dedent => f.write_str("Dedent"),
            Self::StartModule => f.write_str("StartProgram"),
            Self::StartInteractive => f.write_str("StartInteractive"),
            Self::StartExpression => f.write_str("StartExpression"),
            Self::EndOfFile => f.write_str("EOF"),
            Self::Lpar => f.write_str("'('"),
            Self::Rpar => f.write_str("')'"),
            Self::Lsqb => f.write_str("'['"),
            Self::Rsqb => f.write_str("']'"),
            Self::Colon => f.write_str("':'"),
            Self::Comma => f.write_str("','"),
            Self::Semi => f.write_str("';'"),
            Self::Plus => f.write_str("'+'"),
            Self::Minus => f.write_str("'-'"),
            Self::Star => f.write_str("'*'"),
            Self::Slash => f.write_str("'/'"),
            Self::Vbar => f.write_str("'|'"),
            Self::Amper => f.write_str("'&'"),
            Self::Less => f.write_str("'<'"),
            Self::Greater => f.write_str("'>'"),
            Self::Equal => f.write_str("'='"),
            Self::Dot => f.write_str("'.'"),
            Self::Percent => f.write_str("'%'"),
            Self::Lbrace => f.write_str("'{'"),
            Self::Rbrace => f.write_str("'}'"),
            Self::EqEqual => f.write_str("'=='"),
            Self::NotEqual => f.write_str("'!='"),
            Self::LessEqual => f.write_str("'<='"),
            Self::GreaterEqual => f.write_str("'>='"),
            Self::Tilde => f.write_str("'~'"),
            Self::CircumFlex => f.write_str("'^'"),
            Self::LeftShift => f.write_str("'<<'"),
            Self::RightShift => f.write_str("'>>'"),
            Self::DoubleStar => f.write_str("'**'"),
            Self::DoubleStarEqual => f.write_str("'**='"),
            Self::PlusEqual => f.write_str("'+='"),
            Self::MinusEqual => f.write_str("'-='"),
            Self::StarEqual => f.write_str("'*='"),
            Self::SlashEqual => f.write_str("'/='"),
            Self::PercentEqual => f.write_str("'%='"),
            Self::AmperEqual => f.write_str("'&='"),
            Self::VbarEqual => f.write_str("'|='"),
            Self::CircumflexEqual => f.write_str("'^='"),
            Self::LeftShiftEqual => f.write_str("'<<='"),
            Self::RightShiftEqual => f.write_str("'>>='"),
            Self::DoubleSlash => f.write_str("'//'"),
            Self::DoubleSlashEqual => f.write_str("'//='"),
            Self::At => f.write_str("'@'"),
            Self::AtEqual => f.write_str("'@='"),
            Self::Rarrow => f.write_str("'->'"),
            Self::Ellipsis => f.write_str("'...'"),
            Self::False => f.write_str("'False'"),
            Self::None => f.write_str("'None'"),
            Self::True => f.write_str("'True'"),
            Self::And => f.write_str("'and'"),
            Self::As => f.write_str("'as'"),
            Self::Assert => f.write_str("'assert'"),
            Self::Async => f.write_str("'async'"),
            Self::Await => f.write_str("'await'"),
            Self::Break => f.write_str("'break'"),
            Self::Class => f.write_str("'class'"),
            Self::Continue => f.write_str("'continue'"),
            Self::Def => f.write_str("'def'"),
            Self::Del => f.write_str("'del'"),
            Self::Elif => f.write_str("'elif'"),
            Self::Else => f.write_str("'else'"),
            Self::Except => f.write_str("'except'"),
            Self::Finally => f.write_str("'finally'"),
            Self::For => f.write_str("'for'"),
            Self::From => f.write_str("'from'"),
            Self::Global => f.write_str("'global'"),
            Self::If => f.write_str("'if'"),
            Self::Import => f.write_str("'import'"),
            Self::In => f.write_str("'in'"),
            Self::Is => f.write_str("'is'"),
            Self::Lambda => f.write_str("'lambda'"),
            Self::Nonlocal => f.write_str("'nonlocal'"),
            Self::Not => f.write_str("'not'"),
            Self::Or => f.write_str("'or'"),
            Self::Pass => f.write_str("'pass'"),
            Self::Raise => f.write_str("'raise'"),
            Self::Return => f.write_str("'return'"),
            Self::Try => f.write_str("'try'"),
            Self::While => f.write_str("'while'"),
            Self::With => f.write_str("'with'"),
            Self::Yield => f.write_str("'yield'"),
            Self::ColonEqual => f.write_str("':='"),
        }
    }
}
