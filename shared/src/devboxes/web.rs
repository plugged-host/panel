//! plugged.host: VS Code in the browser (docs/features/devboxes.md): which devbox a host of the
//! web domain is, whether a person may use it there, where the edge finds it, and the one-time
//! links that sign a browser in on its host.

use super::{Context, Devbox, codes, view};
use crate::models::{ByUuid, server::Server};

/// Where the edge serves the panel's login on a devbox host (its nginx site maps it to
/// `/api/devbox/web/login`).
pub const LOGIN_PATH: &str = "/_devbox/login";

/// The devbox for a host of the web domain.
pub async fn devbox_for_host(
    state: &crate::State,
    context: &Context,
    host: &str,
) -> Result<Option<Devbox>, sqlx::Error> {
    match slug_of(host, &context.devboxes.web_domain) {
        Some(slug) => Devbox::by_slug(&state.database, slug).await,
        None => Ok(None),
    }
}

fn slug_of<'a>(host: &'a str, domain: &str) -> Option<&'a str> {
    if domain.is_empty() {
        return None;
    }

    host.strip_suffix(domain)
        .and_then(|rest| rest.strip_suffix('.'))
        .filter(|slug| {
            (8..=24).contains(&slug.len())
                && slug
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// The person may use the running devbox in the browser right now.
pub async fn allowed(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
    user_uuid: uuid::Uuid,
) -> Result<bool, anyhow::Error> {
    if !context.ready() || !context.web_enabled() || !devbox.is_running() {
        return Ok(false);
    }
    let Some(game) = Server::by_uuid_optional(&state.database, devbox.game_server_uuid).await?
    else {
        return Ok(false);
    };

    Ok(super::people(state, context, devbox, &game)
        .await?
        .iter()
        .any(|person| person.uuid == user_uuid))
}

/// `host:port` the edge proxies to, `None` while the devbox has no server.
pub async fn upstream(
    state: &crate::State,
    devbox: &Devbox,
) -> Result<Option<String>, anyhow::Error> {
    let Some(uuid) = devbox.server_uuid else {
        return Ok(None);
    };
    let Some(server) = Server::by_uuid_optional(&state.database, uuid).await? else {
        return Ok(None);
    };
    let node = server.node.fetch_cached(&state.database).await?;

    Ok(view::connect_host(&node, &server).map(|(host, port)| {
        if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        }
    }))
}

/// A one-time link that signs the browser in on the devbox's own host.
pub async fn login_url(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
    user_uuid: uuid::Uuid,
) -> Result<String, anyhow::Error> {
    let host = devbox.web_host(&context.devboxes);
    let ticket = codes::ticket(
        state,
        &codes::Ticket {
            devbox: devbox.uuid,
            user: user_uuid,
            host: host.clone(),
        },
    )
    .await?;

    Ok(format!("https://{host}{LOGIN_PATH}?ticket={ticket}"))
}

/// Where a browser without a valid cookie on a devbox host goes, after the panel's sign-in.
pub enum Reopen {
    /// A fresh login link.
    Url(String),
    /// The devbox is stopped or no longer theirs: its game server's VS Code page.
    Server(uuid::Uuid),
    /// No such devbox.
    Panel,
}

pub async fn reopen(
    state: &crate::State,
    user_uuid: uuid::Uuid,
    host: &str,
) -> Result<Reopen, anyhow::Error> {
    let context = Context::load(state).await?;
    let host = host.trim().to_ascii_lowercase();
    let Some(devbox) = devbox_for_host(state, &context, &host).await? else {
        return Ok(Reopen::Panel);
    };
    if !allowed(state, &context, &devbox, user_uuid).await? {
        return Ok(Reopen::Server(devbox.game_server_uuid));
    }

    Ok(Reopen::Url(
        login_url(state, &context, &devbox, user_uuid).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_name_a_devbox_one_level_under_the_domain() {
        assert_eq!(
            slug_of("abcdefgh12345678.example.net", "example.net"),
            Some("abcdefgh12345678")
        );
        assert_eq!(slug_of("a.b.example.net", "example.net"), None);
        assert_eq!(slug_of("short.example.net", "example.net"), None);
        assert_eq!(slug_of("abcdefgh12345678.example.org", "example.net"), None);
        assert_eq!(slug_of("abcdefgh12345678example.net", "example.net"), None);
        assert_eq!(slug_of("abcdefgh12345678.example.net", ""), None);
    }
}
