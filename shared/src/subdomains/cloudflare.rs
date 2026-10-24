//! plugged.host: the few Cloudflare API calls subdomains need: a zone's id, and listing,
//! creating, changing and deleting DNS records.

use serde::{Deserialize, de::DeserializeOwned};

const API: &str = "https://api.cloudflare.com/client/v4";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Deserialize)]
struct Envelope<T> {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    errors: Vec<Message>,
    result: Option<T>,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct Zone {
    id: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Record {
    pub id: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub proxied: Option<bool>,
}

#[derive(Debug)]
pub struct CloudflareError {
    pub message: String,
    /// Cloudflare's error code, when it gave one (81044: the record does not exist).
    pub code: Option<i64>,
}

impl std::fmt::Display for CloudflareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl CloudflareError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
        }
    }

    /// The record id is not (or no longer) in the zone.
    pub fn missing(&self) -> bool {
        matches!(self.code, Some(81044 | 1032))
    }
}

pub struct Cloudflare<'a> {
    client: &'a reqwest::Client,
    token: &'a str,
}

impl<'a> Cloudflare<'a> {
    pub fn new(client: &'a reqwest::Client, token: &'a str) -> Self {
        Self { client, token }
    }

    async fn call<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, CloudflareError> {
        let response = request
            .bearer_auth(self.token)
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|err| CloudflareError::new(format!("Cloudflare did not answer: {err}")))?;
        let status = response.status();
        let envelope: Envelope<T> = response.json().await.map_err(|_| {
            CloudflareError::new(format!("Cloudflare answered {status} without its JSON"))
        })?;

        match (envelope.success, envelope.result) {
            (true, Some(result)) => Ok(result),
            _ => {
                let first = envelope.errors.into_iter().next();

                Err(CloudflareError {
                    message: first
                        .as_ref()
                        .map(|error| error.message.clone())
                        .filter(|message| !message.is_empty())
                        .unwrap_or_else(|| format!("Cloudflare refused the request ({status})")),
                    code: first.map(|error| error.code),
                })
            }
        }
    }

    /// The id of the zone named `name`, `None` when the token sees no such zone.
    pub async fn zone_id(&self, name: &str) -> Result<Option<String>, CloudflareError> {
        let zones: Vec<Zone> = self
            .call(
                self.client
                    .get(format!("{API}/zones"))
                    .query(&[("name", name)]),
            )
            .await?;

        Ok(zones.into_iter().next().map(|zone| zone.id))
    }

    /// The zone's records named exactly `name`, of `kind` when given.
    pub async fn records(
        &self,
        zone: &str,
        name: &str,
        kind: Option<&str>,
    ) -> Result<Vec<Record>, CloudflareError> {
        let mut query = vec![("name", name), ("per_page", "100")];
        if let Some(kind) = kind {
            query.push(("type", kind));
        }

        self.call(
            self.client
                .get(format!("{API}/zones/{zone}/dns_records"))
                .query(&query),
        )
        .await
    }

    pub async fn create(
        &self,
        zone: &str,
        record: &serde_json::Value,
    ) -> Result<Record, CloudflareError> {
        self.call(
            self.client
                .post(format!("{API}/zones/{zone}/dns_records"))
                .json(record),
        )
        .await
    }

    pub async fn update(
        &self,
        zone: &str,
        id: &str,
        record: &serde_json::Value,
    ) -> Result<Record, CloudflareError> {
        self.call(
            self.client
                .patch(format!("{API}/zones/{zone}/dns_records/{id}"))
                .json(record),
        )
        .await
    }

    pub async fn delete(&self, zone: &str, id: &str) -> Result<(), CloudflareError> {
        match self
            .call::<serde_json::Value>(
                self.client
                    .delete(format!("{API}/zones/{zone}/dns_records/{id}")),
            )
            .await
        {
            Err(err) if err.missing() => Ok(()),
            result => result.map(|_| ()),
        }
    }
}
