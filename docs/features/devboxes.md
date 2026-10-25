# Devboxes

A devbox is a small Linux box next to a game server, with the game server's files at `/server`.
People open it in VS Code over SSH (Remote - SSH) or, when the browser edge is set up, as VS Code
in the browser. Inside, `ph` shows the game server's state and console and sends power actions.
Players find it on the server's **VS Code** page, a settings page reached through the jump bar like
the Pelican panel's VS Code section ([server-settings.md](server-settings.md)). In
`shared/src/devboxes/`, the image in `docker/devbox/`.

## What a devbox is

A devbox is an ordinary server of the devbox egg (`docs/eggs/devbox.json`) that the panel makes,
starts, stops and deletes itself:

- It is owned by a system account (`devboxes@system.invalid`, suspended, no password, made on first
  use, kept in `devboxes::system_user_uuid`), so it never shows in anyone's server list, and
  nobody reaches its own pages.
- It runs on the game server's node, on a free allocation of that node (its SSH port), with the
  resources from the settings on top of the game server's.
- Its own volume is `/home/container` (VS Code's server, extensions, shell history, caches). The
  game server's volume is mounted at `/server`.
- The row in `devboxes` holds who it belongs to, its state, its current start's token hash and
  code secret, what its boot reported, and `people`: who could sign in at its last start.

**Kinds.** A personal devbox belongs to one person on one game server. The owner may switch on a
shared devbox instead (Shared › Share on the page): one for the owner and every subuser who may
have one, paid by the owner's membership. Switching it on deletes everyone's personal devbox on
that server; switching it off deletes the shared one.

**Who may have one.** The owner, and subusers holding `devbox.connect` plus SFTP and every file
permission (`files.sftp`, `files.read`, `files.read-content`, `files.create`, `files.update`,
`files.delete`): a devbox can read and change every file anyway, so a subuser without those gets
none. With **Members Only** (the default), the person (or, for a shared devbox, the owner) needs
a membership; the second membership gets the premium resources.

**When it is offered.** Devboxes on, an egg chosen, and abuse protection not off
(`devboxes::offered`). Without abuse protection a devbox would be a free shell nobody checks.

## Lifecycle

| Action | What happens |
|---|---|
| **Start** | The row and its server are made on the first start (`provision::ensure`). Each start makes a new 64-character token and code secret, sets the egg variables, points the devbox's DNS record (browser edge) and starts it in Wings (`power::start`). |
| **Running** | `ph serve` reports its version, whether `/server` is there and its SSH host key (`/api/devbox/boot`), then sends a heartbeat every minute (`/api/devbox/heartbeat`) with its open sessions. |
| **Stop** | Clears the token and code secret at once (every code and session ends) and stops it in Wings; after 60 seconds it is killed (`power::stop`, `power::settle`). |
| **Delete** | The reconcile deletes a devbox nobody used for **Cleanup** days, one whose game server is gone or moved node, and the personal ones of people who lost access. Its server goes with it, Wings first. |

The reconcile (`reconcile::run`, every minute at :40) also stops devboxes that:

- had no open connection for **Idle Stop** minutes, or ran for **Longest Run** hours;
- never reported back within 10 minutes of their start;
- whose game server is suspended, whose game server's disk is full, or whose own server is;
- whose payer lost their membership or got an abuse block;
- that Wings shows as stopped for more than 3 minutes after they booted.

A shared devbox restarts when someone in its `people` lost access, so their keys and codes stop
working. It kills devbox containers running on a node without a row, token or server of their own
(left over from a failed start or a moved game server).

**Limits.** **Running per Person** devboxes at once (personal ones and the shared ones of servers
they own). A node takes no new devbox while its deployment is paused, over 90 % memory, CPU or
disk (node telemetry), or unreachable.

## Signing in

**SSH.** The page shows the SSH command (`ssh -p <port> <user>@<host>`; the host is the
allocation's alias, its address when routable, else the node's public host) and a connect code
`xxxx-xxxx-xxxx-xxxx`, each with a copy button.
The code is the SSH password: derived from the start's code secret for that person
(`codes::derive`, HMAC-SHA256), shown only behind the panel's sign-in, the same for the whole
start, gone at the next stop. `ph` asks the panel (`/api/devbox/auth`) and the panel answers for
the people of that devbox only. Account passwords are never accepted: they would skip two-factor
and let every bot's guess reach the account. People's SSH keys (Account › SSH Keys) also work;
`ph` gets them with the boot and heartbeat answers.

**Open in VS Code** is a `vscode://vscode-remote/ssh-remote+<host>/server` link for the Remote -
SSH extension. The host key is under the SSH command, to compare on the first connection.

**The page** is the Pelican panel's VS Code section: label/value rows (Devbox with its state and a
hint, Connect code, SSH, Shared, and the v3-only Rights of a shared devbox), then one filled button
(Open in VS Code when running, else Start devbox) and "Needs the Remote - SSH extension. Install".
A failed start or stop shows its reason in a row of its own. The state comes from
`shared/src/devboxes/view.rs`: `none`, `stopped`, `creating` (a box's first boot: no version and no
host key reported yet, within the boot window; usually the node downloading the image), `starting`,
`stopping`, `failed`, `mount_missing`, `ready`; the page polls every 3 s while creating, starting or
stopping.

