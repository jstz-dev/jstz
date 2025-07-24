fn main() {
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    if target_arch == "riscv64" {
        // Note: jstz_core does not really need to link atomic. This is actually for
        // jstz_runtime and jstz_proto as they depend on deno and v8 needs to link
        // atomic when the build target is riscv. The actual reason is not clear, but
        // one theory is that atomic needs to be linked before rusty_v8 starts to be
        // compiled, which takes place before both crates get compiled, which means
        // that linking atomic in build.rs of those crates is too late. Since both
        // crates depend on jstz_core, linking atomic here works.
        println!("cargo::rustc-link-lib=atomic");
    }
}
