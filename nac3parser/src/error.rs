//! Define internal parse error types
//! The goal is to provide a matching and a safe error API, maksing errors from LALR
use std::{error::Error, fmt};

use lalrpop_util::ParseError as LalrpopError;

use crate::{ast::Location, token::Tok};

/// Represents an error during lexical scanning.
#[derive(Debug, PartialEq)]
pub struct LexicalError {
    pub error: LexicalErrorType,
    pub location: Location,
}

#[derive(Debug, PartialEq)]
pub enum LexicalErrorType {
    StringError,
    UnicodeError,
    NestingError,
    IndentationError,
    TabError,
    TabsAfterSpaces,
    DefaultArgumentError,
    PositionalArgumentError,
    DuplicateKeywordArgumentError,
    UnrecognizedToken { tok: char },
    FStringError(FStringErrorType),
    LineContinuationError,
    Eof,
    OtherError(String),
}

impl fmt::Display for LexicalErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::StringError => write!(f, "Got unexpected string"),
            Self::FStringError(error) => write!(f, "Got error in f-string: {error}"),
            Self::UnicodeError => write!(f, "Got unexpected unicode"),
            Self::NestingError => write!(f, "Got unexpected nesting"),
            Self::IndentationError => {
                write!(f, "unindent does not match any outer indentation level")
            }
            Self::TabError => {
                write!(f, "inconsistent use of tabs and spaces in indentation")
            }
            Self::TabsAfterSpaces => {
                write!(f, "Tabs not allowed as part of indentation after spaces")
            }
            Self::DefaultArgumentError => {
                write!(f, "non-default argument follows default argument")
            }
            Self::DuplicateKeywordArgumentError => {
                write!(f, "keyword argument repeated")
            }
            Self::PositionalArgumentError => {
                write!(f, "positional argument follows keyword argument")
            }
            Self::UnrecognizedToken { tok } => {
                write!(f, "Got unexpected token {tok}")
            }
            Self::LineContinuationError => {
                write!(f, "unexpected character after line continuation character")
            }
            Self::Eof => write!(f, "unexpected EOF while parsing"),
            Self::OtherError(msg) => write!(f, "{msg}"),
        }
    }
}

// TODO: consolidate these with ParseError
#[derive(Debug, PartialEq)]
pub struct FStringError {
    pub error: FStringErrorType,
    pub location: Location,
}

#[derive(Debug, PartialEq)]
pub enum FStringErrorType {
    UnclosedLbrace,
    UnopenedRbrace,
    ExpectedRbrace,
    InvalidExpression(Box<ParseErrorType>),
    InvalidConversionFlag,
    EmptyExpression,
    MismatchedDelimiter,
    ExpressionNestedTooDeeply,
}

impl fmt::Display for FStringErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UnclosedLbrace => write!(f, "Unclosed '{{'"),
            Self::UnopenedRbrace => write!(f, "Unopened '}}'"),
            Self::ExpectedRbrace => write!(f, "Expected '}}' after conversion flag."),
            Self::InvalidExpression(error) => {
                write!(f, "Invalid expression: {error}")
            }
            Self::InvalidConversionFlag => write!(f, "Invalid conversion flag"),
            Self::EmptyExpression => write!(f, "Empty expression"),
            Self::MismatchedDelimiter => write!(f, "Mismatched delimiter"),
            Self::ExpressionNestedTooDeeply => {
                write!(f, "expressions nested too deeply")
            }
        }
    }
}

impl From<FStringError> for LalrpopError<Location, Tok, LexicalError> {
    fn from(err: FStringError) -> Self {
        Self::User {
            error: LexicalError {
                error: LexicalErrorType::FStringError(err.error),
                location: err.location,
            },
        }
    }
}

/// Represents an error during parsing
#[derive(Debug, PartialEq)]
pub struct ParseError {
    pub error: ParseErrorType,
    pub location: Location,
}

#[derive(Debug, PartialEq)]
pub enum ParseErrorType {
    /// Parser encountered an unexpected end of input
    Eof,
    /// Parser encountered an extra token
    ExtraToken(Tok),
    /// Parser encountered an invalid token
    InvalidToken,
    /// Parser encountered an unexpected token
    UnrecognizedToken(Tok, Option<String>),
    /// Maps to `User` type from `lalrpop-util`
    Lexical(LexicalErrorType),
}

/// Convert `lalrpop_util::ParseError` to our internal type
impl From<LalrpopError<Location, Tok, LexicalError>> for ParseError {
    fn from(err: LalrpopError<Location, Tok, LexicalError>) -> Self {
        match err {
            LalrpopError::ExtraToken { token } => {
                Self { error: ParseErrorType::ExtraToken(token.1), location: token.0 }
            }
            LalrpopError::User { error } => {
                Self { error: ParseErrorType::Lexical(error.error), location: error.location }
            }
            LalrpopError::UnrecognizedToken { token, expected } => {
                // Hacky, but it's how CPython does it. See PyParser_AddToken,
                // in particular "Only one possible expected token" comment.
                let expected = if expected.len() == 1 { Some(expected[0].clone()) } else { None };
                Self {
                    error: ParseErrorType::UnrecognizedToken(token.1, expected),
                    location: token.0,
                }
            }

            LalrpopError::UnrecognizedEof { location, .. }
            // TODO: Are there cases where this isn't an EOF?
            | LalrpopError::InvalidToken { location } => {
                Self { error: ParseErrorType::Eof, location }
            }
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} at {}", self.error, self.location)
    }
}

impl fmt::Display for ParseErrorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Eof => write!(f, "Got unexpected EOF"),
            Self::ExtraToken(ref tok) => write!(f, "Got extraneous token: {tok:?}"),
            Self::InvalidToken => write!(f, "Got invalid token"),
            Self::UnrecognizedToken(ref tok, ref expected) => {
                if *tok == Tok::Indent {
                    write!(f, "unexpected indent")
                } else if expected.as_deref() == Some("Indent") {
                    write!(f, "expected an indented block")
                } else {
                    write!(f, "Got unexpected token {tok}")
                }
            }
            Self::Lexical(ref error) => write!(f, "{error}"),
        }
    }
}

impl Error for ParseErrorType {}

impl ParseErrorType {
    #[must_use]
    pub fn is_indentation_error(&self) -> bool {
        match self {
            Self::Lexical(LexicalErrorType::IndentationError) => true,
            Self::UnrecognizedToken(token, expected) => {
                *token == Tok::Indent || expected.clone() == Some("Indent".to_owned())
            }
            _ => false,
        }
    }
    #[must_use]
    pub const fn is_tab_error(&self) -> bool {
        matches!(
            self,
            Self::Lexical(LexicalErrorType::TabError | LexicalErrorType::TabsAfterSpaces)
        )
    }
}

impl std::ops::Deref for ParseError {
    type Target = ParseErrorType;
    fn deref(&self) -> &Self::Target {
        &self.error
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
