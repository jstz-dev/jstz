use std::env;

fn main() {
    let ticketer_pk = env::var("TICKETER")
        .unwrap_or("KT1F3MuqvT9Yz57TgCS3EkDcKNZe9HpiavUJ".to_string());
    let injector_pk = env::var("INJECTOR")
        .unwrap_or("edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav".to_string());

    println!("cargo:rustc-env=TICKETER={}", ticketer_pk);
    println!("cargo:rustc-env=INJECTOR={}", injector_pk);
}
