// plugged.host: how the Pelican panel's own conventions become Calagopus fields during an import
// (docs/features/import.md): node tags as locations with a flag, a tier and a node role,
// overallocation as a node's capacity, the egg tag that marks FiveM, regional languages, the
// permissions of the plugin pages and schedule steps that would start a server. Pure rules, used
// by `pelican.rs` and `validate.rs`.

use super::{countries, model::*};
use compact_str::CompactString;
use std::collections::{HashMap, HashSet};

/// What a location carries besides its name: `locations.flag`, `tier`, `sort` and `public`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationPlacement {
    /// A lower-case ISO 3166-1 alpha-2 code.
    pub flag: Option<String>,
    /// `premium` or `plus`, the plan tier a player needs for the location.
    pub tier: Option<String>,
    pub sort: i32,
    pub public: bool,
}

impl Default for LocationPlacement {
    fn default() -> Self {
        Self {
            flag: None,
            tier: None,
            sort: 0,
            public: true,
        }
    }
}

/// What a node carries besides upstream's fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodePlacement {
    /// `game` or `internal`; `None` keeps the panel's default (`game`).
    pub role: Option<String>,
    /// The node's tags as the source panel wrote them.
    pub tags: Vec<String>,
    pub srv_target: Option<String>,
    /// Pelican's overallocation in percent; `None` imports the capacity as it is.
    pub memory_overallocate: Option<i64>,
    pub disk_overallocate: Option<i64>,
}

/// The tags of a Pelican node or egg: a JSON list, its entries trimmed, empty ones dropped.
pub fn parse_tags(raw: Option<&str>) -> Vec<String> {
    raw.and_then(|raw| serde_json::from_str::<Vec<serde_json::Value>>(raw).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|tag| match tag {
            serde_json::Value::String(tag) => Some(tag),
            serde_json::Value::Number(tag) => Some(tag.to_string()),
            _ => None,
        })
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect()
}

/// A node's memory or disk as Calagopus counts it, where 0 means unlimited. Pelican lets a node
/// take any amount when the value is 0 or less or its overallocation is negative, and otherwise
/// up to value × (100 + overallocation) %. Without an overallocation (Pterodactyl) the value is
/// kept, negative ones as 0.
pub fn node_capacity(value: i64, overallocate: Option<i64>) -> i64 {
    match overallocate {
        None => value.max(0),
        Some(_) if value <= 0 => 0,
        Some(percent) if percent < 0 => 0,
        Some(percent) => value
            .saturating_mul(100i64.saturating_add(percent))
            .saturating_div(100),
    }
}

/// Whether [`node_capacity`] makes a node unlimited that had a capacity in the source panel.
pub fn becomes_unlimited(value: i64, overallocate: Option<i64>) -> bool {
    value > 0 && overallocate.is_some_and(|percent| percent < 0)
}

/// The Pelican panel's location settings, from its environment.
pub struct TagRules {
    /// `UCS_DEPLOYMENT_TAGS`: when set, only nodes with one of these tags take players' servers.
    pub deployment: Option<HashSet<String>>,
    /// `UCS_PREMIUM_TAG`: the tag of the PlusPlus (`premium`) tier's nodes.
    pub premium: String,
    /// `UCS_PLUS_TAG`: the tag of the Plus (`plus`) tier's nodes.
    pub plus: String,
    /// Tags of nodes the platform keeps for itself: `LIMBO_NODE_TAG`, `backup` and `storage`.
    pub internal: HashSet<String>,
}

impl Default for TagRules {
    fn default() -> Self {
        Self::from_env(|_| None)
    }
}

impl TagRules {
    pub fn from_env(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let tag = |key: &str, default: &str| {
            lookup(key)
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| default.to_string())
        };

        let deployment: HashSet<String> = lookup("UCS_DEPLOYMENT_TAGS")
            .unwrap_or_default()
            .split(',')
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();

        Self {
            deployment: (!deployment.is_empty()).then_some(deployment),
            premium: tag("UCS_PREMIUM_TAG", "premium"),
            plus: tag("UCS_PLUS_TAG", "plus"),
            internal: [
                tag("LIMBO_NODE_TAG", "limbo"),
                "backup".into(),
                "storage".into(),
            ]
            .into_iter()
            .collect(),
        }
    }
}

