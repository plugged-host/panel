//! plugged.host: what a node is for and how it is labelled: `nodes.role`, `nodes.tags`,
//! `nodes.srv_target` and `nodes.deployment_paused` (docs/features/node-telemetry.md).

/// The roles a node can have. A `game` node takes players' servers; an `internal` node is only
/// used by admins (tests, builds, a node being set up) and is never picked for players' servers.
pub const ROLES: &[&str] = &["game", "internal"];

pub const MAX_TAGS: usize = 32;
pub const MAX_TAG_LENGTH: usize = 64;

pub fn default_role() -> compact_str::CompactString {
    "game".into()
}

pub fn validate_role(role: &compact_str::CompactString, _context: &()) -> garde::Result {
    if ROLES.contains(&role.as_str()) {
        Ok(())
    } else {
        Err(garde::Error::new(compact_str::format_compact!(
            "the role must be one of {}",
            ROLES.join(", ")
        )))
    }
}

pub fn validate_optional_role(
    role: &Option<compact_str::CompactString>,
    context: &(),
) -> garde::Result {
    role.as_ref()
        .map_or(Ok(()), |role| validate_role(role, context))
}

/// Tags are short labels (`nvme`, `ddos-protected`): at most [`MAX_TAGS`], each 1 to
/// [`MAX_TAG_LENGTH`] characters of letters, digits, `-`, `_` and `.`.
pub fn validate_tags(tags: &[compact_str::CompactString], _context: &()) -> garde::Result {
    if tags.len() > MAX_TAGS {
        return Err(garde::Error::new(compact_str::format_compact!(
            "a node cannot have more than {MAX_TAGS} tags"
        )));
    }

    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty()
            || tag.chars().count() > MAX_TAG_LENGTH
            || !tag
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(garde::Error::new(compact_str::format_compact!(
                "tags are 1 to {MAX_TAG_LENGTH} letters, digits, dashes, underscores or dots"
            )));
        }
    }

    Ok(())
}

pub fn validate_optional_tags(
    tags: &Option<Vec<compact_str::CompactString>>,
    context: &(),
) -> garde::Result {
    tags.as_ref()
        .map_or(Ok(()), |tags| validate_tags(tags, context))
}

/// Tags as stored: trimmed, lower case, sorted, without duplicates.
pub fn normalize_tags(tags: &[compact_str::CompactString]) -> Vec<String> {
    let mut tags: Vec<String> = tags.iter().map(|tag| tag.trim().to_lowercase()).collect();
    tags.sort();
    tags.dedup();

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(values: &[&str]) -> Vec<compact_str::CompactString> {
        values.iter().map(|value| (*value).into()).collect()
    }

    #[test]
    fn roles() {
        assert!(validate_role(&"game".into(), &()).is_ok());
        assert!(validate_role(&"internal".into(), &()).is_ok());
        assert!(validate_role(&"storage".into(), &()).is_err());
        assert!(validate_optional_role(&None, &()).is_ok());
    }

    #[test]
    fn tag_rules() {
        assert!(validate_tags(&tags(&["nvme", "ddos-protected", "eu_west.2"]), &()).is_ok());
        assert!(validate_tags(&tags(&[""]), &()).is_err());
        assert!(validate_tags(&tags(&["two words"]), &()).is_err());
        assert!(validate_tags(&tags(&["a,b"]), &()).is_err());
        assert!(validate_tags(&tags(&[&"x".repeat(65)]), &()).is_err());
        assert!(validate_tags(&vec!["t".into(); 33], &()).is_err());
    }

    #[test]
    fn tags_are_normalized() {
        assert_eq!(
            normalize_tags(&tags(&[" NVMe", "ssd", "nvme", "ssd "])),
            ["nvme", "ssd"]
        );
    }
}
