use crate::codegen::{model::*, CodeGenContext, CodeGenerator};

/// Fields of [`Slice`]
#[derive(Debug, Clone)]
pub struct SliceFields<'ctx, F: FieldTraversal<'ctx>, N: IntKind<'ctx>> {
    pub start_defined: F::Output<Int<Bool>>,
    pub start: F::Output<Int<N>>,
    pub stop_defined: F::Output<Int<Bool>>,
    pub stop: F::Output<Int<N>>,
    pub step_defined: F::Output<Int<Bool>>,
    pub step: F::Output<Int<N>>,
}

/// An IRRT representation of an (unresolved) slice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Slice<N>(pub N);

impl<'ctx, N: IntKind<'ctx>> StructKind<'ctx> for Slice<N> {
    type Fields<F: FieldTraversal<'ctx>> = SliceFields<'ctx, F, N>;

    fn iter_fields<F: FieldTraversal<'ctx>>(&self, traversal: &mut F) -> Self::Fields<F> {
        Self::Fields {
            start_defined: traversal.add_auto("start_defined"),
            start: traversal.add("start", Int(self.0)),
            stop_defined: traversal.add_auto("stop_defined"),
            stop: traversal.add("stop", Int(self.0)),
            step_defined: traversal.add_auto("step_defined"),
            step: traversal.add("step", Int(self.0)),
        }
    }
}

/// A Rust structure that has [`Slice`] utilities and looks like a [`Slice`] but
/// `start`, `stop` and `step` are held by LLVM registers only and possibly
/// [`Option::None`] if unspecified.
#[derive(Debug, Clone)]
pub struct RustSlice<'ctx, N: IntKind<'ctx>> {
    // It is possible that `start`, `stop`, and `step` are all `None`.
    // We need to know the `int_kind` even when that is the case.
    pub int_kind: N,
    pub start: Option<Instance<'ctx, Int<N>>>,
    pub stop: Option<Instance<'ctx, Int<N>>>,
    pub step: Option<Instance<'ctx, Int<N>>>,
}

impl<'ctx, N: IntKind<'ctx>> RustSlice<'ctx, N> {
    /// Write the contents to an LLVM [`Slice`].
    pub fn write_to_slice<G: CodeGenerator + ?Sized>(
        &self,
        generator: &mut G,
        ctx: &CodeGenContext<'ctx, '_>,
        dst_slice_ptr: Instance<'ctx, Ptr<Struct<Slice<N>>>>,
    ) {
        let false_ = Int(Bool).const_false(generator, ctx.ctx);
        let true_ = Int(Bool).const_true(generator, ctx.ctx);

        match self.start {
            Some(start) => {
                dst_slice_ptr.gep(ctx, |f| f.start_defined).store(ctx, true_);
                dst_slice_ptr.gep(ctx, |f| f.start).store(ctx, start);
            }
            None => dst_slice_ptr.gep(ctx, |f| f.start_defined).store(ctx, false_),
        }

        match self.stop {
            Some(stop) => {
                dst_slice_ptr.gep(ctx, |f| f.stop_defined).store(ctx, true_);
                dst_slice_ptr.gep(ctx, |f| f.stop).store(ctx, stop);
            }
            None => dst_slice_ptr.gep(ctx, |f| f.stop_defined).store(ctx, false_),
        }

        match self.step {
            Some(step) => {
                dst_slice_ptr.gep(ctx, |f| f.step_defined).store(ctx, true_);
                dst_slice_ptr.gep(ctx, |f| f.step).store(ctx, step);
            }
            None => dst_slice_ptr.gep(ctx, |f| f.step_defined).store(ctx, false_),
        }
    }
}

pub mod util {
    use nac3parser::ast::Expr;

    use crate::{
        codegen::{model::*, CodeGenContext, CodeGenerator},
        typecheck::typedef::Type,
    };

    use super::RustSlice;

    /// Generate LLVM IR for an [`ExprKind::Slice`] and convert it into a [`RustSlice`].
    #[allow(clippy::type_complexity)]
    pub fn gen_slice<'ctx, G: CodeGenerator>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        lower: &Option<Box<Expr<Option<Type>>>>,
        upper: &Option<Box<Expr<Option<Type>>>>,
        step: &Option<Box<Expr<Option<Type>>>>,
    ) -> Result<RustSlice<'ctx, Int32>, String> {
        let mut help = |value_expr: &Option<Box<Expr<Option<Type>>>>| -> Result<_, String> {
            Ok(match value_expr {
                None => None,
                Some(value_expr) => {
                    let value_expr = generator
                        .gen_expr(ctx, value_expr)?
                        .unwrap()
                        .to_basic_value_enum(ctx, generator, ctx.primitives.int32)?;

                    let value_expr =
                        Int(Int32).check_value(generator, ctx.ctx, value_expr).unwrap();

                    Some(value_expr)
                }
            })
        };

        let start = help(lower)?;
        let stop = help(upper)?;
        let step = help(step)?;

        Ok(RustSlice { int_kind: Int32, start, stop, step })
    }
}