const INTERNAL_DESCRIPTION: &str =
    "generated from Pelican node tags during import, for nodes that take no players' servers";

/// Where one node goes, see [`place_nodes`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Place {
    /// A location players see: a country and a tier.
    Game {
        country: String,
        tier: Option<&'static str>,
    },
    /// A location only admins see, named after the node's internal tag, else its first one.
    Internal { label: Option<String> },
}

fn place(tags: &[String], rules: &TagRules) -> (Place, Option<String>) {
    let tags: Vec<String> = tags.iter().map(|tag| tag.to_lowercase()).collect();
    let country = tags
        .iter()
        .find(|tag| countries::name(tag).is_some())
        .cloned();
    let tier = if tags.contains(&rules.premium) {
        Some(shared::economy::TIER_PREMIUM)
    } else if tags.contains(&rules.plus) {
        Some(shared::economy::TIER_PLUS)
    } else {
        None
    };
    let internal = tags.iter().any(|tag| rules.internal.contains(tag));
    let deployable = rules
        .deployment
        .as_ref()
        .is_none_or(|deployment| tags.iter().any(|tag| deployment.contains(tag)));

    match country {
        Some(country) if !internal && deployable => (
            Place::Game {
                country: country.clone(),
                tier,
            },
            Some(country),
        ),
        // named after its internal tag (`limbo`), else its first other one
        country => (
            Place::Internal {
                label: tags
                    .iter()
                    .find(|tag| rules.internal.contains(*tag))
                    .or_else(|| {
                        tags.iter()
                            .find(|tag| **tag != rules.premium && **tag != rules.plus)
                    })
                    .cloned(),
            },
            country,
        ),
    }
}

/// Pelican has no locations. Its location catalog offers one location per country tag
/// (ISO 3166-1 alpha-2) of its public nodes, a premium or plus tag puts the location in that
/// tier, and with deployment tags set only nodes carrying one take players' servers. When some
/// node has a country tag, the import follows that catalog: such a node becomes a `game` node in
/// the location of its country and tier (named after the country, with the tier added when the
/// country has nodes of several tiers, public when one of its nodes is), and every other node an
/// `internal` node in a location only admins see (named after its internal tag, else its first
/// tag, `Internal` without one). Without any country tag, upstream's locations (one per first tag)
/// are kept.
pub fn place_nodes(
    locations: Vec<SourceLocation>,
    mut nodes: Vec<SourceNode>,
    rules: &TagRules,
) -> (Vec<SourceLocation>, Vec<SourceNode>) {
    let places: Vec<(Place, Option<String>)> = nodes
        .iter()
        .map(|node| place(&node.placement.tags, rules))
        .collect();
    if places.iter().all(|(_, country)| country.is_none()) {
        return (locations, nodes);
    }

    let mut tiers: HashMap<&str, HashSet<Option<&'static str>>> = HashMap::new();
    let mut public: HashMap<&Place, bool> = HashMap::new();
    for ((place, _), node) in places.iter().zip(&nodes) {
        if let Place::Game { country, tier } = place {
            tiers.entry(country.as_str()).or_default().insert(*tier);
        }
        *public.entry(place).or_default() |= node.public;
    }

    let mut ids: HashMap<&Place, i64> = HashMap::new();
    let mut placed = Vec::new();
    for ((place, _), node) in places.iter().zip(nodes.iter_mut()) {
        let id = match ids.get(place) {
            Some(id) => *id,
            None => {
                let id = placed.len() as i64 + 1;
                let (name, description, placement) = match place {
                    Place::Game { country, tier } => {
                        let mut name = countries::name(country)
                            .unwrap_or(country.as_str())
                            .to_string();
                        if let Some(tier) = tier
                            && tiers
                                .get(country.as_str())
                                .is_some_and(|tiers| tiers.len() > 1)
                        {
                            name.push_str(if *tier == shared::economy::TIER_PREMIUM {
                                " Premium"
                            } else {
                                " Plus"
                            });
                        }

                        (
                            name,
                            format!(
                                "generated from Pelican node tag `{country}`{} during import",
                                tier.map_or_else(String::new, |tier| format!(" ({tier})"))
                            ),
                            LocationPlacement {
                                flag: Some(country.clone()),
                                tier: tier.map(str::to_string),
                                sort: 0,
                                public: public.get(place).copied().unwrap_or(true),
                            },
                        )
                    }
                    Place::Internal { label } => (
                        label.clone().unwrap_or_else(|| "Internal".into()),
                        INTERNAL_DESCRIPTION.to_string(),
                        LocationPlacement {
                            public: false,
                            ..Default::default()
                        },
                    ),
                };

                placed.push(SourceLocation {
                    id,
                    name,
                    description: Some(description),
                    created: node.created,
                    placement,
                });
                ids.insert(place, id);
                id
            }
        };

        node.location_id = id;
        node.placement.role = Some(match place {
            Place::Game { .. } => "game".into(),
            Place::Internal { .. } => "internal".into(),
        });
    }

    (placed, nodes)
}

