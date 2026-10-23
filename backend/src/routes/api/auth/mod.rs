use super::State;
use utoipa_axum::router::OpenApiRouter;

mod email;
mod login;
pub(crate) mod noprofit;
mod oauth;
mod password;
mod register;

pub fn router(state: &State) -> OpenApiRouter<State> {
    OpenApiRouter::new()
        .nest("/login", login::router(state))
        .nest("/email", email::router(state))
        .nest("/register", register::router(state))
        .nest("/password", password::router(state))
        // plugged.host: no OAuth providers while noprofit.space owns sign-in
        .nest(
            "/oauth",
            oauth::router(state).route_layer(axum::middleware::from_fn_with_state(
                state.clone(),
                noprofit::off_while_connected,
            )),
        )
        .nest("/noprofit", noprofit::router(state))
        .with_state(state.clone())
}
