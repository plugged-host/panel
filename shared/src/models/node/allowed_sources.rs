use std::path::{Component, Path, PathBuf};

/// The `allowed_mounts` / `allowed_devices` lists of a node's wings config, used to predict
/// whether wings will bind a mount or device or silently skip it.
///
/// Mirrors wings' `resolve_allowed_path` lexically: wings canonicalizes the configured entries
/// and the source on the node, so symlinks (and, for devices, the char/block file type check)
/// cannot be accounted for here.
pub struct NodeAllowedSources {
    mounts: AllowedPaths,
    devices: AllowedPaths,
}

impl NodeAllowedSources {
    pub fn new<S: AsRef<str>>(
        mounts: impl IntoIterator<Item = S>,
        devices: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            mounts: AllowedPaths::new(mounts),
            devices: AllowedPaths::new(devices),
        }
    }

    pub fn from_config(config: &wings_api::Config) -> Result<Self, serde_json::Error> {
        #[derive(Default, serde::Deserialize)]
        #[serde(default)]
        struct AllowedSourcesConfig {
            allowed_mounts: Vec<compact_str::CompactString>,
            allowed_devices: Vec<compact_str::CompactString>,
        }

        let config: AllowedSourcesConfig = config.view()?;

        Ok(Self::new(&config.allowed_mounts, &config.allowed_devices))
    }

    /// `None` when the answer depends on a relative entry the panel cannot resolve.
    pub fn allows_mount(&self, source: &str, target: &str) -> Option<bool> {
        self.mounts.allows(source, target)
    }

    /// `None` when the answer depends on a relative entry the panel cannot resolve.
    pub fn allows_device(&self, source: &str, target: &str, permissions: &str) -> Option<bool> {
        if permissions.is_empty() || !permissions.bytes().all(|p| matches!(p, b'r' | b'w' | b'm')) {
            return Some(false);
        }

        self.devices.allows(source, target)
    }
}

fn is_plain_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
}

struct AllowedPaths {
    absolute: Vec<PathBuf>,
    /// Relative entries resolve against the wings working directory, which the panel cannot know.
    has_relative: bool,
}

impl AllowedPaths {
    fn new<S: AsRef<str>>(entries: impl IntoIterator<Item = S>) -> Self {
        let mut absolute = Vec::new();
        let mut has_relative = false;

        for entry in entries {
            let entry = Path::new(entry.as_ref());
            if entry.is_absolute() {
                absolute.push(crate::cap::CapFilesystem::resolve_path(entry));
            } else {
                has_relative = true;
            }
        }

        Self {
            absolute,
            has_relative,
        }
    }

