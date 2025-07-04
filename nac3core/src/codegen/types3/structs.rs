use std::marker::PhantomData;

use inkwell::{builder::Builder, context::Context, types::BasicTypeEnum};

use super::{
    Basic, BasicTag, I32, I64, RawPointer, Ref, Struct, Type, TypeTag, Usize, Value, Void,
    make_structs, type_tag_generic,
};

make_structs! {
    #[fields(RangeFields)]
    /// well what
    struct Range {
        /// Start of the range.
        pub start: I32,
        pub stop: I32,
        pub step: I32,
    }

    #[fields(StringFields)]
    struct String {
        pub val: RawPointer,
        pub size: Usize,
    }

    #[fields(ListFields)]
    struct List {
        pub val: RawPointer,
        pub size: Usize,
    }

    #[fields(ExceptionFields)]
    struct Exception {
        pub name: I32,
        pub file: String,
        pub line: I32,
        pub col: I32,
        pub func: String,
        pub msg: String,
        pub param0: I64,
        pub param1: I64,
        pub param2: I64,
    }
}

pub struct Tuple<T>(Void, PhantomData<fn(T) -> T>);
impl<T: TypeTag> TypeTag for Tuple<T> {
    // *** Typed pointers! ***
    type Metadata<'ctx> = Vec<T::Metadata<'ctx>>;
}
type_tag_generic!(Tuple : Struct, Basic);

impl<T: TypeTag> Tuple<T> {
    pub fn ty<'ctx>(
        ctx: &'ctx Context,
        types: impl IntoIterator<Item = Type<'ctx, T>>,
    ) -> Type<'ctx, Self>
    where
        T: BasicTag,
    {
        let (types, info): (Vec<_>, Vec<_>) =
            types.into_iter().map(|ty| ty.get::<BasicTypeEnum>()).unzip();
        unsafe { Type::from_raw_parts(ctx.struct_type(&types, false), info) }
    }
}

type _Range = Ref<Range>;
type _String = Ref<String>;
type _List = Ref<List>;

impl String {
    pub fn new<'ctx>(
        builder: &Builder<'ctx>,
        ty: Type<'ctx, Self>,
        fields: StringFields<'ctx>,
        ptr: Value<'ctx, RawPointer>,
        size: Value<'ctx, Usize>,
    ) -> Value<'ctx, Self> {
        let value = ty.poison();
        fields.val.insert(builder, value, ptr);
        fields.size.insert(builder, value, size);
        value
    }
}
