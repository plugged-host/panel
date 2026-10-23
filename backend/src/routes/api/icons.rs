use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use serde::Serialize;
    use shared::{
        GetState,
        icons::ApiIcon,
        models::{egg_configuration::EggConfiguration, oauth_provider::OAuthProvider},
        response::{ApiResponse, ApiResponseResult},
    };
    use std::collections::{BTreeMap, BTreeSet};
    use utoipa::ToSchema;

    #[derive(ToSchema, Serialize)]
    struct Response {
        icons: BTreeMap<compact_str::CompactString, ApiIcon>,
    }

    #[utoipa::path(get, path = "/", responses(
        (status = OK, body = inline(Response)),
    ))]
    pub async fn route(state: GetState) -> ApiResponseResult {
        let icons = state
            .cache
            .cached(shared::icons::REFERENCED_ICONS_CACHE_KEY, 60, || async {
                let mut identifiers = BTreeSet::new();

                let settings = state.settings.get().await?;
                if let Some(route_order) = &settings.user.route_order {
                    identifiers.extend(
                        route_order
                            .iter()
                            .filter_map(|item| item.icon())
                            .map(compact_str::CompactString::from),
                    );
                }
                drop(settings);

                for egg_configuration in EggConfiguration::all_cached(&state.database).await? {
                    if let Some(config_routes) = &egg_configuration.config_routes {
                        identifiers.extend(
                            config_routes
                                .order
                                .iter()
                                .filter_map(|item| item.icon())
                                .map(compact_str::CompactString::from),
                        );
                    }
                }

                let oauth_providers = state
                    .cache
                    .cached("oauth_providers::usable", 60, || async {
                        OAuthProvider::all_by_usable(&state.database).await
                    })
                    .await?;
                identifiers.extend(
                    oauth_providers
                        .into_iter()
                        .filter_map(|oauth_provider| oauth_provider.icon),
                );

                Ok::<_, anyhow::Error>(shared::icons::resolve_icons(
                    identifiers.iter().map(|identifier| identifier.as_str()),
                ))
            })
            .await?;

        ApiResponse::new_serialized(Response { icons }).ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
