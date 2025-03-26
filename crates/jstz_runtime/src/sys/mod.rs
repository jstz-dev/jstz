//! Raw API bindings for Web APIs

// TODO: We could probably automatically generate these from WebIDL specs

mod interfaces;
pub(crate) mod js;

use deno_core::v8;
pub use interfaces::*;

/// Getter for the `WorkerGlobalScope` object
///
/// [MDN Documentation]
///
/// [MDN Documentation]: ...
pub fn worker_global_scope<'s>(scope: &mut v8::HandleScope<'s>) -> WorkerGlobalScope<'s> {
    let ctx = scope.get_current_context();

    WorkerGlobalScope(ctx.global(scope))
}
