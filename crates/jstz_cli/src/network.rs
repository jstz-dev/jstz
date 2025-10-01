use crate::{
    config::{Config, Network},
    error::{bail_user_error, Result},
};
use clap::{Args, Subcommand};
use log::info;
use prettytable::{format::consts::FORMAT_DEFAULT, Cell, Row, Table};

#[derive(Args, Debug)]
#[group(required = true, multiple = true)]
pub struct UpdateArgs {
    /// Octez node RPC endpoint.
    #[arg(long, default_value = None)]
    octez_node_rpc_endpoint: Option<String>,
    /// Jstz node API endpoint.
    #[arg(long, default_value = None)]
    jstz_node_endpoint: Option<String>,
}

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
    /// Update a network.
    Update {
        /// Name of the network to be updated.
        #[arg(value_name = "NETWORK_NAME")]
        name: String,
        #[command(flatten)]
        args: UpdateArgs,
    },
    /// Delete a network.
    Delete {
        /// Name of the network to be deleted.
        #[arg(value_name = "NETWORK_NAME")]
        name: String,
    },
    /// Retrieve the default network.
    GetDefault,
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
        Command::Update {
            name,
            args:
                UpdateArgs {
                    octez_node_rpc_endpoint,
                    jstz_node_endpoint,
                },
        } => {
            let short_name = trim_long_string(&name, 20);
            match cfg.networks.networks.get_mut(&name) {
                None => bail_user_error!("Network '{short_name}' does not exist."),
                Some(network) => {
                    if let Some(v) = octez_node_rpc_endpoint {
                        network.octez_node_rpc_endpoint = v;
                    }
                    if let Some(v) = jstz_node_endpoint {
                        network.jstz_node_endpoint = v;
                    }
                }
            };

            cfg.save()?;
            info!("Updated network '{short_name}'.");
            Ok(())
        }
        Command::Delete { name } => {
            let short_name = trim_long_string(&name, 20);
            if cfg.networks.networks.remove(&name).is_none() {
                bail_user_error!("Network '{short_name}' does not exist.");
            }

            cfg.save()?;
            info!("Deleted network '{short_name}'.");
            Ok(())
        }
        Command::GetDefault => {
            match cfg.networks.default_network {
                Some(v) => {
                    let name = v.to_string();
                    let size = name.len();
                    if size > 50 {
                        info!(
                            "{} (long network name truncated)",
                            trim_long_string(&name, 50),
                        );
                    } else {
                        info!("{name}")
                    }
                }
                None => info!("Default network is not set."),
            };
            Ok(())
        }
    }
}