    fn allows(&self, source: &str, target: &str) -> Option<bool> {
        let source = Path::new(source);

        if (self.absolute.is_empty() && !self.has_relative)
            || !is_plain_absolute_path(Path::new(target))
            || !is_plain_absolute_path(source)
        {
            return Some(false);
        }

        if self
            .absolute
            .iter()
            .any(|allowed| source.starts_with(allowed))
        {
            Some(true)
        } else if self.has_relative {
            None
        } else {
            Some(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mounts(entries: &[&str]) -> NodeAllowedSources {
        NodeAllowedSources::new(entries.iter().copied(), std::iter::empty())
    }

    fn devices(entries: &[&str]) -> NodeAllowedSources {
        NodeAllowedSources::new(std::iter::empty(), entries.iter().copied())
    }

    // NodeAllowedSources::allows_mount
    #[test]
    fn an_empty_allowlist_allows_nothing() {
        let sources = mounts(&[]);

        assert_eq!(sources.allows_mount("/mnt/data", "/data"), Some(false));
    }

    #[test]
    fn mounts_match_by_path_component_prefix() {
        let sources = mounts(&["/mnt/data/"]);

        assert_eq!(sources.allows_mount("/mnt/data", "/data"), Some(true));
        assert_eq!(
            sources.allows_mount("/mnt/data/sub/dir", "/data"),
            Some(true)
        );
        assert_eq!(sources.allows_mount("/mnt/data2", "/data"), Some(false));
        assert_eq!(sources.allows_mount("/mnt", "/data"), Some(false));
    }

    #[test]
    fn mount_sources_and_targets_must_be_clean_absolute_paths() {
        let sources = mounts(&["/mnt/data"]);

        assert_eq!(sources.allows_mount("mnt/data", "/data"), Some(false));
        assert_eq!(
            sources.allows_mount("/mnt/data/../etc", "/data"),
            Some(false)
        );
        assert_eq!(sources.allows_mount("/mnt/data/./x", "/data"), Some(true));
        assert_eq!(sources.allows_mount("/mnt/data", "data"), Some(false));
        assert_eq!(
            sources.allows_mount("/mnt/data", "/data/../etc"),
            Some(false)
        );
    }

    #[test]
    fn absolute_entries_are_lexically_normalized() {
        let sources = mounts(&["/srv/x/../data"]);

        assert_eq!(sources.allows_mount("/srv/data/file", "/data"), Some(true));
        assert_eq!(sources.allows_mount("/srv/x/data", "/data"), Some(false));
    }

    #[test]
    fn relative_entries_are_unknowable_unless_an_absolute_entry_allows() {
        let sources = mounts(&["relative/data", "/mnt/data"]);

        assert_eq!(sources.allows_mount("/mnt/data/x", "/data"), Some(true));
        assert_eq!(sources.allows_mount("/srv/other", "/data"), None);
    }

    // NodeAllowedSources::allows_device
    #[test]
    fn device_permissions_must_be_nonempty_rwm() {
        let sources = devices(&["/dev/fuse"]);

        assert_eq!(
            sources.allows_device("/dev/fuse", "/dev/fuse", "rwm"),
            Some(true)
        );
        assert_eq!(
            sources.allows_device("/dev/fuse", "/dev/fuse", "r"),
            Some(true)
        );
        assert_eq!(
            sources.allows_device("/dev/fuse", "/dev/fuse", ""),
            Some(false)
        );
        assert_eq!(
            sources.allows_device("/dev/fuse", "/dev/fuse", "rwx"),
            Some(false)
        );
        assert_eq!(
            sources.allows_device("/dev/null", "/dev/null", "rw"),
            Some(false)
        );
    }

    #[test]
    fn mount_and_device_allowlists_are_independent() {
        let mount_only = mounts(&["/dev"]);
        let device_only = devices(&["/mnt"]);

        assert_eq!(
            mount_only.allows_device("/dev/fuse", "/dev/fuse", "rw"),
            Some(false)
        );
        assert_eq!(device_only.allows_mount("/mnt/data", "/data"), Some(false));
    }

    // NodeAllowedSources::from_config
    #[test]
    fn from_config_reads_the_allowlists_and_ignores_unknown_fields() {
        let config = wings_api::Config(serde_json::json!({
            "allowed_mounts": ["/srv"],
            "allowed_devices": ["/dev/fuse"],
            "docker": { "network": { "name": "pterodactyl_nw" } },
            "some_future_section": { "enabled": true, "values": [1, 2, 3] },
        }));

        let sources = NodeAllowedSources::from_config(&config).unwrap();

        assert_eq!(sources.allows_mount("/srv/x", "/data"), Some(true));
        assert_eq!(sources.allows_mount("/etc", "/data"), Some(false));
        assert_eq!(
            sources.allows_device("/dev/fuse", "/dev/fuse", "rw"),
            Some(true)
        );
    }

    #[test]
    fn from_config_without_allowlists_allows_nothing() {
        let config = wings_api::Config(serde_json::json!({ "token": "secret" }));

        let sources = NodeAllowedSources::from_config(&config).unwrap();

        assert_eq!(sources.allows_mount("/srv/x", "/data"), Some(false));
        assert_eq!(
            sources.allows_device("/dev/fuse", "/dev/fuse", "rw"),
            Some(false)
        );
    }
}
