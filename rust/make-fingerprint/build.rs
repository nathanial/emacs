use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let fingerprint_path = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .join("lib")
        .join("fingerprint.c");

    println!("cargo:rerun-if-changed={}", fingerprint_path.display());

    let content = fs::read_to_string(&fingerprint_path)
        .expect("failed to read fingerprint.c");
    let mut bytes: Vec<String> = Vec::new();
    let mut capture = false;
    for line in content.lines() {
        if line.contains("fingerprint[]") {
            capture = true;
            continue;
        }
        if capture {
            if line.contains('}') {
                break;
            }
            for token in line.split(',') {
                let trimmed = token.trim();
                if trimmed.starts_with("0x") {
                    bytes.push(trimmed.to_string());
                }
            }
        }
    }
    assert!(bytes.len() == 32, "expected 32 fingerprint bytes, found {}", bytes.len());

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("placeholder.rs");
    let mut output = String::from("pub const PLACEHOLDER: [u8; 32] = [\n");
    for byte in &bytes {
        output.push_str("    ");
        output.push_str(byte);
        output.push_str(",\n");
    }
    output.push_str("];
");
    fs::write(out_path, output).expect("failed to write placeholder.rs");
}
