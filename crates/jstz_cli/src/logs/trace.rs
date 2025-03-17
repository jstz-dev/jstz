use futures_util::{stream::StreamExt, Future};
use jstz_api::js_log::LogLevel;
use jstz_proto::js_logger::LogRecord;
use log::{debug, error, info};
use reqwest_eventsource::{Event, EventSource};

use crate::{config::NetworkName, error::Result, utils::AddressOrAlias, Config};

pub const DEFAULT_LOG_LEVEL: LogLevel = LogLevel::LOG;

pub async fn exec(
    address_or_alias: AddressOrAlias,
    log_level: LogLevel,
    network: &Option<NetworkName>,
) -> Result<()> {
    let cfg = Config::load().await?;

    let address = address_or_alias.resolve(&cfg)?;
    debug!("resolved `address_or_alias` -> {:?}", address);

    let event_source = cfg.jstz_client(network)?.logs_stream(&address);

    exec_trace(event_source, log_level, || async {
        info!("Connected to smart function '{}'.", address);
    })
    .await?;

    Ok(())
}

pub async fn exec_trace<F, Fut>(
    mut event_source: EventSource,
    log_level: LogLevel,
    on_connect: F,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: Future<Output = ()>,
{
    while let Some(event) = event_source.next().await {
        match event {
            Ok(Event::Open) => {
                on_connect().await;
            }
            Ok(Event::Message(message)) => {
                if let Ok(log_record) = serde_json::from_str::<LogRecord>(&message.data) {
                    let LogRecord { level, text, .. } = log_record;
                    if level <= log_level {
                        info!("[{}]: {}", level.symbol(), text);
                    }
                }
            }
            Err(err) => {
                event_source.close();
                error!("Event source closed with an error: {}", err);
            }
        }
    }

    Ok(())
}
