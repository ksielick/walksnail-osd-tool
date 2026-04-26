use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=./src");

    // Target triple — Cargo passes this in via TARGET env var for build scripts.
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=VERGEN_CARGO_TARGET_TRIPLE={target}");

    // rustc semver — parse from `rustc --version`.
    let rustc_version = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
        .unwrap_or_default();
    println!("cargo:rustc-env=VERGEN_RUSTC_SEMVER={rustc_version}");

    if let Some(git_tag) = Command::new("git")
        .args(["describe", "--exact-match", "--tags", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
    {
        let git_tag = git_tag.trim();
        if !git_tag.is_empty() {
            println!("cargo:rustc-env=GIT_VERSION={git_tag}");
        }
    }

    if let Some(short_commit) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
    {
        let short_commit = short_commit.trim();
        println!("cargo:rustc-env=GIT_COMMIT_HASH={short_commit}");
    }

    // Load icon data
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("icon_bytes");
    let icon = image::ImageReader::open("../resources/icons/256x256.png")
        .expect("Failed to load icon file")
        .decode()
        .expect("Failed to decode icon file");
    let icon_bytes = icon.as_bytes();
    std::fs::write(dest_path, icon_bytes).expect("Failed to write icon bytes");
}
