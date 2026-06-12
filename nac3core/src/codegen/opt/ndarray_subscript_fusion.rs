use itertools::Itertools as _;
use nac3parser::ast::{
    Expr, ExprContext, ExprKind, Located, Stmt,
    fold::{self, Fold},
};

use crate::{
    codegen::CodeGenContext,
    toplevel::helper::PrimDef,
    typecheck::typedef::{Type, Unifier},
};

/// Rewrites chained integer subscripts on ndarrays (`arr[i][j]...`) into a single multi-axis
/// subscript (`arr[i, j, ...]`) throughout `body`, so that the whole access lowers to one indexing
/// operation instead of materializing an intermediate view ndarray per level.
pub fn fuse_ndarray_subscripts(
    ctx: &mut CodeGenContext<'_, '_>,
    body: Vec<Stmt<Option<Type>>>,
) -> anyhow::Result<Vec<Stmt<Option<Type>>>> {
    let mut folder =
        NDArraySubscriptFusion { unifier: &mut ctx.unifier, int32: ctx.primitives.int32 };
    body.into_iter().map(|stmt| folder.fold_stmt(stmt)).collect()
}

/// AST folder backing [`fuse_ndarray_subscripts`].
///
/// This rewrite performs a local transformation of the form `a[i][j] -> a[i, j]` on any subscript
/// whose inner subscript is an ndarray indexed purely by integers.
struct NDArraySubscriptFusion<'a> {
    unifier: &'a mut Unifier,
    int32: Type,
}

impl NDArraySubscriptFusion<'_> {
    /// Returns whether `ty` is an ndarray.
    fn is_ndarray(&self, ty: Option<Type>) -> bool {
        ty.is_some_and(|ty| ty.obj_id(self.unifier).is_some_and(|id| id == PrimDef::NDArray.id()))
    }

    /// Returns whether `ty` is an integer index (`int32`).
    fn is_int_index(&mut self, ty: Option<Type>) -> bool {
        ty.is_some_and(|ty| self.unifier.unioned(ty, self.int32))
    }

    /// Returns whether every index in a subscript `slice` is a single integer.
    fn all_integer_indices(&mut self, slice: &Expr<Option<Type>>) -> bool {
        match &slice.node {
            ExprKind::Tuple { elts, .. } => elts.iter().all(|elt| self.is_int_index(elt.custom)),
            _ => self.is_int_index(slice.custom),
        }
    }

    /// Flattens a subscript `slice` into its list of index expressions.
    fn flatten_slice(slice: Expr<Option<Type>>) -> Vec<Expr<Option<Type>>> {
        match slice.node {
            ExprKind::Tuple { elts, .. } => elts,
            _ => vec![slice],
        }
    }

    /// Returns whether `node` is a fusion candidate - i.e. a subscript whose inner expression is an
    /// ndarray subscript indexed purely by integers.
    fn is_fusable(&mut self, node: &Expr<Option<Type>>) -> bool {
        let ExprKind::Subscript { value, .. } = &node.node else {
            return false;
        };
        let ExprKind::Subscript { value: inner_base, slice: inner_slice, .. } = &value.node else {
            return false;
        };
        self.is_ndarray(inner_base.custom) && self.all_integer_indices(inner_slice)
    }
}

impl Fold<Option<Type>> for NDArraySubscriptFusion<'_> {
    type TargetU = Option<Type>;
    type Error = anyhow::Error;

    fn map_user(&mut self, user: Option<Type>) -> Result<Self::TargetU, Self::Error> {
        Ok(user)
    }

    fn fold_expr(&mut self, node: Expr<Option<Type>>) -> Result<Expr<Self::TargetU>, Self::Error> {
        // Fold children first so longer chains collapse bottom-up (`a[i][j][k]` is handled one
        // level per ascent of the fold).
        let node = fold::fold_expr(self, node)?;

        if !self.is_fusable(&node) {
            return Ok(node);
        }

        let Located { location, custom, node: ExprKind::Subscript { value, slice, ctx } } = node
        else {
            unreachable!()
        };
        let ExprKind::Subscript { value: inner_base, slice: inner_slice, .. } = value.node else {
            unreachable!()
        };

        // `base[a][b] -> base[a, b]`: the inner indices precede the outer indices.
        let elts = Self::flatten_slice(*inner_slice)
            .into_iter()
            .chain(Self::flatten_slice(*slice))
            .collect_vec();
        let fused_slice = Located {
            location,
            custom: None,
            node: ExprKind::Tuple { elts, ctx: ExprContext::Load },
        };

        Ok(Located {
            location,
            custom,
            node: ExprKind::Subscript { value: inner_base, slice: Box::new(fused_slice), ctx },
        })
    }
}
