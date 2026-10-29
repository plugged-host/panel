//! plugged.host: `/r/{code}`, a referral link (shared/src/referrals.rs, docs/features/referrals.md).
//! It keeps the code in a cookie for 30 days, which the sign-up that follows reads, and goes to
//! sign-in. An unknown code, or referrals off, only goes to sign-in.

use super::State;
use utoipa_axum::{router::OpenApiRouter, routes};

mod get {
    use axum::{body::Body, extract::Path, http::StatusCode};
    use shared::{
        GetState,
        response::{ApiResponse, ApiResponseResult},
    };
    use tower_cookies::{Cookie, Cookies, cookie::SameSite};

    #[utoipa::path(get, path = "/{code}", responses(
        (status = TEMPORARY_REDIRECT),
    ), params(
        (
            "code" = String,
            description = "The referral code",
            example = "ab12cd34",
        ),
    ))]
    pub async fn route(
        state: GetState,
        request_host: shared::GetRequestHost,
        cookies: Cookies,
        Path(code): Path<String>,
    ) -> ApiResponseResult {
        let (on, app_url) = {
            let settings = state.settings.get().await?;
            (
                shared::referrals::offered(&settings.economy),
                settings
                    .app
                    .url_for_host(request_host.as_deref())
                    .trim_end_matches('/')
                    .to_string(),
            )
        };

        let code = code.trim().to_ascii_lowercase();
        if on && shared::referrals::code_exists(&state.database, &code).await? {
            cookies.add(
                Cookie::build((shared::referrals::COOKIE, code))
                    .http_only(true)
                    .same_site(SameSite::Lax)
                    .secure(app_url.starts_with("https://"))
                    .path("/")
                    .max_age(tower_cookies::cookie::time::Duration::days(
                        shared::referrals::COOKIE_DAYS,
                    ))
                    .build(),
            );
        }

        ApiResponse::new(Body::empty())
            .with_header("Location", format!("{app_url}/auth/login"))
            .with_header("Cache-Control", "no-store")
            .with_status(StatusCode::TEMPORARY_REDIRECT)
            .ok()
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .routes(routes!(get::route))
        .with_state(state.clone())
}
