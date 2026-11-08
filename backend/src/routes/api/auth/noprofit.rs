//! plugged.host: signing in with noprofit.space (see `shared::noprofit`).
//!
//! `GET /api/auth/noprofit/redirect` sends the browser to noprofit.space with a fresh state (kept
//! in the cache and a cookie for ten minutes); noprofit.space signs the person in and sends them
//! to `GET /api/auth/noprofit/callback?token=…`, which checks the token and its state, finds or
//! makes the account's panel user and starts a session. Every failure lands back on the login
//! page with an `?error=` code the page explains.

use super::State;
use axum::{
    body::Body,
    extract::Query,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use compact_str::ToCompactString;
use serde::Deserialize;
use shared::{
    GetState,
    models::{
        ByUuid, CreatableModel,
        user::{CreateUserOptions, User},
        user_activity::{CreateUserActivityOptions, UserActivity},
        user_session::{CreateUserSessionOptions, UserSession},
    },
    noprofit::{Link, Snapshot},
    response::{ApiResponse, ApiResponseResult},
};
use tower_cookies::{Cookie, Cookies, cookie::SameSite};
use utoipa_axum::router::OpenApiRouter;

const STATE_COOKIE: &str = "noprofit_state";
const COOKIE_PATH: &str = "/api/auth/noprofit";
const STATE_TTL: u64 = 10 * 60;

fn redirect(location: impl AsRef<str>) -> ApiResponseResult {
    ApiResponse::new(Body::empty())
        .with_header("Location", location.as_ref())
        .with_status(StatusCode::TEMPORARY_REDIRECT)
        .ok()
}

/// Route layer for the panel's own OAuth providers and security keys: off while noprofit.space
/// owns sign-in (the settings keep them, for when it is turned off again).
pub(crate) async fn off_while_connected(
    state: GetState,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    match state.settings.get_as(|s| s.noprofit.connected()).await {
        Ok(false) => next.run(req).await,
        Ok(true) => ApiResponse::error("sign-in is managed by noprofit.space")
            .with_status(StatusCode::NOT_FOUND)
            .into_response(),
        Err(err) => ApiResponse::from(err).into_response(),
    }
}

fn not_set_up() -> ApiResponseResult {
    ApiResponse::error("noprofit.space sign-in is not set up")
        .with_status(StatusCode::NOT_FOUND)
        .ok()
}

fn user_agent(headers: &HeaderMap) -> compact_str::CompactString {
    headers
        .get("User-Agent")
        .map(|ua| shared::utils::slice_up_to(ua.to_str().unwrap_or("unknown"), 255))
        .unwrap_or("unknown")
        .into()
}

async fn redirect_route(
    state: GetState,
    ip: shared::GetIp,
    request_host: shared::GetRequestHost,
    cookies: Cookies,
) -> ApiResponseResult {
    let settings = state.settings.get().await?;
    let Some(noprofit) = settings.noprofit.config() else {
        return not_set_up();
    };
    let app_url = settings
        .app
        .url_for_host(request_host.as_deref())
        .trim_end_matches('/')
        .to_compact_string();
    drop(settings);

    state
        .cache
        .ratelimit("auth/noprofit/redirect", 20, 300, ip.to_string())
        .await?;

    let callback = noprofit
        .callback_url
        .clone()
        .unwrap_or_else(|| format!("{app_url}{COOKIE_PATH}/callback"));
    let sso_state = shared::noprofit::new_state();

    state
        .cache
        .set(&format!("noprofit_sso::{sso_state}"), STATE_TTL, &true)
        .await?;

    cookies.add(
        Cookie::build((STATE_COOKIE, sso_state.clone()))
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(app_url.starts_with("https://"))
            .path(COOKIE_PATH)
            .max_age(tower_cookies::cookie::time::Duration::minutes(10))
            .build(),
    );

    redirect(noprofit.sso_url(&callback, &sso_state))
}

#[derive(Deserialize)]
struct CallbackParams {
    token: Option<String>,
}

/// The account's panel user: the linked one, or (with "link by username" on) an unlinked user
/// with its username, or a new one. `Err` is the login page's error code.
async fn resolve_user(
    state: &shared::State,
    noprofit: &shared::noprofit::Config,
    snapshot: &Snapshot,
    cookies: &Cookies,
    ip: std::net::IpAddr,
) -> Result<Result<User, &'static str>, anyhow::Error> {
    if let Some(link) = Link::by_account_id(&state.database, snapshot.account_id).await?
        && let Some(user) = User::by_uuid_optional(&state.database, link.user_uuid).await?
    {
        return Ok(Ok(user));
    }

    let display_name = snapshot
        .display_name
        .as_deref()
        .unwrap_or(&snapshot.username);
    if let Some(user) = User::by_username(&state.database, &snapshot.username).await? {
        if !noprofit.link_by_username
            || Link::by_user_uuid(&state.database, user.uuid)
                .await?
                .is_some()
        {
            return Ok(Err("noprofit_username_taken"));
        }

        return Ok(Ok(user));
    }

    let (name_first, name_last) = shared::noprofit::split_name(display_name);
    let language = state.settings.get_as(|s| s.app.language.clone()).await?;
    let username = shared::noprofit::unique_username(&state.database, &snapshot.username).await?;
    let external_id = compact_str::format_compact!("noprofit:{}", snapshot.account_id);

    let options = CreateUserOptions {
        role_uuid: None,
        external_id: Some(external_id),
        username: username.into(),
        name_first,
        name_last,
        password: None,
        admin: snapshot.is_admin,
        frozen: false,
        suspended: false,
        verify_email: true,
        send_email: false,
        language,
    };

    let user = match User::create(state, options).await {
        Ok(user) => user,
        Err(err) if err.is_unique_violation() => return Ok(Err("noprofit_username_taken")),
        Err(err) => return Err(err.into()),
    };

    // No password here: a noprofit.space account always signs in there.
    sqlx::query("UPDATE users SET password_login_disabled = TRUE WHERE uuid = $1")
        .bind(user.uuid)
        .execute(state.database.write())
        .await?;
    User::invalidate_cached(&state.database, user.uuid).await;
    // who brought the new account (docs/features/referrals.md)
    shared::referrals::record(state, user.uuid, cookies, ip).await;

    Ok(Ok(user))
}

