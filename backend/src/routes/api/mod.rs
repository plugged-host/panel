use super::{GetState, State};
use axum::routing::get;
use shared::response::ApiResponse;
use utoipa_axum::router::OpenApiRouter;

pub mod admin;
mod ads; // plugged.host
mod announcements;
pub mod auth;
pub mod client;
mod devbox; // plugged.host
mod icons;
mod languages;
mod noprofit; // plugged.host
pub mod remote;
mod settings;
mod timezones;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .route(
            "/",
            get(|state: GetState| async move {
                let settings = state.settings.get().await?;

                let mut environment = minijinja::Environment::new();
                environment.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
                environment.add_global(
                    "settings",
                    minijinja::Value::from(minijinja::value::Serde(&*settings)),
                );
                drop(settings);

                let api_html = environment.render_str(
                    include_str!("../../../static/api.html"),
                    minijinja::Value::default(),
                )?;

                ApiResponse::new(axum::body::Body::from(api_html))
                    .with_header("Content-Type", "text/html")
                    .ok()
            }),
        )
        .nest("/settings", settings::router(state))
        .nest("/ads", ads::router(state)) // plugged.host: docs/features/ads.md
        .nest("/announcements", announcements::router(state))
        .nest("/icons", icons::router(state))
        .nest("/languages", languages::router(state))
        .nest("/timezones", timezones::router(state))
        .nest("/auth", auth::router(state))
        .nest("/client", client::router(state))
        .nest("/admin", admin::router(state))
        .nest("/remote", remote::router(state))
        .nest("/devbox", devbox::router(state)) // plugged.host: docs/features/devboxes.md
        .nest("/noprofit", noprofit::router(state)) // plugged.host: docs/noprofit.md
        .with_state(state.clone())
}
