# Ads

Ads pay for free servers. The panel shows Adsterra units in sandboxed frames, Monetag's tags, the
panel owner's own sponsored banners, an ad-funded waiting room on start (start gate 30,
[start-gates.md](start-gates.md)), a note for visitors with an ad blocker and a chat line on
running Minecraft servers. Everything is off until Admin › Settings › Ads › Placement turns it on,
and with ads on nothing shows until a zone, a tag or a banner exists.

Code: `shared/src/ads.rs` (zones, unit documents, who sees ads, in-game messages),
`shared/src/waiting_room.rs` (the gate), `shared/src/models/ad_banner.rs`,
`shared/src/settings/ads.rs`; the frontend in `frontend/src/elements/ads/` and `lib/ads.ts`.

## Who sees ads

`GET /api/client/account/ads` answers `audience` and the live sponsored banners:

| Audience | Who | Ads |
|---|---|---|
| `none` | ads off; the second membership while "Fewer Ads for Members" is on | none, and no waiting room |
| `room` | the first membership while "Fewer Ads for Members" is on (since 3.0.4) | only the waiting room before a start and the start queue page, with their own units (refreshed after `refresh_member_seconds`); no slot on any other page, no Social Bar, Popunder, room interstitial, In-Page Push or Vignette, no ad blocker notice |
| `member` | either membership while "Fewer Ads for Members" is off | all, refreshed after `refresh_member_seconds` (10) instead of `refresh_seconds` (5); no Vignette unless "Vignette for the First Membership" |
| `everyone` | everybody else, admins included | all |

In-game messages skip every account with a membership running, whatever the audience.

Admin pages never show ads. Dashboard pages and server pages each have their own switch.

## Slots

The slots on screen register in one store (`stores/adSlots.ts`) and `allocate()` (`lib/ads.ts`)
fills them: slots pick in rank order, each the widest unit of its sizes that fits the width it has,
and no unit shows twice on a page. A sponsored banner goes before a zone of its size; banners of one
size are shuffled by the page's path, so they take turns from page to page.

| Rank | Slot | Where | Sizes |
|---|---|---|---|
| 0 | Top | above every dashboard and server page | a sponsored top banner only |
| 1 | Server pages strip | under the header of a server's pages, not the console, not the waiting rooms | 468×60, 320×50 |
| 2 | Page rectangle | Create Server, the store, the server list | 300×250 |
| 3 | Page foot | at the foot of every page: a leaderboard and the native unit | leaderboard sizes, native |
| 4 | Waiting rooms | the start queue, the waiting room | any zone left, 160×600 and 160×300 too, and the native unit |

The top and foot slots are not on the waiting pages or on a parked server's page. A slot that gets
nothing takes no room.

