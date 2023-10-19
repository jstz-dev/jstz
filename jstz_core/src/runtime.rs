use std::{
    cell::RefCell,
    collections::VecDeque,
    future::poll_fn,
    io::Read,
    ops::{Deref, DerefMut},
    rc::Rc,
    task::Poll,
};

use boa_engine::{
    builtins::promise::PromiseState, job::NativeJob, object::builtins::JsPromise,
    Context, JsError, JsNativeError, JsResult, JsValue, Source,
};

use crate::{
    future,
    host::{Host, HostRuntime},
    realm::{Module, Realm},
};

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
    /// Thread-local host
    static HOST: RefCell<Option<Host>> = RefCell::new(None)
}

pub fn with_host_runtime<F, R>(hrt: &mut (impl HostRuntime + 'static), f: F) -> R
where
    F: FnOnce() -> R,
{
    HOST.with(|host| *host.borrow_mut() = Some(unsafe { Host::new(hrt) }));

    let result = f();

    HOST.with(|host| *host.borrow_mut() = None);

    result
}

pub fn with_global_host<F, R>(f: F) -> R
where
    F: FnOnce(&mut Host) -> R,
{
    HOST.with(|host| f(host.borrow_mut().as_mut().expect("Host should be set")))
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
            .job_queue(job_queue.clone() as Rc<dyn boa_engine::job::JobQueue>)
            .instructions_remaining(gas_limit)
            .build()
            .unwrap();

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
