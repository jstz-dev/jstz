use std::{
    fs,
    io::{self, Write},
    process,
};

use anyhow::Result;
use clap::Parser;
use jstz_wpt::Wpt;

#[derive(Parser)]
#[command(author, version)]
enum Command {
    // Validate that your environment is configured correctly
    Doctor,
    // Configure your environment
    Init {
        #[clap(long, action)]
        auto_config_hosts: bool,
    },
    // Start the wpt server
    Serve {
        // Enable debug logging
        #[clap(long, action)]
        debug: bool,
    },
    // Update the `manifest.json` file to match the current wpt version
    UpdateManifest {
        // Rebuild the manifest from scratch instead of downloading.
        // Note: This can take up to 3 minutes.
        #[clap(long, action)]
        rebuild: bool,
    },
    // Update the `hosts` file to match the current wpt version
    UpdateHosts,
}

fn run_git(args: &[&str]) -> Result<()> {
    let mut cmd = process::Command::new("git");
    cmd.args(args);
    let status = cmd.status()?;
    if !status.success() {
        println!("Failed to run `git` command");
        return Ok(());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let command = Command::parse();

    match command {
        Command::Doctor => {
            let diagnosis = Wpt::doctor()?;

            if !diagnosis.python_installed {
                println!("Python3 is not installed. Please install it and try again.");
            }

            if !diagnosis.wpt_installed {
                println!("WPT is not installed. Please run `git submodules init` and try again.");
            }

            if !diagnosis.hosts_configured {
                println!("WPT hosts are not configured. Please run `jstz-wpt init` and try again.");
                println!("Alternatively, you can configure them manually by appending `jstz_wpt/hosts` to your `/etc/hosts` file:");
                println!("      cat jstz_wpt/hosts | sudo tee -a /etc/hosts");
            }

            if diagnosis.is_healthy() {
                println!("WPT is setup correctly!")
            }
        }
        Command::Init { auto_config_hosts } => {
            let diagnosis = Wpt::doctor()?;
            if !diagnosis.python_installed {
                println!("Python3 is not installed. Please install it and try again.");
                return Ok(());
            }

            // Run `git submodule init` and `git submodule update` if necessary
            if !diagnosis.wpt_installed {
                run_git(&["submodule", "init"])?;
                run_git(&["submodule", "update"])?;
                println!("Initialized WPT git submodule");
            }

            // Configure hosts if necessary
            if !diagnosis.hosts_configured {
                let auto_config_hosts = auto_config_hosts
                    || {
                        print!("WPT requires certain entries to be present in your `/etc/hosts` file. Should these be configured automatically? (y/N):");
                        io::stdout().flush()?;

                        let mut input = String::new();
                        io::stdin().read_line(&mut input)?;

                        ["y", "Y", "yes", "Yes"].contains(&input.trim())
                    };

                if auto_config_hosts {
                    let hosts = Wpt::read_hosts()?;

                    let mut hosts_file = match fs::OpenOptions::new()
                        .append(true)
                        .open("/etc/hosts")
                    {
                        Ok(host_file) => host_file,
                        Err(err) => {
                            if err.kind() != io::ErrorKind::PermissionDenied {
                                return Err(err.into());
                            }

                            println!("Failed to open `/etc/hosts` (persmission denied). Please run this command again with `sudo`, or configure the entires manually.");
                            return Ok(());
                        }
                    };

                    hosts_file.write_all(hosts.as_bytes())?;
                } else {
                    println!("Please configure the /etc/hosts entries manually:");
                    println!("      cat jstz_wpt/hosts | sudo tee -a /etc/hosts");
                }
            }
        }
        Command::Serve { debug } => {
            let mut wpt = Wpt::new()?.serve(debug).await?;

            ctrlc::set_handler(move || {
                println!("Stopping WPT server...");
                wpt.kill().expect("Failed to kill wpt server");
                process::exit(0);
            })?;

            println!("WPT server started at http://localhost:8000");
            loop {}
        }
        Command::UpdateManifest { rebuild } => Wpt::new()?.update_manifest(rebuild)?,
        Command::UpdateHosts => Wpt::new()?.update_hosts()?,
    }

    Ok(())
}
