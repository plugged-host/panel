pub const CENSORED_PLACEHOLDER: &str = "";

/// Implemented by values carrying secrets, so each one names its own instead of a central list
/// having to track them.
///
/// Censoring happens in place and on an owned value rather than during serialization: the same
/// [`serde::Serialize`] impls back both the API responses and the `jsonb` columns these values are
/// stored in, so a serializer that censored would write the placeholder to the database.
pub trait Censor {
    /// Overwrite every secret this value carries, leaving everything else intact.
    fn censor(&mut self);

    #[inline]
    fn censored(mut self) -> Self
    where
        Self: Sized,
    {
        self.censor();
        self
    }
}

impl<T: Censor> Censor for Option<T> {
    fn censor(&mut self) {
        if let Some(value) = self {
            value.censor();
        }
    }
}

impl Censor for wings_api::Config {
    fn censor(&mut self) {
        censor_paths(&mut self.0, wings_api::SECRET_CONFIG_PATHS);
    }
}

impl Censor for db_agent_api::Config {
    fn censor(&mut self) {
        censor_paths(&mut self.0, db_agent_api::SECRET_CONFIG_PATHS);
    }
}

fn walk_path(
    value: &mut serde_json::Value,
    segments: &[&str],
    leaf: &mut impl FnMut(&mut serde_json::Map<String, serde_json::Value>, &str),
) {
    let serde_json::Value::Object(map) = value else {
        return;
    };
    let Some((segment, rest)) = segments.split_first() else {
        return;
    };

    let keys: Vec<String> = if *segment == "*" {
        map.keys().cloned().collect()
    } else if map.contains_key(*segment) {
        vec![segment.to_string()]
    } else {
        return;
    };

    for key in keys {
        if rest.is_empty() {
            leaf(map, &key);
        } else if let Some(next) = map.get_mut(&key) {
            walk_path(next, rest, leaf);
        }
    }
}

/// Overwrite every non-null value at `paths` (dot separated, `*` matching every key of a map).
pub fn censor_paths(value: &mut serde_json::Value, paths: &[&str]) {
    for path in paths {
        let segments: Vec<&str> = path.split('.').collect();
        walk_path(value, &segments, &mut |map, key| {
            if let Some(entry) = map.get_mut(key)
                && !entry.is_null()
            {
                *entry = CENSORED_PLACEHOLDER.into();
            }
        });
    }
}

