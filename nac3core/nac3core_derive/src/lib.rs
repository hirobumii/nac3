use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Data, DataStruct, Expr, ExprField, ExprMethodCall,
    ExprPath, GenericArgument, Ident, LitStr, Path, PathArguments, Type, TypePath,
};

/// Extracts all generic arguments of a [`Type`] into a [`Vec`].
///
/// Returns [`Some`] of a possibly-empty [`Vec`] if the path of `ty` matches with
/// `expected_ty_name`, otherwise returns [`None`].
fn extract_generic_args(expected_ty_name: &'static str, ty: &Type) -> Option<Vec<GenericArgument>> {
    let Type::Path(TypePath { qself: None, path, .. }) = ty else {
        return None;
    };

    let segments = &path.segments;
    if segments.len() != 1 {
        return None;
    };

    let segment = segments.iter().next().unwrap();
    if segment.ident != expected_ty_name {
        return None;
    }

    let PathArguments::AngleBracketed(path_args) = &segment.arguments else {
        return Some(Vec::new());
    };
    let args = &path_args.args;

    Some(args.iter().cloned().collect::<Vec<_>>())
}

/// Maps a `path` matching one of the `target_idents` into the `replacement` [`Ident`].
fn map_path_to_ident(path: &Path, target_idents: &[&str], replacement: &str) -> Option<Ident> {
    path.require_ident()
        .ok()
        .filter(|ident| target_idents.iter().any(|target| ident == target))
        .map(|ident| Ident::new(replacement, ident.span()))
}

/// Extracts the left-hand side of a dot-expression.
fn extract_dot_operand(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::MethodCall(ExprMethodCall { receiver: operand, .. })
        | Expr::Field(ExprField { base: operand, .. }) => Some(operand),
        _ => None,
    }
}

/// Replaces the top-level receiver of a dot-expression with an [`Ident`], returning `Some(&mut expr)` if the
/// replacement is performed.
///
/// The top-level receiver is the left-most receiver expression, e.g. the top-level receiver of `a.b.c.foo()` is `a`.
fn replace_top_level_receiver(expr: &mut Expr, ident: Ident) -> Option<&mut Expr> {
    if let Expr::MethodCall(ExprMethodCall { receiver: operand, .. })
    | Expr::Field(ExprField { base: operand, .. }) = expr
    {
        return if extract_dot_operand(operand).is_some() {
            if replace_top_level_receiver(operand, ident).is_some() {
                Some(expr)
            } else {
                None
            }
        } else {
            *operand = Box::new(Expr::Path(ExprPath {
                attrs: Vec::default(),
                qself: None,
                path: ident.into(),
            }));

            Some(expr)
        };
    }

    None
}

/// Iterates all operands to the left-hand side of the `.` of an [expression][`Expr`], i.e. the container operand of all
/// [`Expr::Field`] and the receiver operand of all [`Expr::MethodCall`].
///
/// The iterator will return the operand expressions in reverse order of appearance. For example, `a.b.c.func()` will
/// return `vec![c, b, a]`.
fn iter_dot_operands(expr: &Expr) -> impl Iterator<Item = &Expr> {
    let mut o = extract_dot_operand(expr);

    std::iter::from_fn(move || {
        let this = o;
        o = o.as_ref().and_then(|o| extract_dot_operand(o));

        this
    })
}

