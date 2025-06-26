//! Defines LLVM functions and encapsulates native C ABIs. Everything outside this
//! file should be made unaware of the `sret`/`byval` details involved in C ABIs.

use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Debug,
    marker::PhantomData,
};

use inkwell::{
    AddressSpace,
    attributes::{Attribute, AttributeLoc},
    basic_block::BasicBlock,
    builder::Builder,
    module::{Linkage, Module},
    targets::TargetData,
    types::{AnyType, BasicMetadataTypeEnum, BasicType, BasicTypeEnum},
    values::{BasicMetadataValueEnum, BasicValueEnum, CallSiteValue, FunctionValue, PointerValue},
};
use itertools::Itertools;

const INTERNAL_CALL_CONV: u32 = inkwell::llvm_sys::LLVMCallConv::LLVMFastCallConv as _;

/// An LLVM function declaration.
///
/// Created by [`FunctionStore::declare_external`] and [`FunctionStore::declare_internal`].
/// See their documentation for more.
pub struct FunctionDecl<'ctx> {
    name: String,
    _phantom: PhantomData<&'ctx ()>,
}

impl<'ctx> FunctionDecl<'ctx> {
    fn new(name: String) -> Self {
        Self { name, _phantom: PhantomData }
    }
}

enum FunctionInfo<'ctx> {
    // These fields are somewhat deducible from the function value, but we try
    // not to do that in case the processing from function info to function type
    // becomes lossy.
    External {
        ret: Option<TyAndCallConv<'ctx>>,
        params: Vec<TyAndCallConv<'ctx>>,
        is_c_varargs: bool,
    },
    Internal {
        ret: Option<BasicTypeEnum<'ctx>>,
        params: Vec<BasicMetadataTypeEnum<'ctx>>,
        export: bool,
        // do not use native LLVM varargs here; the caller should convert into a
        // Python-compatible list before calling the function
    },
}

#[derive(Clone, Copy)]
struct TyAndCallConv<'ctx> {
    /// The actual type of this parameter or return value.
    ty: BasicTypeEnum<'ctx>,
    /// Whether this value is passed indirectly.
    indirect: bool,
}

/// Functions in an LLVM module, with ABI details encapsulated.
///
/// # Usage
///
/// Construct with [`FunctionStore::default`]. Always keep this in sync
/// with the relevant module; every construction of a [`FunctionStore`]
/// should be right next to some construction of a [`Module`], and vice
/// versa.
///
/// Declare functions using [`declare_external`] or [`declare_internal`].
/// Call the declared function using [`CodeGenContext::build_call_or_invoke`].
///
/// [`declare_external`]: FunctionStore::declare_external
/// [`declare_internal`]: FunctionStore::declare_internal
/// [`CodeGenContext::build_call_or_invoke`]: crate::codegen::CodeGenContext::build_call_or_invoke
#[derive(Default)]
pub struct FunctionStore<'ctx> {
    functions: HashMap<String, (FunctionValue<'ctx>, FunctionInfo<'ctx>)>,
}

