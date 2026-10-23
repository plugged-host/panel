use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct ResponseApp<'a> {
        url: &'a str,
        additional_urls: &'a [compact_str::CompactString],
        icon: &'a str,
        icon_light: Option<&'a str>,
        banner: Option<&'a str>,
        banner_light: Option<&'a str>,
        name: &'a str,
        language: &'a str,
        registration_enabled: bool,
        password_login_enabled: bool,
        email_two_factor_enabled: bool,
        email_verification_required: bool,
        two_factor_accepted_methods: &'a [shared::settings::app::TwoFactorMethod],
        debug: bool,
        release: &'a str,
        /// plugged.host: the theme colour, Theme v2's accent (frontend lib/accent.ts).
        theme_color: &'a str,
    }

    #[derive(ToSchema, Serialize)]
    struct ResponseWebauthn {
        enabled: bool,
        allow_discoverable: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct ResponseServer<'a> {
        max_file_manager_view_size: u64,
        max_file_manager_content_search_size: u64,
        max_file_manager_search_results: u64,
        max_subuser_count: u64,
        max_schedule_step_count: u64,
        max_backup_group_count: u64,
        max_firewall_rule_count: u64,
        max_firewall_rule_source_count: u64,
        max_tunnel_connection_count: u64,
        max_tunnel_port_count: u64,
        max_database_instance_database_count: u64,
        max_database_instance_user_count: u64,

        allow_overwriting_custom_docker_image: bool,
        allow_acknowledging_installation_failure: bool,
        allow_changing_trash_settings: bool,

        container_prelude: &'a str,
    }

    #[derive(ToSchema, Serialize)]
    struct ResponseUser<'a> {
        max_server_group_count: u64,
        max_api_key_count: u64,
        max_command_snippet_count: u64,
        max_security_key_count: u64,
        max_ssh_key_count: u64,

        allow_changing_language: bool,
        allow_changing_avatar: bool,

        route_order: Option<&'a [shared::settings::RouteOrderItem]>,
    }

    /// plugged.host: noprofit.space sign-in, when it is set up.
    #[derive(ToSchema, Serialize)]
    struct ResponseNoprofit<'a> {
        enabled: bool,
        accounts_url: Option<&'a str>,
        admins_only: bool,
        /// SFTP passwords are checked by noprofit.space (its account API is set up).
        sftp_passwords: bool,
    }

    /// plugged.host: what players may do with servers themselves.
    #[derive(ToSchema, Serialize)]
    struct ResponseGames {
        create_enabled: bool,
        allow_resizing: bool,
        allow_deleting: bool,
    }

    #[derive(ToSchema, Serialize)]
    struct Response<'a> {
        time: chrono::DateTime<chrono::Utc>,
        oobe_step: Option<&'a str>,
        disabled_extensions: Vec<compact_str::CompactString>,

        #[schema(inline)]
        captcha_provider: shared::settings::PublicCaptchaProvider<'a>,
        #[schema(inline)]
        app: ResponseApp<'a>,
        #[schema(inline)]
        webauthn: ResponseWebauthn,
        #[schema(inline)]
        server: ResponseServer<'a>,
        #[schema(inline)]
        user: ResponseUser<'a>,
        #[schema(inline)]
        noprofit: ResponseNoprofit<'a>,
        #[schema(inline)]
        games: ResponseGames, // plugged.host
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        let settings = state.settings.get().await?;
        // plugged.host: noprofit.space owns sign-in when connected
        let noprofit_connected = settings.noprofit.connected();

        ApiResponse::new_serialized(Response {
            time: chrono::Utc::now(),
            oobe_step: settings.oobe_step.as_deref(),
            disabled_extensions: state.extensions.inactive(),
            captcha_provider: settings.captcha_provider.to_public_provider(),
            app: ResponseApp {
                url: &settings.app.url,
                additional_urls: &settings.app.additional_urls,
                icon: &settings.app.icon,
                icon_light: settings.app.icon_light.as_deref(),
                banner: settings.app.banner.as_deref(),
                banner_light: settings.app.banner_light.as_deref(),
                name: &settings.app.name,
                language: &settings.app.language,
                registration_enabled: settings.app.registration_enabled && !noprofit_connected,
                password_login_enabled: settings.app.password_login_enabled,
                email_two_factor_enabled: settings.app.email_two_factor_enabled,
                email_verification_required: settings.app.email_verification_required,
                two_factor_accepted_methods: &settings.app.two_factor_accepted_methods,
                debug: state.env.is_debug(),
                release: shared::RELEASE,
                theme_color: &settings.metadata.theme_color,
            },
            webauthn: ResponseWebauthn {
                // plugged.host: no security keys while noprofit.space owns sign-in
                enabled: settings.webauthn.enabled && !noprofit_connected,
                allow_discoverable: settings.webauthn.allow_discoverable,
            },
            server: ResponseServer {
                max_file_manager_view_size: settings.server.max_file_manager_view_size,
                max_file_manager_content_search_size: settings
                    .server
                    .max_file_manager_content_search_size,
                max_file_manager_search_results: settings.server.max_file_manager_search_results,
                max_subuser_count: settings.server.max_subuser_count,
                max_schedule_step_count: settings.server.max_schedule_step_count,
                max_backup_group_count: settings.server.max_backup_group_count,
                max_firewall_rule_count: settings.server.max_firewall_rule_count,
                max_firewall_rule_source_count: settings.server.max_firewall_rule_source_count,
                max_tunnel_connection_count: settings.server.max_tunnel_connection_count,
                max_tunnel_port_count: settings.server.max_tunnel_port_count,
                max_database_instance_database_count: settings
                    .server
                    .max_database_instance_database_count,
                max_database_instance_user_count: settings.server.max_database_instance_user_count,

                allow_overwriting_custom_docker_image: settings
                    .server
                    .allow_overwriting_custom_docker_image,
                allow_acknowledging_installation_failure: settings
                    .server
                    .allow_acknowledging_installation_failure,
                allow_changing_trash_settings: settings.server.allow_changing_trash_settings,
                container_prelude: &settings.server.container_prelude,
            },
            user: ResponseUser {
                max_server_group_count: settings.user.max_server_group_count,
                max_api_key_count: settings.user.max_api_key_count,
                max_command_snippet_count: settings.user.max_command_snippet_count,
                max_security_key_count: settings.user.max_security_key_count,
                max_ssh_key_count: settings.user.max_ssh_key_count,

                allow_changing_language: settings.user.allow_changing_language,
                allow_changing_avatar: settings.user.allow_changing_avatar,

                route_order: settings.user.route_order.as_deref(),
            },
            noprofit: ResponseNoprofit {
                enabled: noprofit_connected,
                accounts_url: noprofit_connected
                    .then(|| settings.noprofit.accounts_url.trim_end_matches('/')),
                admins_only: noprofit_connected && settings.noprofit.admins_only,
                sftp_passwords: settings
                    .noprofit
                    .config()
                    .is_some_and(|noprofit| noprofit.sync_enabled()),
            },
            // plugged.host
            games: ResponseGames {
                create_enabled: settings.games.create_enabled,
                allow_resizing: settings.games.allow_resizing,
                allow_deleting: settings.games.allow_deleting,
            },
        })
        .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
