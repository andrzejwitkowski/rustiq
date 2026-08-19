use std::process::Command;

fn main() {
    let out = Command::new("git")
        .args(["log", "-1", "--format=%h %ci"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let mut parts = out.trim().splitn(2, ' ');
    let hash = parts.next().unwrap_or("");
    let date = parts.next().unwrap_or("").get(..16).unwrap_or("");

    println!("cargo:rustc-env=GIT_HASH={hash}");
    println!("cargo:rustc-env=GIT_DATE={date}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
}
