use std::{collections::HashMap, sync::Arc, time::Duration};

use actix_web::rt::time::interval;
use actix_web_lab::{
    sse::{self, Data, Event, Sse},
    util::InfallibleStream,
};
use futures_util::future;
use jstz_proto::context::account::Address;
use parking_lot::Mutex;
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;

pub struct Broadcaster {
    clients: Mutex<HashMap<Address, Vec<Sender<Event>>>>, // TODO: Use a read-write lock instead?
}

// Pings clients every 10 seconds
const PING_INTERVAL: u64 = 10;

impl Broadcaster {
    /// Constructs new broadcaster and spawns ping loop responsible for removing stale clients.
    pub fn create() -> Arc<Self> {
        let this = Arc::new(Broadcaster::new());

        Broadcaster::spawn_ping(Arc::clone(&this));

        this
    }

    fn new() -> Self {
        Broadcaster {
            clients: Mutex::new(Default::default()),
        }
    }

    /// Pings clients every `PING_INTERVAL` seconds to see if they are alive and remove them from the broadcast
    /// list if not.
    fn spawn_ping(this: Arc<Self>) {
        actix_web::rt::spawn(async move {
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

        let mut responsive_clients: HashMap<Address, Vec<Sender<Event>>> = HashMap::new();

        for (contract_address, senders) in clients {
            let mut responsive_senders = Vec::new();
            for sender in senders {
                if sender
                    .send(sse::Event::Comment("ping".into()))
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
        contract_address: Address,
    ) -> Sse<InfallibleStream<ReceiverStream<sse::Event>>> {
        let (tx, rx) = mpsc::channel(10);

        tx.send(sse::Data::new("connected").into()).await.unwrap();

        self.clients
            .lock()
            .entry(contract_address)
            .or_default()
            .push(tx);

        Sse::from_infallible_receiver(rx)
    }

    /// Broadcasts `msg` to all clients.
    pub async fn broadcast(&self, contract_address: &Address, msg: &str) {
        let clients = self.clients.lock().clone();

        if let Some(clients) = clients.get(contract_address) {
            let send_futures = clients
                .iter()
                .map(|client| client.send(Data::new(msg).into()));

            // try to send to all clients, ignoring failures
            // disconnected clients will get swept up by `remove_stale_clients`
            let _ = future::join_all(send_futures).await;
        }
    }
}