/// Drop every value at `paths` still holding [`CENSORED_PLACEHOLDER`] from a merge patch, so
/// saving back a censored config leaves the real secrets in place.
pub fn strip_censored_paths(patch: &mut serde_json::Value, paths: &[&str]) {
    for path in paths {
        let segments: Vec<&str> = path.split('.').collect();
        walk_path(patch, &segments, &mut |map, key| {
            if map.get(key).and_then(serde_json::Value::as_str) == Some(CENSORED_PLACEHOLDER) {
                map.remove(key);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // censor_paths
    #[test]
    fn censor_paths_replaces_values_at_wildcard_paths_and_keeps_siblings() {
        let mut value = json!({
            "token": "abc",
            "registries": {
                "ghcr.io": { "username": "alice", "password": "p1" },
                "docker.io": { "username": "bob", "password": "p2" },
            },
            "headers": { "X-A": "1", "X-B": "2" },
            "name": "node",
        });

        censor_paths(&mut value, &["token", "registries.*.password", "headers.*"]);

        assert_eq!(
            value,
            json!({
                "token": CENSORED_PLACEHOLDER,
                "registries": {
                    "ghcr.io": { "username": "alice", "password": CENSORED_PLACEHOLDER },
                    "docker.io": { "username": "bob", "password": CENSORED_PLACEHOLDER },
                },
                "headers": { "X-A": CENSORED_PLACEHOLDER, "X-B": CENSORED_PLACEHOLDER },
                "name": "node",
            })
        );
    }

    #[test]
    fn censor_paths_keeps_nulls_and_ignores_missing_or_non_object_paths() {
        let mut value = json!({
            "token": null,
            "registries": "not an object",
            "headers": { "X-A": null },
        });
        let expected = value.clone();

        censor_paths(
            &mut value,
            &[
                "token",
                "registries.*.password",
                "headers.*",
                "missing.deep.path",
            ],
        );

        assert_eq!(value, expected);
    }

    // strip_censored_paths
    #[test]
    fn strip_censored_paths_removes_only_placeholders() {
        let mut patch = json!({
            "token": CENSORED_PLACEHOLDER,
            "token_id": 5,
            "registries": {
                "ghcr.io": { "username": "alice", "password": CENSORED_PLACEHOLDER },
                "docker.io": { "username": "bob", "password": "new-secret" },
            },
            "name": "node",
        });

        strip_censored_paths(&mut patch, &["token", "token_id", "registries.*.password"]);

        assert_eq!(
            patch,
            json!({
                "token_id": 5,
                "registries": {
                    "ghcr.io": { "username": "alice" },
                    "docker.io": { "username": "bob", "password": "new-secret" },
                },
                "name": "node",
            })
        );
    }

    #[test]
    fn a_censored_config_stripped_back_carries_no_secret_overwrites() {
        let original = json!({
            "token": "abc",
            "remote_headers": { "Authorization": "Bearer x" },
            "debug": true,
        });
        let mut patch = original.clone();

        censor_paths(&mut patch, &["token", "remote_headers.*"]);
        strip_censored_paths(&mut patch, &["token", "remote_headers.*"]);

        assert_eq!(patch, json!({ "remote_headers": {}, "debug": true }));
    }

    // Censor for wings_api::Config
    #[test]
    fn wings_config_censors_every_secret_and_keeps_the_rest() {
        let mut config = wings_api::Config(json!({
            "token_id": "id",
            "token": "secret",
            "remote": "https://panel.example",
            "remote_headers": { "X-Auth": "h" },
            "docker": {
                "network": { "name": "pterodactyl_nw" },
                "registries": { "ghcr.io": { "username": "alice", "password": "p" } },
                "log_config": { "type": "local", "config": { "max-size": "5m" } },
            },
            "system": {
                "data": "/var/lib/pterodactyl/volumes",
                "backups": { "restic": { "repository": "/repo", "environment": { "RESTIC_PASSWORD": "r" } } },
            },
        }));

        config.censor();

        assert_eq!(
            config.0,
            json!({
                "token_id": CENSORED_PLACEHOLDER,
                "token": CENSORED_PLACEHOLDER,
                "remote": "https://panel.example",
                "remote_headers": { "X-Auth": CENSORED_PLACEHOLDER },
                "docker": {
                    "network": { "name": "pterodactyl_nw" },
                    "registries": { "ghcr.io": { "username": "alice", "password": CENSORED_PLACEHOLDER } },
                    "log_config": { "type": "local", "config": { "max-size": CENSORED_PLACEHOLDER } },
                },
                "system": {
                    "data": "/var/lib/pterodactyl/volumes",
                    "backups": { "restic": { "repository": "/repo", "environment": { "RESTIC_PASSWORD": CENSORED_PLACEHOLDER } } },
                },
            })
        );
    }

    // Censor for db_agent_api::Config
    #[test]
    fn db_agent_config_censors_every_secret_and_keeps_the_rest() {
        let mut config = db_agent_api::Config(json!({
            "api": { "host": "0.0.0.0", "port": 8080, "token": "secret" },
            "database": { "url": "postgres://u:p@localhost/db", "max_connections": 5 },
            "docker": {
                "registries": { "ghcr.io": { "username": "alice", "password": "p" } },
                "log_config": { "type": "local", "config": { "max-size": "5m" } },
            },
        }));

        config.censor();

        assert_eq!(
            config.0,
            json!({
                "api": { "host": "0.0.0.0", "port": 8080, "token": CENSORED_PLACEHOLDER },
                "database": { "url": CENSORED_PLACEHOLDER, "max_connections": 5 },
                "docker": {
                    "registries": { "ghcr.io": { "username": "alice", "password": CENSORED_PLACEHOLDER } },
                    "log_config": { "type": "local", "config": { "max-size": CENSORED_PLACEHOLDER } },
                },
            })
        );
    }
}
