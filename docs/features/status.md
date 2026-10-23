# Status page

The panel checks the website, itself and every node each minute and publishes the results as
static-shaped files a public status page reads (`/data/*.json`, `/data/feed.xml`). Admins post
incidents, force a component's status for a while (overrides) and choose which components are
listed. The page itself is a separate site; the fork only supplies its data.

The whole feature is off until Admin › Settings › General › Status Page › "Enable the Status Page"
is on. While it is off nothing is checked and `/data/*` answers 404.

## Components

One row per checked thing in `status_components` (`shared/src/status.rs`):

| Kind | Checked by | Listed while |
|---|---|---|
| `site` | `GET` of the Website URL every minute; any 2xx is up | the Website URL is set |
| `panel` | `GET` of the Panel URL every minute (for example `https://panel.example.com/api/settings`) | the Panel URL is set |
| `node` | the node's newest [telemetry](node-telemetry.md) sample (no extra request to Wings) | the node exists |

The panel adds and removes components itself: a node gets one when it is created and loses it with
its uptime when it is deleted, a node's label follows its name, and the website or panel row goes when
its URL is emptied. An admin can hide a component (it is still checked) and set its sort order.

How a check turns into a status (`evaluate_node`, `evaluate_endpoint`, unit tests in the same file):

- **Website and panel**: one failed check is `degraded`, three in a row `offline`; the first good
  check after a failure is still `degraded`.
- **Nodes**: no answer is `degraded` from the second check in a row and `offline` from the third.
  When Wings refused the connection (the machine is up, its servers keep running) it stays at
  `degraded`. A node in maintenance mode shows `maintenance`. CPU at or over the CPU threshold for
  five checks in a row is `degraded` (full memory or disk is not slow, so it does not count). Coming
  back from `degraded` or `offline` takes two good checks. A sample older than 150 seconds counts as
  no answer.

Every check also adds to the component's day in `status_daily_uptime` (checks, operational checks,
worst status of the day); `prune_status` deletes days older than the History setting daily at 04:20.

## Incidents and overrides

On top of the checks, what the files say is decided in this order:

1. The component's checked status.
2. **Incidents** that are not resolved raise their components (none chosen: all) to their impact's
   status if that is worse: `minor` → `degraded`, `major` → `partial_outage`, `critical` →
   `major_outage`, `maintenance` → `maintenance`. Each incident has updates (status and message,
   newest first); posting one with `resolved` resolves it.
3. **Overrides** that are active now (no start or started, no end or not ended) set their component
   (none: all) to their status, whatever the checks and incidents say, with their note.

Statuses, best to worst: `operational`, `maintenance`, `degraded`, `partial_outage`, `major_outage`,
`offline` (`unknown` until the first check). The overall status (`overall`) is `major_outage` when
the website or the panel is `offline` or `major_outage`, any component is `major_outage`, or every
node is `offline`; else `partial_outage` when something is `offline` or `partial_outage` (one node
down is not the whole service), else `degraded`, `maintenance`, `unknown` or `operational`, the first
that any listed component has.

## Files

Served by the panel at `/data/...` with `Access-Control-Allow-Origin: *` and
`Cache-Control: public, max-age=30` (`backend/src/routes/data.rs`); only published components appear.

| File | Holds |
|---|---|
| `current.json` | `overall {status, label, message}`, `components [{label, status, note?}]`, each node's CPU, memory and storage (`nodes`) and their sum (`global`), the release |
| `history.json` | `recent`: the last hour of each node's CPU and memory; `uptime`: per component the uptime percent and up to 90 days `{date, status, total_checks, operational_checks}` |
| `incidents.json` | the newest 50 incidents: `title`, `impact`, `status`, `components` (labels, or "All systems"), `started_at`, `resolved_at`, `updates [{status, message, published_at}]` |
| `targets.json` | each published node's label, host and Wings port, for an outside monitor that probes the nodes when the panel itself is down |
| `feed.xml` | the incidents as RSS 2.0, linking to the Status Page URL |

Every JSON file has `schema_version` 1 and `generated_at`. The shapes are the ones plugged.host's
status site already reads, so the site works unchanged against this panel.

## Settings

Admin › Settings › General › Status Page (`shared/src/settings/status.rs`, section `status`):

| Setting | Default | Means |
|---|---|---|
| Enable the Status Page | off | checks run and `/data/*` is served |
| Website URL | empty | probed each minute; empty: no website component |
| Website Host Header | empty | sent with the website probe when the URL is an address |
| Panel URL | empty | probed each minute; empty: no panel component |
| Status Page URL | empty (the panel URL) | linked from the feed |
| CPU Threshold | 95 % | node CPU that, held for five checks, is `degraded` |
| History | 90 days | daily uptime kept (the files show up to 90) |

## Routes and permissions

| Route | Permission |
|---|---|
| `GET /api/admin/status/components`, `/incidents`, `/overrides` | `status.read` |
| `PATCH /api/admin/status/components/{component}` (`published`, `sort`, `label` of the website and panel) | `status.manage` |
| `POST /api/admin/status/incidents`, `PATCH`/`DELETE /api/admin/status/incidents/{incident}`, `POST /api/admin/status/incidents/{incident}/updates` | `status.manage` |
| `POST /api/admin/status/overrides`, `DELETE /api/admin/status/overrides/{override}` | `status.manage` |
| `GET /data/{current,history,incidents,targets}.json`, `GET /data/feed.xml` | none (public) |

Every change is in the admin activity log (`status:incident.*`, `status:override.*`,
`status:component.update`). The tasks are `collect_status` (every minute at second 45, after the
telemetry sample at second 30) and `prune_status`.

## Without noprofit.space

Nothing changes: the status page only talks to the website, the panel and the node telemetry.

## Check

- Admin › Settings › General › Status Page: turn it on with a Website URL and a Panel URL and save.
  Within a minute Admin › Status › Components lists the website, the panel and every node, each with
  a status and "Last Checked".
- Open `https://<panel>/data/current.json`: it lists the same components; `history.json`,
  `incidents.json`, `targets.json` and `feed.xml` answer too. Turned off, they answer 404.
- Admin › Status › Components › a node › Edit: untick Published; it leaves `current.json`.
- Admin › Status › Incidents › Create: a `major` incident on one node shows that node as
  `partial_outage` in `current.json` and the incident in `incidents.json` and `feed.xml`; Post Update
  with Resolved fills Resolved and the node goes back to its checked status.
- Admin › Status › Overrides › Create: `maintenance` for all components with a note sets every
  component to `maintenance` with the note; delete it and they go back.