The house promo clips (`frontend/public/promo/`) are one more creative in the rotation: a waiting
room slot or a page rectangle shows one instead of its unit **Share of the Slots** percent of the
time (25), and gets one when nothing else fills it; a video in the waiting rooms, the poster in the
page rectangles, never to someone without ads ([promos.md](promos.md#promo-clips)). A waiting page
with no zone or banner at all keeps one slot for a clip.

As the Pelican panel's ads 2.2.2 (2026-10-25, back to its 2026-10-22 layout), the top slot shows no
ad zone (only a live sponsored top banner) and the foot has no 300×250. The server pages strip and
the page rectangles together fill at most **Units per Page** (`max_units_per_page`, 2 by default,
0 fills none of them; the Pelican panel's `ADS_MAX_UNITS_PER_PAGE`), in rank order; the top, the
foot and the waiting rooms do not count.

## Isolation

A zone or the native unit is an `<iframe>` of `GET /api/ads/unit?zone=<key>` (or `?native=true`):
a document the panel builds from the settings only (a request never puts code into it) with
Adsterra's snippet, a policy of its own (`unit_csp()`: anything inside, framed by the panel only)
and `sandbox` without `allow-same-origin`. Its scripts run with an opaque origin: they cannot reach
the panel's pages, cookies or API, and cannot navigate the panel away. In-memory stand-ins for
`localStorage`, `sessionStorage` and `document.cookie` keep scripts that touch them from dying.

The frame reports to the page whether it holds an ad (`postMessage({adUnit, filled})`). A unit that
has not reported one after 6 seconds folds to nothing; a filled unit reloads after the refresh time
in view (half of it visible, the tab in front).

Some networks fill nothing for a frame without an origin. **Ad Frame Address** (Adsterra tab) is
then another address of the panel, best on another domain, for example `https://ads.example.net`:
the frames load from there with `allow-same-origin`, an origin that is not the panel's. Only a
request whose `Host` is that address gets `allow-same-origin` (`is_frame_host()`); the panel's own
address never does, even when it is entered there. Point the address at the panel and let it serve
`/api/ads/unit` only, for example in nginx:

```nginx
server {
    server_name ads.example.net;
    location = /api/ads/unit { proxy_pass http://127.0.0.1:8080; proxy_set_header Host $host; }
    location / { return 404; }
}
```

## Page-level scripts

The Social Bar, the Popunder, the waiting rooms' own zone and Monetag's In-Page Push and Vignette
cannot work in a frame: they run in the page itself, with the page's rights. Their hosts (and
**More Script Hosts**) are added to the panel's `script-src`. Where they load:

| Script | Pages | Waiting rooms | Console |
|---|---|---|---|
| Social Bar | with "Social Bar on Every Page" | with the room switch | never |
| Popunder | never | with the room switch | never |
| Waiting room zone | never | with the room switch | never |
| In-Page Push | yes | never | never |
| Vignette | once per `monetag_vignette_minutes` per browser | never | never |

A loaded script stays until the next full page load, so leaving a waiting room in which one ran is
a full load. Monetag's Push Notifications and MultiTag need a service worker at the site root and
are not supported. **Site Verification** puts `<meta name="monetag">` into every page.

## Sponsored banners

Admin › Sponsored Banners (`ad-banners.*` permissions): an advertiser, an `https` link, an image
(PNG, JPEG, WebP or GIF, at most 4 MB, stored under `publicdata/ad-banners/`), a placement, an
optional start and end, and an active switch. A banner shows while it is active, has an image and
is inside its dates.

- **Ad slots**: the image is an Adsterra size, or twice one (sharp on dense screens, shown at half);
  it takes the place of a zone of that size. Any other size never shows; the form says so.
- **Top**: above every page with ads, as wide as the page allows up to the image's own width.

Banners of one size take turns from page to page. A click goes through `GET
/api/ads/banners/{banner}/click`, which counts it (a browser's prefetch does not count) and
redirects to the stored link only.

## Waiting room

Start gate 30. A start waits in the room when the room is on and none of these hold:

- the user is an admin, or sees no ads (`audience` `none`);
- the server is suspended or has a status (installing, restoring, moving);
- the server is under every line: CPU limit, memory and disk at most `room_cpu`, `room_memory`,
  `room_disk` (a line of 0, or an unlimited server, is always over);
- the user left a room for this server less than `room_pass_minutes` (10) ago;
- it is a Start on a server that is not offline, or Wings does not answer.

The start goes to Wings at once; the room only holds the user. The gate records the room
(`server_waiting_rooms`, one row per server and user, `server:waiting-room.open`) and the browser
goes to `/server/<id>/waiting`. The page polls `GET .../waiting-room` and lets the user through when
the server runs and `room_min_seconds` (15) have passed, or after `room_max_seconds` (30) whatever
the server does: it sends `POST .../waiting-room` (`{blocked}`: whether an ad blocker was found),
which logs `server:waiting-room.release` with the time waited and starts the pass. A start that did
not reach Wings on the click is sent again then.

The page is laid out like the Pelican panel's ad gate, without the sidebar: a status card ("These
ads are paying for your server", counting down to `room_max_seconds` every second, re-seeded from
the poll), the way past the room ([below](#day-pass)), then the room's ads ("What the Rooms Show"
on the tab) in two parts. While the countdown runs, the back button and the panel's
own links stay on the page and the browser asks before a reload or close (`useHoldPage`); a page
with no room to wait in goes to the console.

On both waiting pages (this one and the start queue) the ads come in two parts, each with one
"Sponsored" caption (hidden while no unit shows) and two columns from 1024 px: the first two room
slots under the status card, the other slots and then the native unit further down.

### Day pass

The room sells the way past it: **Day Pass Price** tokens (1) let every start of the account skip
every room, on all its servers, for **Day Pass Length** hours (24), `users.day_pass_until`; a pass
bought while one runs adds to what is left. Under the status card, a "Skip the wait" card says how
many tokens the user has and offers "Skip the wait for 24 hours · 1 token" (the page's filled
button; "Earn tokens" in a new tab while the balance is short), and, while memberships are on with
Fewer Ads for Members and the user lacks it, "Premium skips this", to Account › Membership in a new
tab (the room holds its page). Buying lets the user through at once: the room counts as ready while
a pass runs. While a pass runs the card says until when.

- Sold while ads, the room, the pass and tokens are on. The gate checks the pass right after the
  server's size line, before the free pass of the last room.
- `POST /api/client/account/day-pass` (`economy.purchase`) `{price}` (the price shown) →
  `{day_pass: {tokens, hours, balance, until}}`; 409 short or the price moved, 403 off. It locks the
  user row and writes the tokens, the ledger row (`day_pass`, "Waiting room day pass") and the end
  together. User activity `account:day-pass`.
- `GET .../waiting-room` adds `day_pass` (`tokens`, `hours`, `balance`, `until`, or `null` while it
  is not sold) and `premium` (the second membership's name, or `null`).

## Ad blockers

On pages with ads, the panel looks for a blocker once per page load: a bait element with ad class
names (looked at again after 2.5 seconds, for blockers that hide late) and, with "Test the Ad
Hosts", a request to the first zone's host that fails twice at once (a timeout does not count).
Then, by **How**:

| Mode | Shows |
|---|---|
| Notice | a note above the page, closable; it stays away for `adblock_dismiss_hours` |
| Modal | a dialog that can be closed after `adblock_grace` seconds; stays away likewise |
| Block | a dialog that stays until the blocker is off and the page reloaded |

The console never asks while "Not on the Console" is on, and a parked server's page never does.

## In-game messages

With **In-Game Messages** on, the `send_in_game_ads` task (every minute, at second 30) sends `say
<message>` to running servers of eggs with the `eula` feature (Minecraft) whose owner is neither an
admin nor a member. Each server gets it once per `in_game_interval` minutes, spread by its id over
the interval so a node never gets them all at once. The default message is "This server is hosted
on" and the panel's name.

## Settings

Admin › Settings › Ads, one tab per part: Placement, Adsterra, Monetag, Waiting Room (with the day
pass: Sell a Day Pass `day_pass_enabled` on, Day Pass Price `day_pass_tokens` 1 to 1000, default 1,
Day Pass Length `day_pass_hours` 1 to 720, default 24), Ad Blockers, In-Game, Promotions (the
membership popup, the offers and the promo clips, [promos.md](promos.md#settings)). Script fields take the URL or the whole snippet as the network shows it; zones take
Adsterra's Banner snippets or `key:WIDTHxHEIGHT:host` lines (default host
`www.highperformanceformat.com`, the script is `https://host/<key>/invoke.js`), or
`key:WIDTHxHEIGHT:https://host/path` with the snippet's script URL (newer Adsterra code loads
`https://host/22/<key>`). The native unit writes into `container-<id>`, the id being the
script's (`https://host/<id>/invoke.js`, or `https://host/21/<id>` in newer code) unless a native
container is set. Saving checks everything and says what is wrong.

## Check

- Ads off: no frame, no script, no notice anywhere; `/api/ads/unit?zone=x` is 404.
- One 728×90 and one 300×250 zone: the server list shows the rectangle in the page and the
  leaderboard at the foot, nothing above the page and nothing twice; a phone gets the 320×50 or
  nothing. A live sponsored top banner shows above the page.
- Units per Page 0: the server pages strip and the page rectangles stay empty, the foot still
  shows; back to 2, they fill again.
- A zone that fills nothing folds after a few seconds.
- A slot banner of 600×500 replaces the 300×250 zone; its click counts and lands on the link.
- Waiting room on, lines 0: a start opens the room, the server starts at once, the room lets you
  through after 15 seconds once it runs; the next start within 10 minutes goes straight to the
  console.
- Day pass: with 3 tokens, start a server and press "Skip the wait for 24 hours · 1 token" in the
  room: the toast names the end, the console opens at once and the balance shows 2; a start of
  another server skips the room for 24 hours. With 0 tokens the button is "Earn tokens". As a Plus
  member the card also says "Premium skips this". Sell a Day Pass off: no card (Premium's line
  stays while it applies).
- An ad blocker: the notice (or dialog) appears; not on the console.
- In-game on, interval 5: a running Minecraft server of a non-member gets the line in chat.
