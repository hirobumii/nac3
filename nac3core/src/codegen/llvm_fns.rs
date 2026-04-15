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
    builder::Builder,
    module::Linkage,
    targets::TargetData,
    types::{AnyType, BasicMetadataTypeEnum, BasicType, BasicTypeEnum, PointerType},
    values::{BasicValueEnum, CallSiteValue, FunctionValue, PointerValue},
};
use itertools::Itertools as _;

use crate::codegen::{ModuleContext, TargetMachineOptions};

const INTERNAL_CALL_CONV: u32 = inkwell::llvm_sys::LLVMCallConv::LLVMFastCallConv as _;

/// An LLVM function declaration.
///
/// Created by [`FunctionStore::declare_external`] and [`FunctionStore::declare_internal`].
/// See their documentation for more.
pub struct FunctionDecl<'ctx> {
    name: String,
    _phantom: PhantomData<&'ctx ()>,
}

impl FunctionDecl<'_> {
    const fn new(name: String) -> Self {
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
enum ArgCallConv {
    Direct,
    Indirect(Option<Attribute>),
}

#[derive(Clone, Copy)]
struct TyAndCallConv<'ctx> {
    /// The actual type of this parameter or return value.
    ty: BasicTypeEnum<'ctx>,
    call_conv: ArgCallConv,
}

fn get_attrs(
    a: impl IntoIterator<Item = Option<Attribute>>,
) -> impl Iterator<Item = (AttributeLoc, Attribute)> {
    a.into_iter().enumerate().filter_map(|(i, attr)| Some((AttributeLoc::Param(i as _), attr?)))
}

pub(super) struct FunctionStore<'ctx> {
    functions: HashMap<String, (FunctionValue<'ctx>, FunctionInfo<'ctx>)>,
    arch: String,
}

impl<'ctx> ModuleContext<'ctx> {
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
        name: &str,
        ret: Option<BasicTypeEnum<'ctx>>,
        params: &[BasicMetadataTypeEnum<'ctx>],
        export: bool,
    ) -> (FunctionDecl<'ctx>, FunctionValue<'ctx>) {
        let ModuleContext { ctx, module, fn_store, .. } = self;

        let mut new_fn = None;
        fn_store.functions.entry(name.to_owned()).or_insert_with(|| {
            let f = module.add_function(
                name,
                ret.map_or_else(
                    || ctx.void_type().fn_type(params, false),
                    |ret| ret.fn_type(params, false),
                ),
                None,
            );
            if !export {
                f.set_call_conventions(INTERNAL_CALL_CONV);
            }
            new_fn = Some(f);
            (f, FunctionInfo::Internal { ret, params: params.into(), export })
        });

        let value = new_fn.unwrap_or_else(|| module.get_function(name).unwrap());
        (FunctionDecl::new(name.into()), value)
    }

    /// Declares and registers a function that be defined externally.
    ///
    /// Returns a function declaration. Note that the registered function signature is designed
    /// to match the C ABI, so you might see a slightly different function signature in LLVM IR.
    pub fn declare_external(
        &mut self,
        name: &str,
        ret: Option<BasicTypeEnum<'ctx>>,
        params: &[BasicTypeEnum<'ctx>],
        is_c_varargs: bool,
        fn_attrs: &[&str],
    ) -> FunctionDecl<'ctx> {
        let ModuleContext { ctx, ref module, ref target, ref mut fn_store, .. } = *self;

        let entry = match fn_store.functions.entry(name.into()) {
            Entry::Occupied(_) => return FunctionDecl::new(name.into()),
            Entry::Vacant(v) => v,
        };

        let arch = &*fn_store.arch;
        let layout = target.get_target_data();

        let attr_sret = (arch == "x86_64" || arch == "i686" || arch == "riscv32")
            .then(|| Attribute::get_named_enum_kind_id("sret"));
        let attr_byval = (arch == "x86_64" || arch == "i686")
            .then(|| Attribute::get_named_enum_kind_id("byval"));
        let get_conv = |attr: Option<u32>, ty, indirect_check: fn(_, _, _) -> bool| TyAndCallConv {
            ty,
            call_conv: if indirect_check(arch, &layout, ty) {
                ArgCallConv::Indirect(
                    attr.map(|x| ctx.create_type_attribute(x, AnyType::as_any_type_enum(&ty))),
                )
            } else {
                ArgCallConv::Direct
            },
        };

        let ret = ret.map(|ty| get_conv(attr_sret, ty, indirect_ret));
        let params = params.iter().map(|&ty| get_conv(attr_byval, ty, indirect_arg)).collect_vec();

        let (llvm_ret, sret) = match ret {
            Some(TyAndCallConv { ty, call_conv: ArgCallConv::Direct, .. }) => (Some(ty), None),
            ret => (None, ret),
        };
        let (llvm_params, attrs): (Vec<_>, Vec<_>) = sret
            .into_iter()
            .chain(params.iter().copied())
            .map(|TyAndCallConv { ty, call_conv }| match call_conv {
                ArgCallConv::Indirect(attr) => (ctx.ptr_type(AddressSpace::default()).into(), attr),
                ArgCallConv::Direct => (BasicMetadataTypeEnum::from(ty), None),
            })
            .unzip();

        let fn_ty = llvm_ret.map_or_else(
            || ctx.void_type().fn_type(&llvm_params, is_c_varargs),
            |ret| ret.fn_type(&llvm_params, is_c_varargs),
        );
        let f = module.add_function(name, fn_ty, Some(Linkage::External));
        for (loc, attr) in get_attrs(attrs) {
            f.add_attribute(loc, attr);
        }

        for &attr in fn_attrs {
            f.add_attribute(
                AttributeLoc::Function,
                ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
            );
        }

        let info = FunctionInfo::External { ret, params, is_c_varargs };
        entry.insert((f, info));
        FunctionDecl::new(name.into())
    }
}

