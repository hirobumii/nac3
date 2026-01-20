#![deny(future_incompatible, let_underscore, nonstandard_style, clippy::all)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cognitive_complexity,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::wildcard_imports
)]

mod dwarf;
mod include;
mod linker;
mod symbolizer;

pub use linker::Linker;

use pyo3::pymodule;

#[pymodule]
mod nac3tools {
    use pyo3::prelude::*;
    use pyo3::pyfunction;

    use pyo3::types::{PyAnyMethods, PyBytes, PyList};

    use crate::symbolizer;
    use crate::symbolizer::CallRecord;

    #[pyfunction]
    fn symbolize<'py>(
        elf_bin: &Bound<'py, PyBytes>,
        pc: &Bound<'py, PyList>,
    ) -> PyResult<Vec<CallRecord>> {
        Ok(symbolizer::symbolize(elf_bin.extract()?, pc.extract()?))
    }
}
