use anyhow::Result;
use jstz_proto::runtime::ParsedCode;
use jstz_utils::inbox_builder::InboxBuilder;
use std::env;
use std::process::Command;
use tezos_smart_rollup::types::SmartRollupAddress;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <source_code>", args[0]);
        std::process::exit(1);
    }

    let source = &args[1];

    let mut inbox_builder = InboxBuilder::new(
        SmartRollupAddress::from_b58check("sr1BxufbqiHt3dn6ahV6eZk9xBD6XV1fYowr")
            .unwrap(),
        None,
    );

    // Split source into chunks of max 2000 characters
    let chunk_size = 2000;

    let mut account = &mut inbox_builder.create_accounts(1).unwrap()[0];

    for (_i, chunk) in source
        .chars()
        .collect::<Vec<_>>()
        .chunks(chunk_size)
        .enumerate()
    {
        let chunk_str: String = chunk.iter().collect();
        inbox_builder
            .deploy_function(
                &mut account,
                unsafe { ParsedCode::new_unchecked(chunk_str.to_string()) },
                1000000,
            )
            .unwrap();
    }

    inbox_builder
        .deploy_function(
            &mut account,
            unsafe { ParsedCode::new_unchecked("STOP".to_string()) },
            1000000,
        )
        .unwrap();

    // Write to temp file
    let temp_file = std::env::temp_dir().join("jstz_inbox_message.json");
    let inbox_file = inbox_builder.build();
    inbox_file.save(temp_file.as_path()).unwrap();

    let kernel_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../target/riscv64gc-unknown-linux-musl/release/wpt-test-kernel-executable",
    );

    let output = Command::new("riscv-sandbox")
        .args(&[
            "run",
            "--timings",
            "--address",
            "sr1FXevDx86EyU1BBwhn94gtKvVPTNwoVxUC",
            "--inbox-file",
            temp_file.to_str().unwrap(),
            "--input",
            kernel_path.to_str().unwrap(),
        ])
        .output()?;

    if output.status.success() {
        println!("riscv-sandbox completed successfully");
        println!("Output: {}", String::from_utf8_lossy(&output.stdout));
    } else {
        eprintln!("riscv-sandbox failed with exit code: {}", output.status);
        eprintln!("Error: {}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(1);
    }

    Ok(())
}
