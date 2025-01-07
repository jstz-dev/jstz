use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Duration};

use axum::response::{sse, Sse};
use futures_util::future;
use jstz_proto::context::new_account::NewAddress;
use parking_lot::Mutex;
use tokio::sync::mpsc::{self, Sender};
use tokio::time::interval;
use tokio_stream::wrappers::ReceiverStream;

type InfallibleSseEvent = Result<sse::Event, Infallible>;
pub type InfallibleSSeStream = ReceiverStream<Result<sse::Event, Infallible>>;

/// Broadcasts messages to all connected clients through Server-sent Events
/// <https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events>.
pub struct Broadcaster {
    clients: Mutex<HashMap<NewAddress, Vec<Sender<InfallibleSseEvent>>>>, // TODO: Use a read-write lock instead?
}

// Pings clients every 10 seconds
const PING_INTERVAL: u64 = 10;

impl Broadcaster {
    /// Constructs new broadcaster and spawns ping loop responsible for removing stale clients.
    pub(crate) fn new() -> Arc<Self> {
        let this = Arc::new(Broadcaster::default());

        Broadcaster::spawn_ping(Arc::clone(&this));

        this
    }

    /// Pings clients every `PING_INTERVAL` seconds to see if they are alive and remove them from the broadcast
    /// list if not.
    fn spawn_ping(this: Arc<Self>) {
        tokio::task::spawn(async move {
            let mut interval = interval(Duration::from_secs(PING_INTERVAL));

            loop {
                interval.tick().await;
                this.remove_stale_clients().await;
            }
        });
    }

    /// Removes all non-responsive clients from broadcast list.
    async fn remove_stale_clients(&self) {
        let clients = self.clients.lock().clone();

        let mut responsive_clients: HashMap<NewAddress, Vec<Sender<InfallibleSseEvent>>> =
            HashMap::new();

        for (contract_address, senders) in clients {
            let mut responsive_senders = Vec::new();
            for sender in senders {
                if sender
                    .send(Ok(sse::Event::default().data("ping")))
                    .await
                    .is_ok()
                {
                    responsive_senders.push(sender);
                }
            }
            if !responsive_senders.is_empty() {
                responsive_clients.insert(contract_address, responsive_senders);
            }
        }

        *self.clients.lock() = responsive_clients;
    }

    /// Registers client with broadcaster, returning an SSE response body.
    pub async fn new_client(
        &self,
        function_address: NewAddress,
    ) -> Sse<InfallibleSSeStream> {
        let (tx, rx) = mpsc::channel(10);

        tx.send(Ok(sse::Event::default().data("connected")))
            .await
            .unwrap();

        self.clients
            .lock()
            .entry(function_address)
            .or_default()
            .push(tx);

        let stream = ReceiverStream::new(rx);
        let sse_response = Sse::new(stream);
        sse_response.keep_alive(
            sse::KeepAlive::new()
                .interval(Duration::from_secs(3))
                .text("keep-alive-ping"),
        )
    }

    /// Broadcasts `msg` to all clients.
    pub async fn broadcast(&self, contract_address: &NewAddress, msg: &str) {
        let clients = self.clients.lock().clone();

        if let Some(clients) = clients.get(contract_address) {
            let send_futures = clients
                .iter()
                .map(|client| client.send(Ok(sse::Event::default().data(msg))));
            // try to send to all clients, ignoring failures
            // disconnected clients will get swept up by `remove_stale_clients`
            let _ = future::join_all(send_futures).await;
        }
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Broadcaster {
            clients: Mutex::new(Default::default()),
        }
    }
}
