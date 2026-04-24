use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CFS_DIR");

    let cfs_dir = env::var("CFS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/usr/local/include/cfe"));

    let header = cfs_dir.join("cfe.h");
    if !header.exists() {
        panic!(
            "\n\ncfs-sys: cFS headers not found at {}\n\
             Set the CFS_DIR environment variable to your cFS installation, e.g.:\n\
             \n  CFS_DIR=/path/to/cFS/build/native/default_cpu1/inc cargo build\n\n",
            header.display()
        );
    }

    println!("cargo:rerun-if-changed={}", header.display());

    let bindings = bindgen::Builder::default()
        .header(header.to_str().unwrap())
        .clang_arg(format!("-I{}", cfs_dir.display()))
        // Only pull in the types we need — keeps the generated file small
        .allowlist_type("CFE_MSG_Message_t")
        .allowlist_type("CFE_MSG_TelemetryHeader_t")
        .allowlist_type("CFE_MSG_TelemetrySecondaryHeader_t")
        .allowlist_type("CFE_MSG_CommandHeader_t")
        .allowlist_type("CFE_MSG_CommandSecondaryHeader_t")
        .allowlist_type("CFE_TIME_SysTime_t")
        // Derive useful traits where possible
        .derive_default(true)
        .derive_copy(true)
        .derive_debug(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("cfs-sys: bindgen failed to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("cfs-sys: failed to write bindings.rs");
}
