use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output();

    let git_hash = match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => "unknown".to_string(),
    };

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    let nnue_path = Path::new("data/nnue.bin");
    let nnue_hash = if nnue_path.exists() {
        match fs::read(nnue_path) {
            Ok(data) => {
                let digest = sha256::digest(&data);
                digest
            }
            Err(_) => "unknown".to_string(),
        }
    } else {
        "not_found".to_string()
    };

    println!("cargo:rustc-env=NNUE_SHA256={}", nnue_hash);
}
