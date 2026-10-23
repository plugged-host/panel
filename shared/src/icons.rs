use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    sync::LazyLock,
};
use utoipa::ToSchema;

pub const REFERENCED_ICONS_CACHE_KEY: &str = "icons::referenced";

#[derive(ToSchema, Serialize, Deserialize, Clone)]
pub struct ApiIcon {
    pub width: u32,
    pub height: u32,
    pub path: compact_str::CompactString,
}

type ManifestEntry = (u32, u32, compact_str::CompactString, serde::de::IgnoredAny);

static ICON_MANIFEST: LazyLock<HashMap<compact_str::CompactString, ApiIcon>> =
    LazyLock::new(|| {
        let Some(manifest) = crate::frontend_assets()
            .get_dir("assets/icons")
            .into_iter()
            .flat_map(|dir| dir.files())
            .filter_map(|file| file.path().to_str())
            .find_map(|path| {
                let path = path.strip_suffix(".gz").unwrap_or(path);
                let file_name = path.rsplit('/').next()?;

                if file_name.starts_with("fontawesome.") && file_name.ends_with(".json") {
                    crate::frontend_asset(path)
                } else {
                    None
                }
            })
        else {
            tracing::warn!(
                "no icon manifest found in the embedded frontend, custom icons will fall back"
            );
            return HashMap::new();
        };

        let entries = manifest
            .decompressed()
            .map_err(anyhow::Error::from)
            .and_then(|contents| {
                Ok(serde_json::from_slice::<
                    HashMap<compact_str::CompactString, ManifestEntry>,
                >(&contents)?)
            });

        match entries {
            Ok(entries) => entries
                .into_iter()
                .map(|(identifier, (width, height, path, _))| {
                    (
                        identifier,
                        ApiIcon {
                            width,
                            height,
                            path,
                        },
                    )
                })
                .collect(),
            Err(err) => {
                tracing::error!(
                    "failed to parse icon manifest {}: {:#?}",
                    manifest.path,
                    err
                );
                HashMap::new()
            }
        }
    });

pub fn resolve_icons<'a>(
    identifiers: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<compact_str::CompactString, ApiIcon> {
    identifiers
        .into_iter()
        .filter_map(|identifier| {
            ICON_MANIFEST
                .get_key_value(identifier)
                .map(|(identifier, icon)| (identifier.clone(), icon.clone()))
        })
        .collect()
}

pub fn validate_icon(icon: &str, _context: &()) -> Result<(), garde::Error> {
    let mut segments = icon.split(':');
    let valid = segments.clone().count() >= 2
        && segments.all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });

    if !valid {
        return Err(garde::Error::new(
            "must be an icon identifier like fontawesome:solid:house",
        ));
    }

    Ok(())
}
