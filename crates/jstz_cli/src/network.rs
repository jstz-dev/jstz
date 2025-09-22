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
    fn trim_long_strings(input: &String, cap: usize) -> String {
        if input.len() > cap {
            return format!("{}...", &input[..cap - 3]);
        };
        return input.clone();
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

            for (n, network) in cfg.networks.networks.iter() {
                let name = trim_long_strings(n, 20);
                let octez_endpoint =
                    trim_long_strings(&network.octez_node_rpc_endpoint, 25);
                let jstz_endpoint = trim_long_strings(&network.jstz_node_endpoint, 25);
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
