use std::{
    cell::RefCell,
    collections::VecDeque,
    future::poll_fn,
    io::Read,
    num::NonZeroU32,
    ops::{Deref, DerefMut},
    rc::Rc,
    task::Poll,
};

use boa_engine::{
    builtins::promise::PromiseState,
    context::HostHooks,
    job::NativeJob,
    object::builtins::{JsFunction, JsPromise},
    Context, JsError, JsNativeError, JsResult, JsValue, Source,
};
use chrono::{DateTime, FixedOffset, LocalResult, NaiveDateTime};
use getrandom::{register_custom_getrandom, Error as RandomError};

use crate::{
    future,
    host::{HostRuntime, JsHostRuntime},
    kv::{JsTransaction, Transaction},
    realm::{Module, Realm},
};

// This is the unix timestamp for date 31-07-2023 10:50:26 -- the date of the first commit
const UTC_NOW: i64 = 1690797026;

struct Hooks;

impl HostHooks for Hooks {
    fn ensure_can_compile_strings(
        &self,
        _realm: boa_engine::realm::Realm,
        _context: &mut Context<'_>,
    ) -> JsResult<()> {
        Err(JsNativeError::typ()
            .with_message("eval calls not available")
            .into())
    }

    fn has_source_text_available(
        &self,
        _function: &JsFunction,
        _context: &mut Context<'_>,
    ) -> bool {
        false
    }

    fn utc_now(&self) -> NaiveDateTime {
        NaiveDateTime::from_timestamp_opt(UTC_NOW, 0)
            .expect("Failed to create `NaiveDateTime` from `UTC_NOW`")
    }

    fn local_from_utc(&self, utc: NaiveDateTime) -> DateTime<FixedOffset> {
        DateTime::from_naive_utc_and_offset(utc, FixedOffset::east_opt(0).unwrap())
    }

    fn local_from_naive_local(
        &self,
        _local: NaiveDateTime,
    ) -> LocalResult<DateTime<FixedOffset>> {
        LocalResult::None
    }
}

pub const HOOKS: &'static dyn HostHooks = &Hooks;

// custom getrandom
const GETRANDOM_ERROR_CODE: u32 = RandomError::CUSTOM_START + 42;
fn always_fail(_: &mut [u8]) -> std::result::Result<(), RandomError> {
    let code = NonZeroU32::new(GETRANDOM_ERROR_CODE).unwrap();
    Err(RandomError::from(code))
}

register_custom_getrandom!(always_fail);

/// A 'pollable' job queue
#[derive(Default, Debug)]
struct JobQueue(RefCell<VecDeque<NativeJob>>);

impl JobQueue {
    pub fn new() -> Self {
        Self::default()
    }

    fn next(&self) -> Option<NativeJob> {
        self.0.borrow_mut().pop_front()
    }

    pub fn call_next(&self, context: &mut Context<'_>) -> Option<JsResult<JsValue>> {
        let job = self.next()?;
        Some(job.call(context))
    }
}

impl boa_engine::job::JobQueue for JobQueue {
    fn enqueue_promise_job(
        &self,
        job: NativeJob,
        _context: &mut boa_engine::Context<'_>,
    ) {
        self.0.borrow_mut().push_back(job);
    }

    fn enqueue_future_job(
        &self,
        future: boa_engine::job::FutureJob,
        context: &mut boa_engine::Context<'_>,
    ) {
        let job = future::block_on(future);
        self.enqueue_promise_job(job, context);
    }

    fn run_jobs(&self, context: &mut boa_engine::Context<'_>) {
        while let Some(job) = self.next() {
            // Jobs can fail, it is the final result that determines the value
            let _ = job.call(context);
        }
    }
}

thread_local! {
    /// Thread-local host context
    static JS_HOST_RUNTIME: RefCell<Option<JsHostRuntime>> = RefCell::new(None);

    /// Thread-local transaction
    static JS_TRANSACTION: RefCell<Option<JsTransaction>> = RefCell::new(None);
}

