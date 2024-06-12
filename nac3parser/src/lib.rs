//! This crate can be used to parse python sourcecode into a so
//! called AST (abstract syntax tree).
//!
//! The stages involved in this process are lexical analysis and
//! parsing. The lexical analysis splits the sourcecode into
//! tokens, and the parsing transforms those tokens into an AST.
//!
//! For example, one could do this:
//!
//! ```
//! use nac3parser::{parser, ast};
//!
//! let python_source = "print('Hello world')";
//! let python_ast = parser::parse_expression(python_source).unwrap();
//!
//! ```

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::default_trait_access,
    clippy::doc_markdown,
    clippy::enum_glob_use,
    clippy::fn_params_excessive_bools,
    clippy::if_not_else,
    clippy::implicit_clone,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::redundant_closure_for_method_calls,
    clippy::semicolon_if_nothing_returned,
    clippy::single_match_else,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unnested_or_patterns,
    clippy::unused_self,
    clippy::wildcard_imports
)]

#[macro_use]
extern crate log;
use lalrpop_util::lalrpop_mod;
pub use nac3ast as ast;

pub mod error;
mod fstring;
mod function;
pub mod lexer;
pub mod mode;
pub mod parser;
lalrpop_mod!(
    #[allow(clippy::all, clippy::pedantic)]
    #[allow(unused)]
    python
);
pub mod config_comment_helper;
pub mod token;
