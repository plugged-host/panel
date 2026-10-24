# Subdomains

Players give their server a name like `play.example.com` under the panel's domains. The panel
writes the DNS records in the domain's Cloudflare zone, points them at wherever the server runs,
and follows the server when it moves. In `shared/src/subdomains/`.

## Which servers

The egg's features decide what a server gets (`records::Service::of_features`):

| Feature | Records | Players join with |
|---|---|---|
| `eula` (Minecraft) or `srv-minecraft` | A + `_minecraft._tcp` SRV | `play.example.com` |
| `cfx` (FiveM, RedM) or `srv-fivem` / `srv-cfx` | A + `_cfx._udp` and `_cfx._tcp` SRV (weight 5) | `connect play.example.com` |
| `srv-mumble`, `srv-teamspeak` | A + `_mumble._tcp` / `_ts3._udp` SRV | `play.example.com:<port>` |
| `srv-factorio`, `srv-rust`, `srv-scpsl` | A + `_factorio._udp` / `_rust._udp` / `_scpsl._udp` SRV | `play.example.com:<port>` |
| `subdomain` | A alone | `play.example.com:<port>` (the port left out for 80 and 443) |

A server whose egg has none of these has no Subdomains page (`lib/domain/games.ts`), and its
subdomain routes answer 400.

## The address

`subdomains::target` picks what the A record (AAAA for IPv6) points at:

1. The primary allocation's IP, when it is a real address.
2. For an allocation on `0.0.0.0` (every address), the first of these that resolves: the
   allocation's alias, the node's SRV target (`nodes.srv_target`, Admin › Nodes), the host of the
   node's public URL or URL.

The SRV records name the node's SRV target or URL host when that is a host name, and otherwise the
subdomain itself (its A record). Records are DNS only (`proxied: false`) with automatic TTL.

The records follow the server (`subdomains::follow`, in the background):

- after a transfer (`remote/servers/{server}/transfer/success`);
- after a failover or a limbo restore to another node (`Server::rehome`);
- after the primary allocation changes, by the owner or an admin.

A subdomain whose records could not be updated keeps its old ones; the owner's Write Records Again
(the rotate icon) writes them for the current address.

## Records in a shared zone

Every record carries the comment `Created by <panel name> Subdomains (Server <first 8 of uuid>)`
(SRV: `… (_minecraft._tcp SRV - Server …)`). Before writing, the panel checks the zone: any record of
the same name and type that is not this subdomain's own refuses the subdomain ("is used by another
record"), so records made by hand are never overwritten. Deleting removes the stored record ids, and
any other record of the names whose comment marks it as the panel's for this server.

A subdomain is written in one go: when Cloudflare refuses a record, the records made for it are
removed again, the row is deleted and the tokens come back. A failed rename keeps the old name.

## Limits and tokens

- A server may have `default_limit` subdomains (3), or its own limit (Admin › Servers › a server ›
  Subdomains; empty follows the setting, 0 allows none). Checked under a row lock on the server, so
  two requests cannot both take the last one.
- While tokens are on (Admin › Settings › Economy), the server owner pays a deposit for each:
  `cost_first` (1), `cost_second` (3), `cost_third` (8) and `cost_more` (15) by how many the server
  has. The price is kept on the row (`token_cost`) and refunded when the subdomain is deleted: by
  the player, by an admin, with its domain, or with the server. Transactions `subdomain` and
  `subdomain_refund`. A subuser with `subdomains.create` spends the owner's tokens; only the owner
  sees the balance.
- Names are 2 to 63 lowercase letters, digits and hyphens. A list of names is built in (`www`,
  `panel`, `mail`, `node*`, `ns*` and others, `records::BUILT_IN_BLOCKED`); Blocked Names adds more,
  with `*` matching anything.
- Each name is unique per domain.

The Discord requirement of plugged.host's Pelican plugin (subdomains only for members of the
Discord server) comes with the Discord feature.

## Domains

Admin › Domains (`domains.*`) lists the domains subdomains are made under. Adding one looks its
zone up with the token and is refused when the token does not see it; Look Up the Zone Again (after
the zone moved to another Cloudflare account) does it again. Deleting a domain deletes its
subdomains first (records removed, tokens refunded). The token needs Zone › Zone › Read and Zone ›
DNS › Edit on the zones.

