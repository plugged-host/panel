//! plugged.host: one player of a Minecraft server in detail (docs/features/minecraft.md#player-details):
//! the saved data (`<world>/playerdata/<uuid>.dat`, gzip NBT: health, food, experience, game mode,
//! position, inventory, ender chest), the statistics (`<world>/stats/<uuid>.json`), and the changes
//! the Players page makes to them.
//!
//! A running server keeps an online player's data in memory and writes it every few minutes and
//! when they leave, so a change goes in as a console command while the server runs and the player
//! is online, and into the file only while the server is stopped. An edited file keeps its last
//! version next to it as `<uuid>.dat_old`, as Minecraft does on every save (and reads it when the
//! `.dat` is broken); deleted files go to the server's trash when it has one.

use super::{
    config, files, players,
    players::{Outcome, PlayerError},
};
use crate::models::server::Server;
use fastnbt::Value;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Read};
use utoipa::ToSchema;

type Compound = HashMap<String, Value>;

const PLAYERDATA: &str = "playerdata";
const STATS: &str = "stats";
const ADVANCEMENTS: &str = "advancements";
/// Bigger than any real player file; Wings refuses larger ones and caps what it unpacks.
const MAX_DAT: u64 = 32 * 1024 * 1024;
const MAX_NBT: u64 = 64 * 1024 * 1024;
const MAX_JSON: u64 = 8 * 1024 * 1024;
/// The entries of each statistics list.
const TOP: usize = 5;
/// Minecraft's world border: no coordinate beyond it.
const WORLD_LIMIT: f64 = 30_000_000.0;

pub const OVERWORLD: &str = "minecraft:overworld";
pub const NETHER: &str = "minecraft:the_nether";
pub const END: &str = "minecraft:the_end";

#[derive(ToSchema, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    fn from_id(id: i64) -> Option<Self> {
        match id {
            0 => Some(Self::Survival),
            1 => Some(Self::Creative),
            2 => Some(Self::Adventure),
            3 => Some(Self::Spectator),
            _ => None,
        }
    }

    fn id(self) -> i32 {
        match self {
            Self::Survival => 0,
            Self::Creative => 1,
            Self::Adventure => 2,
            Self::Spectator => 3,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Survival => "survival",
            Self::Creative => "creative",
            Self::Adventure => "adventure",
            Self::Spectator => "spectator",
        }
    }
}

#[derive(ToSchema, Serialize, Debug, Clone, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// `minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end` or a datapack's.
    pub dimension: compact_str::CompactString,
}

#[derive(ToSchema, Serialize, Debug, Clone, PartialEq)]
pub struct Enchantment {
    pub id: compact_str::CompactString,
    /// "Sharpness V", "Mending".
    pub label: String,
}

#[derive(ToSchema, Serialize, Debug, Clone, PartialEq)]
pub struct PlayerItem {
    /// The inventory's slots as before 1.21.5: 0-8 the hotbar, 9-35 the inventory, 100-103 the
    /// feet to the head, -106 the off hand. The ender chest's are 0-26.
    pub slot: i32,
    /// `minecraft:diamond_sword`.
    pub id: compact_str::CompactString,
    /// The item's name from its id ("Diamond Sword").
    pub label: String,
    /// A name given in an anvil or by a command, as plain text.
    pub custom_name: Option<String>,
    pub count: i32,
    pub enchantments: Vec<Enchantment>,
}

#[derive(ToSchema, Serialize, Debug, Clone)]
pub struct PlayerData {
    pub game_mode: Option<GameMode>,
    pub health: f64,
    pub max_health: f64,
    pub food: i32,
    pub saturation: f64,
    pub xp_level: i32,
    /// How far to the next level, 0 to 1.
    pub xp_progress: f64,
    pub xp_total: i32,
    pub position: Option<Position>,
    /// The bed or respawn anchor, when one is set.
    pub respawn: Option<Position>,
    pub last_death: Option<Position>,
    pub inventory: Vec<PlayerItem>,
    pub ender_chest: Vec<PlayerItem>,
}

#[derive(ToSchema, Serialize, Debug, Clone, PartialEq)]
pub struct StatCount {
    pub id: compact_str::CompactString,
    /// From the id ("Oak Log", "Walk").
    pub label: String,
    pub value: u64,
}

#[derive(ToSchema, Serialize, Debug, Clone, Default, PartialEq)]
pub struct StatRanking {
    pub total: u64,
    /// The largest, at most five.
    pub top: Vec<StatCount>,
}

#[derive(ToSchema, Serialize, Debug, Clone, Default, PartialEq)]
pub struct PlayerStats {
    /// Seconds.
    pub play_time: u64,
    pub player_kills: u64,
    pub mob_kills: u64,
    pub deaths: u64,
    /// Player kills per death, two decimals; the kills while there was no death.
    pub kdr: f64,
    /// Blocks travelled, in total and by way of moving (walked, sprinted, by boat …), every one.
    pub distance: StatRanking,
    pub mined: StatRanking,
    pub used: StatRanking,
    pub killed: StatRanking,
}

/// Which of the player's files exist.
#[derive(ToSchema, Serialize, Debug, Clone, Default)]
pub struct PlayerFiles {
    pub player_data: bool,
    pub statistics: bool,
    pub advancements: bool,
}

#[derive(ToSchema, Serialize, Debug)]
pub struct PlayerDetails {
    pub uuid: compact_str::CompactString,
    pub name: Option<compact_str::CompactString>,
    /// A Bedrock player through Floodgate (a UUID with its upper half zero).
    pub bedrock: bool,
    pub whitelisted: bool,
    pub operator: Option<u8>,
    pub ban: Option<players::Ban>,
    pub online_mode: bool,
    pub running: bool,
    /// Whether the server is stopped, so files may be edited.
    pub stopped: bool,
    /// When the server last saved the player.
    pub saved: Option<chrono::DateTime<chrono::Utc>>,
    /// The Minecraft version, for the item icons.
    pub game_version: Option<compact_str::CompactString>,
    pub world: Option<compact_str::CompactString>,
    pub data: Option<PlayerData>,
    /// The data file is there but could not be read.
    pub damaged: bool,
    pub stats: Option<PlayerStats>,
    pub files: PlayerFiles,
    /// Whether deleted files go to the server's trash.
    pub trash: bool,
}

/// The world `server.properties` loads, `None` when its name is not a plain folder name.
pub fn world(properties: Option<&str>) -> Option<String> {
    let name = properties
        .and_then(|raw| config::property(raw, "level-name"))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "world".into());

    super::worlds::valid_name(&name).then_some(name)
}

/// A Floodgate (Bedrock) player: the upper half of the UUID is zero, the lower half the XUID.
pub fn is_bedrock(uuid: uuid::Uuid) -> bool {
    let (high, low) = uuid.as_u64_pair();
    high == 0 && low != 0
}

// ---------------------------------------------------------------------------------------------
// Reading NBT

fn compound<'a>(parent: &'a Compound, key: &str) -> Option<&'a Compound> {
    match parent.get(key) {
        Some(Value::Compound(compound)) => Some(compound),
        _ => None,
    }
}

fn list<'a>(parent: &'a Compound, key: &str) -> Option<&'a [Value]> {
    match parent.get(key) {
        Some(Value::List(list)) => Some(list),
        _ => None,
    }
}

fn number(parent: &Compound, key: &str) -> Option<f64> {
    parent.get(key).and_then(Value::as_f64)
}

fn string<'a>(parent: &'a Compound, key: &str) -> Option<&'a str> {
    parent.get(key).and_then(Value::as_str)
}

/// Three numbers: an int array (`[I; x, y, z]`) or a list.
fn triple(value: Option<&Value>) -> Option<[f64; 3]> {
    let numbers: Vec<f64> = match value? {
        Value::IntArray(array) => array.iter().map(|n| *n as f64).collect(),
        Value::List(list) => list.iter().filter_map(Value::as_f64).collect(),
        _ => return None,
    };

    match numbers.as_slice() {
        [x, y, z] if x.is_finite() && y.is_finite() && z.is_finite() => Some([*x, *y, *z]),
        _ => None,
    }
}

