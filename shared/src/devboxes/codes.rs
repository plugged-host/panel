//! plugged.host: connect codes and the browser version's tickets and sessions
//! (docs/features/devboxes.md).
//!
//! A connect code is what a person types as the SSH password for a devbox: shown in the panel
//! (behind its sign-in and 2FA), one per person per start, derived from the start's code secret,
//! so a stop or a new start retires every code at once. Account passwords are never accepted over
//! SSH: they would skip 2FA and pass every guess a bot makes on to the account.
//!
//! The browser version: "Open in browser" makes a one-time ticket for a devbox, its person and
//! its host; the edge redeems it on the devbox's own host for a host-only cookie
//! ([`COOKIE`], an encrypted [`Session`]) that the edge checks on every request.

use super::Devbox;
use hmac::{Hmac, KeyInit, Mac};
use rand::distr::SampleString;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
const TICKET_LENGTH: usize = 48;
const TICKET_SECONDS: u64 = 60;
/// How long a browser session on a devbox host lasts.
pub const SESSION_HOURS: i64 = 12;
/// The edge's cookie on a devbox host: `__Host-` keeps it on that host alone.
pub const COOKIE: &str = "__Host-devbox";

/// `xxxx-xxxx-xxxx-xxxx` from 31 unambiguous characters (about 79 bits), the same for a person
/// for the whole start.
pub fn derive(secret: &str, devbox_uuid: uuid::Uuid, user_uuid: uuid::Uuid) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac accepts keys of any length");
    mac.update(format!("connect:{devbox_uuid}:{user_uuid}").as_bytes());
    let digest = mac.finalize().into_bytes();

    let mut value = u128::from_be_bytes(digest[..16].try_into().expect("a sha256 is 32 bytes"));
    let mut code = String::with_capacity(19);
    for i in 0..16 {
        if i > 0 && i % 4 == 0 {
            code.push('-');
        }
        code.push(ALPHABET[(value % ALPHABET.len() as u128) as usize] as char);
        value /= ALPHABET.len() as u128;
    }

    code
}

/// The person's code for the devbox's current start, `None` while it isn't running.
pub async fn code_for(
    state: &crate::State,
    devbox: &Devbox,
    user_uuid: uuid::Uuid,
) -> Result<Option<String>, anyhow::Error> {
    let Some(encrypted) = devbox
        .code_secret
        .as_deref()
        .filter(|_| devbox.is_running())
    else {
        return Ok(None);
    };
    let secret = state.database.decrypt_base64(encrypted).await?;

    Ok(Some(derive(&secret, devbox.uuid, user_uuid)))
}

pub async fn check(
    state: &crate::State,
    devbox: &Devbox,
    user_uuid: uuid::Uuid,
    code: &str,
) -> bool {
    match code_for(state, devbox, user_uuid).await {
        Ok(Some(expected)) => equal(
            expected.as_bytes(),
            code.trim().to_ascii_lowercase().as_bytes(),
        ),
        _ => false,
    }
}

/// Compares in constant time for inputs of the same length.
fn equal(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub devbox: uuid::Uuid,
    pub user: uuid::Uuid,
    pub host: String,
}

fn ticket_key(ticket: &str) -> String {
    format!(
        "devboxes::ticket::{}",
        hex::encode(Sha256::digest(ticket.as_bytes()))
    )
}

/// A one-time ticket, good for a minute.
pub async fn ticket(state: &crate::State, ticket: &Ticket) -> Result<String, anyhow::Error> {
    let value = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), TICKET_LENGTH);
    state
        .cache
        .set(&ticket_key(&value), TICKET_SECONDS, ticket)
        .await?;

    Ok(value)
}

/// The ticket's contents, once.
pub async fn redeem(state: &crate::State, value: &str) -> Option<Ticket> {
    if value.len() != TICKET_LENGTH || !value.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }

    let key = ticket_key(value);
    let ticket = state.cache.get::<Ticket>(&key).await.ok().flatten()?;
    state.cache.invalidate(&key).await.ok()?;

    Some(ticket)
}

/// What the edge's cookie holds, encrypted.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Session {
    #[serde(rename = "d")]
    pub devbox: uuid::Uuid,
    #[serde(rename = "u")]
    pub user: uuid::Uuid,
    #[serde(rename = "h")]
    pub host: String,
    /// Unix seconds.
    #[serde(rename = "e")]
    pub expires: i64,
}

impl Session {
    pub fn new(ticket: &Ticket) -> Self {
        Self {
            devbox: ticket.devbox,
            user: ticket.user,
            host: ticket.host.clone(),
            expires: (chrono::Utc::now() + chrono::Duration::hours(SESSION_HOURS)).timestamp(),
        }
    }

    pub async fn seal(&self, state: &crate::State) -> Result<String, anyhow::Error> {
        Ok(state
            .database
            .encrypt_base64(serde_json::to_vec(self)?)
            .await?
            .to_string())
    }

    /// The session in a cookie, when it is for `host` and not expired.
    pub async fn open(state: &crate::State, cookie: &str, host: &str) -> Option<Self> {
        if cookie.is_empty() || cookie.len() > 4096 {
            return None;
        }

        let raw = state.database.decrypt_base64_raw(cookie).await.ok()?;
        let session: Self = serde_json::from_slice(&raw).ok()?;

        session
            .valid_for(host, chrono::Utc::now().timestamp())
            .then_some(session)
    }

    fn valid_for(&self, host: &str, now: i64) -> bool {
        self.host == host && self.expires > now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_per_person_and_start() {
        let devbox = uuid::Uuid::from_u128(1);
        let alice = uuid::Uuid::from_u128(2);
        let bob = uuid::Uuid::from_u128(3);

        let code = derive("secret", devbox, alice);
        assert_eq!(code.len(), 19);
        assert!(
            code.split('-')
                .all(|group| group.len() == 4 && group.bytes().all(|b| ALPHABET.contains(&b)))
        );
        assert_eq!(code, derive("secret", devbox, alice), "stable for a start");
        assert_ne!(code, derive("secret", devbox, bob));
        assert_ne!(code, derive("another start", devbox, alice));
    }

    #[test]
    fn codes_compare_exactly() {
        assert!(equal(b"abcd-efgh", b"abcd-efgh"));
        assert!(!equal(b"abcd-efgh", b"abcd-efgj"));
        assert!(!equal(b"abcd", b"abcd-efgh"));
    }

    #[test]
    fn sessions_belong_to_their_host_until_they_expire() {
        let session = Session {
            devbox: uuid::Uuid::nil(),
            user: uuid::Uuid::nil(),
            host: "abc.example.net".into(),
            expires: 100,
        };

        assert!(session.valid_for("abc.example.net", 99));
        assert!(!session.valid_for("abc.example.net", 100));
        assert!(!session.valid_for("other.example.net", 99));
    }
}
