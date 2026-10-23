use std::{path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let is_git_repo = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if is_git_repo && Path::new("../.git/HEAD").exists() {
        println!("cargo:rerun-if-changed=../.git/HEAD");

        if let Ok(head) = std::fs::read_to_string("../.git/HEAD")
            && head.starts_with("ref: ")
        {
            let head_ref = head.trim_start_matches("ref: ").trim();
            if Path::new("../.git").join(head_ref).exists() {
                println!("cargo:rerun-if-changed=../.git/{head_ref}");
            } else if Path::new("../.git/packed-refs").exists() {
                println!("cargo:rerun-if-changed=../.git/packed-refs");
            }

            println!(
                "cargo:rustc-env=CARGO_GIT_BRANCH={}",
                head_ref.rsplit('/').next().unwrap_or("unknown")
            );
        } else {
            println!("cargo:rustc-env=CARGO_GIT_BRANCH=unknown");
        }
    } else {
        println!("cargo:rustc-env=CARGO_GIT_BRANCH=unknown");
    }

    let mut git_hash = "unknown".to_string();

    if is_git_repo
        && let Ok(output) = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
        && output.status.success()
        && let Ok(hash) = String::from_utf8(output.stdout)
    {
        git_hash = hash.trim().to_string();
    }

    let target_arch =
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".to_string());
    let target_env = if std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|s| s.is_empty()) {
        "unknown".to_string()
    } else {
        std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_else(|_| "unknown".to_string())
    };

    println!("cargo:rustc-env=CARGO_GIT_COMMIT={git_hash}");
    println!("cargo:rustc-env=CARGO_TARGET={target_arch}-{target_env}");

    release(is_git_repo); // plugged.host
}

/// plugged.host: the release, `v{VERSION}.{commit}` (e.g. `v3.0.0.7bc33d4`): the `VERSION` file
/// at the repository root plus the 7-character commit the build was made from.
/// `PANEL_RELEASE` at build time overrides it.
fn release(is_git_repo: bool) {
    println!("cargo:rerun-if-changed=../VERSION");
    println!("cargo:rerun-if-env-changed=PANEL_RELEASE");
    // Any new commit: HEAD, a loose branch ref (a directory is watched recursively) or packed refs.
    // Watching only the ref HEAD names misses a branch that was packed at clone time and is
    // loose after the next commit.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/heads");
    println!("cargo:rerun-if-changed=../.git/packed-refs");

    let version = std::fs::read_to_string("../VERSION")
        .map(|version| version.trim().to_string())
        .unwrap_or_else(|_| "0.0.0".to_string());

    let mut commit = "unknown".to_string();

    if is_git_repo
        && let Ok(output) = Command::new("git")
            .args(["rev-parse", "--short=7", "HEAD"])
            .output()
        && output.status.success()
        && let Ok(hash) = String::from_utf8(output.stdout)
    {
        commit = hash.trim().to_string();
    }

    let release = std::env::var("PANEL_RELEASE")
        .ok()
        .filter(|release| !release.trim().is_empty())
        .unwrap_or_else(|| format!("v{version}.{commit}"));

    println!("cargo:rustc-env=RELEASE_VERSION={version}");
    println!("cargo:rustc-env=RELEASE={release}");
}
