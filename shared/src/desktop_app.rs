//! plugged.host: the desktop app's installer on this box (docs/features/app.md). Read by the
//! download routes and by neo's knowledge, so neo only mentions the app where it is offered.

use std::path::PathBuf;

/// The installer the panel offers: `PLUGGED_APP_INSTALLER`, when it names a file.
pub struct Installer {
    pub path: PathBuf,
    pub size: u64,
    pub filename: String,
    pub version: String,
}

fn env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// The app's version as the panel labels it (`PLUGGED_APP_VERSION`).
pub fn version() -> String {
    env("PLUGGED_APP_VERSION").unwrap_or_else(|| "3.1.0".into())
}

/// The installer, or `None` when the feature is off (unset, or no such file).
pub fn installer() -> Option<Installer> {
    let path = PathBuf::from(env("PLUGGED_APP_INSTALLER")?);
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Setup.exe")
        .to_owned();

    Some(Installer {
        size: metadata.len(),
        filename,
        version: version(),
        path,
    })
}