/// A dimension: a name since 1.16, before that 0, -1 or 1.
fn dimension(value: Option<&Value>) -> compact_str::CompactString {
    match value {
        Some(Value::String(name)) if !name.trim().is_empty() => {
            let name = name.trim();
            if name.contains(':') {
                name.into()
            } else {
                compact_str::format_compact!("minecraft:{name}")
            }
        }
        Some(value) => match value.as_i64() {
            Some(-1) => NETHER.into(),
            Some(1) => END.into(),
            _ => OVERWORLD.into(),
        },
        None => OVERWORLD.into(),
    }
}

fn position(at: [f64; 3], dimension_value: Option<&Value>) -> Position {
    Position {
        x: at[0],
        y: at[1],
        z: at[2],
        dimension: dimension(dimension_value),
    }
}

/// Text without Minecraft's `§` formatting codes.
fn strip_codes(text: &str) -> String {
    let mut clean = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            clean.push(c);
        }
    }

    clean
}

fn json_text(json: &serde_json::Value) -> String {
    match json {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(parts) => parts.iter().map(json_text).collect(),
        serde_json::Value::Object(object) => {
            let mut text = object
                .get("text")
                .and_then(|text| text.as_str())
                .or_else(|| object.get("translate").and_then(|key| key.as_str()))
                .unwrap_or_default()
                .to_string();
            if let Some(extra) = object.get("extra") {
                text.push_str(&json_text(extra));
            }
            text
        }
        _ => String::new(),
    }
}

