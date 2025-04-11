//! Raw API bindings for Web APIs

mod interfaces;
pub(crate) mod js;

use deno_core::v8;
pub use interfaces::*;

/// Getter for the `WorkerGlobalScope` object
///
/// [MDN Documentation]
///
/// [MDN Documentation]: : https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope
pub fn worker_global_scope<'s>(scope: &mut v8::HandleScope<'s>) -> WorkerGlobalScope<'s> {
    let ctx = scope.get_current_context();

    WorkerGlobalScope(ctx.global(scope))
}
