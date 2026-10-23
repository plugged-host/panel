// plugged.host: the ads settings section (docs/features/ads.md, shared/src/ads.rs).

use super::{
    ExtensionSettings, SettingsDeserializeExt, SettingsDeserializer, SettingsSerializeExt,
    SettingsSerializer,
};
use compact_str::{CompactString, ToCompactString};
use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Ads in the panel: Adsterra units in sandboxed frames, Monetag tags, the start waiting room,
/// the ad-block notice and the in-game message. Off by default; with ads on, nothing shows until a
/// zone, a tag or a sponsored banner exists.
#[derive(Clone, ToSchema, Serialize, Deserialize)]
pub struct AppSettingsAds {
    pub enabled: bool,
    /// Ads on the dashboard pages (server list, account, create server, store).
    pub show_account: bool,
    /// Ads on a server's pages.
    pub show_server: bool,
    /// Accounts with the second membership see no ads; the first one's refresh slower.
    pub member_free: bool,

    /// A leaderboard above every page.
    pub slot_top: bool,
    /// A strip under the header of a server's pages (not the console).
    pub slot_server_tools: bool,
    /// A rectangle on Create Server, the Resource Store and the server list.
    pub slot_pages: bool,
    /// A leaderboard, a rectangle and the native unit at the foot of every page.
    pub slot_foot: bool,

    /// Seconds in view after which a unit reloads; 0 never.
    pub refresh_seconds: i32,
    /// The same for the first membership; at least `refresh_seconds`.
    pub refresh_member_seconds: i32,

    /// Adsterra banner zones, one `key:WIDTHxHEIGHT:host` per line.
    pub zones: CompactString,
    /// The Native Banner's script (`https://host/path/invoke.js`) and container id.
    pub native_script: CompactString,
    pub native_container: CompactString,
    /// Height in pixels kept for the native unit.
    pub native_height: i32,
    /// The Social Bar's script; it loads on every page with ads only with `social_bar_everywhere`.
    pub social_bar_script: CompactString,
    pub social_bar_everywhere: bool,
    /// The Popunder's script; it only runs in the waiting rooms.
    pub popunder_script: CompactString,
    /// A Social Bar or Vignette zone of the waiting rooms' own.
    pub room_script: CompactString,

    /// Monetag's In-Page Push and Vignette tags: zone id and script URL.
    pub monetag_push_zone: CompactString,
    pub monetag_push_src: CompactString,
    pub monetag_vignette_zone: CompactString,
    pub monetag_vignette_src: CompactString,
    /// Minutes between two Vignettes for one browser.
    pub monetag_vignette_minutes: i32,
    /// The first membership gets the Vignette too.
    pub monetag_vignette_members: bool,
    /// Monetag's site verification code (`<meta name="monetag">` on the panel's pages).
    pub monetag_verification: CompactString,

    /// More hosts the page-level ad scripts may load scripts from, one per line.
    pub csp_hosts: CompactString,
    /// Another address of the panel (`https://host[:port]`) the ad frames load from, so they get
    /// an origin of their own; empty loads them from the panel's address with an opaque origin.
    pub frame_origin: CompactString,

    /// The start waiting room (start gate 30).
    pub room_enabled: bool,
    /// A server waits when its CPU (%), memory or disk (MiB) is over these; 0 makes every server wait.
    pub room_cpu: i32,
    pub room_memory: i64,
    pub room_disk: i64,
    /// Seconds the room stays open at least, and at most.
    pub room_min_seconds: i32,
    pub room_max_seconds: i32,
    /// Minutes after a room in which the same user starts the server without one.
    pub room_pass_minutes: i32,
    /// What the waiting rooms (start queue included) carry.
    pub room_banner: bool,
    pub room_native: bool,
    pub room_social_bar: bool,
    pub room_popunder: bool,
    pub room_interstitial: bool,

