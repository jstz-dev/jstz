use futures_util::stream::StreamExt;
use jstz_api::js_log::LogLevel;
use jstz_proto::js_logger::LogRecord;
use reqwest_eventsource::Event;

use crate::{error::Result, utils::AddressOrAlias, Config};

const DEFAULT_LOG_LOG_LEVEL: LogLevel = LogLevel::LOG;

pub async fn exec(
    address_or_alias: AddressOrAlias,
    log_level: Option<LogLevel>,
) -> Result<()> {
    let cfg = Config::load()?;

    let address = address_or_alias.resolve(&cfg)?;

    let mut event_source = cfg.jstz_client()?.logs_stream(&address);
    let log_level = log_level.unwrap_or(DEFAULT_LOG_LOG_LEVEL);

    while let Some(event) = event_source.next().await {
        match event {
            Ok(Event::Open) => println!("Event source opened."),
            Ok(Event::Message(message)) => {
                if let Ok(log_record) = serde_json::from_str::<LogRecord>(&message.data) {
                    let LogRecord { level, text, .. } = log_record;
                    if level <= log_level {
                        println!("[{}]: {}", level.symbol(), text);
                    }
                }
            }
            Err(err) => {
                event_source.close();
                eprintln!("Event source closed with an error: {}", err);
            }
        }
    }

    Ok(())
}
