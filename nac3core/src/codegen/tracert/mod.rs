use std::panic::Location;

use inkwell::{
    AtomicOrdering, context::Context, memory_buffer::MemoryBuffer, module::Module,
    values::BasicValueEnum,
};

use crate::codegen::{CodeGenContext, extern_fns};

#[derive(Clone, Default, Eq, PartialEq)]
pub struct TraceRuntimeConfig {
    pub enabled_tags: Vec<String>,
}

#[derive(Eq, Clone, PartialEq)]
pub struct TraceRuntimeState {
    config: TraceRuntimeConfig,
    indent: usize,
}

impl TraceRuntimeState {
    #[must_use]
    pub fn create(config: TraceRuntimeConfig) -> TraceRuntimeState {
        TraceRuntimeState { config, indent: 0 }
    }
}

#[must_use]
pub fn load_tracert<'ctx>(ctx: &'ctx Context, config: &TraceRuntimeConfig) -> Option<Module<'ctx>> {
    if cfg!(feature = "tracing") && !config.enabled_tags.is_empty() {
        let bitcode_buf = MemoryBuffer::create_from_memory_range(
            include_bytes!(concat!(env!("OUT_DIR"), "/tracert.bc")),
            "tracert_bitcode_buffer",
        );
        let module = Module::parse_bitcode_from_buffer(&bitcode_buf, ctx).unwrap();

        return Some(module);
    }

    None
}

// TODO: Might need to redesign how trace logging should be done

pub fn trace_log<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    tag: &'static str,
    format: &'static str,
    args: &[BasicValueEnum<'ctx>],
) {
    if ctx.tracert_state.is_none() {
        return;
    }

    // TODO: Add indentation
    let str = format!("[TRACING] {tag} - {format}\n\0");
    extern_fns::call_printf(ctx, &str, args);
}

#[track_caller]
pub fn trace_log_with_location<'ctx>(
    ctx: &CodeGenContext<'ctx, '_>,
    tag: &'static str,
    format: &str,
    args: &[BasicValueEnum<'ctx>],
) {
    if ctx.tracert_state.is_none() {
        return;
    }

    // TODO: Add indentation
    let caller_loc = Location::caller();
    let str = format!(
        "[TRACING] {}:{}:{}: {tag} - {format}\n\0",
        caller_loc.file(),
        caller_loc.line(),
        caller_loc.column()
    );
    extern_fns::call_printf(ctx, &str, args);
}

pub fn trace_push_level(ctx: &mut CodeGenContext<'_, '_>) {
    let Some(tracert_state) = &mut ctx.tracert_state else {
        return;
    };

    debug_assert!(tracert_state.indent < usize::MAX);
    if tracert_state.indent < usize::MAX {
        tracert_state.indent = tracert_state.indent.saturating_add(1);
    }
}

pub fn trace_pop_level(ctx: &mut CodeGenContext<'_, '_>) {
    let Some(tracert_state) = &mut ctx.tracert_state else {
        return;
    };

    debug_assert!(tracert_state.indent > 0);
    if tracert_state.indent > 0 {
        tracert_state.indent = tracert_state.indent.saturating_sub(1);
    }
}

#[inline]
pub fn mfence(ctx: &mut CodeGenContext<'_, '_>) {
    if ctx.tracert_state.is_some() {
        ctx.builder.build_fence(AtomicOrdering::SequentiallyConsistent, 0, "").unwrap();
    }
}
