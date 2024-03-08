//! This build script is used to copy the:
//!   - jstz_kernel.wasm
//!   - sandbox-params.json
//!   - sandbox.json
//! files to the OUT_DIR directory.
//! It also generates the sandbox_paths.rs file which contains the paths to the files in the OUT_DIR.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

const JSTZ_KERNEL_PATH: &str = "./jstz_kernel.wasm";

const SANDBOX_PARAMS_PATH: &str = "./sandbox-params.json";

const SANDBOX_PATH: &str = "./sandbox.json";

fn generate_path_getter_code(out_dir: &Path, name: &str, file: &str) -> String {
    let name_upper = name.to_uppercase();
    format!(
        r#"
        const {}_PATH: &str = "{}";

        fn {}_path() -> PathBuf {{
            let path = PathBuf::from({}_PATH);
            if path.exists() {{
                path
            }} else {{
                PathBuf::from("/usr/share/jstz/{}")
            }}
        }}
        "#,
        &name_upper,
        out_dir.join(file).to_str().expect("Invalid path"),
        name,
        &name_upper,
        file
    )
}

fn generate_code(out_dir: &Path) {
    let mut code = String::new();

    code.push_str(&generate_path_getter_code(
        out_dir,
        "jstz_kernel",
        "jstz_kernel.wasm",
    ));

    code.push_str(&generate_path_getter_code(
        out_dir,
        "sandbox_params",
        "sandbox-params.json",
    ));

    code.push_str(&generate_path_getter_code(
        out_dir,
        "sandbox",
        "sandbox.json",
    ));

    fs::write(out_dir.join("sandbox_paths.rs"), code).expect("Failed to write paths.rs");
}

fn main() {
    println!("cargo:rerun-if-changed={}", JSTZ_KERNEL_PATH);
    println!("cargo:rerun-if-changed={}", SANDBOX_PARAMS_PATH);
    println!("cargo:rerun-if-changed={}", SANDBOX_PATH);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Build jstz_kernel.wasm
    fs::copy(JSTZ_KERNEL_PATH, out_dir.join("jstz_kernel.wasm"))
        .expect("Failed to copy jstz_kernel.wasm to OUT_DIR");

    // Copy sandbox-params.json to out_dir
    fs::copy(SANDBOX_PARAMS_PATH, out_dir.join("sandbox-params.json"))
        .expect("Failed to copy sandbox-params.json to OUT_DIR");

    // Copy sandbox.json to out_dir
    fs::copy(SANDBOX_PATH, out_dir.join("sandbox.json"))
        .expect("Failed to copy sandbox.json to OUT_DIR");

    generate_code(&out_dir);
}
