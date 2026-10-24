//! plugged.host: one proxied DNS record per devbox for VS Code in the browser:
//! `<slug>.<web domain>` → the edge. One level under the domain on purpose: Cloudflare's free edge
//! certificate covers `*.<domain>`, not deeper names. No catch-all wildcard, and only records
//! carrying [`COMMENT`] are ever changed: the zone may hold other records (subdomains write to
//! user-content zones too).

use super::{Context, Devbox};
use crate::subdomains::cloudflare::Cloudflare;

/// Marks the records devboxes made.
pub const COMMENT: &str = "devbox record (managed by the panel)";
/// How long a zone id is remembered.
const ZONE_SECONDS: u64 = 86_400;

fn client<'a>(
    state: &'a crate::State,
    context: &'a Context,
) -> Result<Cloudflare<'a>, anyhow::Error> {
    let token = context
        .cloudflare_token
        .as_deref()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| anyhow::anyhow!("no Cloudflare token for the devbox domain"))?;

    Ok(Cloudflare::new(&state.client, token))
}

/// The zone holding the web domain (the domain, then its parents), remembered for a day.
async fn zone(
    state: &crate::State,
    cloudflare: &Cloudflare<'_>,
    domain: &str,
) -> Result<String, anyhow::Error> {
    let key = format!("devboxes::zone::{domain}");
    if let Ok(Some(zone)) = state.cache.get::<String>(&key).await {
        return Ok(zone);
    }

    let labels: Vec<&str> = domain.split('.').collect();
    for start in 0..labels.len().saturating_sub(1) {
        let name = labels[start..].join(".");
        if let Some(zone) = cloudflare
            .zone_id(&name)
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))?
        {
            if let Err(err) = state.cache.set(&key, ZONE_SECONDS, &zone).await {
                tracing::debug!("failed to remember a Cloudflare zone: {err:#}");
            }
            return Ok(zone);
        }
    }

    anyhow::bail!("no Cloudflare zone the token can see holds {domain}")
}

/// Makes sure the devbox's record exists and points at the edge.
pub async fn ensure(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
) -> Result<(), anyhow::Error> {
    let ip = context.devboxes.web_origin_ip.trim();
    if ip.parse::<std::net::Ipv4Addr>().is_err() {
        anyhow::bail!("the edge address is not an IPv4 address");
    }

    let name = devbox.web_host(&context.devboxes);
    let cloudflare = client(state, context)?;
    let zone = zone(state, &cloudflare, &context.devboxes.web_domain).await?;
    let record = serde_json::json!({
        "type": "A",
        "name": name,
        "content": ip,
        "proxied": true,
        "ttl": 1,
        "comment": COMMENT,
    });

    let existing = cloudflare
        .records(&zone, &name, None)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    if let Some(record_found) = existing.first() {
        if existing
            .iter()
            .any(|found| found.comment.as_deref() != Some(COMMENT))
        {
            anyhow::bail!("{name} already has a DNS record the panel didn't make; not touching it");
        }
        if record_found.kind.as_deref() == Some("A")
            && record_found.content.as_deref() == Some(ip)
            && record_found.proxied == Some(true)
        {
            return Ok(());
        }

        cloudflare
            .update(&zone, &record_found.id, &record)
            .await
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        return Ok(());
    }

    cloudflare
        .create(&zone, &record)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))?;

    Ok(())
}

/// Removes the devbox's record (only one the panel made). Nothing to do without a web domain.
pub async fn remove(
    state: &crate::State,
    context: &Context,
    devbox: &Devbox,
) -> Result<(), anyhow::Error> {
    if context.devboxes.web_domain.is_empty() || context.cloudflare_token.is_none() {
        return Ok(());
    }

    let name = devbox.web_host(&context.devboxes);
    let cloudflare = client(state, context)?;
    let zone = zone(state, &cloudflare, &context.devboxes.web_domain).await?;
    for record in cloudflare
        .records(&zone, &name, None)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))?
    {
        if record.comment.as_deref() == Some(COMMENT) {
            cloudflare
                .delete(&zone, &record.id)
                .await
                .map_err(|err| anyhow::anyhow!("{err}"))?;
        }
    }

    Ok(())
}
