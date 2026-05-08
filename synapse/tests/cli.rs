use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("synapse-cli-{name}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_import_fixture(dir: &std::path::Path) -> PathBuf {
    fs::write(
        dir.join("frame_descriptor.syn"),
        r#"namespace frame_descriptor
struct FrameDescriptor { gain: f32 }
"#,
    )
    .unwrap();
    let root = dir.join("postcard.syn");
    fs::write(
        &root,
        r#"namespace camera
import "frame_descriptor.syn"
struct Postcard { fd: frame_descriptor::FrameDescriptor }
"#,
    )
    .unwrap();
    root
}

#[test]
fn out_dir_generates_import_closure_by_default() {
    let dir = test_dir("closure");
    let root = write_import_fixture(&dir);
    let out_dir = dir.join("generated");

    let output = Command::new(env!("CARGO_BIN_EXE_synapse"))
        .arg("--lang")
        .arg("c")
        .arg("-o")
        .arg(&out_dir)
        .arg(&root)
        .output()
        .expect("run synapse");

    assert!(
        output.status.success(),
        "synapse failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("frame_descriptor.h").exists());
    assert!(out_dir.join("postcard.h").exists());
}

#[test]
fn single_file_writes_only_root_file() {
    let dir = test_dir("single-file");
    let root = write_import_fixture(&dir);
    let out_dir = dir.join("generated");

    let output = Command::new(env!("CARGO_BIN_EXE_synapse"))
        .arg("--lang")
        .arg("c")
        .arg("-o")
        .arg(&out_dir)
        .arg("--single-file")
        .arg(&root)
        .output()
        .expect("run synapse");

    assert!(
        output.status.success(),
        "synapse failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!out_dir.join("frame_descriptor.h").exists());
    assert!(out_dir.join("postcard.h").exists());
}