async fn callback_route(
    state: GetState,
    ip: shared::GetIp,
    request_host: shared::GetRequestHost,
    headers: HeaderMap,
    cookies: Cookies,
    Query(params): Query<CallbackParams>,
) -> ApiResponseResult {
    let Some(noprofit) = state.settings.get().await?.noprofit.config() else {
        return not_set_up();
    };

    state
        .cache
        .ratelimit("auth/noprofit/callback", 20, 300, ip.to_string())
        .await?;

    let settings = state.settings.get().await?;
    let app_url = settings
        .app
        .url_for_host(request_host.as_deref())
        .trim_end_matches('/')
        .to_compact_string();
    drop(settings);

    let fail = |code: &str| redirect(format!("{app_url}/auth/login?error={code}"));

    let state_cookie = cookies
        .get(STATE_COOKIE)
        .map(|cookie| cookie.value().to_owned());
    cookies.remove(Cookie::build(STATE_COOKIE).path(COOKIE_PATH).build());

    let Some(token) = params.token else {
        return fail("noprofit_failed");
    };

    let claims = match noprofit.decode_token(&token) {
        Ok(claims) => claims,
        Err(err) => {
            tracing::info!(ip = %ip.0, "refused a noprofit.space sign-in: {err}");
            return fail("noprofit_failed");
        }
    };

    // The token must carry the state this browser started with, once.
    let state_key = format!("noprofit_sso::{}", claims.state);
    if state_cookie.as_deref() != Some(claims.state.as_str())
        || !state.cache.exists(&state_key).await?
    {
        return fail("noprofit_expired");
    }
    state.cache.invalidate(&state_key).await?;

    let nonce_key = format!("noprofit_sso_nonce::{}", claims.nonce);
    if state.cache.exists(&nonce_key).await? {
        return fail("noprofit_expired");
    }
    state.cache.set(&nonce_key, STATE_TTL, &true).await?;

    let snapshot = claims.snapshot;

    if noprofit.admins_only && !snapshot.is_admin {
        return fail("noprofit_admins_only");
    }

    let mut user = match resolve_user(&state, &noprofit, &snapshot, &cookies, ip.0).await? {
        Ok(user) => user,
        Err(code) => return fail(code),
    };

    shared::noprofit::apply_snapshot(&state, &mut user, &snapshot).await?;

    let key = UserSession::create(
        &state,
        CreateUserSessionOptions {
            user_uuid: user.uuid,
            ip: ip.0.into(),
            user_agent: user_agent(&headers),
        },
    )
    .await?;
    cookies.add(UserSession::get_cookie(&state, request_host.as_deref(), key).await?);

    if let Err(err) = UserActivity::create(
        &state,
        CreateUserActivityOptions {
            user_uuid: user.uuid,
            impersonator_uuid: None,
            api_key_uuid: None,
            event: "auth:success".into(),
            ip: Some(ip.0.into()),
            data: serde_json::json!({
                "using": "noprofit.space",
                "account_id": snapshot.account_id,
                "user_agent": user_agent(&headers),
            }),
            created: None,
        },
    )
    .await
    {
        tracing::warn!(user = %user.uuid, "failed to log user activity: {:#?}", err);
    }

    redirect(&*app_url)
}

