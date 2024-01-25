use crate::Config;
use anyhow::Result;
use futures_util::stream::StreamExt;
use jstz_api::js_log::LogLevel;
use jstz_proto::js_logger::LogRecord;
use reqwest_eventsource::{Event, EventSource};

const DEFAULT_LOG_LOG_LEVEL: LogLevel = LogLevel::LOG;

pub async fn exec(
    address_or_alias: String,
    log_level: Option<LogLevel>,
    cfg: &Config,
) -> Result<()> {
    let address = cfg.accounts.get_address(&address_or_alias)?;
    let url = format!(
        "http://127.0.0.1:{}/logs/{}/stream",
        cfg.sandbox()?.jstz_node_port,
        &address.to_base58()
    );

    let mut event_source = EventSource::get(&url);
    let log_level = log_level.unwrap_or(DEFAULT_LOG_LOG_LEVEL);

    while let Some(event) = event_source.next().await {
        match event {
            Ok(Event::Open) => println!("Connection open with {}", url),
            Ok(Event::Message(message)) => {
                if let Ok(log_record) = serde_json::from_str::<LogRecord>(&message.data) {
                    let LogRecord { level, text, .. } = log_record;
                    if level <= log_level {
                        println!("[{}]: {}", level.symbol(), text);
                    }
                }
            }
            Err(err) => {
                println!("Event source error: {}", err);
                event_source.close();
            }
        }
    }

    Ok(())
}
