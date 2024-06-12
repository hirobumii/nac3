//! Datatypes to support source location information.
use crate::ast_gen::StrRef;
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileName(pub StrRef);
impl Default for FileName {
    fn default() -> Self {
        FileName("unknown".into())
    }
}

impl From<String> for FileName {
    fn from(s: String) -> Self {
        FileName(s.into())
    }
}

/// A location somewhere in the sourcecode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Location {
    pub row: usize,
    pub column: usize,
    pub file: FileName,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file.0, self.row, self.column)
    }
}

impl Ord for Location {
    fn cmp(&self, other: &Self) -> Ordering {
        let file_cmp = self.file.0.to_string().cmp(&other.file.0.to_string());
        if file_cmp != Ordering::Equal {
            return file_cmp;
        }

        let row_cmp = self.row.cmp(&other.row);
        if row_cmp != Ordering::Equal {
            return row_cmp;
        }

        self.column.cmp(&other.column)
    }
}

impl PartialOrd for Location {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Location {
    pub fn visualize<'a>(
        &self,
        line: &'a str,
        desc: impl fmt::Display + 'a,
    ) -> impl fmt::Display + 'a {
        struct Visualize<'a, D: fmt::Display> {
            loc: Location,
            line: &'a str,
            desc: D,
        }
        impl<D: fmt::Display> fmt::Display for Visualize<'_, D> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    f,
                    "{}\n{}\n{arrow:>pad$}",
                    self.desc,
                    self.line,
                    pad = self.loc.column,
                    arrow = "↑",
                )
            }
        }
        Visualize { loc: *self, line, desc }
    }
}

impl Location {
    pub fn new(row: usize, column: usize, file: FileName) -> Self {
        Location { row, column, file }
    }

    pub fn row(&self) -> usize {
        self.row
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn reset(&mut self) {
        self.row = 1;
        self.column = 1;
    }

    pub fn go_right(&mut self) {
        self.column += 1;
    }

    pub fn go_left(&mut self) {
        self.column -= 1;
    }

    pub fn newline(&mut self) {
        self.row += 1;
        self.column = 1;
    }
}