impl<'ctx> FunctionStore<'ctx> {
    /// Declares and registers a function that is defined internally.
    ///
    /// Returns a `(decl, value)` pair.
    /// Call the function using `decl` and define the function using `value`.
    ///
    /// If `export` is set, the function is to be _used_ externally (e.g. for entry points).
    /// You will not be able to use it internally (if that is necessary, create an `export`
    /// wrapper around the non-`export` function).
    /// Note that there is no C ABI handling here, so you have to perform any manipulation
    /// yourself. In the vast majority of cases, however, functions that we export need no
    /// explicit ABI manipulation.
    ///
    /// The registered `value` is guaranteed to have the exact LLVM function signature as
    /// what was requested. However, this means it would not follow the native C calling
    /// convention.
    pub fn declare_internal(
        &mut self,
        module: &Module<'ctx>,
        name: &str,
        ret: Option<BasicTypeEnum<'ctx>>,
        params: &[BasicMetadataTypeEnum<'ctx>],
        export: bool,
    ) -> (FunctionDecl<'ctx>, FunctionValue<'ctx>) {
        let mut new_fn = None;
        self.functions.entry(name.to_owned()).or_insert_with(|| {
            let f = module.add_function(
                &name,
                match ret {
                    Some(ret) => ret.fn_type(params, false),
                    None => module.get_context().void_type().fn_type(params, false),
                },
                None,
            );
            if !export {
                f.set_call_conventions(INTERNAL_CALL_CONV);
            }
            new_fn = Some(f);
            (f, FunctionInfo::Internal { ret, params: params.into(), export })
        });

        let value = new_fn.unwrap_or_else(|| module.get_function(&name).unwrap());
        (FunctionDecl::new(name.into()), value)
    }

    /// Declares and registers a function that be defined externally.
    ///
    /// Returns a function declaration. Note that the registered function signature is designed
    /// to match the C ABI, so you might see a slightly different function signature in LLVM IR.
    pub fn declare_external<'a>(
        &'a mut self,
        module: &Module<'ctx>,
        name: &str,
        ret: Option<BasicTypeEnum<'ctx>>,
        params: &[BasicTypeEnum<'ctx>],
        is_c_varargs: bool,
        fn_attrs: &[&str],
    ) -> FunctionDecl<'ctx> {
        let entry = match self.functions.entry(name.into()) {
            Entry::Occupied(_) => return FunctionDecl::new(name.into()),
            Entry::Vacant(v) => v,
        };

        let ctx = module.get_context();
        let triple = module.get_triple();
        let arch = triple.as_str().to_str().unwrap().split('-').next().unwrap();

        let ret = ret.map(|ty| TyAndCallConv { ty, indirect: indirect_ret(arch, module, ty) });
        let params = params
            .iter()
            .map(|&ty| TyAndCallConv { ty, indirect: indirect_arg(arch, module, ty) })
            .collect_vec();
        let (llvm_ret, sret) = match ret {
            None => (None, None),
            Some(TyAndCallConv { ty, indirect: false }) => (Some(ty), None),
            Some(TyAndCallConv { ty, indirect: true }) => (None, Some(ty)),
        };
        let [attr_sret, attr_byval] = ["sret", "byval"].map(Attribute::get_named_enum_kind_id);
        let ptr = |ty: BasicTypeEnum<'ctx>| ty.ptr_type(AddressSpace::default()).into();
        let (llvm_params, attrs): (Vec<_>, Vec<_>) = sret
            .into_iter()
            .map(|ty| (ptr(ty), (Some((attr_sret, ty)))))
            .chain(params.iter().copied().map(|TyAndCallConv { ty, indirect }| {
                if indirect {
                    // It appears that only `x86_64` emits an actual "byval" attribute for ABI purposes.
                    (ptr(ty), (arch == "x86_64").then_some((attr_byval, ty)))
                } else {
                    (BasicMetadataTypeEnum::from(ty), None)
                }
            }))
            .unzip();
        let info = FunctionInfo::External { ret, params, is_c_varargs };

        let fn_ty = match llvm_ret {
            None => ctx.void_type().fn_type(&llvm_params, is_c_varargs),
            Some(ret) => ret.fn_type(&llvm_params, is_c_varargs),
        };
        let f = module.add_function(name, fn_ty, Some(Linkage::External));
        for (i, (attr, ty)) in attrs.iter().enumerate().filter_map(|(i, &attr)| Some((i, attr?))) {
            f.add_attribute(
                AttributeLoc::Param(i as _),
                ctx.create_type_attribute(attr, ty.as_any_type_enum()),
            );
        }

        for &attr in fn_attrs {
            f.add_attribute(
                AttributeLoc::Function,
                ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        entry.insert((f, info));
        FunctionDecl::new(name.into())
    }

    fn do_call<T>(
        &self,
        decl: &FunctionDecl<'ctx>,
        builder: &Builder<'ctx>,
        args: &[T],
        call: impl FnOnce(FunctionValue<'ctx>, &[T]) -> CallSiteValue<'ctx>,
        mut alloca: impl FnMut(BasicTypeEnum<'ctx>) -> PointerValue<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>>
    where
        T: Copy + TryInto<BasicValueEnum<'ctx>, Error: Debug>,
        BasicValueEnum<'ctx>: Into<T>,
    {
        let ptr_to_t = |p| BasicValueEnum::from(p).into();

        let (value, ref info) = self.functions[&decl.name];
        match info {
            FunctionInfo::External { ret, params, is_c_varargs } => {
                let mut args = args.iter();

                let slot = match *ret {
                    Some(TyAndCallConv { ty, indirect: true }) => Some(alloca(ty)),
                    _ => None,
                };
                let normal_args = params.iter().map(|&TyAndCallConv { ty, indirect }| {
                    let next = *args.next().expect("arguments fewer than parameters");
                    if indirect {
                        let p = alloca(ty);
                        builder.build_store(p, next.try_into().unwrap()).unwrap();
                        ptr_to_t(p)
                    } else {
                        next
                    }
                });
                let mut llvm_args: Vec<_> =
                    slot.into_iter().map(ptr_to_t).chain(normal_args).collect();
                if *is_c_varargs {
                    llvm_args.extend(args.copied());
                } else {
                    assert!(args.as_slice().is_empty(), "too many arguments");
                }

                let result = call(value, &llvm_args).try_as_basic_value().left();
                let result = if let Some(slot) = slot {
                    assert!(result.is_none());
                    Some(builder.build_load(slot, "slot").unwrap())
                } else {
                    result
                };
                assert_eq!(result.map(|val| val.get_type()), ret.map(|ret_type| ret_type.ty));
                result
            }
            FunctionInfo::Internal { ret, params, export } => {
                assert!(!export, "attempted to call a non-exported function");

                let args = args
                    .iter()
                    .zip(params)
                    .map(|(&arg, param)| {
                        if let BasicMetadataTypeEnum::PointerType(p) = *param {
                            let arg = arg.try_into().unwrap().into_pointer_value();
                            if p.get_element_type().is_struct_type()
                                && arg.get_type().get_element_type().is_struct_type()
                            {
                                // HACK(ivan): Ignore mismatches in element types of pointers.
                                // This is because we had implemented inheritance by reinterpreting
                                // types of pointers liberally:
                                // https://git.m-labs.hk/M-Labs/nac3/pulls/295
                                // Fix the root cause of this when migrating to untyped pointers.

                                let arg = builder.build_pointer_cast(arg, p, "").unwrap();
                                return ptr_to_t(arg);
                            }
                        }
                        arg
                    })
                    .collect_vec();

                let inst = call(value, &args);
                inst.set_call_convention(INTERNAL_CALL_CONV);
                let result = inst.try_as_basic_value().left();
                assert_eq!(result.map(|val| val.get_type()), *ret);
                result
            }
        }
    }

    /// Calls a function given its declaration.
    pub(crate) fn call(
        &self,
        decl: &FunctionDecl<'ctx>,
        builder: &Builder<'ctx>,
        args: &[BasicMetadataValueEnum<'ctx>],
        name: &str,
        alloca: impl FnMut(BasicTypeEnum<'ctx>) -> PointerValue<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        self.do_call(
            decl,
            builder,
            args,
            |value, args| builder.build_call(value, args, name).unwrap(),
            alloca,
        )
    }

    /// Calls a function given its declaration, with exception handling.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn invoke(
        &self,
        decl: &FunctionDecl<'ctx>,
        builder: &Builder<'ctx>,
        args: &[BasicValueEnum<'ctx>],
        then_block: BasicBlock<'ctx>,
        catch_block: BasicBlock<'ctx>,
        name: &str,
        alloca: impl FnMut(BasicTypeEnum<'ctx>) -> PointerValue<'ctx>,
    ) -> Option<BasicValueEnum<'ctx>> {
        self.do_call(
            decl,
            builder,
            args,
            |value, args| builder.build_invoke(value, args, then_block, catch_block, name).unwrap(),
            alloca,
        )
    }
}

/// Whether `sret` is needed for a return value with type `ty`.
///
/// When returning a large data structure (e.g. structures that do not fit in 1-2 native words of
/// the target processor) by value, a synthetic parameter with a pointer type will be passed in the
/// slot of the first parameter to act as the location of which the return value is passed into.
///
/// See <https://releases.llvm.org/16.0.0/docs/LangRef.html#parameter-attributes> for more
/// information.
///
/// # Implementation notes
///
/// This (and [`indirect_arg`]) are incomplete and probably wrong, but this is almost impossible
/// to get completely correct if we don't hook into a big compiler. Review this when adding more types
/// in call signatures.
///
/// Zig's approach seems the easiest; might refer to it if any more ABI problems show up.
/// <https://github.com/ziglang/zig/tree/cf1a7bbd44b9542552c7b5dc6532aafb5142bf7a/src/arch>
///
/// Also refer to rustc's impl:
/// <https://github.com/rust-lang/rust/tree/255aa220821c05c3eac7605fce4ea1c9ab2cbdb4/compiler/rustc_target/src/callconv>
fn indirect_ret(arch: &str, module: &Module<'_>, ret: BasicTypeEnum<'_>) -> bool {
    // LLVM's TargetTriple has methods to access separate components, but inkwell does not
    // expose them. We use a rudimentary approach to parse the triple.
    match arch {
        "x86_64" => x86_64_indirect_ret(module, ret),
        "armv7" => arm_indirect_ret(module, ret, false),
        "aarch64" => arm_indirect_ret(module, ret, true),
        "riscv32" => riscv_indirect_ret(module, ret),
        _ => unimplemented!("unsupported arch for extern fn: {arch}"),
    }
}

fn indirect_arg(arch: &str, module: &Module<'_>, ty: BasicTypeEnum<'_>) -> bool {
    // armv7 appears to never pass arguments indirectly at all
    arch != "armv7" && indirect_ret(arch, module, ty)
}

fn bits_of(module: &Module<'_>, ty: BasicTypeEnum<'_>) -> u64 {
    TargetData::create(module.get_data_layout().as_str().to_str().unwrap()).get_bit_size(&ty)
}

fn arm_homogeneous_aggregate(module: &Module<'_>, ty: BasicTypeEnum<'_>) -> Option<u32> {
    // On ARM architectures, returning a struct of exactly 1-4 floats is through registers.
    match ty {
        BasicTypeEnum::FloatType(_) => Some(1),
        BasicTypeEnum::IntType(_) | BasicTypeEnum::PointerType(_) if bits_of(module, ty) <= 64 => {
            None
        }
        BasicTypeEnum::StructType(s) => s
            .get_field_types_iter()
            .map(|ty| arm_homogeneous_aggregate(module, ty))
            .sum::<Option<u32>>()
            .filter(|&n| n <= 4),
        _ => unreachable!(),
    }
}

fn arm_indirect_ret(module: &Module<'_>, ret: BasicTypeEnum<'_>, aarch64: bool) -> bool {
    !matches!(
        ret,
        BasicTypeEnum::FloatType(_) | BasicTypeEnum::IntType(_) | BasicTypeEnum::PointerType(_)
    ) && bits_of(module, ret) > if aarch64 { 128 } else { 32 }
        && arm_homogeneous_aggregate(module, ret).is_none()
}

fn riscv_indirect_ret(module: &Module<'_>, ret: BasicTypeEnum<'_>) -> bool {
    match ret {
        BasicTypeEnum::FloatType(_) | BasicTypeEnum::IntType(_) | BasicTypeEnum::PointerType(_) => {
            false
        }
        BasicTypeEnum::StructType(s) => {
            let (mut f, mut i) = (0, 0);
            for field in s.get_field_types_iter() {
                match field {
                    BasicTypeEnum::FloatType(_) => f += 1,
                    BasicTypeEnum::IntType(_) => i += 1,
                    _ => return true,
                }
            }
            (f + i) <= 2 && i <= 1
        }
        _ if bits_of(module, ret) > 64 => true,
        _ => unreachable!(),
    }
}

fn x86_64_indirect_ret(module: &Module<'_>, ret: BasicTypeEnum<'_>) -> bool {
    // There's a lot of logic determining which class each "EIGHTBYTE" (64-bit) component refers to.
    // However, if we limit ourselves to:
    // - not have unaligned values;
    // - not have SIMD vectors;
    // - only care about return values (where we always have enough registers);
    // then the "minimum" class that each EIGHTBYTE component can be assigned to is INTEGER,
    // unless the size of the struct is > 128 bits, where everything is assigned MEMORY.
    //
    // So for our specific case, `need_sret` is just a size check.
    bits_of(module, ret) > 128
}