A server picks the domain when there is more than one; with one, the field shows it as a suffix.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/servers/{server}/subdomains` | `subdomains.read` | `{subdomains, limit, domains, service, address, unavailable, next_cost, balance}`; `unavailable` is `off`, `unsupported`, `no_domains`, `no_address` or `null`; `balance` only for the owner while tokens are on |
| `POST /api/client/servers/{server}/subdomains` | `subdomains.create` | `{name, domain_uuid?}`: 400 bad or blocked name, unknown domain or too few tokens; 409 taken, limit reached, or another record of the name exists; 502 Cloudflare refused |
| `PATCH /api/client/servers/{server}/subdomains/{subdomain}` | `subdomains.update` | `{name?}`: renames, or without a name writes the records again |
| `DELETE /api/client/servers/{server}/subdomains/{subdomain}` | `subdomains.delete` | `{refunded}` |
| `GET /api/admin/domains` | `domains.read` | `{domains, token_set}` |
| `POST /api/admin/domains` | `domains.create` | `{name}` |
| `POST /api/admin/domains/{domain}/sync` | `domains.update` | Looks the zone up again |
| `DELETE /api/admin/domains/{domain}` | `domains.delete` | With its subdomains |
| `GET /api/admin/servers/{server}/subdomains` | `servers.subdomains` | `{subdomains, limit, default_limit}` |
| `PUT /api/admin/servers/{server}/subdomains` | `servers.subdomains` | `{limit}` (`null` follows the setting) |
| `DELETE /api/admin/servers/{server}/subdomains/{subdomain}` | `servers.subdomains` | `{refunded}` |

Server activity: `server:subdomain.create`, `.update`, `.delete`. Admin activity:
`server:subdomains.limit`, `server:subdomain.delete` (from the server's tab), `domain:create`,
`domain:sync`, `domain:delete`.

## Settings

Admin › Settings › Hosting › Subdomains (`shared/src/settings/subdomains.rs`):

| Setting | Default | Range |
|---|---|---|
| Subdomains | off | |
| Cloudflare API Token | none | encrypted; empty keeps it |
| Subdomains per Server | 3 | 0 to 100 |
| First, Second, Third Subdomain (tokens) | 1, 3, 8 | 0 to 1000000 |
| Every Further Subdomain (tokens) | 15 | 0 to 1000000 |
| Blocked Names | none | comma or line separated, `*` wildcard |

Subdomains are on for players only with the switch on and a token set (`games.subdomains_enabled`
in `/api/settings`).

## Tables

- `domains`: `name` (unique), `zone_id` (`NULL` while the token does not see the zone), `created`.
- `server_subdomains`: `server_uuid` (deleted with the server), `domain_uuid`, `name` (unique per
  domain), `record_type` (`A`, `AAAA` or `SRV`), `records` (the Cloudflare record ids by key: `a`,
  `srv`, `srv_udp`, `srv_tcp`), `token_cost`, `created`.
- `servers.subdomain_limit`: the server's own limit, `NULL` follows the setting.

## Without noprofit.space

Nothing changes: the deposit is the panel's tokens, which work without it.

## Differences from Pelican

- Records are found by their comment and stored ids instead of by name alone, so a shared zone's
  hand-made records are safe.
- The deposit is kept per subdomain and refunded from it, so changing the prices does not change
  refunds of existing subdomains.
- Settings are a settings tab instead of `.env`; the SRV target is the node's field.

## Checks

- Admin › Settings › Hosting › Subdomains: on, a token with DNS edit rights. Admin › Domains: add
  the zone's domain (Linked); add a domain the token does not see: refused.
- On a Minecraft server: Subdomains, Add `play`: the page shows `play.<domain>` as Spreading, then
  Active; Cloudflare shows an A and a `_minecraft._tcp` SRV record with the comment. Join with the
  name alone.
- A FiveM server: `connect <name>` works; the zone has `_cfx._udp` and `_cfx._tcp` records.
- With tokens on, the second subdomain costs 3; delete it: 3 tokens back (Tokens page shows
  Subdomain refunded). With too few tokens the Add button in the modal is disabled.
- Make a record `taken.<domain>` by hand; adding `taken` is refused and the record is untouched.
- Change the server's primary allocation (or move it to another node): the records follow within a
  few seconds.
- Delete the server: its records are gone from Cloudflare and the owner got the tokens back.
- Admin › Servers › the server › Subdomains: set the limit to 1 with two subdomains: no more can be
  added; empty goes back to the default.
