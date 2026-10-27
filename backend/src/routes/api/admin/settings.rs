use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        models::user::GetPermissionManager,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response<'a> {
        #[schema(inline)]
        settings: &'a shared::settings::AppSettings,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState, permissions: GetPermissionManager) -> ApiResponseResult {
        permissions.has_admin_permission("settings.read")?;

        let settings = state.settings.get().await?;

        ApiResponse::new_serialized(Response {
            settings: &settings,
        })
        .ok()
    }
}

mod put {
    use axum::http::StatusCode;
    use garde::Validate;
    use serde::{Deserialize, Serialize};
    use shared::{
        ApiError, GetState,
        models::{
            admin_activity::GetAdminActivityLogger,
            oauth_provider::OAuthProvider,
            server::TrashSettings,
            user::{GetPermissionManager, GetUser},
            user_api_key::UserApiKey,
            user_oauth_link::UserOAuthLink,
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadApp {
        #[garde(length(chars, min = 1, max = 64))]
        name: Option<compact_str::CompactString>,
        #[garde(length(chars, min = 1, max = 255))]
        icon: Option<compact_str::CompactString>,
        #[garde(length(chars, min = 1, max = 255))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        icon_light: Option<Option<compact_str::CompactString>>,
        #[garde(length(chars, min = 1, max = 255))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        banner: Option<Option<compact_str::CompactString>>,
        #[garde(length(chars, min = 1, max = 255))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        banner_light: Option<Option<compact_str::CompactString>>,
        #[garde(url)]
        url: Option<compact_str::CompactString>,
        #[garde(length(max = 32), inner(inner(url)))]
        additional_urls: Option<Vec<compact_str::CompactString>>,
        #[garde(
            length(chars, min = 2, max = 15),
            inner(custom(shared::utils::validate_language))
        )]
        language: Option<compact_str::CompactString>,
        #[garde(skip)]
        two_factor_requirement: Option<shared::settings::app::TwoFactorRequirement>,
        #[garde(skip)]
        email_two_factor_enabled: Option<bool>,
        #[garde(skip)]
        two_factor_accepted_methods: Option<Vec<shared::settings::app::TwoFactorMethod>>,
        #[garde(skip)]
        email_verification_required: Option<bool>,
        #[garde(length(chars, min = 1, max = 255))]
        session_cookie: Option<compact_str::CompactString>,
        #[garde(range(min = 60, max = 31536000))]
        session_duration_seconds: Option<u64>,
        #[garde(skip)]
        telemetry_enabled: Option<bool>,
        #[garde(skip)]
        registration_enabled: Option<bool>,
        #[garde(skip)]
        password_login_enabled: Option<bool>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadMetadata {
        #[garde(length(chars, min = 1, max = 320))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        description: Option<Option<compact_str::CompactString>>,
        #[garde(pattern("^#[0-9a-fA-F]{6}$"))]
        theme_color: Option<compact_str::CompactString>,
        #[garde(skip)]
        seasonal_theme: Option<shared::settings::metadata::SeasonalTheme>, // plugged.host
        #[garde(length(chars, min = 1, max = 255))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        og_image: Option<Option<compact_str::CompactString>>,
        #[garde(skip)]
        twitter_card: Option<shared::settings::metadata::TwitterCard>,
        #[garde(skip)]
        indexable: Option<bool>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadWebauthn {
        #[garde(skip)]
        enabled: Option<bool>,
        #[garde(skip)]
        allow_discoverable: Option<bool>,

        #[garde(length(chars, min = 1, max = 255))]
        rp_id: Option<compact_str::CompactString>,
        #[garde(url)]
        rp_origin: Option<compact_str::CompactString>,

        #[garde(range(min = 30, max = 600))]
        #[schema(minimum = 30, maximum = 600)]
        authentication_timeout_seconds: Option<u64>,
        #[garde(range(min = 30, max = 600))]
        #[schema(minimum = 30, maximum = 600)]
        registration_timeout_seconds: Option<u64>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadServer {
        #[garde(skip)]
        max_file_manager_view_size: Option<u64>,
        #[garde(skip)]
        max_file_manager_content_search_size: Option<u64>,
        #[garde(skip)]
        max_file_manager_search_results: Option<u64>,
        #[garde(skip)]
        max_subuser_count: Option<u64>,
        #[garde(skip)]
        max_schedule_step_count: Option<u64>,
        #[garde(skip)]
        max_backup_group_count: Option<u64>,
        #[garde(skip)]
        max_firewall_rule_count: Option<u64>,
        #[garde(skip)]
        max_firewall_rule_source_count: Option<u64>,
        #[garde(skip)]
        max_tunnel_connection_count: Option<u64>,
        #[garde(skip)]
        max_tunnel_port_count: Option<u64>,
        #[garde(skip)]
        max_database_instance_database_count: Option<u64>,
        #[garde(skip)]
        max_database_instance_user_count: Option<u64>,

        #[garde(skip)]
        allow_overwriting_custom_docker_image: Option<bool>,
        #[garde(skip)]
        allow_viewing_installation_logs: Option<bool>,
        #[garde(skip)]
        allow_acknowledging_installation_failure: Option<bool>,
        #[garde(skip)]
        allow_viewing_transfer_progress: Option<bool>,

        #[garde(dive)]
        #[schema(inline)]
        trash: Option<TrashSettings>,
        #[garde(skip)]
        allow_changing_trash_settings: Option<bool>,

        #[garde(length(chars, min = 1, max = 255))]
        container_prelude: Option<compact_str::CompactString>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadUser {
        #[garde(skip)]
        max_server_group_count: Option<u64>,
        #[garde(skip)]
        max_api_key_count: Option<u64>,
        #[garde(skip)]
        max_command_snippet_count: Option<u64>,
        #[garde(skip)]
        max_security_key_count: Option<u64>,
        #[garde(skip)]
        max_ssh_key_count: Option<u64>,
        #[garde(skip)]
        max_settings_count: Option<u64>,
        #[garde(skip)]
        max_settings_value_bytes: Option<u64>,

        #[garde(skip)]
        allow_changing_language: Option<bool>,
        #[garde(skip)]
        allow_changing_avatar: Option<bool>,

        #[garde(length(max = 100), dive)]
        #[schema(max_items = 100)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        route_order: Option<Option<Vec<shared::settings::RouteOrderItem>>>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadActivity {
        #[garde(range(min = 1, max = 3650))]
        #[schema(minimum = 1, maximum = 3650)]
        admin_log_retention_days: Option<u16>,
        #[garde(range(min = 1))]
        #[schema(minimum = 1)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        admin_log_retention_count: Option<Option<u64>>,
        #[garde(range(min = 1, max = 3650))]
        #[schema(minimum = 1, maximum = 3650)]
        user_log_retention_days: Option<u16>,
        #[garde(range(min = 1))]
        #[schema(minimum = 1)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        user_log_retention_count: Option<Option<u64>>,
        #[garde(range(min = 1, max = 3650))]
        #[schema(minimum = 1, maximum = 3650)]
        server_log_retention_days: Option<u16>,
        #[garde(range(min = 1))]
        #[schema(minimum = 1)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        server_log_retention_count: Option<Option<u64>>,

        #[garde(skip)]
        server_log_admin_activity: Option<bool>,
        #[garde(skip)]
        server_log_schedule_activity: Option<bool>,
        #[garde(skip)]
        server_hide_activity_ips: Option<shared::settings::activity::ActivityIpHiding>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadRatelimits {
        #[garde(dive)]
        auth_register: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        auth_login: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        auth_login_checkpoint: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        auth_login_checkpoint_email: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        auth_login_security_key: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        auth_password_forgot: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        auth_password_reset: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        auth_email_verification: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        client: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        client_account_email_resend_verification:
            Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        client_servers_backups_create: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        client_servers_files_pull: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        client_servers_files_pull_query:
            Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        remote: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        remote_sftp_auth: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(dive)]
        remote_enroll: Option<shared::settings::ratelimits::RatelimitConfiguration>,
        #[garde(length(max = 256))]
        #[schema(value_type = Option<Vec<String>>)]
        exempt_ips: Option<Vec<sqlx::types::ipnetwork::IpNetwork>>,
        #[garde(length(max = 256))]
        exempt_api_keys: Option<Vec<uuid::Uuid>>,
    }

    // plugged.host: noprofit.space (shared/src/settings/noprofit.rs)
    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadNoprofit {
        #[garde(skip)]
        enabled: Option<bool>,
        #[garde(url)]
        accounts_url: Option<compact_str::CompactString>,
        #[garde(length(chars, min = 1, max = 64))]
        sso_client: Option<compact_str::CompactString>,
        #[garde(length(chars, min = 1, max = 128))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        sso_audience: Option<Option<compact_str::CompactString>>,
        #[garde(url)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        sso_callback_url: Option<Option<compact_str::CompactString>>,
        #[garde(skip)]
        admins_only: Option<bool>,
        #[garde(skip)]
        link_by_email: Option<bool>,
        /// Days until the user and servers of an account noprofit.space no longer has are
        /// deleted; 0 never.
        #[garde(range(min = 0, max = 365))]
        deleted_account_days: Option<i32>,
        /// At least 32 characters. An empty string keeps the stored secret, `null` removes it.
        #[garde(length(chars, max = 512))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        sso_secret: Option<Option<compact_str::CompactString>>,
        /// At least 32 characters. An empty string keeps the stored secret, `null` removes it.
        #[garde(length(chars, max = 512))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        api_secret: Option<Option<compact_str::CompactString>>,
        /// neo's gateway on noprofit.space (docs/features/neo.md). At least 32 characters. An
        /// empty string keeps the stored secret, `null` removes it.
        #[garde(length(chars, max = 512))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        neo_secret: Option<Option<compact_str::CompactString>>,
    }

    // plugged.host: the status page (shared/src/settings/status.rs)
    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadStatus {
        #[garde(skip)]
        enabled: Option<bool>,
        #[garde(url)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        site_url: Option<Option<compact_str::CompactString>>,
        #[garde(length(chars, min = 1, max = 255))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        site_host: Option<Option<compact_str::CompactString>>,
        #[garde(url)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        panel_url: Option<Option<compact_str::CompactString>>,
        #[garde(url)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        page_url: Option<Option<compact_str::CompactString>>,
        #[garde(range(min = 1, max = 365))]
        history_days: Option<i32>,
        #[garde(range(min = 1, max = 100))]
        cpu_threshold: Option<i32>,
    }

    // plugged.host: abuse protection (shared/src/settings/abuse.rs)
    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadAbuse {
        #[garde(skip)]
        mode: Option<shared::settings::abuse::AbuseMode>,
        /// A Discord webhook URL. The placeholder keeps the stored one, `null` removes it.
        #[garde(length(chars, max = 512))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        alert_webhook: Option<Option<compact_str::CompactString>>,
        #[garde(range(min = 50, max = 100))]
        cpu_percent: Option<i32>,
        #[garde(range(min = 5, max = 1440))]
        cpu_minutes: Option<i32>,
        #[garde(range(min = 0, max = 8760))]
        delete_after_hours: Option<i32>,
        /// The panel's own account standing (docs/features/standing.md).
        #[garde(range(min = 1, max = 100))]
        standing_limited_at: Option<i32>,
        #[garde(range(min = 1, max = 3650))]
        standing_limited_days: Option<i32>,
        #[garde(range(min = 1, max = 100))]
        standing_very_limited_at: Option<i32>,
        #[garde(range(min = 1, max = 3650))]
        standing_very_limited_days: Option<i32>,
        #[garde(range(min = 1, max = 100))]
        standing_suspended_at: Option<i32>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct PayloadUpdates {
        #[garde(length(chars, min = 1, max = 1024))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        wings_release_token: Option<Option<compact_str::CompactString>>,
        #[garde(length(chars, min = 1, max = 1024))]
        #[serde(default, with = "::serde_with::rust::double_option")]
        db_agent_release_token: Option<Option<compact_str::CompactString>>,
    }

    #[derive(ToSchema, Validate, Deserialize)]
    pub struct Payload {
        #[garde(skip)]
        #[serde(default, with = "::serde_with::rust::double_option")]
        oobe_step: Option<Option<compact_str::CompactString>>,

        #[garde(dive)]
        storage_driver: Option<shared::settings::StorageDriver>,
        #[garde(dive)]
        mail_mode: Option<shared::settings::MailMode>,
        #[garde(dive)]
        captcha_provider: Option<shared::settings::CaptchaProvider>,

        #[schema(inline)]
        #[garde(dive)]
        app: Option<PayloadApp>,
        #[schema(inline)]
        #[garde(dive)]
        metadata: Option<PayloadMetadata>,
        #[schema(inline)]
        #[garde(dive)]
        webauthn: Option<PayloadWebauthn>,
        #[schema(inline)]
        #[garde(dive)]
        server: Option<PayloadServer>,
        #[schema(inline)]
        #[garde(dive)]
        user: Option<PayloadUser>,
        #[schema(inline)]
        #[garde(dive)]
        activity: Option<PayloadActivity>,
        #[schema(inline)]
        #[garde(dive)]
        ratelimits: Option<PayloadRatelimits>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        noprofit: Option<PayloadNoprofit>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        status: Option<PayloadStatus>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        abuse: Option<PayloadAbuse>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        games: Option<shared::settings::games::PayloadGames>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        placement: Option<shared::settings::placement::PayloadPlacement>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        tour: Option<shared::settings::tour::PayloadTour>,
        // plugged.host: docs/features/reviews.md
        #[schema(inline)]
        #[garde(dive)]
        reviews: Option<shared::settings::reviews::PayloadReviews>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        recovery: Option<shared::settings::recovery::PayloadRecovery>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        economy: Option<shared::settings::economy::PayloadEconomy>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        queue: Option<shared::settings::queue::PayloadQueue>,
        // plugged.host
        #[schema(inline)]
        #[garde(dive)]
        limbo: Option<shared::settings::limbo::PayloadLimbo>,
        // plugged.host: docs/features/ads.md
        #[schema(inline)]
        #[garde(dive)]
        ads: Option<shared::settings::ads::PayloadAds>,
        // plugged.host: docs/features/subdomains.md
        #[schema(inline)]
        #[garde(dive)]
        subdomains: Option<shared::settings::subdomains::PayloadSubdomains>,
        // plugged.host: docs/features/discord.md
        #[schema(inline)]
        #[garde(dive)]
        discord: Option<shared::settings::discord::PayloadDiscord>,
        // plugged.host: docs/features/devboxes.md
        #[schema(inline)]
        #[garde(dive)]
        devboxes: Option<shared::settings::devboxes::PayloadDevboxes>,
        // plugged.host: docs/features/neo.md
        #[schema(inline)]
        #[garde(dive)]
        neo: Option<shared::settings::neo::PayloadNeo>,
        // plugged.host: docs/features/rux.md
        #[schema(inline)]
        #[garde(dive)]
        rux: Option<shared::settings::rux::PayloadRux>,
        // plugged.host: docs/features/neo-discord.md
        #[schema(inline)]
        #[garde(dive)]
        neo_discord: Option<shared::settings::neo_discord::PayloadNeoDiscord>,
        // plugged.host: docs/features/partners.md
        #[schema(inline)]
        #[garde(dive)]
        partners: Option<shared::settings::partners::PayloadPartners>,
        #[schema(inline)]
        #[garde(dive)]
        updates: Option<PayloadUpdates>,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {}

    #[utoipa::path(put, path = "/", responses(
        (status = OK, body = inline(Response)),
        (status = BAD_REQUEST, body = ApiError),
    ), request_body = inline(Payload))]
    pub async fn route(
        state: GetState,
        permissions: GetPermissionManager,
        user: GetUser,
        activity_logger: GetAdminActivityLogger,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        if let Err(errors) = shared::utils::validate_data(&data) {
            return ApiResponse::new_serialized(ApiError::new_strings_value(errors))
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        permissions.has_admin_permission("settings.update")?;

        if let Some(oobe_step) = data.oobe_step {
            state.settings.set_oobe_step(oobe_step).await?;
        }

        let mut settings = state.settings.get_mut().await?;

        if let Some(storage_driver) = data.storage_driver {
            settings.storage_driver = storage_driver;
        }
        if let Some(mail_mode) = data.mail_mode {
            settings.mail_mode = mail_mode;
        }
        if let Some(captcha_provider) = data.captcha_provider {
            settings.captcha_provider = captcha_provider;
        }
        if let Some(app) = data.app {
            if let Some(name) = app.name {
                settings.app.name = name;
            }
            if let Some(icon) = app.icon {
                settings.app.icon = icon;
            }
            if let Some(icon_light) = app.icon_light {
                settings.app.icon_light = icon_light;
            }
            if let Some(banner) = app.banner {
                settings.app.banner = banner;
            }
            if let Some(banner_light) = app.banner_light {
                settings.app.banner_light = banner_light;
            }
            if let Some(url) = app.url {
                settings.app.url = url.trim_end_matches('/').into();
            }
            if let Some(additional_urls) = app.additional_urls {
                let mut deduped: Vec<compact_str::CompactString> = Vec::new();
                for url in additional_urls {
                    let url = url.trim_end_matches('/');
                    if url != settings.app.url && !deduped.iter().any(|u| u == url) {
                        deduped.push(url.into());
                    }
                }

                settings.app.additional_urls = deduped;
            }
            if let Some(language) = app.language {
                settings.app.language = language;
            }
            if let Some(two_factor_requirement) = app.two_factor_requirement {
                settings.app.two_factor_requirement = two_factor_requirement;
            }
            if let Some(email_two_factor_enabled) = app.email_two_factor_enabled {
                settings.app.email_two_factor_enabled = email_two_factor_enabled;
            }
            if let Some(two_factor_accepted_methods) = app.two_factor_accepted_methods {
                let mut deduped: Vec<shared::settings::app::TwoFactorMethod> = Vec::new();
                for method in two_factor_accepted_methods {
                    if !deduped.contains(&method) {
                        deduped.push(method);
                    }
                }

                settings.app.two_factor_accepted_methods = deduped;
            }
            if let Some(email_verification_required) = app.email_verification_required {
                settings.app.email_verification_required = email_verification_required;
            }
            if let Some(session_cookie) = app.session_cookie {
                settings.app.session_cookie = session_cookie;
            }
            if let Some(session_duration_seconds) = app.session_duration_seconds {
                settings.app.session_duration_seconds = session_duration_seconds;
            }
            if let Some(telemetry_enabled) = app.telemetry_enabled {
                settings.app.telemetry_enabled = telemetry_enabled;
            }
            if let Some(registration_enabled) = app.registration_enabled {
                settings.app.registration_enabled = registration_enabled;
            }
            if let Some(password_login_enabled) = app.password_login_enabled {
                settings.app.password_login_enabled = password_login_enabled;
            }
        }
        if let Some(metadata) = data.metadata {
            if let Some(description) = metadata.description {
                settings.metadata.description = description;
            }
            if let Some(theme_color) = metadata.theme_color {
                settings.metadata.theme_color = theme_color;
            }
            // plugged.host
            if let Some(seasonal_theme) = metadata.seasonal_theme {
                settings.metadata.seasonal_theme = seasonal_theme;
            }
            if let Some(og_image) = metadata.og_image {
                settings.metadata.og_image = og_image;
            }
            if let Some(twitter_card) = metadata.twitter_card {
                settings.metadata.twitter_card = twitter_card;
            }
            if let Some(indexable) = metadata.indexable {
                settings.metadata.indexable = indexable;
            }
        }
        if let Some(user) = data.user {
            if let Some(max_server_group_count) = user.max_server_group_count {
                settings.user.max_server_group_count = max_server_group_count;
            }
            if let Some(max_api_key_count) = user.max_api_key_count {
                settings.user.max_api_key_count = max_api_key_count;
            }
            if let Some(max_command_snippet_count) = user.max_command_snippet_count {
                settings.user.max_command_snippet_count = max_command_snippet_count;
            }
            if let Some(max_security_key_count) = user.max_security_key_count {
                settings.user.max_security_key_count = max_security_key_count;
            }
            if let Some(max_ssh_key_count) = user.max_ssh_key_count {
                settings.user.max_ssh_key_count = max_ssh_key_count;
            }
            if let Some(max_settings_count) = user.max_settings_count {
                settings.user.max_settings_count = max_settings_count;
            }
            if let Some(max_settings_value_bytes) = user.max_settings_value_bytes {
                settings.user.max_settings_value_bytes = max_settings_value_bytes;
            }
            if let Some(allow_changing_language) = user.allow_changing_language {
                settings.user.allow_changing_language = allow_changing_language;
            }
            if let Some(allow_changing_avatar) = user.allow_changing_avatar {
                settings.user.allow_changing_avatar = allow_changing_avatar;
            }
            if let Some(route_order) = user.route_order {
                settings.user.route_order = route_order;
            }
        }
        if let Some(webauthn) = data.webauthn {
            if let Some(enabled) = webauthn.enabled {
                settings.webauthn.enabled = enabled;
            }
            if let Some(allow_discoverable) = webauthn.allow_discoverable {
                settings.webauthn.allow_discoverable = allow_discoverable;
            }
            if let Some(rp_id) = webauthn.rp_id {
                settings.webauthn.rp_id = rp_id;
            }
            if let Some(rp_origin) = webauthn.rp_origin {
                settings.webauthn.rp_origin = rp_origin;
            }
            if let Some(authentication_timeout_seconds) = webauthn.authentication_timeout_seconds {
                settings.webauthn.authentication_timeout_seconds = authentication_timeout_seconds;
            }
            if let Some(registration_timeout_seconds) = webauthn.registration_timeout_seconds {
                settings.webauthn.registration_timeout_seconds = registration_timeout_seconds;
            }
        }
        if let Some(server) = data.server {
            if let Some(max_file_manager_view_size) = server.max_file_manager_view_size {
                settings.server.max_file_manager_view_size = max_file_manager_view_size;
            }
            if let Some(max_file_manager_content_search_size) =
                server.max_file_manager_content_search_size
            {
                settings.server.max_file_manager_content_search_size =
                    max_file_manager_content_search_size;
            }
            if let Some(max_file_manager_search_results) = server.max_file_manager_search_results {
                settings.server.max_file_manager_search_results = max_file_manager_search_results;
            }
            if let Some(max_subuser_count) = server.max_subuser_count {
                settings.server.max_subuser_count = max_subuser_count;
            }
            if let Some(max_schedule_step_count) = server.max_schedule_step_count {
                settings.server.max_schedule_step_count = max_schedule_step_count;
            }
            if let Some(max_backup_group_count) = server.max_backup_group_count {
                settings.server.max_backup_group_count = max_backup_group_count;
            }
            if let Some(max_firewall_rule_count) = server.max_firewall_rule_count {
                settings.server.max_firewall_rule_count = max_firewall_rule_count;
            }
            if let Some(max_firewall_rule_source_count) = server.max_firewall_rule_source_count {
                settings.server.max_firewall_rule_source_count = max_firewall_rule_source_count;
            }
            if let Some(max_tunnel_connection_count) = server.max_tunnel_connection_count {
                settings.server.max_tunnel_connection_count = max_tunnel_connection_count;
            }
            if let Some(max_tunnel_port_count) = server.max_tunnel_port_count {
                settings.server.max_tunnel_port_count = max_tunnel_port_count;
            }
            if let Some(max_database_instance_database_count) =
                server.max_database_instance_database_count
            {
                settings.server.max_database_instance_database_count =
                    max_database_instance_database_count;
            }
            if let Some(max_database_instance_user_count) = server.max_database_instance_user_count
            {
                settings.server.max_database_instance_user_count = max_database_instance_user_count;
            }
            if let Some(allow_overwriting_custom_docker_image) =
                server.allow_overwriting_custom_docker_image
            {
                settings.server.allow_overwriting_custom_docker_image =
                    allow_overwriting_custom_docker_image;
            }
            if let Some(allow_viewing_installation_logs) = server.allow_viewing_installation_logs {
                settings.server.allow_viewing_installation_logs = allow_viewing_installation_logs;
            }
            if let Some(allow_acknowledging_installation_failure) =
                server.allow_acknowledging_installation_failure
            {
                settings.server.allow_acknowledging_installation_failure =
                    allow_acknowledging_installation_failure;
            }
            if let Some(allow_viewing_transfer_progress) = server.allow_viewing_transfer_progress {
                settings.server.allow_viewing_transfer_progress = allow_viewing_transfer_progress;
            }
            if let Some(trash) = server.trash {
                settings.server.trash = trash;
            }
            if let Some(allow_changing_trash_settings) = server.allow_changing_trash_settings {
                settings.server.allow_changing_trash_settings = allow_changing_trash_settings;
            }
            if let Some(container_prelude) = server.container_prelude {
                settings.server.container_prelude = container_prelude;
            }
        }
        if let Some(activity) = data.activity {
            if let Some(admin_log_retention_days) = activity.admin_log_retention_days {
                settings.activity.admin_log_retention_days = admin_log_retention_days;
            }
            if let Some(admin_log_retention_count) = activity.admin_log_retention_count {
                settings.activity.admin_log_retention_count = admin_log_retention_count;
            }
            if let Some(user_log_retention_days) = activity.user_log_retention_days {
                settings.activity.user_log_retention_days = user_log_retention_days;
            }
            if let Some(user_log_retention_count) = activity.user_log_retention_count {
                settings.activity.user_log_retention_count = user_log_retention_count;
            }
            if let Some(server_log_retention_days) = activity.server_log_retention_days {
                settings.activity.server_log_retention_days = server_log_retention_days;
            }
            if let Some(server_log_retention_count) = activity.server_log_retention_count {
                settings.activity.server_log_retention_count = server_log_retention_count;
            }
            if let Some(server_log_admin_activity) = activity.server_log_admin_activity {
                settings.activity.server_log_admin_activity = server_log_admin_activity;
            }
            if let Some(server_log_schedule_activity) = activity.server_log_schedule_activity {
                settings.activity.server_log_schedule_activity = server_log_schedule_activity;
            }
            if let Some(server_hide_activity_ips) = activity.server_hide_activity_ips {
                settings.activity.server_hide_activity_ips = server_hide_activity_ips;
            }
        }
        if let Some(ratelimits) = data.ratelimits {
            if let Some(auth_register) = ratelimits.auth_register {
                settings.ratelimits.auth_register = auth_register;
            }
            if let Some(auth_login) = ratelimits.auth_login {
                settings.ratelimits.auth_login = auth_login;
            }
            if let Some(auth_login_checkpoint) = ratelimits.auth_login_checkpoint {
                settings.ratelimits.auth_login_checkpoint = auth_login_checkpoint;
            }
            if let Some(auth_login_checkpoint_email) = ratelimits.auth_login_checkpoint_email {
                settings.ratelimits.auth_login_checkpoint_email = auth_login_checkpoint_email;
            }
            if let Some(auth_login_security_key) = ratelimits.auth_login_security_key {
                settings.ratelimits.auth_login_security_key = auth_login_security_key;
            }
            if let Some(auth_password_forgot) = ratelimits.auth_password_forgot {
                settings.ratelimits.auth_password_forgot = auth_password_forgot;
            }
            if let Some(auth_password_reset) = ratelimits.auth_password_reset {
                settings.ratelimits.auth_password_reset = auth_password_reset;
            }
            if let Some(auth_email_verification) = ratelimits.auth_email_verification {
                settings.ratelimits.auth_email_verification = auth_email_verification;
            }
            if let Some(client) = ratelimits.client {
                settings.ratelimits.client = client;
            }
            if let Some(client_account_email_resend_verification) =
                ratelimits.client_account_email_resend_verification
            {
                settings.ratelimits.client_account_email_resend_verification =
                    client_account_email_resend_verification;
            }
            if let Some(client_servers_backups_create) = ratelimits.client_servers_backups_create {
                settings.ratelimits.client_servers_backups_create = client_servers_backups_create;
            }
            if let Some(client_servers_files_pull) = ratelimits.client_servers_files_pull {
                settings.ratelimits.client_servers_files_pull = client_servers_files_pull;
            }
            if let Some(client_servers_files_pull_query) =
                ratelimits.client_servers_files_pull_query
            {
                settings.ratelimits.client_servers_files_pull_query =
                    client_servers_files_pull_query;
            }
            if let Some(remote) = ratelimits.remote {
                settings.ratelimits.remote = remote;
            }
            if let Some(remote_sftp_auth) = ratelimits.remote_sftp_auth {
                settings.ratelimits.remote_sftp_auth = remote_sftp_auth;
            }
            if let Some(remote_enroll) = ratelimits.remote_enroll {
                settings.ratelimits.remote_enroll = remote_enroll;
            }
            if let Some(exempt_ips) = ratelimits.exempt_ips {
                settings.ratelimits.exempt_ips = exempt_ips;
            }
            if let Some(mut exempt_api_keys) = ratelimits.exempt_api_keys {
                exempt_api_keys.sort_unstable();
                exempt_api_keys.dedup();

                let existing = UserApiKey::by_uuids(&state.database, &exempt_api_keys).await?;
                if let Some(unknown) = exempt_api_keys
                    .iter()
                    .find(|uuid| !existing.iter().any(|api_key| api_key.uuid == **uuid))
                {
                    return ApiResponse::error(format!("api key {unknown} does not exist"))
                        .with_status(StatusCode::BAD_REQUEST)
                        .ok();
                }

                settings.ratelimits.exempt_api_keys = exempt_api_keys;
            }
        }
        if let Some(updates) = data.updates {
            if let Some(wings_release_token) = updates.wings_release_token {
                settings.updates.wings_release_token = wings_release_token;
            }
            if let Some(db_agent_release_token) = updates.db_agent_release_token {
                settings.updates.db_agent_release_token = db_agent_release_token;
            }
        }

        // plugged.host: noprofit.space
        if let Some(noprofit) = data.noprofit {
            let was_connected = settings.noprofit.connected();

            if let Some(enabled) = noprofit.enabled {
                settings.noprofit.enabled = enabled;
            }
            if let Some(accounts_url) = noprofit.accounts_url {
                settings.noprofit.accounts_url = accounts_url.trim_end_matches('/').into();
            }
            if let Some(sso_client) = noprofit.sso_client {
                settings.noprofit.sso_client = sso_client;
            }
            if let Some(sso_audience) = noprofit.sso_audience {
                settings.noprofit.sso_audience = sso_audience;
            }
            if let Some(sso_callback_url) = noprofit.sso_callback_url {
                settings.noprofit.sso_callback_url = sso_callback_url;
            }
            if let Some(admins_only) = noprofit.admins_only {
                settings.noprofit.admins_only = admins_only;
            }
            if let Some(link_by_email) = noprofit.link_by_email {
                settings.noprofit.link_by_email = link_by_email;
            }
            if let Some(deleted_account_days) = noprofit.deleted_account_days {
                settings.noprofit.deleted_account_days = deleted_account_days;
            }
            let stored_secrets = &mut settings.noprofit;
            for (secret, stored) in [
                (noprofit.sso_secret, &mut stored_secrets.sso_secret),
                (noprofit.api_secret, &mut stored_secrets.api_secret),
                (noprofit.neo_secret, &mut stored_secrets.neo_secret), // plugged.host: neo
            ] {
                match secret {
                    // the placeholder the API shows a stored secret as
                    Some(Some(secret)) if secret == shared::censor::CENSORED_PLACEHOLDER => {}
                    Some(Some(secret)) if secret.trim().len() < 32 => {
                        return ApiResponse::error(
                            "the noprofit.space secrets must be at least 32 characters",
                        )
                        .with_status(StatusCode::BAD_REQUEST)
                        .ok();
                    }
                    Some(secret) => *stored = secret.map(|secret| secret.trim().into()),
                    None => {}
                }
            }

            if settings.noprofit.enabled && !settings.noprofit.connected() {
                return ApiResponse::error(
                    "noprofit.space needs its accounts URL, the sign-in client and the sign-in secret",
                )
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
            }
            // Users made by noprofit.space have no password: turning it off would lock them out,
            // the one saving this included.
            if was_connected && !settings.noprofit.connected() && !user.has_password {
                return ApiResponse::error(
                    "noprofit.space sign-in can only be turned off by an admin with a panel password",
                )
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
            }
        }

        // plugged.host: the status page
        if let Some(status) = data.status {
            if let Some(enabled) = status.enabled {
                settings.status.enabled = enabled;
            }
            let stored_status = &mut settings.status;
            for (value, stored) in [
                (status.site_url, &mut stored_status.site_url),
                (status.site_host, &mut stored_status.site_host),
                (status.panel_url, &mut stored_status.panel_url),
                (status.page_url, &mut stored_status.page_url),
            ] {
                if let Some(value) = value {
                    *stored = value
                        .map(|value| value.trim().into())
                        .filter(|value: &compact_str::CompactString| !value.is_empty());
                }
            }
            if let Some(history_days) = status.history_days {
                settings.status.history_days = history_days;
            }
            if let Some(cpu_threshold) = status.cpu_threshold {
                settings.status.cpu_threshold = cpu_threshold;
            }
        }

        // plugged.host: abuse protection
        if let Some(abuse) = data.abuse {
            if let Some(mode) = abuse.mode {
                settings.abuse.mode = mode;
            }
            match abuse.alert_webhook {
                // empty (the placeholder the API shows a stored secret as) keeps it
                Some(Some(webhook))
                    if webhook.trim().is_empty()
                        || webhook == shared::censor::CENSORED_PLACEHOLDER => {}
                Some(Some(webhook)) => {
                    if !reqwest::Url::parse(webhook.trim())
                        .is_ok_and(|url| url.scheme() == "https" && url.host().is_some())
                    {
                        return ApiResponse::error("the alert webhook must be an https url")
                            .with_status(StatusCode::BAD_REQUEST)
                            .ok();
                    }
                    settings.abuse.alert_webhook = Some(webhook.trim().into());
                }
                Some(None) => settings.abuse.alert_webhook = None,
                None => {}
            }
            if let Some(cpu_percent) = abuse.cpu_percent {
                settings.abuse.cpu_percent = cpu_percent;
            }
            if let Some(cpu_minutes) = abuse.cpu_minutes {
                settings.abuse.cpu_minutes = cpu_minutes;
            }
            if let Some(delete_after_hours) = abuse.delete_after_hours {
                settings.abuse.delete_after_hours = delete_after_hours;
            }
            if let Some(limited_at) = abuse.standing_limited_at {
                settings.abuse.standing_limited_at = limited_at;
            }
            if let Some(limited_days) = abuse.standing_limited_days {
                settings.abuse.standing_limited_days = limited_days;
            }
            if let Some(very_limited_at) = abuse.standing_very_limited_at {
                settings.abuse.standing_very_limited_at = very_limited_at;
            }
            if let Some(very_limited_days) = abuse.standing_very_limited_days {
                settings.abuse.standing_very_limited_days = very_limited_days;
            }
            if let Some(suspended_at) = abuse.standing_suspended_at {
                settings.abuse.standing_suspended_at = suspended_at;
            }
            if !settings.abuse.standing_rules().ordered() {
                return ApiResponse::error("the standing thresholds must not go down")
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }
        }

        // plugged.host: what players may create, the deployment pause
        let allowance_before = settings.games.allowance();
        if let Some(games) = data.games
            && let Err(error) = games.apply(&mut settings.games)
        {
            return ApiResponse::error(&error)
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        if let Some(placement) = data.placement {
            placement.apply(&mut settings.placement);
        }
        // plugged.host: welcome tour
        if let Some(tour) = data.tour {
            tour.apply(&mut settings.tour);
        }
        // plugged.host: asking players for a review
        if let Some(reviews) = data.reviews
            && let Err(error) = reviews.apply(&mut settings.reviews)
        {
            return ApiResponse::error(error.message())
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        // plugged.host: deleted servers' copies
        if let Some(recovery) = data.recovery {
            recovery.apply(&mut settings.recovery);
        }
        // plugged.host: credits, tokens, memberships, the Resource Store
        if let Some(economy) = data.economy
            && let Err(error) = economy.apply(&mut settings.economy)
        {
            return ApiResponse::error(error)
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        // plugged.host: the start queue
        if let Some(queue) = data.queue {
            queue.apply(&mut settings.queue);
        }
        // plugged.host: limbo
        if let Some(limbo) = data.limbo {
            limbo.apply(&mut settings.limbo);
        }
        // plugged.host: ads
        if let Some(ads) = data.ads
            && let Err(error) = ads.apply(&mut settings.ads)
        {
            return ApiResponse::error(&error)
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        // plugged.host: subdomains
        if let Some(subdomains) = data.subdomains {
            subdomains.apply(&mut settings.subdomains);
        }
        // plugged.host: Discord; a smaller Discord tier is given back by the members' servers
        let discord_tier = |settings: &shared::settings::AppSettings| {
            if settings.discord.standalone(settings.noprofit.connected()) {
                settings.discord.tier()
            } else {
                shared::allowance::Resources::default()
            }
        };
        let discord_tier_before = discord_tier(&*settings);
        let discord_provider_before = settings.discord.oauth_provider_uuid;
        if let Some(discord) = data.discord
            && let Err(error) = discord.apply(&mut settings.discord)
        {
            return ApiResponse::error(error.message())
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        let discord_tier_after = discord_tier(&*settings);
        // plugged.host: devboxes
        if let Some(devboxes) = data.devboxes {
            let panel_host = reqwest::Url::parse(&settings.app.url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_string));
            let app_name = settings.app.name.to_string();
            if let Err(error) =
                devboxes.apply(&mut settings.devboxes, panel_host.as_deref(), &app_name)
            {
                return ApiResponse::error(error.message())
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }
        }
        // plugged.host: neo
        if let Some(neo) = data.neo
            && let Err(error) = neo.apply(&mut settings.neo)
        {
            return ApiResponse::error(&error.message())
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        // plugged.host: docs/features/rux.md
        if let Some(rux) = data.rux
            && let Err(error) = rux.apply(&mut settings.rux)
        {
            return ApiResponse::error(&error.message())
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        // plugged.host: neo on Discord
        if let Some(neo_discord) = data.neo_discord {
            neo_discord.apply(&mut settings.neo_discord);
        }
        // plugged.host: the partner program
        if let Some(partners) = data.partners
            && let Err(error) = partners.apply(&mut settings.partners)
        {
            return ApiResponse::error(error)
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }

        if !settings.app.password_login_enabled {
            if !OAuthProvider::exists_usable_except(&state.database, None).await? {
                return ApiResponse::error(
                    "an enabled oauth provider is required before password login can be disabled",
                )
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
            }

            if !(user.has_security_key && settings.webauthn.enabled)
                && !UserOAuthLink::exists_usable_by_user_uuid(&state.database, user.uuid).await?
            {
                return ApiResponse::error(
                    "a link to an enabled oauth provider or a security key is required before password login can be disabled",
                )
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
            }
        }
        if settings.app.two_factor_accepted_methods.is_empty()
            && !matches!(
                settings.app.two_factor_requirement,
                shared::settings::app::TwoFactorRequirement::None
            )
        {
            return ApiResponse::error(
                "at least one two-factor method must be accepted while two-factor is required",
            )
            .with_status(StatusCode::BAD_REQUEST)
            .ok();
        }
        if matches!(settings.mail_mode, shared::settings::MailMode::None)
            && (settings.app.email_two_factor_enabled || settings.app.email_verification_required)
        {
            return ApiResponse::error(
                "email two-factor and email verification require a mail transport to be configured",
            )
            .with_status(StatusCode::BAD_REQUEST)
            .ok();
        }

        // plugged.host: a smaller allowance is given back by the servers over it
        // (docs/features/allowance-cuts.md)
        let allowance_decrease = (!settings.noprofit.connected()).then(|| {
            shared::allowance_cuts::decrease(allowance_before, settings.games.allowance())
        });

        let settings_json = settings.censored();
        settings.save().await?;

        if let Some(decrease) = allowance_decrease {
            shared::allowance_cuts::record_for_everyone(
                &state.database,
                decrease,
                shared::allowance_cuts::REASON_SETTINGS,
            )
            .await?;
        }
        // plugged.host: docs/features/discord.md
        shared::discord::members::record_tier_cut(
            &state.database,
            discord_provider_before,
            discord_tier_before,
            discord_tier_after,
        )
        .await?;

        activity_logger.log("settings:update", settings_json).await;

        ApiResponse::new_serialized(Response {}).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .routes(routes!(put::route))
        .with_state(state.clone())
}
