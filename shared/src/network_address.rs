//! plugged.host: the IP address the server's Network page shows next to an allocation's host name,
//! as the Pelican panel's Network tab did (docs/features/allocations.md#network-addresses).
//!
//! Nodes usually give their allocations a host name as alias, so the address a player's client
//! dials was nowhere on the page; whoever needed it (DNS not there yet, a launcher that wants an
//! IP, a firewall rule) had to ask.

use crate::models::{node::Node, server_allocation::ApiServerAllocation};
use std::collections::HashMap;

/// How long a host name's address is remembered, as on the Pelican panel.
const CACHE_SECONDS: u64 = 300;

/// Fills `resolved_ip` of a page of a server's allocations on `node`: what the shown host (the
/// alias, else the IP) resolves to, else what the node's host name resolves to, else the
/// allocation's IP when it is a real one (not 0.0.0.0). Left empty when it is what is shown.
pub async fn fill(state: &crate::State, node: &Node, allocations: &mut [ApiServerAllocation]) {
    let node_host = node
        .public_url
        .as_ref()
        .and_then(crate::subdomains::host_of)
        .or_else(|| crate::subdomains::host_of(&node.url));
    let mut known = HashMap::<String, Option<String>>::new();

    for allocation in allocations {
        let shown = allocation
            .ip_alias
            .as_deref()
            .unwrap_or(allocation.ip.as_str())
            .trim()
            .to_string();

        let mut ip = None;
        for host in [Some(shown.as_str()), node_host.as_deref()]
            .into_iter()
            .flatten()
        {
            let host = host.trim();
            if host.is_empty() {
                continue;
            }

            let found = match known.get(host) {
                Some(found) => found.clone(),
                None => {
                    let found = resolve(state, host).await;
                    known.insert(host.to_string(), found.clone());
                    found
                }
            };
            if found.is_some() {
                ip = found;
                break;
            }
        }

        let ip = ip.or_else(|| {
            allocation
                .ip
                .parse::<std::net::IpAddr>()
                .ok()
                .filter(|ip| crate::subdomains::routable(*ip))
                .map(|ip| ip.to_string())
        });
        allocation.resolved_ip = beside(&shown, ip).map(Into::into);
    }
}

/// A host name's address, remembered for [`CACHE_SECONDS`] (a failed lookup too).
async fn resolve(state: &crate::State, host: &str) -> Option<String> {
    let lookup = || async {
        crate::subdomains::resolve(host)
            .await
            .map(|ip| ip.to_string())
    };

    match state
        .cache
        .cached(
            &format!("network_address::{host}"),
            CACHE_SECONDS,
            || async { Ok::<_, anyhow::Error>(lookup().await) },
        )
        .await
    {
        Ok(ip) => ip,
        Err(err) => {
            tracing::debug!(host, "network address cache unavailable: {err:#}");
            lookup().await
        }
    }
}

/// The IP to show beside `shown`: none when there is none or it is `shown` itself.
fn beside(shown: &str, ip: Option<String>) -> Option<String> {
    ip.filter(|ip| !ip.eq_ignore_ascii_case(shown))
}

#[cfg(test)]
mod tests {
    use super::beside;

    #[test]
    fn the_ip_is_shown_only_when_it_adds_something() {
        assert_eq!(
            beside("play.example.com", Some("192.0.2.10".into())),
            Some("192.0.2.10".into())
        );
        assert_eq!(beside("192.0.2.10", Some("192.0.2.10".into())), None);
        assert_eq!(beside("play.example.com", None), None);
    }
}
