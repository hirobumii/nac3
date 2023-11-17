use inkwell::{
    IntPredicate,
    types::BasicType,
    values::PointerValue,
};
use nac3parser::ast::StrRef;
use crate::{
    codegen::{
        CodeGenContext,
        CodeGenerator,
        irrt::{call_ndarray_calc_size, call_ndarray_init_dims},
        stmt::gen_for_callback
    },
    symbol_resolver::ValueEnum,
    toplevel::DefinitionId,
    typecheck::typedef::{FunSignature, Type, TypeEnum},
};

/// LLVM-typed implementation for generating the implementation for constructing an `NDArray`.
///
/// * `elem_ty` - The element type of the NDArray.
/// * `var_name` - The variable name of the NDArray.
/// * `shape` - The `shape` parameter used to construct the NDArray.
fn call_ndarray_impl<'ctx, 'a>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, 'a>,
    elem_ty: Type,
    var_name: Option<&str>,
    shape: PointerValue<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
    let ndarray_ty_enum = TypeEnum::ndarray(&mut ctx.unifier, Some(elem_ty), None, &ctx.primitives);
    let ndarray_ty = ctx.unifier.add_ty(ndarray_ty_enum);

    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_usize = generator.get_size_type(ctx.ctx);

    let llvm_pndarray_t = ctx.get_llvm_type(generator, ndarray_ty).into_pointer_type();
    let llvm_ndarray_t = llvm_pndarray_t.get_element_type().into_struct_type();
    let llvm_ndarray_data_t = ctx.get_llvm_type(generator, elem_ty).as_basic_type_enum();
    assert!(llvm_ndarray_data_t.is_sized());

    // Assert that all dimensions are non-negative
    gen_for_callback(
        generator,
        ctx,
        |_, ctx| {
            let i = ctx.builder.build_alloca(llvm_usize, "");
            ctx.builder.build_store(i, llvm_usize.const_zero());

            Ok(i)
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();
            let shape_len = ctx.build_gep_and_load(
                shape,
                &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
                None,
            ).into_int_value();

            Ok(ctx.builder.build_int_compare(IntPredicate::ULE, i, shape_len, ""))
        },
        |generator, ctx, i_addr| {
            let shape_elems = ctx.build_gep_and_load(
                shape,
                &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                None
            ).into_pointer_value();

            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();
            let shape_dim = ctx.build_gep_and_load(
                shape_elems,
                &[i],
                None
            ).into_int_value();

            let shape_dim_gez = ctx.builder.build_int_compare(
                IntPredicate::SGE,
                shape_dim,
                llvm_i32.const_zero(),
                ""
            );

            ctx.make_assert(
                generator,
                shape_dim_gez,
                "0:ValueError",
                "negative dimensions not supported",
                [None, None, None],
                ctx.current_loc,
            );

            Ok(())
        },
        |_, ctx, i_addr| {
            let i = ctx.builder
                .build_load(i_addr, "")
                .into_int_value();
            let i = ctx.builder.build_int_add(i, llvm_usize.const_int(1, true), "");
            ctx.builder.build_store(i_addr, i);

            Ok(())
        },
    )?;

    let ndarray = ctx.builder.build_alloca(
        llvm_ndarray_t,
        var_name.unwrap_or_default()
    );

    let num_dims = ctx.build_gep_and_load(
        shape,
        &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
        None
    ).into_int_value();

    let ndarray_num_dims = unsafe {
        ctx.builder.build_in_bounds_gep(
            ndarray,
            &[llvm_i32.const_zero(), llvm_i32.const_zero()],
            "",
        )
    };
    ctx.builder.build_store(ndarray_num_dims, num_dims);

    let ndarray_dims = unsafe {
        ctx.builder.build_in_bounds_gep(
            ndarray,
            &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
            "",
        )
    };

    let ndarray_num_dims = ctx.build_gep_and_load(
        ndarray,
        &[llvm_i32.const_zero(), llvm_i32.const_zero()],
        None,
    ).into_int_value();

    ctx.builder.build_store(
        ndarray_dims,
        ctx.builder.build_array_alloca(
            llvm_usize,
            ndarray_num_dims,
            "",
        ),
    );

    call_ndarray_init_dims(generator, ctx, ndarray, shape);

    let ndarray_num_elems = call_ndarray_calc_size(generator, ctx, shape);

    let ndarray_data = unsafe {
        ctx.builder.build_in_bounds_gep(
            ndarray,
            &[llvm_i32.const_zero(), llvm_i32.const_int(2, true)],
            "",
        )
    };
    ctx.builder.build_store(
        ndarray_data,
        ctx.builder.build_array_alloca(
            llvm_ndarray_data_t,
            ndarray_num_elems,
            "",
        ),
    );

    Ok(ndarray)
}

/// Generates LLVM IR for `ndarray.empty`.
pub fn gen_ndarray_empty<'ctx, 'a>(
    context: &mut CodeGenContext<'ctx, 'a>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<PointerValue<'ctx>, String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let shape_ty = fun.0.args[0].ty;
    let shape_arg_name = args[0].0;
    let shape_arg = args[0].1.clone()
        .to_basic_value_enum(context, generator, shape_ty)?;

    call_ndarray_impl(
        generator,
        context,
        context.primitives.float,
        shape_arg_name.map(|name| name.to_string()).as_deref(),
        shape_arg.into_pointer_value(),
    )
}