/// plugged.host: the desktop app's sign-in. The app opens its browser on
/// `/api/auth/noprofit/app/redirect`, accounts.noprofit.space sends the signed token to the app's
/// loopback listener, and the app trades it here for a panel session. The state is one-shot and
/// lives in the cache; the token is the same signed noprofit.space token the browser flow uses.
mod app_redirect {
    use super::State;
    use axum::{extract::Query, http::StatusCode};
    use serde::Deserialize;
    use shared::{
        GetState,
        response::{ApiResponse, ApiResponseResult},
    };
    use utoipa_axum::{router::OpenApiRouter, routes};

    #[derive(Deserialize)]
    pub struct Params {
        port: u16,
        state: String,
    }

    #[utoipa::path(get, path = "/redirect", responses((status = TEMPORARY_REDIRECT)))]
    pub async fn route(
        state: GetState,
        ip: shared::GetIp,
        Query(params): Query<Params>,
    ) -> ApiResponseResult {
        let settings = state.settings.get().await?;
        let Some(noprofit) = settings.noprofit.config() else {
            return super::not_set_up();
        };
        if !(1024..=65535).contains(&params.port)
            || !(32..=128).contains(&params.state.len())
            || !params.state.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return ApiResponse::error("invalid app sign-in request")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        state
            .cache
            .ratelimit("auth/noprofit/app/redirect", 20, 300, ip.to_string())
            .await?;
        let callback = format!(
            "http://127.0.0.1:{}/callback?state={}",
            params.port, params.state
        );
        state
            .cache
            .set(&format!("noprofit_app::{}", params.state), 600, &true)
            .await?;
        let url = format!(
            "{}/sso?client=plugged-app&return_to={}&state={}",
            noprofit.accounts_url,
            urlencoding::encode(&callback),
            urlencoding::encode(&params.state),
        );
        super::redirect(url)
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(route))
            .with_state(state.clone())
    }
}

mod app_exchange {
    use super::State;
    use axum::http::StatusCode;
    use serde::{Deserialize, Serialize};
    use shared::{
        GetState,
        models::{
            CreatableModel,
            user_activity::{CreateUserActivityOptions, UserActivity},
            user_session::{CreateUserSessionOptions, UserSession},
        },
        response::{ApiResponse, ApiResponseResult},
    };
    use tower_cookies::Cookies;
    use utoipa::ToSchema;
    use utoipa_axum::{router::OpenApiRouter, routes};