    /// The ad-block notice: `notice`, `modal` or `block`.
    pub adblock_enabled: bool,
    pub adblock_mode: CompactString,
    /// Seconds before the modal can be closed.
    pub adblock_grace: i32,
    /// Hours a dismissed notice stays away.
    pub adblock_dismiss_hours: i32,
    /// Also test whether the ad hosts answer.
    pub adblock_probe: bool,
    /// Never on the console.
    pub adblock_skip_console: bool,
    /// Replaces the default text.
    pub adblock_message: CompactString,

    /// A `say` to running Minecraft servers of accounts without a membership.
    pub in_game_enabled: bool,
    pub in_game_message: CompactString,
    pub in_game_interval: i32,
}

impl Default for AppSettingsAds {
    fn default() -> Self {
        Self {
            enabled: false,
            show_account: true,
            show_server: true,
            member_free: true,
            slot_top: true,
            slot_server_tools: true,
            slot_pages: true,
            slot_foot: true,
            refresh_seconds: 5,
            refresh_member_seconds: 10,
            zones: CompactString::default(),
            native_script: CompactString::default(),
            native_container: CompactString::default(),
            native_height: 320,
            social_bar_script: CompactString::default(),
            social_bar_everywhere: false,
            popunder_script: CompactString::default(),
            room_script: CompactString::default(),
            monetag_push_zone: CompactString::default(),
            monetag_push_src: CompactString::default(),
            monetag_vignette_zone: CompactString::default(),
            monetag_vignette_src: CompactString::default(),
            monetag_vignette_minutes: 10,
            monetag_vignette_members: false,
            monetag_verification: CompactString::default(),
            csp_hosts: CompactString::default(),
            frame_origin: CompactString::default(),
            room_enabled: false,
            room_cpu: 0,
            room_memory: 0,
            room_disk: 0,
            room_min_seconds: 15,
            room_max_seconds: 30,
            room_pass_minutes: 10,
            room_banner: true,
            room_native: true,
            room_social_bar: true,
            room_popunder: true,
            room_interstitial: true,
            adblock_enabled: false,
            adblock_mode: "notice".into(),
            adblock_grace: 4,
            adblock_dismiss_hours: 12,
            adblock_probe: true,
            adblock_skip_console: true,
            adblock_message: CompactString::default(),
            in_game_enabled: false,
            in_game_message: CompactString::default(),
            in_game_interval: 60,
        }
    }
}

impl AppSettingsAds {
    /// What `script-src` of the panel's pages has to allow: the page-level scripts' hosts and the
    /// extra hosts. Empty while ads are off.
    pub fn to_csp_script_src(&self) -> String {
        if !self.enabled {
            return String::new();
        }

        let mut hosts: Vec<CompactString> = Vec::new();
        for script in [
            &self.social_bar_script,
            &self.popunder_script,
            &self.room_script,
            &self.monetag_push_src,
            &self.monetag_vignette_src,
        ] {
            if let Some(host) = crate::ads::script_host(script) {
                hosts.push(host);
            }
        }
        hosts.extend(crate::ads::hosts(&self.csp_hosts));
        hosts.sort();
        hosts.dedup();

        hosts
            .iter()
            .map(|host| format!(" https://{host}"))
            .collect()
    }
}

const ADBLOCK_MODES: &[&str] = &["notice", "modal", "block"];

/// The ads tabs of Admin › Settings: every field optional, `null` leaves it alone. Script fields
/// take the URL or the whole snippet; zones take `key:WIDTHxHEIGHT:host` lines or the snippets.
#[derive(ToSchema, Validate, Deserialize)]
pub struct PayloadAds {
    #[garde(skip)]
    enabled: Option<bool>,
    #[garde(skip)]
    show_account: Option<bool>,
    #[garde(skip)]
    show_server: Option<bool>,
    #[garde(skip)]
    member_free: Option<bool>,
    #[garde(skip)]
    slot_top: Option<bool>,
    #[garde(skip)]
    slot_server_tools: Option<bool>,
    #[garde(skip)]
    slot_pages: Option<bool>,
    #[garde(skip)]
    slot_foot: Option<bool>,
    #[garde(inner(range(min = 0, max = 3600)))]
    refresh_seconds: Option<i32>,
    #[garde(inner(range(min = 0, max = 3600)))]
    refresh_member_seconds: Option<i32>,

