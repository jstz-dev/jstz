use anyhow::Result;
use jstz_crypto::{public_key::PublicKey, public_key_hash::PublicKeyHash};
use rand::Rng;
use sha2::{Digest, Sha256};
use tezos_crypto_rs::bls::keypair_from_ikm;

use crate::{accounts::account::Account, config::Config};

extern crate bs58;

fn passphrase_to_ikm(passphrase: &str) -> [u8; 32] {
    let hash = Sha256::digest(passphrase.as_bytes());
    let mut ikm = [0u8; 32];
    ikm.copy_from_slice(&hash);
    ikm
}

fn create_address(pk: PublicKey) -> String {
    PublicKeyHash::try_from(&pk).unwrap().to_base58()
}

pub fn generate_passphrase() -> String {
    let mut rng = rand::thread_rng();
    let mut passphrase = String::new();
    for _ in 0..10 {
        let random_number: u8 = rng.gen();
        let random_char = (random_number % 26) + 97;
        passphrase.push(random_char as char);
    }
    passphrase
}

pub fn create_account(
    passphrase: String,
    alias: String,
    cfg: &mut Config,
) -> Result<String> {
    let ikm = passphrase_to_ikm(passphrase.as_str());
    let (sk, pk) = keypair_from_ikm(ikm).unwrap();
    println!("Secret key: {}", sk);
    println!("Public key: {}", pk);
    let address = create_address(jstz_crypto::public_key::PublicKey::Bls(pk));

    cfg.accounts().upsert(Account::new(alias, address.clone()));
    cfg.save()?;

    Ok(address)
}