    #[derive(ToSchema, Deserialize)]
    pub struct Payload {
        token: String,
        state: String,
    }

    #[derive(ToSchema, Serialize)]
    struct Response {
        user: shared::models::user::ApiFullUser,
    }

    #[utoipa::path(post, path = "/exchange", responses((status = OK, body = inline(Response))))]
    pub async fn route(
        state: GetState,
        ip: shared::GetIp,
        request_host: shared::GetRequestHost,
        headers: axum::http::HeaderMap,
        cookies: Cookies,
        shared::Payload(data): shared::Payload<Payload>,
    ) -> ApiResponseResult {
        let settings = state.settings.get().await?;
        let Some(noprofit) = settings.noprofit.config() else {
            return super::not_set_up();
        };
        drop(settings);

        let cache_key = format!("noprofit_app::{}", data.state);
        if state.cache.get::<bool>(&cache_key).await?.is_none() {
            return ApiResponse::error("this sign-in expired; try again")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        state.cache.invalidate(&cache_key).await?;

        let claims = match noprofit.decode_token(&data.token) {
            Ok(claims) => claims,
            Err(err) => {
                tracing::info!(ip = %ip.0, "refused a desktop app sign-in: {err}");
                return ApiResponse::error("noprofit.space sign-in failed")
                    .with_status(StatusCode::BAD_REQUEST)
                    .ok();
            }
        };
        if claims.state != data.state {
            return ApiResponse::error("this sign-in does not match its request")
                .with_status(StatusCode::BAD_REQUEST)
                .ok();
        }
        if noprofit.admins_only && !claims.snapshot.is_admin {
            return ApiResponse::error("only noprofit.space admins may sign in right now")
                .with_status(StatusCode::FORBIDDEN)
                .ok();
        }

        let user =
            match super::resolve_user(&state, &noprofit, &claims.snapshot, &cookies, ip.0).await? {
                Ok(user) => user,
                Err(code) => {
                    return ApiResponse::error(code)
                        .with_status(StatusCode::BAD_REQUEST)
                        .ok();
                }
            };

        let key = UserSession::create(
            &state,
            CreateUserSessionOptions {
                user_uuid: user.uuid,
                ip: ip.0.into(),
                user_agent: headers
                    .get("User-Agent")
                    .map(|ua| shared::utils::slice_up_to(ua.to_str().unwrap_or("unknown"), 255))
                    .unwrap_or("unknown")
                    .into(),
            },
        )
        .await?;
        cookies.add(UserSession::get_cookie(&state, request_host.as_deref(), key).await?);

        if let Err(err) = UserActivity::create(
            &state,
            CreateUserActivityOptions {
                user_uuid: user.uuid,
                impersonator_uuid: None,
                api_key_uuid: None,
                event: "auth:success".into(),
                ip: Some(ip.0.into()),
                data: serde_json::json!({
                    "using": "noprofit.space",
                    "client": "desktop-app",
                    "user_agent": headers
                        .get("User-Agent")
                        .map(|ua| shared::utils::slice_up_to(ua.to_str().unwrap_or("unknown"), 255))
                        .unwrap_or("unknown"),
                }),
                created: None,
            },
        )
        .await
        {
            tracing::warn!(user = %user.uuid, "failed to log user activity: {:#?}", err);
        }

        ApiResponse::new_serialized(Response {
            user: user
                .into_api_full_object(&state, &state.storage.retrieve_urls().await?)
                .await?,
        })
        .ok()
    }

    pub fn router(state: &State) -> OpenApiRouter<State> {
        OpenApiRouter::new()
            .routes(routes!(route))
            .with_state(state.clone())
    }
}

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .route("/redirect", get(redirect_route))
        .route("/callback", get(callback_route))
        .nest(
            "/app",
            app_redirect::router(state).merge(app_exchange::router(state)),
        )
        .with_state(state.clone())
}