    #[garde(inner(length(chars, max = 20000)))]
    zones: Option<CompactString>,
    #[garde(inner(length(chars, max = 2000)))]
    native_script: Option<CompactString>,
    #[garde(inner(length(chars, max = 128)))]
    native_container: Option<CompactString>,
    #[garde(inner(range(min = 50, max = 2000)))]
    native_height: Option<i32>,
    #[garde(inner(length(chars, max = 2000)))]
    social_bar_script: Option<CompactString>,
    #[garde(skip)]
    social_bar_everywhere: Option<bool>,
    #[garde(inner(length(chars, max = 2000)))]
    popunder_script: Option<CompactString>,
    #[garde(inner(length(chars, max = 2000)))]
    room_script: Option<CompactString>,

    #[garde(inner(length(chars, max = 12)))]
    monetag_push_zone: Option<CompactString>,
    #[garde(inner(length(chars, max = 2000)))]
    monetag_push_src: Option<CompactString>,
    #[garde(inner(length(chars, max = 12)))]
    monetag_vignette_zone: Option<CompactString>,
    #[garde(inner(length(chars, max = 2000)))]
    monetag_vignette_src: Option<CompactString>,
    #[garde(inner(range(min = 0, max = 1440)))]
    monetag_vignette_minutes: Option<i32>,
    #[garde(skip)]
    monetag_vignette_members: Option<bool>,
    #[garde(inner(length(chars, max = 128)))]
    monetag_verification: Option<CompactString>,

    #[garde(inner(length(chars, max = 4000)))]
    csp_hosts: Option<CompactString>,
    #[garde(inner(length(chars, max = 255)))]
    frame_origin: Option<CompactString>,

    #[garde(skip)]
    room_enabled: Option<bool>,
    #[garde(inner(range(min = 0, max = 1_000_000)))]
    room_cpu: Option<i32>,
    #[garde(inner(range(min = 0, max = 100_000_000)))]
    room_memory: Option<i64>,
    #[garde(inner(range(min = 0, max = 100_000_000)))]
    room_disk: Option<i64>,
    #[garde(inner(range(min = 0, max = 600)))]
    room_min_seconds: Option<i32>,
    #[garde(inner(range(min = 1, max = 600)))]
    room_max_seconds: Option<i32>,
    #[garde(inner(range(min = 0, max = 1440)))]
    room_pass_minutes: Option<i32>,
    #[garde(skip)]
    room_banner: Option<bool>,
    #[garde(skip)]
    room_native: Option<bool>,
    #[garde(skip)]
    room_social_bar: Option<bool>,
    #[garde(skip)]
    room_popunder: Option<bool>,
    #[garde(skip)]
    room_interstitial: Option<bool>,

    #[garde(skip)]
    adblock_enabled: Option<bool>,
    #[garde(inner(length(chars, max = 16)))]
    adblock_mode: Option<CompactString>,
    #[garde(inner(range(min = 0, max = 60)))]
    adblock_grace: Option<i32>,
    #[garde(inner(range(min = 0, max = 720)))]
    adblock_dismiss_hours: Option<i32>,
    #[garde(skip)]
    adblock_probe: Option<bool>,
    #[garde(skip)]
    adblock_skip_console: Option<bool>,
    #[garde(inner(length(chars, max = 1000)))]
    adblock_message: Option<CompactString>,

    #[garde(skip)]
    in_game_enabled: Option<bool>,
    #[garde(inner(length(chars, max = 200)))]
    in_game_message: Option<CompactString>,
    #[garde(inner(range(min = 5, max = 1440)))]
    in_game_interval: Option<i32>,
}

