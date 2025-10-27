use anyhow::Result;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

include!("build_config.rs");

// The RISC-V kernel executable path for origination
const JSTZ_RISCV_KERNEL_PATH: &str =
    "./resources/jstz_rollup/lightweight-kernel-executable";
const JSTZ_PARAMETERS_TY_PATH: &str = "./resources/jstz_rollup/parameters_ty.json";
/// Generated file that contains path getter functions
const JSTZ_ROLLUP_PATH: &str = "jstz_rollup_path.rs";
const BOOTSTRAP_ACCOUNT_PATH: &str = "./resources/bootstrap_account/accounts.json";

/// Build script that validates built-in bootstrap accounts and generates and saves
/// the following files in OUT_DIR:
///
/// Files copied:
/// - parameters_ty.json: JSON file containing parameter types
///
/// Files generated:
/// - kernel_installer.hex: Hex-encoded kernel installer binary
/// - preimages/: Directory containing kernel preimages
/// - jstz_rollup_path.rs: Generated Rust code with path getters
///
/// The generated jstz_rollup_path.rs provides the following functions:
/// - kernel_installer_path(): Path to the kernel installer hex file
/// - parameters_ty_path(): Path to the parameters type JSON file
/// - preimages_path(): Path to the preimages directory
fn main() {
    println!("cargo:rerun-if-changed={JSTZ_RISCV_KERNEL_PATH}");
    println!("cargo:rerun-if-changed={JSTZ_PARAMETERS_TY_PATH}");
    println!("cargo:rerun-if-changed={BOOTSTRAP_ACCOUNT_PATH}");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // 1. Copy parameters_ty.json to OUT_DIR
    fs::copy(JSTZ_PARAMETERS_TY_PATH, out_dir.join("parameters_ty.json"))
        .expect("Failed to copy parameters_ty.json to OUT_DIR");

    // 3. Compute RISC-V kernel checksum and generate path getters
    let riscv_kernel_checksum = compute_sha256(Path::new(JSTZ_RISCV_KERNEL_PATH))
        .expect("Failed to compute RISC-V kernel checksum");
    generate_code(&out_dir, &riscv_kernel_checksum);

    println!(
        "cargo:warning=Build script output directory: {}",
        out_dir.display()
    );
    if let Ok(p) = env::var("KERNEL_DEST_DIR") {
        println!("cargo:warning=Copying content in output directory to: {p}");
        fs::create_dir_all(&p).unwrap_or_else(|e| {
            panic!("Failed to create destination directory '{}': {:?}", &p, e)
        });
        copy_dir_all(out_dir, &p).unwrap_or_else(|e| {
            panic!("Failed to copy kernel files to '{}': {:?}", &p, e)
        });
    }
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry_ in fs::read_dir(src)? {
        let entry = entry_?;
        let src_path = entry.path();
        let dst_path = dst.as_ref().join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_all(src_path, dst_path)?;
        } else {
            fs::copy(src_path, dst_path)?;
        }
    }
    Ok(())
}

/// Generates Rust code for path getters to access files in OUT_DIR
///
/// Generates the following functions:
/// - kernel_installer_path(): Path to the kernel installer hex file
/// - parameters_ty_path(): Path to the parameters type JSON file
/// - preimages_path(): Path to the preimages directory
fn generate_code(out_dir: &Path, riscv_kernel_checksum: &str) {
    let mut code = String::new();
    code.push_str(&generate_path_getter_code(
        out_dir,
        "parameters_ty",
        "parameters_ty.json",
    ));
    code.push_str(&generate_path_getter_code(
        out_dir,
        "preimages",
        "preimages",
    ));
    // RISC-V kernel path & checksum getters
    // TODO: maybe use the path getter fn
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let riscv_kernel_absolute_path = PathBuf::from(&manifest_dir)
        .join(JSTZ_RISCV_KERNEL_PATH)
        .canonicalize()
        .expect("Failed to canonicalize RISC-V kernel path");

    code.push_str(&format!(
        r#"
        const RISCV_KERNEL_PATH: &str = "{}";
        pub fn riscv_kernel_path() -> std::path::PathBuf {{
            std::path::PathBuf::from(RISCV_KERNEL_PATH)
        }}

        const RISCV_KERNEL_CHECKSUM: &str = "{}";
        pub fn riscv_kernel_checksum() -> &'static str {{
            RISCV_KERNEL_CHECKSUM
        }}
        "#,
        riscv_kernel_absolute_path.display(),
        riscv_kernel_checksum
    ));

    fs::write(out_dir.join(JSTZ_ROLLUP_PATH), code).expect("Failed to write paths.rs");
}

/// Generates a path getter function
///
/// # Arguments
/// * `out_dir` - The output directory
/// * `fn_name` - The name of the function to generate (e.g., "kernel_installer" generates kernel_installer_path())
/// * `path_suffix` - The path component to append to out_dir
///
/// # Example
/// ```
/// // Generates:
/// // const KERNEL_INSTALLER_PATH: &str = "/path/to/out_dir/kernel_installer.hex";
/// // pub fn kernel_installer_path() -> PathBuf { PathBuf::from(KERNEL_INSTALLER_PATH) }
/// generate_path_getter_code(out_dir, "kernel_installer", "kernel_installer.hex");
/// ```
fn generate_path_getter_code(out_dir: &Path, fn_name: &str, path_suffix: &str) -> String {
    let name_upper = fn_name.to_uppercase();
    format!(
        r#"
        const {}_PATH: &str = "{}";
        pub fn {}_path() -> std::path::PathBuf {{
            let path = std::path::PathBuf::from({}_PATH);
            if path.exists() {{
                path
            }} else {{
                std::path::PathBuf::from("/usr/share/jstzd/{}")
            }}
        }}
        "#,
        &name_upper,
        out_dir.join(path_suffix).to_str().expect("Invalid path"),
        fn_name,
        &name_upper,
        path_suffix
    )
}

fn compute_sha256(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    if !path.exists() {
        return Err(anyhow::anyhow!(
            "RISC-V kernel file not found: {}",
            path.display()
        ));
    }
    let content = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(format!("{:x}", hasher.finalize()))
}