impl<'ctx> FunctionStore<'ctx> {
    pub(crate) fn new(options: &TargetMachineOptions) -> Self {
        Self {
            functions: HashMap::default(),
            arch: options.triple.split('-').next().unwrap().to_owned(),
        }
    }

    pub(crate) fn do_call<T>(
        &self,
        decl: &FunctionDecl<'ctx>,
        builder: &Builder<'ctx>,
        args: &[T],
        call: impl FnOnce(FunctionValue<'ctx>, &[T]) -> anyhow::Result<CallSiteValue<'ctx>>,
        mut alloca: impl FnMut(BasicTypeEnum<'ctx>) -> anyhow::Result<PointerValue<'ctx>>,
    ) -> anyhow::Result<Option<BasicValueEnum<'ctx>>>
    where
        T: Copy + TryInto<BasicValueEnum<'ctx>, Error: Debug>,
        BasicValueEnum<'ctx>: Into<T>,
    {
        let ptr_to_t = |p| BasicValueEnum::from(p).into();

        let fixup_ptr_arg = |arg: T, _param: PointerType<'ctx>| {
            // With opaque pointers all pointers are the same type. No casting needed.
            anyhow::Ok(arg)
        };

        let (value, ref info) = self.functions[&decl.name];
        match info {
            FunctionInfo::External { ret, params, is_c_varargs } => {
                let mut args = args.iter();

                let slot = match *ret {
                    Some(TyAndCallConv { ty, call_conv: ArgCallConv::Indirect(attr) }) => {
                        Some((alloca(ty)?, ty, attr))
                    }
                    _ => None,
                };
                let normal_args = params
                    .iter()
                    .map(|&TyAndCallConv { ty, call_conv }| {
                        let mut next = *args.next().expect("arguments fewer than parameters");
                        if let BasicTypeEnum::PointerType(p) = ty {
                            next = fixup_ptr_arg(next, p)?;
                        }

                        if let ArgCallConv::Indirect(attr) = call_conv {
                            let p = alloca(ty)?;
                            let next_bve: BasicValueEnum = next.try_into().unwrap();
                            builder.build_store(p, next_bve)?;
                            anyhow::Ok((ptr_to_t(p), attr))
                        } else {
                            Ok((next, None))
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let normal_slot = slot.map(|(p, _ty, attr)| (ptr_to_t(p), attr));
                let (mut llvm_args, attrs): (Vec<_>, Vec<_>) =
                    normal_slot.into_iter().chain(normal_args).unzip();
                if *is_c_varargs {
                    llvm_args.extend(args.copied());
                } else {
                    assert!(args.as_slice().is_empty(), "too many arguments");
                }

                let result = call(value, &llvm_args)?;
                for (loc, attr) in get_attrs(attrs) {
                    result.add_attribute(loc, attr);
                }

                let mut result = result.try_as_basic_value().basic();
                if let Some((ptr, ty, _)) = slot {
                    assert!(result.is_none());
                    result = Some(builder.build_load(ty, ptr, "slot")?);
                }
                assert_eq!(result.map(|val| val.get_type()), ret.map(|ret_type| ret_type.ty));
                Ok(result)
            }
            FunctionInfo::Internal { ret, params, export } => {
                assert!(!export, "attempted to call a non-exported function");

                let args = args
                    .iter()
                    .zip(params)
                    .map(|(&arg, param)| {
                        if let BasicMetadataTypeEnum::PointerType(p) = *param {
                            fixup_ptr_arg(arg, p)
                        } else {
                            Ok(arg)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let inst = call(value, &args)?;
                inst.set_call_convention(INTERNAL_CALL_CONV);
                let result = inst.try_as_basic_value().basic();
                assert_eq!(result.map(|val| val.get_type()), *ret);
                Ok(result)
            }
        }
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
fn indirect_ret(arch: &str, layout: &TargetData, ret: BasicTypeEnum<'_>) -> bool {
    // LLVM's TargetTriple has methods to access separate components, but inkwell does not
    // expose them. We use a rudimentary approach to parse the triple.
    match arch {
        "x86_64" => x86_64_indirect_ret(layout, ret),
        "armv7" => arm_indirect_ret(layout, ret, false),
        "aarch64" => arm_indirect_ret(layout, ret, true),
        "riscv32" => riscv_indirect_ret(layout, ret),
        "i686" => x86_indirect_ret(layout, ret),
        arch => unimplemented!("unsupported arch for extern fn: {arch}"),
    }
}

fn indirect_arg(arch: &str, layout: &TargetData, ty: BasicTypeEnum<'_>) -> bool {
    // armv7 appears to never pass arguments indirectly at all
    arch != "armv7" && indirect_ret(arch, layout, ty)
}

fn arm_homogeneous_aggregate(layout: &TargetData, ty: BasicTypeEnum<'_>) -> Option<u32> {
    // On ARM architectures, returning a struct of exactly 1-4 floats is through registers.
    match ty {
        BasicTypeEnum::FloatType(_) => Some(1),
        BasicTypeEnum::IntType(_) | BasicTypeEnum::PointerType(_)
            if layout.get_bit_size(&ty) <= 64 =>
        {
            None
        }
        BasicTypeEnum::StructType(s) => s
            .get_field_types_iter()
            .map(|ty| arm_homogeneous_aggregate(layout, ty))
            .sum::<Option<u32>>()
            .filter(|&n| n <= 4),
        _ => unreachable!(),
    }
}

fn arm_indirect_ret(layout: &TargetData, ret: BasicTypeEnum<'_>, aarch64: bool) -> bool {
    !matches!(
        ret,
        BasicTypeEnum::FloatType(_) | BasicTypeEnum::IntType(_) | BasicTypeEnum::PointerType(_)
    ) && layout.get_bit_size(&ret) > if aarch64 { 128 } else { 32 }
        && arm_homogeneous_aggregate(layout, ret).is_none()
}

fn riscv_indirect_ret(layout: &TargetData, ret: BasicTypeEnum<'_>) -> bool {
    match ret {
        BasicTypeEnum::FloatType(_) | BasicTypeEnum::IntType(_) | BasicTypeEnum::PointerType(_) => {
            false
        }
        _ if layout.get_bit_size(&ret) <= 64 => false,
        BasicTypeEnum::StructType(s) => {
            let (mut f, mut i) = (0, 0);
            for field in s.get_field_types_iter() {
                match field {
                    BasicTypeEnum::FloatType(_) => f += 1,
                    BasicTypeEnum::IntType(_) => i += 1,
                    _ => return true,
                }
            }
            !((f + i) <= 2 && i <= 1)
        }
        _ => unreachable!(),
    }
}

fn x86_64_indirect_ret(layout: &TargetData, ret: BasicTypeEnum<'_>) -> bool {
    // There's a lot of logic determining which class each "EIGHTBYTE" (64-bit) component refers to.
    // However, if we limit ourselves to:
    // - not have unaligned values;
    // - not have SIMD vectors;
    // - only care about return values (where we always have enough registers);
    // then the "minimum" class that each EIGHTBYTE component can be assigned to is INTEGER,
    // unless the size of the struct is > 128 bits, where everything is assigned MEMORY.
    //
    // So for our specific case, `need_sret` is just a size check.
    layout.get_bit_size(&ret) > 128
}

fn x86_indirect_ret(_layout: &TargetData, ret: BasicTypeEnum<'_>) -> bool {
    // All aggregates are passed indirectly, even those with just 1 element.
    ret.is_struct_type() || ret.is_array_type()
}