impl PayloadAds {
    pub fn apply(self, settings: &mut AppSettingsAds) -> Result<(), String> {
        macro_rules! set {
            ($($field:ident),* $(,)?) => {
                $(if let Some(value) = self.$field {
                    settings.$field = value;
                })*
            };
        }
        // a script field: the URL or the snippet it is in; empty clears it
        macro_rules! script {
            ($($field:ident => $name:literal),* $(,)?) => {
                $(if let Some(value) = self.$field {
                    settings.$field = if value.trim().is_empty() {
                        CompactString::default()
                    } else {
                        crate::ads::clean_script_url(&value)
                            .ok_or_else(|| format!("the {} is not a script URL", $name))?
                            .into()
                    };
                })*
            };
        }

        set!(
            enabled,
            show_account,
            show_server,
            member_free,
            slot_top,
            slot_server_tools,
            slot_pages,
            slot_foot,
            refresh_seconds,
            refresh_member_seconds,
            native_height,
            social_bar_everywhere,
            monetag_vignette_minutes,
            monetag_vignette_members,
            room_enabled,
            room_cpu,
            room_memory,
            room_disk,
            room_min_seconds,
            room_max_seconds,
            room_pass_minutes,
            room_banner,
            room_native,
            room_social_bar,
            room_popunder,
            room_interstitial,
            adblock_enabled,
            adblock_grace,
            adblock_dismiss_hours,
            adblock_probe,
            adblock_skip_console,
            in_game_enabled,
            in_game_interval,
        );
        script!(
            native_script => "native banner script",
            social_bar_script => "social bar script",
            popunder_script => "popunder script",
            room_script => "waiting room script",
            monetag_push_src => "in-page push script",
            monetag_vignette_src => "vignette script",
        );

        if let Some(zones) = self.zones {
            settings.zones = crate::ads::zones_to_text(&crate::ads::parse_zones(&zones)?).into();
        }
        if let Some(container) = self.native_container {
            let container = container.trim();
            if !container.is_empty() && !crate::ads::valid_id(container) {
                return Err("the native banner container is not an element id".into());
            }
            settings.native_container = container.into();
        }
        for (value, target, name) in [
            (
                self.monetag_push_zone,
                &mut settings.monetag_push_zone,
                "in-page push zone",
            ),
            (
                self.monetag_vignette_zone,
                &mut settings.monetag_vignette_zone,
                "vignette zone",
            ),
        ] {
            if let Some(value) = value {
                let value = value.trim();
                if !value.is_empty() && !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(format!("the {name} must be a number"));
                }
                *target = value.into();
            }
        }
        if let Some(code) = self.monetag_verification {
            let code = code.trim();
            if !code.is_empty() && !crate::ads::valid_id(code) {
                return Err(
                    "the monetag verification code may only hold letters, digits, - and _".into(),
                );
            }
            settings.monetag_verification = code.into();
        }
        if let Some(hosts) = self.csp_hosts {
            let mut list = Vec::new();
            for line in hosts.lines().map(str::trim).filter(|line| !line.is_empty()) {
                let host = line
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
                    .trim_start_matches("//")
                    .trim_end_matches('/');
                if !crate::ads::valid_host(host) {
                    return Err(format!("{line} is not a host name"));
                }
                list.push(host.to_ascii_lowercase());
            }
            settings.csp_hosts = list.join("\n").into();
        }
        if let Some(origin) = self.frame_origin {
            let origin = origin.trim().trim_end_matches('/');
            if origin.is_empty() {
                settings.frame_origin = CompactString::default();
            } else {
                match crate::ads::frame_origin(origin) {
                    Some(clean) => settings.frame_origin = clean.into(),
                    None => {
                        return Err(
                            "the ad frame address must be https://host or https://host:port".into(),
                        );
                    }
                }
            }
        }
        if let Some(mode) = self.adblock_mode {
            if !ADBLOCK_MODES.contains(&mode.as_str()) {
                return Err("the ad-block mode must be notice, modal or block".into());
            }
            settings.adblock_mode = mode;
        }
        if let Some(message) = self.adblock_message {
            settings.adblock_message = message.trim().into();
        }
        if let Some(message) = self.in_game_message {
            let message = message.trim();
            if message.chars().any(char::is_control) {
                return Err("the in-game message must be one line".into());
            }
            settings.in_game_message = message.into();
        }

