use anyhow::Result;
use jstz_crypto::{public_key::PublicKey, secret_key::SecretKey};
//use jstz_node::sequencer::inbox::parsing::RollupType;
use jstz_proto::context::account::Nonce;
use jstz_proto::operation::{Content, DeployFunction, Operation, SignedOperation};
use jstz_proto::runtime::ParsedCode;
use std::env;
use std::process::Command;
use tezos_data_encoding::enc::BinWriter;
use tezos_smart_rollup::inbox::ExternalMessageFrame;
use tezos_smart_rollup::types::SmartRollupAddress;
use tezos_smart_rollup::utils::inbox::file::{InboxFile, Message};

// tag + 20 byte address
const EXTERNAL_FRAME_SIZE: usize = 21;

fn create_inbox_message(source: &str) -> Result<Message> {
    // Create the secret key and public key
    let sk =
        SecretKey::from_base58("edsk38mmuJeEfSYGiwLE1qHr16BPYKMT5Gg1mULT7dNUtg3ti4De3a")?;
    let pk =
        PublicKey::from_base58("edpkurYYUEb4yixA3oxKdvstG8H86SpKKUGmadHS6Ju2mM1Mz1w5or")?;

    // Create a new message with the source
    // We'll use the unsafe constructor to avoid thread safety issues with parsing
    let operation = Operation {
        public_key: pk,
        nonce: Nonce(1), // Use the constructor directly
        content: Content::DeployFunction(DeployFunction {
            function_code: unsafe { ParsedCode::new_unchecked(source.to_string()) },
            account_credit: 1000000,
        }),
    };

    let hash = operation.hash();
    let signature = sk.sign(hash.as_ref())?;
    let signed_operation = SignedOperation::new(signature, operation);

    let bytes = bincode::encode_to_vec(&signed_operation, bincode::config::legacy())?;
    let mut external = Vec::with_capacity(bytes.len() + EXTERNAL_FRAME_SIZE);

    let frame = ExternalMessageFrame::Targetted {
        contents: bytes,
        address: SmartRollupAddress::from_b58check(
            "sr1BxufbqiHt3dn6ahV6eZk9xBD6XV1fYowr",
        )
        .unwrap(),
    };

    frame.bin_write(&mut external)?;

    //println!("binary_contents: {:?}", external);

    let message = Message::External { external };
    Ok(message)
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <source_code>", args[0]);
        std::process::exit(1);
    }

    let source = &args[1];

    // Split source into chunks of max 1000 characters
    let chunk_size = 2000;
    let mut messages = Vec::new();

    for (_i, chunk) in source
        .chars()
        .collect::<Vec<_>>()
        .chunks(chunk_size)
        .enumerate()
    {
        let chunk_str: String = chunk.iter().collect();
        /*println!(
            "Creating message {} with {} characters",
            i + 1,
            chunk_str.len()
        );*/

        let message = create_inbox_message(&chunk_str)?;
        messages.push(message);
    }

    //println!("Created {} messages from source code", messages.len());
    messages.push(create_inbox_message("STOP")?);

    /*let encoded_op = signed_operation
        .encode()
        .map_err(|e| anyhow!("Failed to serialize operation: {e}"))?;

    let message_frame = ExternalMessageFrame::Targetted {
        address: SmartRollupAddress::from_b58check(
            "sr1BxufbqiHt3dn6ahV6eZk9xBD6XV1fYowr",
        )
        .unwrap(),
        contents: encoded_op,
    };
    let mut binary_contents = Vec::new();
    message_frame
        .bin_write(&mut binary_contents)
        .map_err(|_| anyhow!("Failed to write binary frame"))?;

    println!("binary_contents: {:?}", binary_contents);

    let inbox_message = InboxMessage::External::<MichelsonUnit>(&binary_contents);

    println!("inbox_message: {:?}", inbox_message);
    let mut bytes = Vec::new();
    inbox_message.serialize(&mut bytes)?;
    let message = Message::External(bytes);*/

    // Create the external message structure
    //let external_message = json!({
    //    "external": hex_data
    //});

    // Create the inbox message structure (array of levels, each level contains array of messages)
    //let inbox_message = json!([[message]]);

    // Write to temp file
    let temp_file = std::env::temp_dir().join("jstz_inbox_message.json");
    let inbox_file = InboxFile(vec![messages]);
    inbox_file.save(temp_file.as_path()).unwrap();
    //std::fs::write(&temp_file, serde_json::to_string_pretty(&inbox_file)?)?;

    //println!("Inbox message written to: {:?}", temp_file);

    // Run the riscv-sandbox command with the temporary inbox file
    //println!("Running riscv-sandbox with the temporary inbox file...");

    let output = Command::new("riscv-sandbox")
        .args(&[
            "run",
            "--timings",
            "--address",
            "sr1FXevDx86EyU1BBwhn94gtKvVPTNwoVxUC",
            "--inbox-file",
            temp_file.to_str().unwrap(),
            "--input",
            "/Users/alanmarko/projects/jstz_attempt2/jstz/target/riscv64gc-unknown-linux-musl/release/wpt-test-kernel-executable",
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
