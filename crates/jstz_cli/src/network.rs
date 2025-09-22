use crate::{config::Config, error::Result};
use clap::Subcommand;
use log::info;
use prettytable::{format::consts::FORMAT_DEFAULT, Cell, Row, Table};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List known networks.
    List,
}

pub async fn exec(command: Command) -> Result<()> {
    fn trim_long_string(input: &str, cap: usize) -> String {
        if input.len() > cap {
            return format!("{}...", &input[..cap - 3]);
        };
        input.to_owned()
    }

    let cfg = Config::load_path(None).await?;
    match command {
        Command::List => {
            let mut table = Table::new();
            table.set_titles(Row::new(vec![
                Cell::new("Name"),
                Cell::new("Octez RPC endpoint"),
                Cell::new("Jstz node endpoint"),
            ]));

            let mut rows = cfg
                .networks
                .networks
                .iter()
                .map(|(n, network)| {
                    let name = trim_long_string(n, 20);
                    let octez_endpoint =
                        trim_long_string(&network.octez_node_rpc_endpoint, 25);
                    let jstz_endpoint = trim_long_string(&network.jstz_node_endpoint, 25);
                    (name, octez_endpoint, jstz_endpoint)
                })
                .collect::<Vec<_>>();
            rows.sort_by(|a, b| a.0.cmp(&b.0));

            for (name, octez_endpoint, jstz_endpoint) in rows {
                table.add_row(Row::new(vec![
                    Cell::new(&name),
                    Cell::new(&octez_endpoint),
                    Cell::new(&jstz_endpoint),
                ]));
            }

            table.set_format({
                let mut format = *FORMAT_DEFAULT;
                format.indent(2);
                format
            });

            info!("{table}");
            Ok(())
        }
    }
}
