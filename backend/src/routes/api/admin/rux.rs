//! plugged.host: rux's state for Admin › Settings › Accounts › rux (shared/src/rux,
//! docs/features/rux.md): the settings it runs on, what stops it, the moderation bot's
//! connection, the links that add it to its servers and the AI models it tries.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod status {
    pub mod get {
        use serde::Serialize;
        use shared::{
            GetState,
            discord::gateway::{Bot, BotStatus},
            models::user::GetPermissionManager,
            response::{ApiResponse, ApiResponseResult},
            rux::{
                Blocker,
                ai::{self, Purpose, Source},
                config::{self, Guild, Managed},
            },
            settings::rux::AppSettingsRux,
        };
        use utoipa::ToSchema;

        #[derive(ToSchema, Serialize)]
        struct Invite {
            guild_id: String,
            name: String,
            url: String,
        }

        #[derive(ToSchema, Serialize)]
        struct Response {
            /// The settings rux runs on: the panel's, or noprofit.space's laid over them.
            #[schema(inline)]
            settings: AppSettingsRux,
            /// Where the settings are kept while not here (noprofit.space).
            managed: Option<Managed>,
            /// The servers rux works on, the main one first.
            guilds: Vec<Guild>,
            /// What stops rux; empty while nothing does.
            blockers: Vec<Blocker>,
            /// Admins are noprofit.space's (else the panel's), and the page offers the secret of
            /// noprofit.space's rux routes.
            noprofit_connected: bool,
            /// The moderation bot's connection, as the primary instance last stored it.
            bot: Option<BotStatus>,
            /// The links that add the bot to each server (none without a token).
            invites: Vec<Invite>,
            /// Where the AI providers come from, and the models orders and message checks try, in
            /// order ("Label (model)").
            ai_source: Source,
            order_models: Vec<String>,
            check_models: Vec<String>,
        }

        #[utoipa::path(get, path = "/", responses(
            (status = OK, body = inline(Response)),
        ))]
        pub async fn route(
            state: GetState,
            permissions: GetPermissionManager,
        ) -> ApiResponseResult {
            permissions.has_admin_permission("settings.read")?;

            let (discord, noprofit_connected) = {
                let settings = state.settings.get().await?;
                (settings.discord.clone(), settings.noprofit.connected())
            };
            let config = config::load(&state).await?;
            let (neo, ai_source) = ai::providers(&state).await?;
            let order_models = ai::chain_labels(&neo, Purpose::Order);
            let check_models = ai::chain_labels(&neo, Purpose::Check);

            let application_id = discord
                .moderation_bot_token
                .as_deref()
                .and_then(config::application_id_of);
            let invites = match &application_id {
                Some(application_id) => config
                    .guilds
                    .iter()
                    .map(|guild| Invite {
                        guild_id: guild.id.clone(),
                        name: guild.host_name.clone(),
                        url: config::Config::invite_url(application_id, Some(&guild.id)),
                    })
                    .collect(),
                None => Vec::new(),
            };
            let bot = shared::discord::gateway::status(&state)
                .await
                .and_then(|bots| bots.into_iter().find(|bot| bot.bot == Bot::Moderation));

            ApiResponse::new_serialized(Response {
                blockers: shared::rux::blockers(
                    &discord,
                    &config,
                    noprofit_connected,
                    !order_models.is_empty(),
                ),
                noprofit_connected,
                settings: config.settings.clone(),
                managed: config.managed.clone(),
                guilds: config.guilds.clone(),
                bot,
                invites,
                ai_source,
                order_models,
                check_models,
            })
            .ok()
        }
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest(
            "/status",
            OpenApiRouter::new()
                .routes(routes!(status::get::route))
                .with_state(state.clone()),
        )
        .with_state(state.clone())
}