## What `ph` may do with the game server

`ph` uses the devbox's token. Every answer is limited to that devbox's game server and checks
access again.

| Endpoint | Personal devbox | Shared devbox |
|---|---|---|
| `POST /api/devbox/game/status` | always | **See its state and resources** |
| `POST /api/devbox/game/power` | what the person holds (`control.start`, `control.restart`, `control.stop`) | **Start, stop and restart it** |
| `POST /api/devbox/game/websocket` | a Wings websocket token with what the person holds of `control.read-console`, `control.console` and `control.stop` | **Read the console** / **Send console commands**, plus stop with power |

Starts go through the start gates like the Start button: a held start answers with a link to the
queue or waiting room instead. A websocket token never carries `control.start` or
`control.restart`. Power actions are logged on the game server as `server:devbox.power`.

## VS Code in the browser

Optional. Each devbox then answers at `https://<slug>.<browser domain>`, `<slug>` being 16 random
lowercase letters and digits. An nginx server (the edge) asks the panel on every request whether
the browser's devbox cookie belongs to that host, devbox and person, then proxies to the devbox's
node and port over TLS with a client certificate:

- `ph` serves its web side (code-server, Coder's MIT VS Code build) only to the edge's client
  certificate, with a certificate for its own host name that the panel issues at every start from
  its devbox CA (`pki::issue`, P-256, 30 days);
- nginx checks each devbox's certificate against that CA;
- the panel creates one proxied A record per devbox (`dns::ensure`, comment `devbox record
  (managed by the panel)`), pointing at the edge, and removes it with the devbox. Records it did
  not make are never touched.

The browser flow:

1. **Open in browser** on the page asks for a one-time ticket (one minute) and opens
   `https://<slug>.<domain>/_devbox/login?ticket=…`.
2. The edge passes that to `GET /api/devbox/web/login`, which redeems the ticket and sets
   `__Host-devbox` (encrypted: devbox, person, host, expiry; 12 hours, `Secure; HttpOnly;
   SameSite=Lax`, on that host only).
3. Every request: the edge asks `GET /api/devbox/web/authorize` with the host (`X-Devbox-Host`)
   and the cookie. `204` carries `X-Devbox-Upstream` (`host:port`); `401` sends the browser to
   the panel's `/account/devbox/open?host=…`, which after the panel's own sign-in either opens a
   fresh link, shows the server's VS Code page (the devbox is stopped or not theirs) or, for a
   devbox that is gone, the server list.

The browser domain must be a domain of its own: never the panel's registrable domain or a name
under or above it, which the panel's cookies could reach (checked when it is saved).

## Abuse

Devbox servers are scanned like every other server (abuse protection). A critical finding on a
devbox (`blocks::on_findings`):

- stops every running devbox of the person who pays for it, and records a block in
  `devbox_blocks`: they get no devbox until an admin dismisses or resolves the finding
  (`blocks::on_finding_status`);
- keeps the devbox's server (stopped) while the finding is open, since its evidence goes with it;
- sweeps the person, not the system account (`abuse::sweep_owner`).

## Kept out of other features

A devbox server has no system backups, is never parked by limbo, never failed over, never moved
by a transfer or a node lightening and never cut by allowance cuts: the reconcile makes a new one
where its game server is. Each of those queries has a `NOT EXISTS (… devboxes …)`.

Upstream's mounts are removed ([upstream.md](../upstream.md#removed-features)). The only mount Wings
gets is a devbox's mount of its own game server's volume (`provision::MOUNT_SQL`): read from the
`devboxes` row, and only while the devbox and its game server are on the same node and the source
ends with the game server's UUID. Before a devbox is made, the node's `allowed_mounts` must allow
that source (`NodeAllowedSources`, from the node's Wings configuration).

## Settings

Admin › Settings › Hosting › Devboxes (`devboxes::*`, `shared/src/settings/devboxes.rs`):

| Field | Default | Meaning |
|---|---|---|
| `enabled` | off | Players see the VS Code page (with an egg chosen and abuse protection on). |
| `egg_uuid` | none | The devbox egg, imported from `docs/eggs/devbox.json`. |
| `image` | empty | The image; empty takes the egg's first image. A digest pins one build. Picked up at the next start. |
| `members_only` | on | Only members get a devbox. |
| `cpu`, `memory`, `disk` | 200 %, 2048 MiB, 5120 MiB | On top of the game server's. |
| `premium_cpu`, `premium_memory`, `premium_disk` | 400 %, 4096 MiB, 10240 MiB | For the second membership. |
| `idle_minutes` | 15 | Stopped after this long without a connection. |
| `max_hours` | 12 | Stopped this long after its start. |
| `running_per_user` | 2 | Running at once per person. |
| `cleanup_days` | 14 | Deleted after this long unused. |
| `panel_url` | empty | Where devboxes reach the panel; empty: the panel's URL. |
| `web_domain`, `web_origin_ip` | empty | The browser domain and the edge's public IPv4. |
| `cloudflare_token` | none | Zone › DNS › Edit on the browser domain; empty uses the subdomains token. Encrypted. |
| `ca_cert`, `ca_key`, `edge_cert`, `edge_key` | none | Made by **Create Certificates**; keys encrypted, never sent to the browser. |

## Setting it up

### 1. The egg and the image

Import `docs/eggs/devbox.json` into any nest (Admin › Nests › Import). The image is built from
`docker/devbox` by `.github/workflows/devbox-image.yml` as `ghcr.io/<owner>/<repository>-devbox`
(`:latest` and `:sha-<commit>`; the egg names `ghcr.io/plugged-host/panel-devbox:latest`); pin a
digest in the settings. The package must be public, or every node needs a registry login. Pull it
on each node ahead of the first start:

```bash
docker pull ghcr.io/<owner>/<repository>-devbox@sha256:<digest>
```

### 2. Each node

**Allow the mount.** Wings takes `allowed_mounts` from its own configuration, never from the
panel. Add its data directory (`system.data`, by default `/var/lib/calagopus-wings/volumes`) to
the node's Wings `config.yml` and restart Wings; running game servers keep running:

```yaml
allowed_mounts:
  - /var/lib/calagopus-wings/volumes
```

This is safe here only because the panel sends Wings no other mount. A node without it refuses
new devboxes ("the node isn't ready for devboxes").

**The node firewall.** A devbox is a full shell on the node's container network. Keep it, and
every game container, away from the node itself, the cloud metadata service and private networks,
and from sending mail. Its own nftables table, before Docker's rules (never in
`/etc/nftables.conf`, whose `flush ruleset` wipes Docker's). Take the bridge and subnets from
`docker.network` in the Wings configuration (the defaults are shown), the allocation range from
Admin › Nodes, and the Wings API port (devboxes' `ph logs` and `ph console` reach Wings' websocket
at the node's address):

```bash
sudo install -d /etc/nftables.d
sudo tee /etc/nftables.d/node-guard.nft >/dev/null <<'EOF'
#!/usr/sbin/nft -f
define WINGS_BRIDGE = "calagopus_nw"
define WINGS_V4 = 172.18.0.0/16
define WINGS_V6 = fdba:17c8:6c94::/64
define GAME_PORTS = { <FIRST_PORT>-<LAST_PORT> }
define WINGS_API_PORT = 8080

table inet node_guard
delete table inet node_guard

table inet node_guard {
    chain forward {
        type filter hook forward priority -10; policy accept;
        iifname != $WINGS_BRIDGE return
        ct state established,related return
        ip daddr $WINGS_V4 return
        ip6 daddr $WINGS_V6 return
        ip daddr { 10.0.0.0/8, 100.64.0.0/10, 169.254.0.0/16, 172.16.0.0/12, 192.168.0.0/16 } counter drop
        ip6 daddr { fc00::/7, fe80::/10 } counter drop
        tcp dport 25 counter drop
    }

    chain input {
        type filter hook input priority -10; policy accept;
        iifname != $WINGS_BRIDGE return
        ct state established,related return
        meta l4proto { icmp, ipv6-icmp } return
        tcp dport $GAME_PORTS return
        udp dport $GAME_PORTS return
        tcp dport $WINGS_API_PORT return
        counter drop
    }
}
EOF
sudo nft -c -f /etc/nftables.d/node-guard.nft
```

Load it with a oneshot unit (`ExecStart=/usr/sbin/nft -f /etc/nftables.d/node-guard.nft`,
`Before=docker.service`). Add `return` lines for what containers must reach on the host or a
private network (a database host on the node, a resolver on the host, a panel reached at a
private address).

**Spare allocations.** Each devbox takes one free allocation of its node, open to the internet
like the game ports.

### 3. Switch it on

In Admin › Settings › Hosting › Devboxes choose the egg, pin the image, check the resources and
the lifecycle, then switch **Devboxes** on. Abuse protection must not be off.

### 4. VS Code in the browser (optional)

1. Put the browser domain (`example.net` here) on Cloudflare as a zone of its own. Create an
   origin certificate for `*.example.net` and save it on the edge box; set SSL to Full (strict).
2. In the settings set **Browser Domain**, **Edge Address** and, unless the subdomains token sees
   that zone, a Cloudflare token. Click **Create Certificates**.
3. Write the certificates for nginx, on a machine with the panel's environment:

   ```bash
   panel-rs devboxes edge-certificates --dir /etc/nginx/devbox
   ```

   It writes `ca.crt`, `edge.crt` and `edge.key` (mode 0600). Run it again after **Replace
   Certificates**, then restart running devboxes.
4. Add the nginx site below and reload nginx. Running devboxes get their certificate and DNS
   record at their next start.

```nginx
proxy_cache_path /var/cache/nginx/devbox-auth levels=1 keys_zone=devbox_auth:2m max_size=20m inactive=5m use_temp_path=off;

map $http_upgrade $devbox_connection {
    default upgrade;
    ''      close;
}

server {
    listen 80;
    server_name "~^[a-z0-9]{16}\.example\.net$";
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl;
    http2 on;
    server_name "~^[a-z0-9]{16}\.example\.net$";

    ssl_certificate     /etc/ssl/example.net/fullchain.pem;
    ssl_certificate_key /etc/ssl/example.net/privkey.pem;
    client_max_body_size 256m;

    # node host names come from the panel per request
    resolver 1.1.1.1 8.8.8.8 ipv6=off valid=300s;
    resolver_timeout 5s;

    # The panel's answer for this host and cookie, cached 30 s. nginx can't name a cookie with a
    # hyphen ($cookie___Host-devbox would end at the hyphen), so the key is the whole header.
    location = /_devbox/auth {
        internal;
        proxy_pass https://panel.example.com/api/devbox/web/authorize;
        proxy_ssl_server_name on;
        proxy_set_header X-Devbox-Host $host;
        proxy_set_header Cookie $http_cookie;
        proxy_pass_request_body off;
        proxy_set_header Content-Length "";
        proxy_cache devbox_auth;
        proxy_cache_key "$host|$http_cookie";
        proxy_cache_valid 204 30s;
        proxy_cache_valid any 0;
    }

    # the one-time ticket becomes the devbox cookie, on this host
    location = /_devbox/login {
        proxy_pass https://panel.example.com/api/devbox/web/login$is_args$args;
        proxy_ssl_server_name on;
        proxy_set_header X-Devbox-Host $host;
        proxy_set_header Cookie "";
    }

    # no valid cookie: through the panel's sign-in for a fresh link
    location @devbox_reopen {
        return 302 https://panel.example.com/account/devbox/open?host=$host;
    }

    location / {
        auth_request /_devbox/auth;
        auth_request_set $devbox_upstream $upstream_http_x_devbox_upstream;
        error_page 401 = @devbox_reopen;

        proxy_pass https://$devbox_upstream;
        proxy_ssl_certificate         /etc/nginx/devbox/edge.crt;
        proxy_ssl_certificate_key     /etc/nginx/devbox/edge.key;
        proxy_ssl_trusted_certificate /etc/nginx/devbox/ca.crt;
        proxy_ssl_verify on;
        proxy_ssl_verify_depth 2;
        proxy_ssl_server_name on;
        proxy_ssl_name $host;
        proxy_ssl_session_reuse on;

        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $devbox_connection;
        proxy_set_header X-Forwarded-Proto https;
        proxy_set_header X-Forwarded-Host $host;
        proxy_buffering off;
        proxy_request_buffering off;
        proxy_read_timeout 1d;
        proxy_send_timeout 1d;
    }
}
```

When the edge runs on the panel's box, the two panel locations can go to the panel directly
(`proxy_pass http://127.0.0.1:<port>/api/devbox/web/…` with `proxy_set_header Host
panel.example.com`).

## Checking a node

1. As someone who may have a devbox, open a server on that node › Settings › **VS Code** › **Start
   devbox**. The Devbox row goes from "Creating…" (first start on the node) or "Starting…" to
   "Running".
2. Connect with the SSH command and the connect code. `ls /server` lists the game server's files;
   a file changed there shows in the game server's file manager. `ph status` shows the game server.
3. From the devbox: `curl -sm5 https://example.com` works, `curl -sm5 http://169.254.169.254/`
   times out, the node's SSH port at the bridge gateway fails, `ph logs -n 5` prints console lines.
4. **Stop** shows "Stopping…" until Wings confirms it.

## Code

| Part | Where |
|---|---|
| Rows, kinds, membership, people, refusals | `shared/src/devboxes/mod.rs` |
| Making and deleting servers, the mount | `shared/src/devboxes/provision.rs` |
| Start, stop, restart | `shared/src/devboxes/power.rs` |
| The reconcile | `shared/src/devboxes/reconcile.rs` (`backend/src/tasks.rs`) |
| Connect codes, tickets, browser sessions | `shared/src/devboxes/codes.rs` |
| CA and certificates | `shared/src/devboxes/pki.rs` |
| DNS records | `shared/src/devboxes/dns.rs` |
| What `ph` asks | `shared/src/devboxes/agent.rs`, `backend/src/routes/api/devbox/` |
| The browser edge | `shared/src/devboxes/web.rs`, `backend/src/routes/api/devbox/web.rs` |
| Abuse blocks | `shared/src/devboxes/blocks.rs` |
| The page | `shared/src/devboxes/view.rs`, `backend/src/routes/api/client/servers/_server_/devbox/`, `frontend/src/pages/server/devbox/` |
| Reopen in the browser | `backend/src/routes/api/client/account/devboxes.rs`, `frontend/src/pages/dashboard/devbox/` |
| CLI | `backend/src/commands/devboxes/` |
| Settings | `shared/src/settings/devboxes.rs`, `frontend/src/pages/admin/settings/devboxes/` |
| The image and `ph` | `docker/devbox/` |
