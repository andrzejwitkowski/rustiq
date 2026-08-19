use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(["log", "-1", "--format=%h"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let date = Command::new("git")
        .args(["log", "-1", "--format=%ci"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    // "2026-08-19 15:00:00 +0200" → "2026-08-19 15:00"
    let date_short = date.trim().get(..16).unwrap_or("").to_string();

    println!("cargo:rustc-env=GIT_HASH={}", hash.trim());
    println!("cargo:rustc-env=GIT_DATE={date_short}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
}