/// Features an egg gets from its Pelican tags: the `cfx` tag marks FiveM and RedM eggs, which
/// Calagopus knows by the `cfx` feature (`shared::fivem::FEATURE`).
pub fn tag_features(raw_tags: Option<&str>) -> Vec<CompactString> {
    if parse_tags(raw_tags)
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case(shared::fivem::FEATURE))
    {
        vec![shared::fivem::FEATURE.into()]
    } else {
        Vec::new()
    }
}

/// `features` with `extra` added where missing.
pub fn with_features(
    mut features: Vec<CompactString>,
    extra: &[CompactString],
) -> Vec<CompactString> {
    for feature in extra {
        if !features.contains(feature) {
            features.push(feature.clone());
        }
    }

    features
}

/// The language the panel ships for a regional code it does not (`pt_BR` → `pt`); `None` when
/// the code is shipped as it is or its language is not.
pub fn regional_language(language: &str, offered: &[CompactString]) -> Option<CompactString> {
    let language = language.trim();
    if offered.iter().any(|offered| offered == language) {
        return None;
    }

    let primary = language
        .split(['_', '-'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    offered.iter().find(|offered| **offered == primary).cloned()
}

/// Calagopus permissions of pages the Pelican panel's plugins gated with its core permissions,
/// each with the Pelican permissions a subuser needed for everything the Calagopus permission
/// covers: a subuser gets it only with all of them, so nobody can do more than before. Resizing
/// and deleting a server were the owner's alone on Pelican and are not given.
const DERIVED_PERMISSIONS: &[(&str, &[&str])] = &[
    // plugins and mods: browse and install (file.create), turn off (file.update), delete
    (
        "minecraft.content",
        &["file.create", "file.update", "file.delete"],
    ),
    // server.properties and the Bukkit files: read (file.read + file.read-content), save
    (
        "minecraft.settings",
        &["file.read", "file.read-content", "file.update"],
    ),
    // whitelist, operators and bans: shown from the files, changed in them
    (
        "minecraft.players",
        &["file.read", "file.read-content", "file.update"],
    ),
    (
        "minecraft.worlds",
        &[
            "file.read",
            "file.read-content",
            "file.create",
            "file.update",
            "file.delete",
        ],
    ),
    // Geyser and Floodgate: installed and then restarted
    ("minecraft.bedrock", &["file.create", "control.restart"]),
    (
        "fivem.config",
        &["file.read", "file.read-content", "file.update"],
    ),
    // the players page had no permission of its own
    ("fivem.players", &[]),
    ("fivem.artifacts", &["startup.read", "startup.update"]),
    (
        "fivem.frameworks",
        &[
            "file.read",
            "file.read-content",
            "file.create",
            "file.delete",
            "control.restart",
            "database.create",
        ],
    ),
    // the subdomains plugin gated its pages with the allocation permissions
    ("subdomains.read", &["allocation.read"]),
    ("subdomains.create", &["allocation.create"]),
    ("subdomains.update", &["allocation.update"]),
    ("subdomains.delete", &["allocation.delete"]),
    // switching the software was a reinstall
    ("settings.software", &["settings.reinstall"]),
];

/// The permissions [`DERIVED_PERMISSIONS`] gives a subuser with the Pelican `permissions`.
pub fn derived_permissions(permissions: &[CompactString]) -> Vec<&'static str> {
    DERIVED_PERMISSIONS
        .iter()
        .filter(|(_, needed)| {
            needed
                .iter()
                .all(|needed| permissions.iter().any(|permission| permission == needed))
        })
        .map(|(permission, _)| *permission)
        .collect()
}

/// The schedules allowed to start their server: those of servers owned by an admin. Calagopus
/// refuses a schedule step that starts a server for everyone else (a schedule runs on Wings,
/// past the start gates, docs/features/start-gates.md), so the import leaves those steps out.
pub fn schedules_that_may_start(source: &SourceData) -> HashSet<i64> {
    let admins: HashSet<i64> = source
        .users
        .iter()
        .filter(|user| user.admin)
        .map(|user| user.id)
        .collect();
    let servers: HashSet<i64> = source
        .servers
        .iter()
        .filter(|server| admins.contains(&server.owner_id))
        .map(|server| server.id)
        .collect();

    source
        .schedules
        .iter()
        .filter(|schedule| servers.contains(&schedule.server_id))
        .map(|schedule| schedule.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: i64, tags: &[&str], public: bool) -> SourceNode {
        SourceNode {
            id,
            uuid: Ok(uuid::Uuid::from_u128(id as u128)),
            location_id: 0,
            backup_config_id: None,
            name: format!("node{id}"),
            description: None,
            public,
            maintenance_mode: false,
            scheme: "https".into(),
            fqdn: format!("node{id}.example.com"),
            daemon_listen: 8080,
            daemon_connect: None,
            daemon_sftp: 2022,
            sftp_alias: None,
            memory: 1024,
            disk: 1024,
            token_id: "abcdefghijklmnop".into(),
            token: Ok("token".into()),
            created: None,
            placement: NodePlacement {
                tags: tags.iter().map(|tag| tag.to_string()).collect(),
                ..Default::default()
            },
        }
    }

    fn rules(deployment: &str) -> TagRules {
        let deployment = deployment.to_string();
        TagRules::from_env(move |key| (key == "UCS_DEPLOYMENT_TAGS").then(|| deployment.clone()))
    }

    fn location_of<'a>(
        locations: &'a [SourceLocation],
        nodes: &[SourceNode],
        id: i64,
    ) -> &'a SourceLocation {
        let node = nodes.iter().find(|node| node.id == id).unwrap();
        locations
            .iter()
            .find(|location| location.id == node.location_id)
            .unwrap()
    }

    fn role(nodes: &[SourceNode], id: i64) -> &str {
        nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap()
            .placement
            .role
            .as_deref()
            .unwrap()
    }

    #[test]
    fn parse_tags_reads_json_lists() {
        assert_eq!(
            parse_tags(Some(r#"["premium"," de ",""]"#)),
            ["premium", "de"]
        );
        assert_eq!(parse_tags(Some("[]")), Vec::<String>::new());
        assert_eq!(parse_tags(Some("not json")), Vec::<String>::new());
        assert_eq!(parse_tags(None), Vec::<String>::new());
    }

    #[test]
    fn node_capacity_follows_pelican_overallocation() {
        // Pterodactyl: as it is
        assert_eq!(node_capacity(2048, None), 2048);
        assert_eq!(node_capacity(-5, None), 0);
        // unlimited: no value or a negative overallocation
        assert_eq!(node_capacity(0, Some(0)), 0);
        assert_eq!(node_capacity(-1, Some(10)), 0);
        assert_eq!(node_capacity(1_024_000, Some(-1)), 0);
        // limited: value plus the overallocation
        assert_eq!(node_capacity(1000, Some(0)), 1000);
        assert_eq!(node_capacity(1000, Some(50)), 1500);
        assert_eq!(node_capacity(i64::MAX, Some(100)), i64::MAX / 100);

        assert!(becomes_unlimited(1_024_000, Some(-1)));
        assert!(!becomes_unlimited(0, Some(-1)));
        assert!(!becomes_unlimited(1000, Some(0)));
        assert!(!becomes_unlimited(1000, None));
    }

    #[test]
    fn tag_rules_read_the_pelican_environment() {
        let rules = TagRules::from_env(|key| match key {
            "UCS_DEPLOYMENT_TAGS" => Some(" FR, de,,nl ".into()),
            "UCS_PREMIUM_TAG" => Some("Gold".into()),
            "LIMBO_NODE_TAG" => Some("parked".into()),
            _ => None,
        });
        assert_eq!(
            rules.deployment,
            Some(["fr", "de", "nl"].map(String::from).into_iter().collect())
        );
        assert_eq!(rules.premium, "gold");
        assert_eq!(rules.plus, "plus");
        assert!(rules.internal.contains("parked"));
        assert!(rules.internal.contains("backup"));
        assert!(!rules.internal.contains("limbo"));

        let defaults = TagRules::default();
        assert_eq!(defaults.deployment, None);
        assert!(defaults.internal.contains("limbo"));
    }

    #[test]
    fn without_country_tags_upstream_locations_are_kept() {
        let locations = vec![SourceLocation {
            id: 1,
            name: "pelican".into(),
            description: None,
            created: None,
            placement: LocationPlacement::default(),
        }];
        let mut nodes = vec![node(1, &["eu-west"], true)];
        nodes[0].location_id = 1;

        let (locations, nodes) = place_nodes(locations, nodes, &TagRules::default());
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].name, "pelican");
        assert_eq!(nodes[0].location_id, 1);
        assert_eq!(nodes[0].placement.role, None);
    }

    #[test]
    fn country_tags_become_locations_with_flag_tier_and_role() {
        let nodes = vec![
            node(1, &["premium", "de"], true),
            node(2, &[], true),
            node(73, &["fr"], true),
            node(77, &["nl"], true),
            node(78, &["NL"], true),
            node(86, &["limbo"], false),
            node(94, &["th"], true),
            node(99, &["us"], true),
        ];

        let (locations, nodes) = place_nodes(Vec::new(), nodes, &rules("fr,de,nl,th"));

        let germany = location_of(&locations, &nodes, 1);
        assert_eq!(germany.name, "Germany");
        assert_eq!(
            germany.placement,
            LocationPlacement {
                flag: Some("de".into()),
                tier: Some("premium".into()),
                sort: 0,
                public: true,
            }
        );
        assert_eq!(role(&nodes, 1), "game");

        let netherlands = location_of(&locations, &nodes, 77);
        assert_eq!(netherlands.name, "Netherlands");
        assert_eq!(netherlands.placement.flag.as_deref(), Some("nl"));
        assert_eq!(netherlands.placement.tier, None);
        assert_eq!(location_of(&locations, &nodes, 78).id, netherlands.id);

        // no tags: internal, not public
        let internal = location_of(&locations, &nodes, 2);
        assert_eq!(internal.name, "Internal");
        assert!(!internal.placement.public);
        assert_eq!(internal.placement.flag, None);
        assert_eq!(role(&nodes, 2), "internal");

        // the limbo node
        let limbo = location_of(&locations, &nodes, 86);
        assert_eq!(limbo.name, "limbo");
        assert!(!limbo.placement.public);
        assert_eq!(role(&nodes, 86), "internal");

        // a country outside the deployment tags takes no players' servers
        let outside = location_of(&locations, &nodes, 99);
        assert_eq!(outside.name, "us");
        assert!(!outside.placement.public);
        assert_eq!(role(&nodes, 99), "internal");

        let ids: Vec<i64> = locations.iter().map(|location| location.id).collect();
        assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn a_country_with_several_tiers_names_them() {
        let nodes = vec![
            node(1, &["de"], true),
            node(2, &["de", "premium"], true),
            node(3, &["plus", "de"], true),
            node(4, &["fr", "plus"], true),
        ];

        let (locations, nodes) = place_nodes(Vec::new(), nodes, &TagRules::default());

        assert_eq!(location_of(&locations, &nodes, 1).name, "Germany");
        assert_eq!(location_of(&locations, &nodes, 2).name, "Germany Premium");
        assert_eq!(location_of(&locations, &nodes, 3).name, "Germany Plus");
        let france = location_of(&locations, &nodes, 4);
        assert_eq!(france.name, "France");
        assert_eq!(france.placement.tier.as_deref(), Some("plus"));
    }

    #[test]
    fn internal_tags_and_private_nodes() {
        let nodes = vec![
            node(1, &["fr", "backup"], true),
            node(2, &["fr", "storage"], true),
            node(3, &["fr"], false),
            node(4, &["pl"], false),
            node(5, &["pl"], true),
        ];

        let (locations, nodes) = place_nodes(Vec::new(), nodes, &TagRules::default());

        // named after the internal tag, not the country
        assert_eq!(role(&nodes, 1), "internal");
        assert_eq!(location_of(&locations, &nodes, 1).name, "backup");
        assert_eq!(role(&nodes, 2), "internal");
        assert_eq!(location_of(&locations, &nodes, 2).name, "storage");
        assert!(!location_of(&locations, &nodes, 2).placement.public);

        // a game location is public when one of its nodes is (Pelican offers public nodes only)
        assert_eq!(role(&nodes, 3), "game");
        assert!(!location_of(&locations, &nodes, 3).placement.public);
        assert!(location_of(&locations, &nodes, 4).placement.public);
    }

    #[test]
    fn the_cfx_tag_becomes_the_cfx_feature() {
        assert_eq!(tag_features(Some(r#"["cfx"]"#)), ["cfx"]);
        assert_eq!(tag_features(Some(r#"[" CFX "]"#)), ["cfx"]);
        assert!(tag_features(Some(r#"["minecraft","java"]"#)).is_empty());
        assert!(tag_features(None).is_empty());

        assert_eq!(
            with_features(vec!["eula".into()], &["cfx".into()]),
            ["eula", "cfx"]
        );
        assert_eq!(with_features(vec!["cfx".into()], &["cfx".into()]), ["cfx"]);
    }

    #[test]
    fn regional_languages_map_to_the_shipped_one() {
        let offered: Vec<CompactString> = ["en", "de", "pt", "uk"].map(Into::into).to_vec();

        assert_eq!(regional_language("pt_BR", &offered).as_deref(), Some("pt"));
        assert_eq!(regional_language("pt-PT", &offered).as_deref(), Some("pt"));
        assert_eq!(regional_language("DE_at", &offered).as_deref(), Some("de"));
        // shipped as it is: nothing to map
        assert_eq!(regional_language("pt", &offered), None);
        assert_eq!(regional_language("zh_CN", &offered), None);
        assert_eq!(regional_language("es", &offered), None);
        assert_eq!(regional_language("", &offered), None);
    }

    #[test]
    fn derived_permissions_need_every_pelican_permission() {
        let permissions = |list: &[&str]| -> Vec<CompactString> {
            list.iter().map(|permission| (*permission).into()).collect()
        };

        assert_eq!(derived_permissions(&[]), ["fivem.players"]);

        let files = permissions(&[
            "file.read",
            "file.read-content",
            "file.create",
            "file.update",
            "file.delete",
        ]);
        let derived = derived_permissions(&files);
        for permission in [
            "minecraft.content",
            "minecraft.settings",
            "minecraft.players",
            "minecraft.worlds",
            "fivem.config",
        ] {
            assert!(derived.contains(&permission), "{permission}");
        }
        assert!(!derived.contains(&"minecraft.bedrock"));
        assert!(!derived.contains(&"fivem.frameworks"));

        let read_only = derived_permissions(&permissions(&["file.read", "file.read-content"]));
        assert_eq!(read_only, ["fivem.players"]);

        let derived = derived_permissions(&permissions(&[
            "allocation.read",
            "allocation.delete",
            "settings.reinstall",
            "startup.read",
            "startup.update",
        ]));
        assert!(derived.contains(&"subdomains.read"));
        assert!(derived.contains(&"subdomains.delete"));
        assert!(!derived.contains(&"subdomains.create"));
        assert!(derived.contains(&"settings.software"));
        assert!(derived.contains(&"fivem.artifacts"));
        assert!(!derived.contains(&"settings.resize"));
        assert!(!derived.contains(&"settings.delete"));
    }

    #[test]
    fn derived_permissions_exist_in_calagopus() {
        let known = shared::permissions::base_server_permission_keys();

        for (permission, _) in DERIVED_PERMISSIONS {
            assert!(known.contains(*permission), "{permission}");
        }
    }
}