/// Enters a new host context, running the closure `f` with the new context
pub fn enter_js_host_context<F, R>(
    hrt: &mut (impl HostRuntime + 'static),
    tx: &mut Transaction,
    f: F,
) -> R
where
    F: FnOnce() -> R,
{
    JS_HOST_RUNTIME
        .with(|js_hrt| *js_hrt.borrow_mut() = Some(unsafe { JsHostRuntime::new(hrt) }));

    JS_TRANSACTION
        .with(|js_tx| *js_tx.borrow_mut() = Some(unsafe { JsTransaction::new(tx) }));

    let result = f();

    JS_HOST_RUNTIME.with(|hrt| {
        *hrt.borrow_mut() = None;
    });

    JS_TRANSACTION.with(|tx| {
        *tx.borrow_mut() = None;
    });

    result
}

/// Returns a reference to the host runtime in the current js host context
pub fn with_js_hrt<F, R>(f: F) -> R
where
    F: FnOnce(&mut JsHostRuntime) -> R,
{
    JS_HOST_RUNTIME.with(|hrt| {
        f(hrt
            .borrow_mut()
            .as_mut()
            .expect("`JS_HOST_RUNTIME` should be set"))
    })
}

/// Returns a reference to the transaction in the current js host context
pub fn with_js_tx<F, R>(f: F) -> R
where
    F: FnOnce(&mut Transaction) -> R,
{
    JS_TRANSACTION.with(|tx| {
        f(tx.borrow_mut()
            .as_mut()
            .expect("`JS_TRANSACTION` should be set"))
    })
}

pub fn with_js_hrt_and_tx<F, R>(f: F) -> R
where
    F: FnOnce(&mut JsHostRuntime, &mut Transaction) -> R,
{
    with_js_hrt(|hrt| with_js_tx(|tx| f(hrt, tx)))
}

#[derive(Debug)]
pub struct Runtime<'host> {
    context: Context<'host>,
    realm: Realm,
    // There will only ever be 2 references to the `job_queue`.
    // The context's internal reference and the runtime's reference.
    job_queue: Rc<JobQueue>,
}

impl<'host> Deref for Runtime<'host> {
    type Target = Context<'host>;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl<'host> DerefMut for Runtime<'host> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}

impl<'host> Runtime<'host> {
    pub fn new(gas_limit: usize) -> JsResult<Self> {
        // 1. Initialize job queue
        let job_queue = Rc::new(JobQueue::new());

        // 2. Initialize context with job queue
        // NB: At this point, the context contains a 'default' realm
        let mut context = Context::builder()
            .host_hooks(HOOKS)
            .job_queue(job_queue.clone() as Rc<dyn boa_engine::job::JobQueue>)
            .instructions_remaining(gas_limit)
            .build()?;

        // 3. Initialize specialized realm
        let realm = Realm::new(&mut context)?;

        context.enter_realm(realm.inner.clone());

        Ok(Self {
            context,
            realm,
            job_queue,
        })
    }

    /// Parses, loads, links and evaluates a module.
    ///
    /// Returns the module instance and the module promise. Implementors must manually
    /// call `Runtime::run_event_loop` or poll/resolve the promise to drive the
    /// module's evaluation.  
    pub fn eval_module(&mut self, module: &Module) -> JsResult<JsPromise> {
        self.realm.eval_module(module, &mut self.context)
    }

    /// Parses, compiles and evaluates the script `src`.
    pub fn eval<R: Read>(&mut self, src: Source<'_, R>) -> JsResult<JsValue> {
        self.realm.eval(src, &mut self.context)
    }

    pub fn context(&mut self) -> &mut Context<'host> {
        self.deref_mut()
    }

    pub fn realm(&self) -> &Realm {
        &self.realm
    }

    /// Runs the event loop (job queue) to completion
    pub async fn run_event_loop(&mut self) {
        poll_fn(|_| self.poll_event_loop()).await
    }

    /// Runs a single tick of the event loop
    pub fn poll_event_loop(&mut self) -> Poll<()> {
        match self.job_queue.call_next(&mut self.context) {
            None => {
                self.context.clear_kept_objects();
                Poll::Ready(())
            }
            Some(_) => Poll::Pending,
        }
    }

    fn poll_promise(promise: JsPromise) -> Poll<JsResult<JsValue>> {
        match promise.state()? {
            PromiseState::Pending => Poll::Pending,
            PromiseState::Fulfilled(result) => Poll::Ready(Ok(result)),
            PromiseState::Rejected(err) => Poll::Ready(Err(JsError::from_opaque(err))),
        }
    }

    /// Polls a given value to resolve by stepping the event loop
    pub fn poll_value(&mut self, value: &JsValue) -> Poll<JsResult<JsValue>> {
        match value.as_promise() {
            Some(promise) => {
                let promise = JsPromise::from_object(promise.clone())?;
                match Self::poll_promise(promise) {
                    Poll::Ready(val) => Poll::Ready(val),
                    Poll::Pending => match self.poll_event_loop() {
                        Poll::Ready(()) => Poll::Ready(Err(JsNativeError::error()
                            .with_message("Event loop did not resolve the promise")
                            .into())),
                        Poll::Pending => Poll::Pending,
                    },
                }
            }
            None => Poll::Ready(Ok(value.clone())),
        }
    }

    /// Waits for the given value to resolve while polling the event loop
    pub async fn resolve_value(&mut self, value: &JsValue) -> JsResult<JsValue> {
        poll_fn(|_| self.poll_value(value)).await
    }
}
