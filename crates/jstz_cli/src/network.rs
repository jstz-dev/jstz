use crate::{
    config::{Config, Network},
    error::{bail_user_error, Result},
};
use clap::Subcommand;
use log::info;
use prettytable::{format::consts::FORMAT_DEFAULT, Cell, Row, Table};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List known networks.
    List,
    /// Add a new network.
    Add {
        /// Name of the new network.
        #[arg(value_name = "NETWORK_NAME")]
        name: String,
        /// Octez node RPC endpoint.
        #[arg(long)]
        octez_node_rpc_endpoint: String,
        /// Jstz node API endpoint.
        #[arg(long)]
        jstz_node_endpoint: String,
        /// Overwrites an existing network name.
        #[arg(short, long)]
        force: bool,
    },
}

pub async fn exec(command: Command) -> Result<()> {
    fn trim_long_string(input: &str, cap: usize) -> String {
        if input.len() > cap {
            return format!("{}...", &input[..cap - 3]);
        };
        input.to_owned()
    }

    let mut cfg = Config::load_path(None).await?;
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
        Command::Add {
            name,
            octez_node_rpc_endpoint,
            jstz_node_endpoint,
            force,
        } => {
            let short_name = trim_long_string(&name, 20);
            if !force && cfg.networks.networks.contains_key(&name) {
                bail_user_error!("Network '{short_name}' already exists. Use `--force` to overwrite the network.")
            }
            cfg.networks.networks.insert(
                name.clone(),
                Network {
                    octez_node_rpc_endpoint,
                    jstz_node_endpoint,
                },
            );

            cfg.save()?;
            info!("Added network '{short_name}'.");
            Ok(())
        }
    }
}