/// A text component as plain text: JSON in a string until 1.21.4, NBT since 1.21.5, a plain
/// string with `§` codes before 1.13.
fn plain_text(value: &Value) -> Option<String> {
    let text = match value {
        Value::String(raw) => {
            let trimmed = raw.trim();
            match trimmed.chars().next() {
                Some('{' | '[' | '"') => serde_json::from_str::<serde_json::Value>(trimmed)
                    .map(|json| json_text(&json))
                    .unwrap_or_else(|_| raw.clone()),
                _ => raw.clone(),
            }
        }
        Value::Compound(compound) => {
            let mut text = string(compound, "text")
                .or_else(|| string(compound, ""))
                .or_else(|| string(compound, "translate"))
                .unwrap_or_default()
                .to_string();
            if let Some(extra) = compound.get("extra") {
                text.push_str(&plain_text(extra).unwrap_or_default());
            }
            text
        }
        Value::List(parts) => parts.iter().filter_map(plain_text).collect(),
        _ => return None,
    };

    let text = strip_codes(&text).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The words of an id, as Minecraft writes names: "heart_of_the_sea" is "Heart of the Sea".
fn words(id: &str) -> Vec<String> {
    let path = id.rsplit(':').next().unwrap_or(id);
    let path = path.rsplit('/').next().unwrap_or(path);

    path.split(['_', '.', ' '])
        .filter(|word| !word.is_empty())
        .enumerate()
        .map(|(index, word)| {
            if index > 0 && matches!(word, "of" | "the" | "and" | "on" | "a" | "in" | "with") {
                return word.to_string();
            }
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect()
}

/// An id's name: `minecraft:oak_log` is "Oak Log".
pub fn label(id: &str) -> String {
    words(id).join(" ")
}

fn roman(level: i64) -> String {
    const NUMERALS: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];

    match level {
        1..=10 => NUMERALS[level as usize - 1].to_string(),
        level => level.to_string(),
    }
}

/// "Sharpness V"; enchantments that have one level only show none, as in the game.
fn enchantment(id: &str, level: i64) -> Enchantment {
    let path = id.rsplit(':').next().unwrap_or(id);
    let name = match path {
        "binding_curse" => "Curse of Binding".to_string(),
        "vanishing_curse" => "Curse of Vanishing".to_string(),
        "sweeping" => "Sweeping Edge".to_string(),
        _ => label(id),
    };
    let single = matches!(
        path,
        "mending"
            | "silk_touch"
            | "infinity"
            | "flame"
            | "channeling"
            | "multishot"
            | "aqua_affinity"
            | "binding_curse"
            | "vanishing_curse"
    );

    Enchantment {
        id: id.into(),
        label: if single && level == 1 {
            name
        } else {
            format!("{name} {}", roman(level))
        },
    }
}

/// The enchantments of an item, from either format: a compound of `id: level` (inside `levels`
/// until 1.21.4) or, before 1.20.5, a list of `{id, lvl}`.
fn enchantments(value: Option<&Value>) -> Vec<Enchantment> {
    match value {
        Some(Value::Compound(map)) => {
            let levels = compound(map, "levels").unwrap_or(map);
            let mut found: Vec<(String, i64)> = levels
                .iter()
                .filter_map(|(id, level)| Some((id.clone(), level.as_i64()?)))
                .collect();
            found.sort();
            found
                .into_iter()
                .map(|(id, level)| enchantment(&id, level))
                .collect()
        }
        Some(Value::List(list)) => list
            .iter()
            .filter_map(|entry| match entry {
                Value::Compound(entry) => {
                    let level = number(entry, "lvl")? as i64;
                    let id = match entry.get("id")? {
                        Value::String(id) => id.clone(),
                        numeric => format!("enchantment #{}", numeric.as_i64()?),
                    };
                    Some(enchantment(&id, level))
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// One item stack, either format: 1.20.5's `{id, count, components}` or the older
/// `{id, StatCount, tag}` (and a number for an id before 1.8).
fn item(entry: &Compound, slot: Option<i32>) -> Option<PlayerItem> {
    let id: compact_str::CompactString = match entry.get("id")? {
        Value::String(id) if !id.trim().is_empty() => id.trim().into(),
        Value::String(_) => return None,
        numeric => compact_str::format_compact!("legacy:{}", numeric.as_i64()?),
    };
    if id == "minecraft:air" {
        return None;
    }

    let count = number(entry, "count")
        .or_else(|| number(entry, "Count"))
        .unwrap_or(1.0) as i32;
    if count <= 0 {
        return None;
    }

    let (custom_name, mut enchanted) = if let Some(components) = compound(entry, "components") {
        (
            components
                .get("minecraft:custom_name")
                .or_else(|| components.get("minecraft:item_name"))
                .and_then(plain_text),
            enchantments(components.get("minecraft:enchantments")),
        )
    } else if let Some(tag) = compound(entry, "tag") {
        (
            compound(tag, "display")
                .and_then(|display| display.get("Name"))
                .and_then(plain_text),
            enchantments(tag.get("Enchantments").or_else(|| tag.get("ench"))),
        )
    } else {
        (None, Vec::new())
    };

    // an enchanted book's enchantments are stored, not applied
    let stored = compound(entry, "components")
        .and_then(|components| components.get("minecraft:stored_enchantments"))
        .or_else(|| compound(entry, "tag").and_then(|tag| tag.get("StoredEnchantments")));
    enchanted.extend(enchantments(stored));

    Some(PlayerItem {
        slot: slot.or_else(|| number(entry, "Slot").map(|slot| slot as i32))?,
        label: if id.starts_with("legacy:") {
            format!("Item {}", &id["legacy:".len()..])
        } else {
            label(&id)
        },
        id,
        custom_name,
        count,
        enchantments: enchanted,
    })
}

fn items(entries: Option<&[Value]>) -> Vec<PlayerItem> {
    entries
        .unwrap_or_default()
        .iter()
        .filter_map(|entry| match entry {
            Value::Compound(entry) => item(entry, None),
            _ => None,
        })
        .collect()
}

/// The player's maximum health: the attribute's base with its modifiers, 20 without one.
fn max_health(root: &Compound) -> f64 {
    let attributes = list(root, "attributes")
        .or_else(|| list(root, "Attributes"))
        .unwrap_or_default();

    for attribute in attributes {
        let Value::Compound(attribute) = attribute else {
            continue;
        };
        let name = string(attribute, "id")
            .or_else(|| string(attribute, "Name"))
            .unwrap_or_default();
        if !matches!(
            name,
            "minecraft:max_health"
                | "minecraft:generic.max_health"
                | "generic.max_health"
                | "generic.maxHealth"
                | "max_health"
        ) {
            continue;
        }

        let base = number(attribute, "base")
            .or_else(|| number(attribute, "Base"))
            .unwrap_or(20.0);
        let (mut add, mut multiply_base, mut multiply_total) = (0.0, 0.0, 1.0);
        let modifiers = list(attribute, "modifiers")
            .or_else(|| list(attribute, "Modifiers"))
            .unwrap_or_default();
        for modifier in modifiers {
            let Value::Compound(modifier) = modifier else {
                continue;
            };
            let amount = number(modifier, "amount")
                .or_else(|| number(modifier, "Amount"))
                .unwrap_or_default();
            let operation = match modifier
                .get("operation")
                .or_else(|| modifier.get("Operation"))
            {
                Some(Value::String(operation)) => match operation.as_str() {
                    "add_multiplied_base" => 1,
                    "add_multiplied_total" => 2,
                    _ => 0,
                },
                Some(value) => value.as_i64().unwrap_or_default(),
                None => 0,
            };
            match operation {
                1 => multiply_base += amount,
                2 => multiply_total *= 1.0 + amount,
                _ => add += amount,
            }
        }

        let health = (base + add) * (1.0 + multiply_base) * multiply_total;
        return if health.is_finite() {
            health.max(1.0)
        } else {
            20.0
        };
    }

    20.0
}

/// What the Players page shows of a player data file.
pub fn player_data(root: &Compound) -> PlayerData {
    // before 1.9 `Health` was a short and `HealF` the real value
    let health = match root.get("Health") {
        Some(Value::Short(short)) => number(root, "HealF").unwrap_or(*short as f64),
        Some(value) => value.as_f64().unwrap_or(20.0),
        None => 20.0,
    };

    let mut inventory = items(list(root, "Inventory"));
    // 1.21.5 moved armour and the off hand out of the inventory
    if let Some(equipment) = compound(root, "equipment") {
        for (key, slot) in [
            ("feet", 100),
            ("legs", 101),
            ("chest", 102),
            ("head", 103),
            ("offhand", -106),
        ] {
            if let Some(Value::Compound(entry)) = equipment.get(key)
                && let Some(item) = item(entry, Some(slot))
            {
                inventory.retain(|existing| existing.slot != slot);
                inventory.push(item);
            }
        }
    }
    inventory.sort_by_key(|item| item.slot);

    let respawn = match compound(root, "respawn") {
        Some(respawn) => {
            triple(respawn.get("pos")).map(|at| position(at, respawn.get("dimension")))
        }
        None => match (
            number(root, "SpawnX"),
            number(root, "SpawnY"),
            number(root, "SpawnZ"),
        ) {
            (Some(x), Some(y), Some(z)) => Some(position([x, y, z], root.get("SpawnDimension"))),
            _ => None,
        },
    };

    PlayerData {
        game_mode: number(root, "playerGameType").and_then(|id| GameMode::from_id(id as i64)),
        health: health.max(0.0),
        max_health: max_health(root),
        food: number(root, "foodLevel").unwrap_or(20.0) as i32,
        saturation: number(root, "foodSaturationLevel").unwrap_or_default(),
        xp_level: number(root, "XpLevel").unwrap_or_default() as i32,
        xp_progress: number(root, "XpP").unwrap_or_default().clamp(0.0, 1.0),
        xp_total: number(root, "XpTotal").unwrap_or_default() as i32,
        position: triple(root.get("Pos")).map(|at| position(at, root.get("Dimension"))),
        respawn,
        last_death: compound(root, "LastDeathLocation").and_then(|death| {
            triple(death.get("pos")).map(|at| position(at, death.get("dimension")))
        }),
        inventory,
        ender_chest: {
            let mut items = items(list(root, "EnderItems"));
            items.sort_by_key(|item| item.slot);
            items
        },
    }
}

/// The world's spawn from `level.dat`: `Data.spawn` since 1.21.9, `Data.SpawnX/Y/Z` before.
pub fn world_spawn(level: &Compound) -> Option<Position> {
    let data = compound(level, "Data")?;

    if let Some(spawn) = compound(data, "spawn")
        && let Some(at) = triple(spawn.get("pos"))
    {
        return Some(position(
            [at[0] + 0.5, at[1], at[2] + 0.5],
            spawn.get("dimension"),
        ));
    }

    Some(Position {
        x: number(data, "SpawnX")? + 0.5,
        y: number(data, "SpawnY")?,
        z: number(data, "SpawnZ")? + 0.5,
        dimension: OVERWORLD.into(),
    })
}

// ---------------------------------------------------------------------------------------------
// Statistics

/// A number of a statistics file.
fn stat_value(value: &serde_json::Value) -> u64 {
    value
        .as_u64()
        .or_else(|| value.as_i64().map(|n| n.max(0) as u64))
        .or_else(|| value.as_f64().map(|n| n.max(0.0) as u64))
        .unwrap_or_default()
}

/// `walkOneCm` → `walk_one_cm`.
fn snake(camel: &str) -> String {
    let mut snake = String::with_capacity(camel.len() + 4);
    for c in camel.chars() {
        if c.is_ascii_uppercase() {
            if !snake.is_empty() {
                snake.push('_');
            }
            snake.push(c.to_ascii_lowercase());
        } else {
            snake.push(c);
        }
    }

    snake
}

fn ranking(entries: HashMap<String, u64>) -> StatRanking {
    let total = entries.values().sum();
    let mut top: Vec<(String, u64)> = entries.into_iter().filter(|(_, n)| *n > 0).collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top.truncate(TOP);

    StatRanking {
        total,
        top: top
            .into_iter()
            .map(|(id, value)| StatCount {
                label: label(&id),
                id: id.into(),
                value,
            })
            .collect(),
    }
}

/// The Players page's statistics from a statistics file: 1.13's `{"stats": {"minecraft:custom":
/// {...}, "minecraft:mined": {...}}}` or the older flat `{"stat.walkOneCm": n}`.
pub fn stats(json: &serde_json::Value) -> PlayerStats {
    let mut custom: HashMap<String, u64> = HashMap::new();
    let mut mined: HashMap<String, u64> = HashMap::new();
    let mut used: HashMap<String, u64> = HashMap::new();
    let mut killed: HashMap<String, u64> = HashMap::new();

    if let Some(categories) = json.get("stats").and_then(|stats| stats.as_object()) {
        for (category, entries) in categories {
            let target = match category.as_str() {
                "minecraft:custom" => &mut custom,
                "minecraft:mined" => &mut mined,
                "minecraft:used" => &mut used,
                "minecraft:killed" => &mut killed,
                _ => continue,
            };
            for (id, value) in entries.as_object().into_iter().flatten() {
                *target.entry(id.clone()).or_default() += stat_value(value);
            }
        }
    } else if let Some(flat) = json.as_object() {
        for (key, value) in flat {
            let value = stat_value(value);
            let Some(rest) = key.strip_prefix("stat.") else {
                continue;
            };
            // ids were written with dots: stat.mineBlock.minecraft.stone
            let id = |name: &str| match name.split_once('.') {
                Some((namespace, path)) => format!("{namespace}:{path}"),
                None => format!("minecraft:{}", name.to_ascii_lowercase()),
            };
            if let Some(block) = rest.strip_prefix("mineBlock.") {
                *mined.entry(id(block)).or_default() += value;
            } else if let Some(item) = rest.strip_prefix("useItem.") {
                *used.entry(id(item)).or_default() += value;
            } else if let Some(entity) = rest.strip_prefix("killEntity.") {
                *killed.entry(id(entity)).or_default() += value;
            } else if !rest.contains('.') {
                let name = match rest {
                    "playOneMinute" => "play_time".to_string(),
                    "diveOneCm" => "walk_under_water_one_cm".to_string(),
                    other => snake(other),
                };
                *custom.entry(format!("minecraft:{name}")).or_default() += value;
            }
        }
    }

    let get = |key: &str| custom.get(key).copied().unwrap_or_default();
    // ticks; called play_one_minute from 1.13 to 1.16
    let ticks = get("minecraft:play_time").max(get("minecraft:play_one_minute"));
    let player_kills = get("minecraft:player_kills");
    let deaths = get("minecraft:deaths");

    let distances: HashMap<String, u64> = custom
        .iter()
        .filter(|(id, _)| id.ends_with("_one_cm"))
        .map(|(id, cm)| (id.clone(), *cm))
        .collect();
    let total_cm: u64 = distances.values().sum();
    let mut kinds: Vec<(String, u64)> = distances
        .into_iter()
        .map(|(id, cm)| (id, cm.saturating_add(50) / 100))
        .filter(|(_, blocks)| *blocks > 0)
        .collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    PlayerStats {
        play_time: ticks / 20,
        player_kills,
        mob_kills: get("minecraft:mob_kills"),
        deaths,
        kdr: (player_kills as f64 / deaths.max(1) as f64 * 100.0).round() / 100.0,
        distance: StatRanking {
            total: total_cm.saturating_add(50) / 100,
            top: kinds
                .into_iter()
                .map(|(id, value)| StatCount {
                    label: label(id.trim_end_matches("_one_cm")),
                    id: id.into(),
                    value,
                })
                .collect(),
        },
        mined: ranking(mined),
        used: ranking(used),
        killed: ranking(killed),
    }
}

// ---------------------------------------------------------------------------------------------
// Editing NBT

/// What an offline edit changes in the player data file.
#[derive(Debug, Clone)]
pub enum Edit {
    GameMode(GameMode),
    Move(Position),
    ClearXp,
    ClearInventory,
    ClearEnderChest,
}

/// Sets a number, keeping the type the file has (a float stays a float).
fn set_number(parent: &mut Compound, key: &str, value: f64, default: fn(f64) -> Value) {
    let typed = match parent.get(key) {
        Some(Value::Byte(_)) => Value::Byte(value as i8),
        Some(Value::Short(_)) => Value::Short(value as i16),
        Some(Value::Int(_)) => Value::Int(value as i32),
        Some(Value::Long(_)) => Value::Long(value as i64),
        Some(Value::Float(_)) => Value::Float(value as f32),
        Some(Value::Double(_)) => Value::Double(value),
        _ => default(value),
    };
    parent.insert(key.to_string(), typed);
}

fn int(value: f64) -> Value {
    Value::Int(value as i32)
}

fn float(value: f64) -> Value {
    Value::Float(value as f32)
}

fn pos_list(at: &Position) -> Value {
    Value::List(vec![
        Value::Double(at.x),
        Value::Double(at.y),
        Value::Double(at.z),
    ])
}

/// Applies an edit to a player data file's root. Refuses a dimension the file's format cannot
/// hold (before 1.16 only the three vanilla ones, as numbers).
pub fn apply(root: &mut Compound, edit: &Edit) -> Result<(), String> {
    match edit {
        Edit::GameMode(mode) => {
            set_number(root, "playerGameType", mode.id() as f64, int);
        }
        Edit::Move(at) => {
            let dimension = match root.get("Dimension") {
                Some(Value::String(_)) | None => Value::String(at.dimension.to_string()),
                Some(_) => match at.dimension.as_str() {
                    OVERWORLD => Value::Int(0),
                    NETHER => Value::Int(-1),
                    END => Value::Int(1),
                    _ => return Err("this server's minecraft has no such dimension".into()),
                },
            };
            root.insert("Pos".into(), pos_list(at));
            root.insert("Dimension".into(), dimension);
            // no fall damage from where they were
            for key in ["FallDistance", "fall_distance"] {
                if root.contains_key(key) {
                    set_number(root, key, 0.0, float);
                }
            }
            // a ridden animal comes along, or the game would put them back on it where it was
            if let Some(Value::Compound(vehicle)) = root.get_mut("RootVehicle")
                && let Some(Value::Compound(entity)) = vehicle.get_mut("Entity")
            {
                entity.insert("Pos".into(), pos_list(at));
            }
        }
        Edit::ClearXp => {
            set_number(root, "XpLevel", 0.0, int);
            set_number(root, "XpP", 0.0, float);
            set_number(root, "XpTotal", 0.0, int);
            if root.contains_key("Score") {
                set_number(root, "Score", 0.0, int);
            }
        }
        Edit::ClearInventory => {
            root.insert("Inventory".into(), Value::List(Vec::new()));
            root.remove("equipment");
            root.remove("SelectedItem");
        }
        Edit::ClearEnderChest => {
            root.insert("EnderItems".into(), Value::List(Vec::new()));
        }
    }

    Ok(())
}

/// A player data file's root: gzip NBT, or already unpacked (Wings unpacks gzip files it hands
/// over).
pub fn decode(bytes: &[u8]) -> Result<Compound, anyhow::Error> {
    let unpacked;
    let raw = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .take(MAX_NBT)
            .read_to_end(&mut out)?;
        unpacked = out;
        &unpacked[..]
    } else {
        bytes
    };

    match fastnbt::from_bytes::<Value>(raw)? {
        Value::Compound(root) => Ok(root),
        _ => anyhow::bail!("the file does not hold a compound"),
    }
}

/// A player data file as Minecraft writes it: gzip NBT with an unnamed root compound.
pub fn encode(root: &Compound) -> Result<Vec<u8>, anyhow::Error> {
    gzip(&fastnbt::to_bytes(root)?)
}

fn gzip(raw: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    use std::io::Write;

    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(raw)?;

    Ok(encoder.finish()?)
}

// ---------------------------------------------------------------------------------------------
// Through Wings

fn uuid_name(uuid: uuid::Uuid) -> String {
    uuid.hyphenated().to_string()
}

fn dat_path(world: &str, uuid: uuid::Uuid) -> String {
    format!("{world}/{PLAYERDATA}/{}.dat", uuid_name(uuid))
}

/// Which of these files exist, by the paths asked for.
async fn existing(
    client: &wings_api::client::WingsClient,
    server: &Server,
    paths: Vec<String>,
) -> Result<HashMap<String, chrono::DateTime<chrono::Utc>>, PlayerError> {
    let entries = client
        .post_servers_server_files_stat(
            server.uuid,
            &wings_api::servers_server_files_stat::post::RequestBody {
                root: "/".into(),
                files: paths.into_iter().map(Into::into).collect(),
            },
        )
        .await
        .map_err(|err| PlayerError::Wings(err.into()))?
        .entries;

    Ok(entries
        .into_iter()
        .filter(|entry| entry.file)
        .map(|entry| {
            (
                entry.name.trim_start_matches('/').to_string(),
                entry.modified.with_timezone(&chrono::Utc),
            )
        })
        .collect())
}

async fn power(
    client: &wings_api::client::WingsClient,
    server: &Server,
) -> Option<wings_api::ServerState> {
    files::power_state(client, server).await
}

/// One player's page.
pub async fn details(
    state: &crate::State,
    server: &Server,
    uuid: uuid::Uuid,
) -> Result<PlayerDetails, PlayerError> {
    let client = files::client(state, server)
        .await
        .map_err(PlayerError::Wings)?;
    let power = power(&client, server).await;
    let running = matches!(power, Some(wings_api::ServerState::Running));
    let stopped = matches!(power, Some(wings_api::ServerState::Offline));

    let lists = players::Files::read(&client, server)
        .await
        .map_err(PlayerError::Wings)?;
    let id = uuid_name(uuid);
    let listed = lists
        .roster(running)
        .players
        .into_iter()
        .find(|player| player.uuid.eq_ignore_ascii_case(&id));
    let cached = files::read_text(&client, server, players::USERCACHE, players::MAX_SIZE)
        .await
        .ok()
        .flatten();
    let cached_name = players::usercache(cached.as_deref())
        .into_iter()
        .find(|(cached, _)| cached.eq_ignore_ascii_case(&id))
        .map(|(_, name)| name)
        .filter(|name| !name.is_empty());

    let world = world(lists.properties.as_deref());
    let mut files_found = PlayerFiles::default();
    let (mut data, mut damaged, mut saved, mut stats) = (None, false, None, None);

    if let Some(world) = &world {
        let dat = dat_path(world, uuid);
        let stats_path = format!("{world}/{STATS}/{id}.json");
        let advancements = format!("{world}/{ADVANCEMENTS}/{id}.json");
        let found = existing(
            &client,
            server,
            vec![dat.clone(), stats_path.clone(), advancements.clone()],
        )
        .await?;

        files_found = PlayerFiles {
            player_data: found.contains_key(&dat),
            statistics: found.contains_key(&stats_path),
            advancements: found.contains_key(&advancements),
        };
        saved = found.get(&dat).copied();

        if files_found.player_data {
            match files::read_bytes(&client, server, &dat, MAX_DAT).await {
                Ok(Some(bytes)) => match decode(&bytes) {
                    Ok(root) => data = Some(player_data(&root)),
                    Err(err) => {
                        tracing::warn!(server = %server.uuid, "reading {dat}: {err:#}");
                        damaged = true;
                    }
                },
                Ok(None) => files_found.player_data = false,
                Err(err) => return Err(PlayerError::Wings(err)),
            }
        }

        if files_found.statistics
            && let Some(raw) = files::read_text(&client, server, &stats_path, MAX_JSON)
                .await
                .map_err(PlayerError::Wings)?
        {
            stats = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .map(|json| self::stats(&json));
        }
    }

    if listed.is_none() && cached_name.is_none() && !files_found.player_data && !damaged {
        return Err(PlayerError::NotFound);
    }

    let game_version = match super::software(state, server).await {
        Ok(software) => software.game_version(state).await,
        Err(_) => None,
    };

    Ok(PlayerDetails {
        uuid: id.into(),
        name: listed
            .as_ref()
            .map(|player| player.name.clone())
            .filter(|name| !name.is_empty())
            .or(cached_name),
        bedrock: is_bedrock(uuid),
        whitelisted: listed.as_ref().is_some_and(|player| player.whitelisted),
        operator: listed.as_ref().and_then(|player| player.operator),
        ban: listed.and_then(|player| player.ban),
        online_mode: lists.flag("online-mode", true),
        running,
        stopped,
        saved,
        game_version,
        world: world.map(Into::into),
        data,
        damaged,
        stats,
        files: files_found,
        trash: files::trash_enabled(state, server).await.unwrap_or(false),
    })
}

/// The player's name as the server last saw it (`usercache.json`), for the activity log.
pub async fn name_of(
    state: &crate::State,
    server: &Server,
    uuid: uuid::Uuid,
) -> Option<compact_str::CompactString> {
    let client = files::client(state, server).await.ok()?;
    let cached = files::read_text(&client, server, players::USERCACHE, players::MAX_SIZE)
        .await
        .ok()
        .flatten();
    let id = uuid_name(uuid);

    players::usercache(cached.as_deref())
        .into_iter()
        .find(|(cached, _)| cached.eq_ignore_ascii_case(&id))
        .map(|(_, name)| name)
        .filter(|name| !name.is_empty())
}

/// What the caller may do, by the server's state: the console while it runs, the files while
/// it is stopped.
#[derive(Debug, Clone, Copy)]
pub struct Access {
    pub console: bool,
    pub edit: bool,
    pub delete: bool,
}

/// The console's quick actions on an online player.
#[derive(ToSchema, Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlayerAction {
    Heal,
    Feed,
    Kill,
}

impl PlayerAction {
    /// The command, aimed at the player's UUID (any name works, Floodgate's too). Instant
    /// health and saturation at amplifier 20 fill any health and hunger bar at once.
    fn command(self, target: &str) -> String {
        match self {
            Self::Heal => format!("effect give {target} minecraft:instant_health 1 20 true"),
            Self::Feed => format!("effect give {target} minecraft:saturation 1 20 true"),
            Self::Kill => format!("kill {target}"),
        }
    }

    pub fn event(self) -> &'static str {
        match self {
            Self::Heal => "server:minecraft.players.heal",
            Self::Feed => "server:minecraft.players.feed",
            Self::Kill => "server:minecraft.players.kill",
        }
    }
}

/// Sends a command for an online player: refused on a server that is not running, and when the
/// console says the player is not online.
async fn live(
    state: &crate::State,
    client: &wings_api::client::WingsClient,
    server: &Server,
    uuid: uuid::Uuid,
    command: &str,
) -> Result<(), PlayerError> {
    let online = players::online(state, server, true).await?;
    if !online.running {
        return Err(PlayerError::NotRunning);
    }
    if online.has(&uuid_name(uuid)) == Some(false) {
        return Err(PlayerError::NotOnline);
    }

    files::command(client, server, command)
        .await
        .map_err(PlayerError::Wings)
}

pub async fn act(
    state: &crate::State,
    server: &Server,
    uuid: uuid::Uuid,
    action: PlayerAction,
) -> Result<(), PlayerError> {
    let client = files::client(state, server)
        .await
        .map_err(PlayerError::Wings)?;
    if !matches!(
        power(&client, server).await,
        Some(wings_api::ServerState::Running)
    ) {
        return Err(PlayerError::NotRunning);
    }

    live(
        state,
        &client,
        server,
        uuid,
        &action.command(&uuid_name(uuid)),
    )
    .await
}

/// Reads, edits and writes back the player data file of a stopped server, keeping the version
/// before as `<uuid>.dat_old`.
async fn edit_file(
    client: &wings_api::client::WingsClient,
    server: &Server,
    world: &str,
    uuid: uuid::Uuid,
    edits: &[Edit],
) -> Result<(), PlayerError> {
    let path = dat_path(world, uuid);
    let bytes = files::read_bytes(client, server, &path, MAX_DAT)
        .await
        .map_err(PlayerError::Wings)?
        .ok_or(PlayerError::NoData)?;
    let mut root = decode(&bytes).map_err(PlayerError::Damaged)?;
    // the file as it was; Wings may have handed it over unpacked
    let original = if bytes.starts_with(&[0x1f, 0x8b]) {
        bytes
    } else {
        gzip(&bytes).map_err(PlayerError::Damaged)?
    };

    for edit in edits {
        apply(&mut root, edit).map_err(PlayerError::Invalid)?;
    }
    let written = encode(&root).map_err(PlayerError::Damaged)?;

    files::write_bytes(client, server, &format!("{path}_old"), original)
        .await
        .map_err(PlayerError::Wings)?;
    files::write_bytes(client, server, &path, written)
        .await
        .map_err(PlayerError::Wings)
}

/// The world of a server whose files may be edited: stopped, with a plain world name.
async fn stopped_world(
    client: &wings_api::client::WingsClient,
    server: &Server,
) -> Result<String, PlayerError> {
    if !matches!(
        power(client, server).await,
        Some(wings_api::ServerState::Offline)
    ) {
        return Err(PlayerError::Running);
    }

    let properties = files::read_text(client, server, super::PROPERTIES_FILE, players::MAX_SIZE)
        .await
        .map_err(PlayerError::Wings)?;
    world(properties.as_deref()).ok_or_else(|| {
        PlayerError::Invalid(
            "the world's name in server.properties is not a plain folder name".into(),
        )
    })
}

pub async fn set_game_mode(
    state: &crate::State,
    server: &Server,
    uuid: uuid::Uuid,
    mode: GameMode,
    access: Access,
) -> Result<Outcome, PlayerError> {
    let client = files::client(state, server)
        .await
        .map_err(PlayerError::Wings)?;

    if matches!(
        power(&client, server).await,
        Some(wings_api::ServerState::Running)
    ) {
        if !access.console {
            return Err(PlayerError::Forbidden("control.console"));
        }
        let command = format!("gamemode {} {}", mode.as_str(), uuid_name(uuid));
        live(state, &client, server, uuid, &command).await?;

        return Ok(Outcome { live: true });
    }

    if !access.edit {
        return Err(PlayerError::Forbidden("files.update"));
    }
    let world = stopped_world(&client, server).await?;
    edit_file(&client, server, &world, uuid, &[Edit::GameMode(mode)]).await?;

    Ok(Outcome { live: false })
}

/// Where a teleport goes.
#[derive(ToSchema, Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Destination {
    /// A place; without a dimension, the one the player is in.
    Coordinates {
        x: f64,
        y: f64,
        z: f64,
        dimension: Option<compact_str::CompactString>,
    },
    /// The world's spawn.
    Spawn,
    /// Another player: online on a running server, their saved place on a stopped one.
    Player { uuid: uuid::Uuid },
}

/// A dimension's id: `namespace:path` in lowercase.
fn valid_dimension(id: &str) -> bool {
    id.len() <= 128
        && id.split_once(':').is_some_and(|(namespace, path)| {
            !namespace.is_empty()
                && !path.is_empty()
                && namespace
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
                && path
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-/".contains(&b))
        })
}

impl Destination {
    /// Refuses coordinates outside the world border and dimension names that are not ids.
    pub fn validate(&self) -> Result<(), PlayerError> {
        if let Self::Coordinates { x, y, z, dimension } = self {
            if [x, y, z]
                .iter()
                .any(|n| !n.is_finite() || n.abs() > WORLD_LIMIT)
            {
                return Err(PlayerError::Invalid(
                    "coordinates must be numbers within the world border (30 million)".into(),
                ));
            }
            if dimension.as_deref().is_some_and(|id| !valid_dimension(id)) {
                return Err(PlayerError::Invalid(
                    "a dimension looks like minecraft:the_nether".into(),
                ));
            }
        }

        Ok(())
    }
}

/// A coordinate for a command: plain decimals, never an exponent.
fn coordinate(n: f64) -> String {
    let text = format!("{:.3}", n);
    let text = text.trim_end_matches('0').trim_end_matches('.');

    if matches!(text, "" | "-" | "-0") {
        "0".into()
    } else {
        text.to_string()
    }
}

async fn read_root(
    client: &wings_api::client::WingsClient,
    server: &Server,
    path: &str,
) -> Result<Option<Compound>, PlayerError> {
    match files::read_bytes(client, server, path, MAX_DAT)
        .await
        .map_err(PlayerError::Wings)?
    {
        Some(bytes) => decode(&bytes).map(Some).map_err(PlayerError::Damaged),
        None => Ok(None),
    }
}

async fn spawn_of(
    client: &wings_api::client::WingsClient,
    server: &Server,
    world: &str,
) -> Result<Position, PlayerError> {
    read_root(client, server, &format!("{world}/level.dat"))
        .await?
        .as_ref()
        .and_then(world_spawn)
        .ok_or_else(|| PlayerError::Invalid("the world has no spawn yet".into()))
}

pub async fn teleport(
    state: &crate::State,
    server: &Server,
    uuid: uuid::Uuid,
    destination: &Destination,
    access: Access,
) -> Result<Outcome, PlayerError> {
    destination.validate()?;
    if matches!(destination, Destination::Player { uuid: target } if *target == uuid) {
        return Err(PlayerError::Invalid(
            "pick another player to teleport to".into(),
        ));
    }

    let client = files::client(state, server)
        .await
        .map_err(PlayerError::Wings)?;
    let target = uuid_name(uuid);

    if matches!(
        power(&client, server).await,
        Some(wings_api::ServerState::Running)
    ) {
        if !access.console {
            return Err(PlayerError::Forbidden("control.console"));
        }

        let command = match destination {
            Destination::Coordinates {
                x,
                y,
                z,
                dimension: Some(dimension),
            } => format!(
                "execute in {dimension} run tp {target} {} {} {}",
                coordinate(*x),
                coordinate(*y),
                coordinate(*z)
            ),
            Destination::Coordinates { x, y, z, .. } => format!(
                "tp {target} {} {} {}",
                coordinate(*x),
                coordinate(*y),
                coordinate(*z)
            ),
            Destination::Spawn => {
                let properties =
                    files::read_text(&client, server, super::PROPERTIES_FILE, players::MAX_SIZE)
                        .await
                        .map_err(PlayerError::Wings)?;
                let world = world(properties.as_deref()).ok_or_else(|| {
                    PlayerError::Invalid(
                        "the world's name in server.properties is not a plain folder name".into(),
                    )
                })?;
                let spawn = spawn_of(&client, server, &world).await?;
                format!(
                    "execute in {} run tp {target} {} {} {}",
                    spawn.dimension,
                    coordinate(spawn.x),
                    coordinate(spawn.y),
                    coordinate(spawn.z)
                )
            }
            Destination::Player { uuid: other } => {
                let online = players::online(state, server, true).await?;
                if online.has(&uuid_name(*other)) == Some(false) {
                    return Err(PlayerError::Invalid(
                        "the player to teleport to is not online".into(),
                    ));
                }
                format!("tp {target} {}", uuid_name(*other))
            }
        };
        live(state, &client, server, uuid, &command).await?;

        return Ok(Outcome { live: true });
    }

    if !access.edit {
        return Err(PlayerError::Forbidden("files.update"));
    }
    let world = stopped_world(&client, server).await?;

    let at = match destination {
        Destination::Coordinates { x, y, z, dimension } => {
            let dimension = match dimension {
                Some(dimension) => dimension.clone(),
                None => read_root(&client, server, &dat_path(&world, uuid))
                    .await?
                    .ok_or(PlayerError::NoData)?
                    .get("Dimension")
                    .map(|value| self::dimension(Some(value)))
                    .unwrap_or_else(|| OVERWORLD.into()),
            };
            Position {
                x: *x,
                y: *y,
                z: *z,
                dimension,
            }
        }
        Destination::Spawn => spawn_of(&client, server, &world).await?,
        Destination::Player { uuid: other } => {
            let other = read_root(&client, server, &dat_path(&world, *other))
                .await?
                .ok_or_else(|| {
                    PlayerError::Invalid("the player to teleport to has no saved place".into())
                })?;
            player_data(&other).position.ok_or_else(|| {
                PlayerError::Invalid("the player to teleport to has no saved place".into())
            })?
        }
    };
    edit_file(&client, server, &world, uuid, &[Edit::Move(at)]).await?;

    Ok(Outcome { live: false })
}

/// What "Delete player data" removes.
#[derive(ToSchema, Deserialize, Serialize, Debug, Clone, Copy, Default)]
pub struct Removal {
    /// Experience points and levels.
    #[serde(default)]
    pub xp: bool,
    /// The inventory, armour and off hand.
    #[serde(default)]
    pub inventory: bool,
    #[serde(default)]
    pub ender_chest: bool,
    /// The whole player data file (the player starts over at the spawn).
    #[serde(default)]
    pub player_data: bool,
    #[serde(default)]
    pub statistics: bool,
    #[serde(default)]
    pub advancements: bool,
}

impl Removal {
    fn edits(&self) -> Vec<Edit> {
        if self.player_data {
            // the file goes, and everything in it
            return Vec::new();
        }

        let mut edits = Vec::new();
        if self.xp {
            edits.push(Edit::ClearXp);
        }
        if self.inventory {
            edits.push(Edit::ClearInventory);
        }
        if self.ender_chest {
            edits.push(Edit::ClearEnderChest);
        }
        edits
    }

    /// For the activity log.
    pub fn names(&self) -> Vec<&'static str> {
        [
            (self.xp, "xp"),
            (self.inventory, "inventory"),
            (self.ender_chest, "ender_chest"),
            (self.player_data, "player_data"),
            (self.statistics, "statistics"),
            (self.advancements, "advancements"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect()
    }
}

/// Deletes parts of a player's data on a stopped server: the edits in the data file, then the
/// files (into the trash when the server has one). A data file goes together with its
/// `.dat_old`, which the game would otherwise load in its place.
pub async fn remove(
    state: &crate::State,
    server: &Server,
    uuid: uuid::Uuid,
    removal: Removal,
    access: Access,
) -> Result<(), PlayerError> {
    let edits = removal.edits();
    if edits.is_empty() && !removal.player_data && !removal.statistics && !removal.advancements {
        return Err(PlayerError::Invalid("choose what to delete".into()));
    }
    if !edits.is_empty() && !access.edit {
        return Err(PlayerError::Forbidden("files.update"));
    }
    if (removal.player_data || removal.statistics || removal.advancements) && !access.delete {
        return Err(PlayerError::Forbidden("files.delete"));
    }

    let client = files::client(state, server)
        .await
        .map_err(PlayerError::Wings)?;
    let world = stopped_world(&client, server).await?;
    let id = uuid_name(uuid);

    if !edits.is_empty() {
        edit_file(&client, server, &world, uuid, &edits).await?;
    }

    let mut doomed: Vec<(String, String)> = Vec::new();
    if removal.player_data {
        doomed.push((format!("{world}/{PLAYERDATA}"), format!("{id}.dat")));
        doomed.push((format!("{world}/{PLAYERDATA}"), format!("{id}.dat_old")));
    }
    if removal.statistics {
        doomed.push((format!("{world}/{STATS}"), format!("{id}.json")));
    }
    if removal.advancements {
        doomed.push((format!("{world}/{ADVANCEMENTS}"), format!("{id}.json")));
    }

    let found = existing(
        &client,
        server,
        doomed
            .iter()
            .map(|(folder, name)| format!("{folder}/{name}"))
            .collect(),
    )
    .await?;

    let mut by_folder: Vec<(String, Vec<compact_str::CompactString>)> = Vec::new();
    for (folder, name) in doomed {
        if !found.contains_key(&format!("{folder}/{name}")) {
            continue;
        }
        match by_folder
            .iter_mut()
            .find(|(existing, _)| *existing == folder)
        {
            Some((_, names)) => names.push(name.into()),
            None => by_folder.push((folder, vec![name.into()])),
        }
    }

    for (folder, names) in by_folder {
        files::remove(state, &client, server, &format!("/{folder}"), names)
            .await
            .map_err(PlayerError::Wings)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compound(entries: Vec<(&str, Value)>) -> Value {
        Value::Compound(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }

    fn root(value: Value) -> Compound {
        match value {
            Value::Compound(root) => root,
            _ => unreachable!(),
        }
    }

    /// A 1.21.5+ player: `count`, components, armour in `equipment`.
    fn modern() -> Compound {
        root(compound(vec![
            ("DataVersion", Value::Int(4325)),
            ("Health", Value::Float(13.0)),
            ("foodLevel", Value::Int(17)),
            ("foodSaturationLevel", Value::Float(2.5)),
            ("XpLevel", Value::Int(12)),
            ("XpP", Value::Float(0.25)),
            ("XpTotal", Value::Int(250)),
            ("playerGameType", Value::Int(1)),
            ("Dimension", Value::String("minecraft:the_nether".into())),
            (
                "Pos",
                Value::List(vec![
                    Value::Double(-951.34),
                    Value::Double(76.0),
                    Value::Double(-410.03),
                ]),
            ),
            ("FallDistance", Value::Float(12.0)),
            (
                "attributes",
                Value::List(vec![compound(vec![
                    ("id", Value::String("minecraft:max_health".into())),
                    ("base", Value::Double(20.0)),
                    (
                        "modifiers",
                        Value::List(vec![compound(vec![
                            ("id", Value::String("minecraft:bonus".into())),
                            ("amount", Value::Double(4.0)),
                            ("operation", Value::String("add_value".into())),
                        ])]),
                    ),
                ])]),
            ),
            (
                "Inventory",
                Value::List(vec![
                    compound(vec![
                        ("Slot", Value::Byte(0)),
                        ("id", Value::String("minecraft:diamond_sword".into())),
                        ("count", Value::Int(1)),
                        (
                            "components",
                            compound(vec![
                                (
                                    "minecraft:enchantments",
                                    compound(vec![
                                        ("minecraft:sharpness", Value::Int(5)),
                                        ("minecraft:mending", Value::Int(1)),
                                    ]),
                                ),
                                (
                                    "minecraft:custom_name",
                                    compound(vec![("text", Value::String("Excalibur".into()))]),
                                ),
                            ]),
                        ),
                    ]),
                    compound(vec![
                        ("Slot", Value::Byte(9)),
                        ("id", Value::String("minecraft:oak_log".into())),
                        ("count", Value::Int(64)),
                    ]),
                ]),
            ),
            (
                "equipment",
                compound(vec![
                    (
                        "head",
                        compound(vec![
                            ("id", Value::String("minecraft:turtle_helmet".into())),
                            ("count", Value::Int(1)),
                        ]),
                    ),
                    (
                        "offhand",
                        compound(vec![
                            ("id", Value::String("minecraft:shield".into())),
                            ("count", Value::Int(1)),
                        ]),
                    ),
                ]),
            ),
            (
                "EnderItems",
                Value::List(vec![compound(vec![
                    ("Slot", Value::Byte(26)),
                    ("id", Value::String("minecraft:enchanted_book".into())),
                    ("count", Value::Int(1)),
                    (
                        "components",
                        compound(vec![(
                            "minecraft:stored_enchantments",
                            compound(vec![("minecraft:binding_curse", Value::Int(1))]),
                        )]),
                    ),
                ])]),
            ),
            (
                "respawn",
                compound(vec![
                    (
                        "pos",
                        Value::IntArray(fastnbt::IntArray::new(vec![10, 64, -20])),
                    ),
                    ("dimension", Value::String("minecraft:overworld".into())),
                ]),
            ),
            (
                "LastDeathLocation",
                compound(vec![
                    (
                        "pos",
                        Value::IntArray(fastnbt::IntArray::new(vec![1, 2, 3])),
                    ),
                    ("dimension", Value::String("minecraft:the_end".into())),
                ]),
            ),
        ]))
    }

    #[test]
    fn a_modern_player_is_read() {
        let data = player_data(&modern());

        assert_eq!(data.game_mode, Some(GameMode::Creative));
        assert_eq!(data.health, 13.0);
        assert_eq!(data.max_health, 24.0);
        assert_eq!(data.food, 17);
        assert_eq!(data.xp_level, 12);
        assert_eq!(data.xp_progress, 0.25);
        let position = data.position.unwrap();
        assert_eq!(
            (position.x, position.y, position.z),
            (-951.34, 76.0, -410.03)
        );
        assert_eq!(position.dimension, NETHER);
        assert_eq!(data.respawn.unwrap().x, 10.0);
        assert_eq!(data.last_death.unwrap().dimension, END);

        let slots: Vec<(i32, &str, i32)> = data
            .inventory
            .iter()
            .map(|item| (item.slot, item.id.as_str(), item.count))
            .collect();
        assert_eq!(
            slots,
            vec![
                (-106, "minecraft:shield", 1),
                (0, "minecraft:diamond_sword", 1),
                (9, "minecraft:oak_log", 64),
                (103, "minecraft:turtle_helmet", 1),
            ]
        );
        let sword = &data.inventory[1];
        assert_eq!(sword.label, "Diamond Sword");
        assert_eq!(sword.custom_name.as_deref(), Some("Excalibur"));
        let enchantments: Vec<&str> = sword
            .enchantments
            .iter()
            .map(|enchantment| enchantment.label.as_str())
            .collect();
        assert_eq!(enchantments, vec!["Mending", "Sharpness V"]);

        assert_eq!(data.ender_chest[0].slot, 26);
        assert_eq!(
            data.ender_chest[0].enchantments[0].label,
            "Curse of Binding"
        );
    }

    #[test]
    fn an_old_player_is_read() {
        // 1.12: `StatCount` and `tag`, JSON-less names with colour codes, a number for the dimension
        let old = root(compound(vec![
            ("Health", Value::Float(20.0)),
            ("Dimension", Value::Int(-1)),
            (
                "Pos",
                Value::List(vec![
                    Value::Double(1.5),
                    Value::Double(64.0),
                    Value::Double(2.5),
                ]),
            ),
            (
                "Attributes",
                Value::List(vec![compound(vec![
                    ("Name", Value::String("generic.maxHealth".into())),
                    ("Base", Value::Double(30.0)),
                ])]),
            ),
            (
                "Inventory",
                Value::List(vec![
                    compound(vec![
                        ("Slot", Value::Byte(100)),
                        ("id", Value::String("minecraft:iron_boots".into())),
                        ("Count", Value::Byte(1)),
                        (
                            "tag",
                            compound(vec![
                                (
                                    "display",
                                    compound(vec![("Name", Value::String("§6Swift".into()))]),
                                ),
                                (
                                    "Enchantments",
                                    Value::List(vec![compound(vec![
                                        ("id", Value::String("minecraft:feather_falling".into())),
                                        ("lvl", Value::Short(4)),
                                    ])]),
                                ),
                            ]),
                        ),
                    ]),
                    compound(vec![
                        ("Slot", Value::Byte(-106)),
                        ("id", Value::String("minecraft:torch".into())),
                        ("Count", Value::Byte(32)),
                    ]),
                    // 1.20.4: a JSON name
                    compound(vec![
                        ("Slot", Value::Byte(1)),
                        ("id", Value::String("minecraft:stone".into())),
                        ("Count", Value::Byte(3)),
                        (
                            "tag",
                            compound(vec![(
                                "display",
                                compound(vec![(
                                    "Name",
                                    Value::String(
                                        r#"{"text":"Rock","extra":[{"text":"y"}]}"#.into(),
                                    ),
                                )]),
                            )]),
                        ),
                    ]),
                ]),
            ),
        ]));
        let data = player_data(&old);

        assert_eq!(data.max_health, 30.0);
        assert_eq!(data.position.unwrap().dimension, NETHER);
        assert_eq!(data.game_mode, None);
        let boots = data.inventory.iter().find(|item| item.slot == 100).unwrap();
        assert_eq!(boots.custom_name.as_deref(), Some("Swift"));
        assert_eq!(boots.enchantments[0].label, "Feather Falling IV");
        assert_eq!(
            data.inventory
                .iter()
                .find(|item| item.slot == -106)
                .unwrap()
                .count,
            32
        );
        assert_eq!(
            data.inventory
                .iter()
                .find(|item| item.slot == 1)
                .unwrap()
                .custom_name
                .as_deref(),
            Some("Rocky")
        );
    }

    #[test]
    fn edits_keep_the_types_and_round_trip() {
        let mut data = modern();
        apply(&mut data, &Edit::GameMode(GameMode::Survival)).unwrap();
        apply(
            &mut data,
            &Edit::Move(Position {
                x: 0.5,
                y: 70.0,
                z: 0.5,
                dimension: OVERWORLD.into(),
            }),
        )
        .unwrap();
        apply(&mut data, &Edit::ClearXp).unwrap();
        apply(&mut data, &Edit::ClearInventory).unwrap();
        apply(&mut data, &Edit::ClearEnderChest).unwrap();

        let read = decode(&encode(&data).unwrap()).unwrap();
        assert_eq!(read.get("playerGameType"), Some(&Value::Int(0)));
        assert_eq!(read.get("XpP"), Some(&Value::Float(0.0)));
        assert_eq!(read.get("FallDistance"), Some(&Value::Float(0.0)));
        assert_eq!(
            read.get("Dimension"),
            Some(&Value::String(OVERWORLD.into()))
        );
        assert_eq!(read.get("Inventory"), Some(&Value::List(Vec::new())));
        assert!(!read.contains_key("equipment"));
        // untouched values keep their types
        assert_eq!(read.get("DataVersion"), Some(&Value::Int(4325)));
        assert_eq!(read, data);

        let after = player_data(&read);
        assert_eq!(after.xp_level, 0);
        assert!(after.inventory.is_empty() && after.ender_chest.is_empty());

        // a server before 1.16 keeps its number, and has no other dimensions
        let mut old = root(compound(vec![("Dimension", Value::Int(0))]));
        let nether = Position {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            dimension: NETHER.into(),
        };
        apply(&mut old, &Edit::Move(nether.clone())).unwrap();
        assert_eq!(old.get("Dimension"), Some(&Value::Int(-1)));
        let custom = Position {
            dimension: "mypack:moon".into(),
            ..nether
        };
        assert!(apply(&mut old, &Edit::Move(custom)).is_err());
    }

    #[test]
    fn unpacked_files_are_read_too() {
        let data = modern();
        let raw = fastnbt::to_bytes(&data).unwrap();
        assert_eq!(decode(&raw).unwrap(), data);
        assert!(decode(b"not nbt").is_err());
    }

    #[test]
    fn the_world_spawn_is_found() {
        let new = root(compound(vec![(
            "Data",
            compound(vec![(
                "spawn",
                compound(vec![
                    (
                        "pos",
                        Value::IntArray(fastnbt::IntArray::new(vec![8, 70, -8])),
                    ),
                    ("dimension", Value::String(OVERWORLD.into())),
                ]),
            )]),
        )]));
        let spawn = world_spawn(&new).unwrap();
        assert_eq!((spawn.x, spawn.y, spawn.z), (8.5, 70.0, -7.5));

        let old = root(compound(vec![(
            "Data",
            compound(vec![
                ("SpawnX", Value::Int(0)),
                ("SpawnY", Value::Int(64)),
                ("SpawnZ", Value::Int(0)),
            ]),
        )]));
        assert_eq!(world_spawn(&old).unwrap().y, 64.0);
        assert!(world_spawn(&root(compound(vec![]))).is_none());
    }

    #[test]
    fn statistics_add_up() {
        let json = serde_json::json!({
            "stats": {
                "minecraft:custom": {
                    "minecraft:play_time": 72_000,
                    "minecraft:player_kills": 3,
                    "minecraft:deaths": 2,
                    "minecraft:mob_kills": 40,
                    "minecraft:walk_one_cm": 150_000,
                    "minecraft:sprint_one_cm": 40_049,
                    "minecraft:fall_one_cm": 30,
                    "minecraft:jump": 99
                },
                "minecraft:mined": {
                    "minecraft:stone": 120, "minecraft:dirt": 30, "minecraft:oak_log": 30,
                    "minecraft:a": 1, "minecraft:b": 1, "minecraft:c": 1
                },
                "minecraft:killed": { "minecraft:zombie": 7 }
            },
            "DataVersion": 3955
        });
        let stats = stats(&json);

        assert_eq!(stats.play_time, 3600);
        assert_eq!(stats.kdr, 1.5);
        assert_eq!(stats.mob_kills, 40);
        // 1900.79 blocks in all; 30 cm of falling rounds to nothing and is left out
        assert_eq!(stats.distance.total, 1901);
        let kinds: Vec<(&str, &str, u64)> = stats
            .distance
            .top
            .iter()
            .map(|count| (count.id.as_str(), count.label.as_str(), count.value))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("minecraft:walk_one_cm", "Walk", 1500),
                ("minecraft:sprint_one_cm", "Sprint", 400),
            ]
        );
        assert_eq!(stats.mined.total, 183);
        assert_eq!(stats.mined.top.len(), 5);
        assert_eq!(stats.mined.top[0].label, "Stone");
        // equal counts by id
        assert_eq!(stats.mined.top[1].id, "minecraft:dirt");
        assert_eq!(stats.killed.top[0].value, 7);
        assert_eq!(stats.used, StatRanking::default());

        // no deaths: the kills
        let none = stats_of_kills(4, 0);
        assert_eq!(none.kdr, 4.0);
        assert_eq!(stats_of_kills(1, 3).kdr, 0.33);
    }

    fn stats_of_kills(kills: u64, deaths: u64) -> PlayerStats {
        stats(&serde_json::json!({"stats": {"minecraft:custom": {
            "minecraft:player_kills": kills, "minecraft:deaths": deaths
        }}}))
    }

    #[test]
    fn old_statistics_are_read() {
        let json = serde_json::json!({
            "stat.playOneMinute": 24_000,
            "stat.deaths": 1,
            "stat.walkOneCm": 1_000,
            "stat.mineBlock.minecraft.stone": 5,
            "stat.killEntity.Zombie": 2,
            "achievement.openInventory": 1
        });
        let stats = stats(&json);

        assert_eq!(stats.play_time, 1200);
        assert_eq!(stats.deaths, 1);
        assert_eq!(stats.distance.top[0].id, "minecraft:walk_one_cm");
        assert_eq!(stats.mined.top[0].id, "minecraft:stone");
        assert_eq!(stats.killed.top[0].id, "minecraft:zombie");
    }

    #[test]
    fn labels_read_like_the_game() {
        assert_eq!(label("minecraft:heart_of_the_sea"), "Heart of the Sea");
        assert_eq!(label("minecraft:oak_log"), "Oak Log");
        assert_eq!(
            enchantment("minecraft:protection", 12).label,
            "Protection 12"
        );
        assert_eq!(
            enchantment("minecraft:sweeping", 3).label,
            "Sweeping Edge III"
        );
    }

    #[test]
    fn bedrock_players_and_worlds_are_told() {
        assert!(is_bedrock(
            uuid::Uuid::parse_str("00000000-0000-0000-0009-01f64f65c7c3").unwrap()
        ));
        assert!(!is_bedrock(
            uuid::Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap()
        ));
        assert_eq!(
            world(Some("level-name=survival\n")).as_deref(),
            Some("survival")
        );
        assert_eq!(world(None).as_deref(), Some("world"));
        assert_eq!(world(Some("level-name=../etc\n")), None);
    }

    #[test]
    fn destinations_are_checked() {
        let far = Destination::Coordinates {
            x: 40_000_000.0,
            y: 64.0,
            z: 0.0,
            dimension: None,
        };
        assert!(far.validate().is_err());
        let odd = Destination::Coordinates {
            x: 0.0,
            y: 64.0,
            z: 0.0,
            dimension: Some("minecraft:the_end run op me".into()),
        };
        assert!(odd.validate().is_err());
        let fine = Destination::Coordinates {
            x: -951.34,
            y: 76.0,
            z: 0.0,
            dimension: Some("mypack:sky/islands".into()),
        };
        assert!(fine.validate().is_ok());
        assert_eq!(coordinate(-951.34), "-951.34");
        assert_eq!(coordinate(76.0), "76");
        assert_eq!(coordinate(-0.0001), "0");
        assert_eq!(coordinate(1e7), "10000000");
    }

    #[test]
    fn removals_say_what_they_do() {
        let all = Removal {
            xp: true,
            inventory: true,
            ender_chest: true,
            player_data: true,
            statistics: false,
            advancements: true,
        };
        // the file goes, so nothing in it is edited first
        assert!(all.edits().is_empty());
        assert_eq!(
            all.names(),
            vec![
                "xp",
                "inventory",
                "ender_chest",
                "player_data",
                "advancements"
            ]
        );
        let some = Removal {
            xp: true,
            ender_chest: true,
            ..Default::default()
        };
        assert_eq!(some.edits().len(), 2);
    }
}
