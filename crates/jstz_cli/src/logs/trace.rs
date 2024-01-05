use crate::Config;
use anyhow::Result;
use futures_util::stream::StreamExt;
use jstz_proto::executor::contract::LogRecord;
use reqwest_eventsource::{Event, EventSource};

pub async fn exec(address_or_alias: String, cfg: &Config) -> Result<()> {
    let address = cfg.accounts.get_address(&address_or_alias)?;
    let url = format!(
        "http://127.0.0.1:{}/logs/{}/stream",
        cfg.sandbox()?.jstz_node_port,
        &address.to_base58()
    );

    let mut event_source = EventSource::get(&url);

    while let Some(event) = event_source.next().await {
        match event {
            Ok(Event::Open) => println!("Connection open with {}", url),
            Ok(Event::Message(message)) => {
                if let Ok(log_record) = serde_json::from_str::<LogRecord>(&message.data) {
                    let LogRecord { level, text, .. } = log_record;
                    println!("[{}]: {}", level.symbol(), text);
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
