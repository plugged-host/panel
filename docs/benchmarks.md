# Speed compared with the Pelican panel

Measured **2026-10-25**: plugged.host's production Pelican panel (v2.2.1 with plugged.host's plugins)
against this panel (v3, release `v3.0.0.0376cb9`), median (p50) of each page's time in milliseconds.

| Page | Pelican v2.2.1 | v3 before today's work | v3 now (idle) | v3 now (under load) | Faster than Pelican |
|---|---|---|---|---|---|
| Server list (`/`) | 203 | 53.4 | 30.1 | 19.6 | 85 % |
| Console (`/server/{id}`) | 317 | 60.9 | 35.2 | 32.9 | 88 % |
| File manager (`/server/{id}/files`) | 337 | 74.7 | 47.3 | 44.5 | 85 % |

"Faster than Pelican" compares the idle v3 column, the slower of the two, with Pelican:
`1 − v3 / Pelican`, rounded down. The best page is **88 % faster**; the other two are 85 %.

## Method

The two panels were measured differently. Two of the differences favour Pelican: only its HTML is
counted, and it is timed one hop from the panel. One favours v3: Pelican carries production's real
load, v3 a handful of users.

- **Pelican**: the origin time (`$upstream_response_time`) of every real page request in the HA
  proxy's access log over about 36 hours of production traffic. That is only the page's HTML. A
  Pelican page is drawn on the server, so the HTML already shows the page, but its Livewire requests
  afterwards are not counted. The time starts at the proxy, one private-network hop (3–11 ms) from
  the panel.
- **v3**: the whole chain a browser with the JavaScript already cached runs before the page is
  drawn. That is the HTML (with the first API answers inlined, [speed](features/speed.md)), then
  every API request the page still makes, stage by stage; a stage starts when the one before it is
  done. For the console that is the server's websocket token and its announcements; for the file
  manager, the directory listing, which asks the node. Each stage pays a full round trip from the
  measuring box, about 13 ms away over the private network. The script is
  `deploy/bench/panel-bench.py` in plugged.host's private ops repository: 30 runs per page after 3
  warm-up runs, a pause between pages so one account stays under the rate limit, and the same
  account and server throughout.
- **Under load**: the same run with 6 requests per second of a production-like mix (server, websocket
  token, balance, server list, settings, file listing) running alongside. That is the most one test
  account sends without reaching its rate limit. The connections stayed warm, which is why it came
  out faster than the idle run. It is not production's real load: v3 serves a handful of users,
  Pelican serves all of them.

Not included, for either panel: Cloudflare, the visitor's own network, and downloading the
JavaScript on a first visit (hashed files, cached by the browser and at Cloudflare's edge).

## What changed today

The "before" column is v3 as it was this morning (`v3.0.0.3e521f1`). Today's work made the three
pages 37–44 % faster (idle):

- **Page answers in the HTML**: the shell's and the page's first API answers come inlined in the
  page, so the dashboard draws after one round trip instead of three ([speed](features/speed.md)).
- **Prepared statements kept**: the database statement cache grew from 100 to 512 per connection,
  so queries are no longer planned again on every use (the server list went from about 10 ms to
  about 3 ms at the database).

## Earlier measurement: sign-in

Measured the same day, before this work, over Cloudflare from the webserver; 15–25 alternating runs
per panel, median of `curl`'s `time_total`:

| Journey | Pelican | v3 | Faster |
|---|---|---|---|
| Sign-in, the panel's own redirects only (`/auth/login` to the accounts app) | 245 ms | 83 ms | 66 % |
| Sign-in until the accounts page answers (both redirects followed) | 351 ms | 194 ms | 45 % |
| Signed-out API answer (`/api/client/account`, 401) | 113 ms | 57 ms | 50 % |

## The claim

**"Up to 88 % faster page loads than v2.2."** That is measured, not padded: the console page, 317 ms
on Pelican against 35 ms on v3. If a single figure for every page is wanted: **"85 % faster or
more on every page measured."**