        if settings.room_min_seconds > settings.room_max_seconds {
            return Err("the waiting room's shortest stay is longer than its longest".into());
        }
        settings.refresh_member_seconds = if settings.refresh_seconds == 0 {
            settings.refresh_member_seconds
        } else if settings.refresh_member_seconds == 0 {
            0
        } else {
            settings
                .refresh_member_seconds
                .max(settings.refresh_seconds)
        };

        Ok(())
    }
}

#[async_trait::async_trait]
impl SettingsSerializeExt for AppSettingsAds {
    async fn serialize(
        &self,
        serializer: SettingsSerializer,
    ) -> Result<SettingsSerializer, anyhow::Error> {
        macro_rules! write_all {
            ($serializer:expr, $($field:ident),* $(,)?) => {
                $serializer$(.write_raw_setting(stringify!($field), self.$field.to_compact_string()))*
            };
        }

        Ok(write_all!(
            serializer,
            enabled,
            show_account,
            show_server,
            member_free,
            slot_top,
            slot_server_tools,
            slot_pages,
            slot_foot,
            refresh_seconds,
            refresh_member_seconds,
            zones,
            native_script,
            native_container,
            native_height,
            social_bar_script,
            social_bar_everywhere,
            popunder_script,
            room_script,
            monetag_push_zone,
            monetag_push_src,
            monetag_vignette_zone,
            monetag_vignette_src,
            monetag_vignette_minutes,
            monetag_vignette_members,
            monetag_verification,
            csp_hosts,
            frame_origin,
            room_enabled,
            room_cpu,
            room_memory,
            room_disk,
            room_min_seconds,
            room_max_seconds,
            room_pass_minutes,
            room_banner,
            room_native,
            room_social_bar,
            room_popunder,
            room_interstitial,
            adblock_enabled,
            adblock_mode,
            adblock_grace,
            adblock_dismiss_hours,
            adblock_probe,
            adblock_skip_console,
            adblock_message,
            in_game_enabled,
            in_game_message,
            in_game_interval,
        ))
    }
}

pub struct AppSettingsAdsDeserializer;

#[async_trait::async_trait]
impl SettingsDeserializeExt for AppSettingsAdsDeserializer {
    async fn deserialize_boxed(
        &self,
        mut deserializer: SettingsDeserializer<'_>,
    ) -> Result<ExtensionSettings, anyhow::Error> {
        let defaults = AppSettingsAds::default();

        macro_rules! read {
            ($($field:ident),* $(,)?) => {
                AppSettingsAds {
                    $($field: deserializer
                        .take_raw_setting(stringify!($field))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(defaults.$field),)*
                }
            };
        }

        Ok(Box::new(read!(
            enabled,
            show_account,
            show_server,
            member_free,
            slot_top,
            slot_server_tools,
            slot_pages,
            slot_foot,
            refresh_seconds,
            refresh_member_seconds,
            zones,
            native_script,
            native_container,
            native_height,
            social_bar_script,
            social_bar_everywhere,
            popunder_script,
            room_script,
            monetag_push_zone,
            monetag_push_src,
            monetag_vignette_zone,
            monetag_vignette_src,
            monetag_vignette_minutes,
            monetag_vignette_members,
            monetag_verification,
            csp_hosts,
            frame_origin,
            room_enabled,
            room_cpu,
            room_memory,
            room_disk,
            room_min_seconds,
            room_max_seconds,
            room_pass_minutes,
            room_banner,
            room_native,
            room_social_bar,
            room_popunder,
            room_interstitial,
            adblock_enabled,
            adblock_mode,
            adblock_grace,
            adblock_dismiss_hours,
            adblock_probe,
            adblock_skip_console,
            adblock_message,
            in_game_enabled,
            in_game_message,
            in_game_interval,
        )))
    }
}
