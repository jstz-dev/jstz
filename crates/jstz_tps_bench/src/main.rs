// SPDX-FileCopyrightText: 2024 TriliTech <contact@trili.tech>
//
// SPDX-License-Identifier: MIT
use std::path::Path;

use clap::{Parser, Subcommand};
use jstz_tps_bench::fa2_bench_generator::{handle_generate, handle_generate_script};
use jstz_tps_bench::generate_other::handle_generate_other;
use jstz_tps_bench::results::handle_results;

const DEFAULT_ROLLUP_ADDRESS: &str = "sr163Lv22CdE8QagCwf48PWDTquk6isQwv57";

#[derive(Debug, Parser)]
#[command(long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Generate inbox.json file")]
    Generate {
        #[command(subcommand)]
        generate_command: GenerateCommands,
    },
    #[command(about = "Generate inbox.sh script")]
    GenerateScript {
        #[arg(long, default_value = DEFAULT_ROLLUP_ADDRESS)]
        address: String,
        #[arg(long)]
        transfers: usize,
        #[arg(long, default_value = "inbox.sh")]
        script_file: Box<Path>,
    },
    #[command(about = "Extract results from inbox.json & log file")]
    Results {
        #[arg(long)]
        inbox_file: Box<Path>,
        #[arg(long)]
        log_file: Vec<Box<Path>>,
        #[arg(long)]
        expected_transfers: usize,
    },
}

#[derive(Debug, Subcommand)]
enum GenerateCommands {
    #[command(about = "Generate FA2 transactions")]
    Fa2 {
        #[arg(long, default_value = DEFAULT_ROLLUP_ADDRESS)]
        address: String,
        #[arg(long)]
        transfers: usize,
        #[arg(long, default_value = "inbox.json")]
        inbox_file: Box<Path>,
    },
    #[command(about = "Generate other types of operations and benchmarking setup")]
    Other {
        #[arg(long, default_value = DEFAULT_ROLLUP_ADDRESS)]
        address: String,
        #[arg(long)]
        num_operations: usize,
        #[arg(long, default_value = "inbox.json")]
        inbox_file: Box<Path>,
        #[arg(long)]
        smart_function: Box<Path>,
        #[arg(long)]
        init_endpoint: Option<String>,
        #[arg(long)]
        run_endpoint: Option<String>,
        #[arg(long)]
        check_endpoint: Option<String>,
    },
}

fn main() -> jstz_tps_bench::Result<()> {
    match Cli::parse().command {
        Commands::Generate { generate_command } => match generate_command {
            GenerateCommands::Fa2 {
                address,
                inbox_file,
                transfers,
            } => handle_generate(&address, &inbox_file, transfers)?,
            GenerateCommands::Other {
                address,
                inbox_file,
                num_operations,
                smart_function,
                init_endpoint,
                run_endpoint,
                check_endpoint,
            } => handle_generate_other(
                &address,
                &inbox_file,
                num_operations,
                &smart_function,
                init_endpoint.as_deref(),
                run_endpoint.as_deref(),
                check_endpoint.as_deref(),
            )?,
        },
        Commands::GenerateScript {
            address,
            script_file,
            transfers,
        } => handle_generate_script(&address, &script_file, transfers)?,
        Commands::Results {
            inbox_file,
            log_file,
            expected_transfers,
        } => handle_results(inbox_file, log_file, expected_transfers)?,
    }

    Ok(())
}
