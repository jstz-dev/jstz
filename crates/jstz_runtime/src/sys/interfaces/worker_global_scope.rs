use deno_core::v8;

pub struct WorkerGlobalScope<'a>(pub(crate) v8::Local<'a, v8::Object>);
