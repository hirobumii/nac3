//! This module provides a type-safe interface to getting and calling Python attributes using the
//! Python interpreter, while providing caching for attributes to minimize FFI calls.
//!
//! Python modules are organized into modules in Rust. Within each module, there are four methods
//! to access Python attributes:
//!
//! - `get_*`: Returns a reference to a [`Bound`] handle to the Python class or variable with the
//!   given name.
//! - `*_fn`: Returns a reference to a [`Bound`] handle to the Python function with the given name.
//! - `call_*`: Calls the Python function with the given name, returning a [`Bound`] handle to the
//!   return value.
//! - `extract_*`: Calls the Python function with the given name, returning the Rust representation
//!   of the return value.
//!
//! Moreover, each module also provides a `module` function to obtain a reference to a [`Bound`]
//! handle to the Python module itself.

pub use builtins::*;

/// The [`builtins`](https://docs.python.org/3/library/builtins.html) module.
///
/// Functions in this module can also be directly accessed via the `py_interp` module for
/// consistency with Python.
pub mod builtins {
    use pyo3::{
        prelude::*,
        sync::GILOnceCell,
        types::{PyAnyMethods, PyBool, PyCFunction, PyInt, PyModule, PyString, PyType},
    };

    /// Returns a reference to this module.
    pub fn module(py: Python<'_>) -> PyResult<&Bound<'_, PyModule>> {
        static MODULE: GILOnceCell<Py<PyModule>> = GILOnceCell::new();

        MODULE
            .get_or_try_init(py, || {
                let module = PyModule::import(py, "builtins")?;
                Ok(module.unbind())
            })
            .map(|module| module.bind(py))
    }

    /// Returns a reference to the
    /// [`Exception`](https://docs.python.org/3/library/exceptions.html#Exception) class.
    pub fn get_exception_class(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
        static EXCEPTION_CLASS: GILOnceCell<Py<PyType>> = GILOnceCell::new();

        EXCEPTION_CLASS.import(py, "builtins", "Exception")
    }

    /// Returns a reference to the [`id`](https://docs.python.org/3/library/functions.html#id)
    /// function.
    pub fn id_fn(py: Python<'_>) -> PyResult<&Bound<'_, PyCFunction>> {
        static ID_FN: GILOnceCell<Py<PyCFunction>> = GILOnceCell::new();

        ID_FN.import(py, "builtins", "id")
    }

    /// Invokes [`id(object)`][id_fn], extracting its value and returning a [`u64`] representing
    /// the result.
    pub fn extract_id(object: &Bound<'_, PyAny>) -> PyResult<u64> {
        id_fn(object.py())?.call1((object,))?.downcast_into::<PyInt>()?.extract()
    }

    /// Returns a reference to the
    /// [`issubclass`](https://docs.python.org/3/library/functions.html#issubclass) function.
    pub fn issubclass_fn(py: Python<'_>) -> PyResult<&Bound<'_, PyCFunction>> {
        static ISSUBCLASS_FN: GILOnceCell<Py<PyCFunction>> = GILOnceCell::new();

        ISSUBCLASS_FN.import(py, "builtins", "issubclass")
    }

    /// Invokes [`issubclass(object)`][issubclass_fn] extracing its value and returning a [`bool`]
    /// representing the result.
    pub fn extract_issubclass(
        class: &Bound<'_, PyAny>,
        classinfo: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        issubclass_fn(class.py())?.call1((class, classinfo))?.downcast_into::<PyBool>()?.extract()
    }

    /// Returns a reference to the [`len`](https://docs.python.org/3/library/functions.html#len)
    /// function.
    pub fn len_fn(py: Python<'_>) -> PyResult<&Bound<'_, PyCFunction>> {
        static LEN_FN: GILOnceCell<Py<PyCFunction>> = GILOnceCell::new();

        LEN_FN.import(py, "builtins", "len")
    }

    /// Invokes [`len(object)`][len_fn], extracting its value and returning a [`usize`]
    /// representing the result.
    pub fn extract_len(object: &Bound<'_, PyAny>) -> PyResult<usize> {
        len_fn(object.py())?.call1((object,))?.downcast_into::<PyInt>()?.extract()
    }

    /// Returns a reference to the
    /// [`repr`](https://docs.python.org/3/library/functions.html#repr) function.
    pub fn repr_fn(py: Python<'_>) -> PyResult<&Bound<'_, PyCFunction>> {
        static REPR_FN: GILOnceCell<Py<PyCFunction>> = GILOnceCell::new();

        REPR_FN.import(py, "builtins", "repr")
    }

    /// Invokes [`repr(object)`][repr_fn], extracting its value and returning a [`String`]
    /// representing the result.
    pub fn extract_repr(object: &Bound<'_, PyAny>) -> PyResult<String> {
        repr_fn(object.py())?.call1((object,))?.downcast_into::<PyString>()?.extract()
    }

    /// Returns a reference to the
    /// [`type`](https://docs.python.org/3/library/functions.html#type) class.
    pub fn get_type_class(py: Python<'_>) -> PyResult<&Bound<'_, PyType>> {
        static TYPE_FN: GILOnceCell<Py<PyType>> = GILOnceCell::new();

        TYPE_FN.import(py, "builtins", "type")
    }

    /// Invokes [`type(object)`][type_fn], returning a [`PyType`] representing the result.
    pub fn call_type<'py>(object: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyType>> {
        Ok(get_type_class(object.py())?.call1((object,))?.downcast_into()?)
    }
}

/// The `typing` module.
pub mod typing {
    use pyo3::{
        prelude::*,
        sync::GILOnceCell,
        types::{PyAnyMethods, PyFunction, PyModule, PyTuple},
    };

    /// Returns a reference to this module.
    #[allow(dead_code, reason = "For API consistency between all `py_interp` modules.")]
    pub fn module(py: Python<'_>) -> PyResult<&Bound<'_, PyModule>> {
        static MODULE: GILOnceCell<Py<PyModule>> = GILOnceCell::new();

        MODULE
            .get_or_try_init(py, || {
                let module = PyModule::import(py, "typing")?;
                Ok(module.unbind())
            })
            .map(|module| module.bind(py))
    }

    /// Returns a reference to the
    /// [`typing.get_args`](https://docs.python.org/3/library/typing.html#typing.get_args) function.
    pub fn get_args_fn(py: Python<'_>) -> PyResult<&Bound<'_, PyFunction>> {
        static GET_ARGS_FN: GILOnceCell<Py<PyFunction>> = GILOnceCell::new();

        GET_ARGS_FN.import(py, "typing", "get_args")
    }

    /// Invokes [`typing.get_args(tp)`][get_args_fn], returning a [`PyTuple`] representing the
    /// result.
    pub fn call_get_args<'py>(tp: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
        Ok(get_args_fn(tp.py())?.call1((tp,))?.downcast_into()?)
    }

    /// Returns a reference to the
    /// [`typing.get_origin`](https://docs.python.org/3/library/typing.html#typing.get_origin)
    /// function.
    pub fn get_origin_fn(py: Python<'_>) -> PyResult<&Bound<'_, PyFunction>> {
        static GET_ORIGIN_FN: GILOnceCell<Py<PyFunction>> = GILOnceCell::new();

        GET_ORIGIN_FN.import(py, "typing", "get_origin")
    }

    /// Invokes [`typing.get_origin(tp)`][get_origin_fn], returning a [`PyAny`] representing the
    /// result.
    pub fn call_get_origin<'py>(tp: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        Ok(get_origin_fn(tp.py())?.call1((tp,))?.downcast_into()?)
    }
}
