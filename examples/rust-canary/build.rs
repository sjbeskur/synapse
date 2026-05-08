use std::{env, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let input = manifest_dir.join("syn/demo_msgs.syn");
    let out_dir = manifest_dir.join("generated");

    println!("cargo:rerun-if-changed=syn/mission_ids.syn");
    println!("cargo:rerun-if-changed=syn/demo_msgs.syn");

    cfs_synapse::generate_files(&input, &out_dir, cfs_synapse::Lang::Rust)
        .expect("generate Synapse Rust bindings");
}
