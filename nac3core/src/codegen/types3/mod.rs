//! Helpers on top of Inkwell primitives.

#![allow(unused)]

mod aggregates;
mod llvm;
mod mem;
mod option;
mod primitives;
mod structs;
mod traits;

pub use aggregates::{LlvmArrayField, LlvmStructField, make_structs};
pub use llvm::{Any, Array, Basic, BasicTag, Int, RawPointer, Struct};
pub use mem::{Memory, Ref, TypedArray};
pub use option::Optional;
pub use primitives::{Bool, I32, I64, Usize};
pub use structs::{
    Exception, ExceptionFields, List, ListFields, Range, RangeFields, String, StringFields,
};
pub use traits::{
    Meta, SubtypeOf, Type, TypeExt, TypeTag, Value, ValueExt, type_tag, type_tag_generic,
};

// Helper empty type used in type tags.
enum Void {}
