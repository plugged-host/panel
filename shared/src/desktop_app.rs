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

/// What the panel can offer: the Windows installer, and for Linux an AppImage and a .deb.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    LinuxAppImage,
    LinuxDeb,
}

impl Platform {
    pub const ALL: [Platform; 3] = [
        Platform::Windows,
        Platform::LinuxAppImage,
        Platform::LinuxDeb,
    ];

    /// `windows`, `linux` (the AppImage, which the Linux app updates itself from) or `linux-deb`.
    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value.unwrap_or("windows") {
            "windows" | "" => Some(Platform::Windows),
            "linux" | "linux-appimage" => Some(Platform::LinuxAppImage),
            "linux-deb" | "deb" => Some(Platform::LinuxDeb),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::LinuxAppImage => "linux",
            Platform::LinuxDeb => "linux-deb",
        }
    }

    fn env_key(self) -> &'static str {
        match self {
            Platform::Windows => "PLUGGED_APP_INSTALLER",
            Platform::LinuxAppImage => "PLUGGED_APP_LINUX_APPIMAGE",
            Platform::LinuxDeb => "PLUGGED_APP_LINUX_DEB",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Platform::Windows => "application/vnd.microsoft.portable-executable",
            Platform::LinuxAppImage => "application/vnd.appimage",
            Platform::LinuxDeb => "application/vnd.debian.binary-package",
        }
    }
}

/// The Windows installer, or `None` when the feature is off (unset, or no such file).
pub fn installer() -> Option<Installer> {
    installer_for(Platform::Windows)
}

/// Whether the panel offers the app for any platform.
pub fn offered() -> bool {
    Platform::ALL
        .iter()
        .any(|platform| installer_for(*platform).is_some())
}

/// The file for one platform, or `None` when it is not offered.
pub fn installer_for(platform: Platform) -> Option<Installer> {
    let path = PathBuf::from(env(platform.env_key())?);
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("plugged.host-app")
        .to_owned();

    Some(Installer {
        size: metadata.len(),
        filename,
        version: version(),
        path,
    })
}
