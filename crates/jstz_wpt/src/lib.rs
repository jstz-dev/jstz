mod manifest;
mod report;
mod serve;

pub use manifest::*;
pub use report::*;
pub use serve::*;

use std::{
    ffi::OsStr,
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use anyhow::Result;

pub(crate) fn root_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

pub(crate) struct PythonOptions {
    pub stdout: Stdio,
    pub stderr: Stdio,
}

impl Default for PythonOptions {
    fn default() -> Self {
        Self {
            stdout: Stdio::inherit(),
            stderr: Stdio::inherit(),
        }
    }
}

// Write a function to execute a python binary with arguments
pub(crate) fn run_python<I, S>(args: I, options: PythonOptions) -> Result<Child>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("python3");

    cmd.args(args);
    cmd.stdout(options.stdout);
    cmd.stderr(options.stderr);
    cmd.current_dir(root_dir()?);

    Ok(cmd.spawn()?)
}

const WPT_CMD: &str = "./wpt/wpt";

pub struct Wpt {
    _private: (),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Diagnosis {
    pub python_installed: bool,
    pub wpt_installed: bool,
    pub hosts_configured: bool,
}

impl Diagnosis {
    pub fn is_healthy(&self) -> bool {
        self.python_installed && self.wpt_installed && self.hosts_configured
    }
}

impl Wpt {
    pub fn doctor() -> Result<Diagnosis> {
        let python_installed = {
            let python_version = run_python(
                ["--version"],
                PythonOptions {
                    stdout: Stdio::piped(),
                    ..Default::default()
                },
            )?
            .wait_with_output()?;

            python_version.status.success()
                && python_version.stdout.starts_with(b"Python 3")
        };

        let wpt_installed = {
            // Check that wpt is a directory and that it contains a `wpt` file
            let wpt_dir = root_dir()?.join("wpt");
            wpt_dir.exists() && wpt_dir.is_dir() && wpt_dir.join("wpt").is_file()
        };

        let hosts_configured = {
            let hosts = fs::read_to_string("/etc/hosts")?;
            hosts.contains("web-platform.test")
        };

        Ok(Diagnosis {
            python_installed,
            wpt_installed,
            hosts_configured,
        })
    }

    pub fn new() -> Result<Self> {
        let dianosis = Self::doctor()?;
        if !dianosis.is_healthy() {
            return Err(anyhow::anyhow!(
                "Environment is not configured correctly for WPT"
            ));
        }

        Ok(Self { _private: () })
    }

    pub fn hosts(&self) -> Result<String> {
        let args = [WPT_CMD, "make-hosts-file"];

        let output = run_python(
            args,
            PythonOptions {
                stdout: Stdio::piped(),
                ..Default::default()
            },
        )?
        .wait_with_output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to update wpt hosts"));
        }

        let hosts = String::from_utf8(output.stdout)?;

        Ok(hosts)
    }

    pub fn update_hosts(&self) -> Result<()> {
        let hosts = self.hosts()?;
        fs::write(root_dir()?.join("hosts"), hosts)?;

        Ok(())
    }

    pub fn read_hosts() -> Result<String> {
        let hosts = fs::read_to_string(root_dir()?.join("hosts"))?;
        Ok(hosts)
    }

    pub fn update_manifest(&self, rebuild: bool) -> Result<()> {
        let args = [
            WPT_CMD,
            "manifest",
            "--tests-root",
            ".",
            "-p",
            "./manifest.json",
            if rebuild { "--rebuild" } else { "" },
        ];

        let status = run_python(args, Default::default())?.wait()?;
        if !status.success() {
            return Err(anyhow::anyhow!("Failed to generate wpt manifest"));
        }

        Ok(())
    }

    pub fn read_manifest() -> Result<WptManifest> {
        let manifest = std::fs::read_to_string(root_dir()?.join("manifest.json"))?;
        Ok(serde_json::from_str(&manifest)?)
    }

    pub async fn serve(&self, debug: bool) -> Result<WptServe> {
        let args = [WPT_CMD, "serve", "--config", "./config.json"];

        let options = if !debug {
            PythonOptions {
                stdout: Stdio::null(),
                stderr: Stdio::null(),
            }
        } else {
            Default::default()
        };

        let mut child = run_python(args, options)?;

        // Wait for the server to start
        let mut attempts = 0;
        loop {
            // If we waited more than 10 seconds, give up
            if attempts > 10 {
                child.kill()?;
                child.wait()?;
                return Err(anyhow::anyhow!("Failed to start wpt server"));
            }

            match reqwest::get("http://localhost:8000").await {
                Ok(res) if res.status().is_success() => break,
                _ => {
                    attempts += 1;
                    std::thread::sleep(Duration::from_millis(1000))
                }
            }
        }

        Ok(WptServe { process: child })
    }
}