/// Normalizes a value expression for use when creating an instance of this structure, returning a
/// [`proc_macro2::TokenStream`] of tokens representing the normalized expression.
fn normalize_value_expr(expr: &Expr) -> proc_macro2::TokenStream {
    match &expr {
        Expr::Path(ExprPath { qself: None, path, .. }) => {
            if let Some(ident) = map_path_to_ident(path, &["usize", "size_t"], "llvm_usize") {
                quote! { #ident }
            } else {
                abort!(
                    path,
                    format!(
                        "Expected one of `size_t`, `usize`, or an implicit call expression in #[value_type(...)], found {}", 
                        quote!(#expr).to_string(),
                    )
                )
            }
        }

        Expr::Call(_) => {
            quote! { ctx.#expr }
        }

        Expr::MethodCall(_) => {
            let base_receiver = iter_dot_operands(expr).last();

            match base_receiver {
                // `usize.{...}`, `size_t.{...}` -> Rewrite the identifiers to `llvm_usize`
                Some(Expr::Path(ExprPath { qself: None, path, .. }))
                    if map_path_to_ident(path, &["usize", "size_t"], "llvm_usize").is_some() =>
                {
                    let ident =
                        map_path_to_ident(path, &["usize", "size_t"], "llvm_usize").unwrap();

                    let mut expr = expr.clone();
                    let expr = replace_top_level_receiver(&mut expr, ident).unwrap();

                    quote!(#expr)
                }

                // `ctx.{...}`, `context.{...}` -> Rewrite the identifiers to `ctx`
                Some(Expr::Path(ExprPath { qself: None, path, .. }))
                    if map_path_to_ident(path, &["ctx", "context"], "ctx").is_some() =>
                {
                    let ident = map_path_to_ident(path, &["ctx", "context"], "ctx").unwrap();

                    let mut expr = expr.clone();
                    let expr = replace_top_level_receiver(&mut expr, ident).unwrap();

                    quote!(#expr)
                }

                // No reserved identifier prefix -> Prepend `ctx.` to the entire expression
                _ => quote! { ctx.#expr },
            }
        }

        _ => {
            abort!(
                expr,
                format!(
                    "Expected one of `size_t`, `usize`, or an implicit call expression in #[value_type(...)], found {}", 
                    quote!(#expr).to_string(),
                )
            )
        }
    }
}

/// Derives an implementation of `codegen::types::structure::StructFields`.
///
/// The benefit of using `#[derive(StructFields)]` is that all index- or order-dependent logic required by
/// `impl StructFields` is automatically generated by this implementation, including the field index as required by
/// `StructField::new` and the fields as returned by `StructFields::to_vec`.
///
/// # Prerequisites
///
/// In order to derive from [`StructFields`], you must implement (or derive) [`Eq`] and [`Copy`] as required by
/// `StructFields`.
///
/// Moreover, `#[derive(StructFields)]` can only be used for `struct`s with named fields, and may only contain fields
/// with either `StructField` or [`PhantomData`] types.
///
/// # Attributes for [`StructFields`]
///
/// Each `StructField` field must be declared with the `#[value_type(...)]` attribute. The argument of `value_type`
/// accepts one of the following:
///
/// - An expression returning an instance of `inkwell::types::BasicType` (with or without the receiver `ctx`/`context`).
/// For example, `context.i8_type()`, `ctx.i8_type()`, and `i8_type()` all refer to `i8`.
/// - The reserved identifiers `usize` and `size_t` referring to an `inkwell::types::IntType` of the platform-dependent
/// integer size. `usize` and `size_t` can also be used as the receiver to other method calls, e.g.
/// `usize.array_type(3)`.
///
/// # Example
///
/// The following is an example of an LLVM slice implemented using `#[derive(StructFields)]`.
///
/// ```rust,ignore
/// use nac3core::{
///     codegen::types::structure::StructField,
///     inkwell::{
///         values::{IntValue, PointerValue},
///         AddressSpace,
///     },
/// };
/// use nac3core_derive::StructFields;
///
/// // All classes that implement StructFields must also implement Eq and Copy
/// #[derive(PartialEq, Eq, Clone, Copy, StructFields)]
/// pub struct SliceValue<'ctx> {
///     // Declares ptr have a value type of i8*
///     //
///     // Can also be written as `ctx.i8_type().ptr_type(...)` or `context.i8_type().ptr_type(...)`
///     #[value_type(i8_type().ptr_type(AddressSpace::default()))]
///     ptr: StructField<'ctx, PointerValue<'ctx>>,
///
///     // Declares len have a value type of usize, depending on the target compilation platform
///     #[value_type(usize)]
///     len: StructField<'ctx, IntValue<'ctx>>,
/// }
/// ```
#[proc_macro_derive(StructFields, attributes(value_type))]
#[proc_macro_error]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let ident = &input.ident;

    let Data::Struct(DataStruct { fields, .. }) = &input.data else {
        abort!(input, "Only structs with named fields are supported");
    };
    if let Err(err_span) =
        fields
            .iter()
            .try_for_each(|field| if field.ident.is_some() { Ok(()) } else { Err(field.span()) })
    {
        abort!(err_span, "Only structs with named fields are supported");
    };

    // Check if struct<'ctx>
    if input.generics.params.len() != 1 {
        abort!(input.generics, "Expected exactly 1 generic parameter")
    }

    let phantom_info = fields
        .iter()
        .filter(|field| extract_generic_args("PhantomData", &field.ty).is_some())
        .map(|field| field.ident.as_ref().unwrap())
        .cloned()
        .collect::<Vec<_>>();

    let field_info = fields
        .iter()
        .filter(|field| extract_generic_args("PhantomData", &field.ty).is_none())
        .map(|field| {
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;

            let Some(_) = extract_generic_args("StructField", ty) else {
                abort!(field, "Only StructField and PhantomData are allowed")
            };

            let attrs = &field.attrs;
            let Some(value_type_attr) =
                attrs.iter().find(|attr| attr.path().is_ident("value_type"))
            else {
                abort!(field, "Expected #[value_type(...)] attribute for field");
            };

            let Ok(value_type_expr) = value_type_attr.parse_args::<Expr>() else {
                abort!(value_type_attr, "Expected expression in #[value_type(...)]");
            };

            let value_expr_toks = normalize_value_expr(&value_type_expr);

            (ident.clone(), value_expr_toks)
        })
        .collect::<Vec<_>>();

    // `<*>::new` impl of `StructField` and `PhantomData` for `StructFields::new`
    let phantoms_create = phantom_info
        .iter()
        .map(|id| quote! { #id: ::std::marker::PhantomData })
        .collect::<Vec<_>>();
    let fields_create = field_info
        .iter()
        .map(|(id, ty)| {
            let id_lit = LitStr::new(&id.to_string(), id.span());
            quote! {
                #id: ::nac3core::codegen::types::structure::StructField::create(
                    &mut counter,
                    #id_lit,
                    #ty,
                )
            }
        })
        .collect::<Vec<_>>();

    // `.into()` impl of `StructField` for `StructFields::to_vec`
    let fields_into =
        field_info.iter().map(|(id, _)| quote! { self.#id.into() }).collect::<Vec<_>>();

    let impl_block = quote! {
        impl<'ctx> ::nac3core::codegen::types::structure::StructFields<'ctx> for #ident<'ctx> {
            fn new(ctx: impl ::nac3core::inkwell::context::AsContextRef<'ctx>, llvm_usize: ::nac3core::inkwell::types::IntType<'ctx>) -> Self {
                let ctx = unsafe { ::nac3core::inkwell::context::ContextRef::new(ctx.as_ctx_ref()) };

                let mut counter = ::nac3core::codegen::types::structure::FieldIndexCounter::default();

                #ident {
                    #(#fields_create),*
                    #(#phantoms_create),*
                }
            }

            fn to_vec(&self) -> ::std::vec::Vec<(&'static str, ::nac3core::inkwell::types::BasicTypeEnum<'ctx>)> {
                vec![
                    #(#fields_into),*
                ]
            }
        }
    };

    impl_block.into()
}
