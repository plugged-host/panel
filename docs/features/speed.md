# Page answers in the HTML

A page of the panel used to need three or four rounds before it could be drawn: the HTML, then the
shell's data (settings, languages, icons, the account), then the account's balance and settings,
then the page's own data (the server list, or the server a server page shows). Every round costs the
visitor a round trip to the panel. The panel now puts those first answers into the page's HTML, so
the server list is drawn after one round and a server page after two (its websocket and, for the
file manager, the files Wings lists come after).

## How

`backend/src/prefetch.rs`, called by the frontend fallback in `backend/src/lib.rs` for every page
(`GET` of the frontend's `index.html`):

1. Always `/api/settings`, `/api/languages` and `/api/icons`.
2. With a session cookie (not on `/auth/*`): `/api/client/account`, `/api/client/account/balance`
   and `/api/client/account/settings`; on `/server/{id}/…` also `/api/client/servers/{id}`.
3. On the dashboard (`/` without a query), once the account settings are known, the server list's
   first page as the dashboard asks for it: `/api/client/servers?other=…&page=1&per_page=…` from
   `dashboard::server_list_show_others` and `table::per_page`; not when the account starts on the
   grouped view.

The answers come from the panel's own routes, called in-process (`State::send_router_oneshot`) with
the visitor's cookie and address headers: authorization, permissions, rate limits and the answers'
shape are exactly what the browser would get. Only `200` JSON answers under 256 KiB that come within
400 ms are used; the account's answers only while `/api/client/account` succeeds (an expired session
then signs in as before). Nothing that asks Wings is inlined.

They go into `<script type="application/json" id="prefetched">` (every `<` escaped), the
`<link rel="preload">` of an inlined answer is dropped, and a page with the account's answers is sent
with `Cache-Control: private, no-store`.

The frontend (`frontend/src/api/prefetched.ts`) reads the script once and removes it; an axios
request interceptor (`frontend/src/api/axios.ts`) answers the first `GET` of an inlined URL from it,
within 15 seconds of the page loading, and never while impersonating someone. URLs are compared
normalized: path, then the non-empty query parameters sorted by name. Everything else, and every
later request for the same URL, goes to the server as before.

## Prepared statements

Each database connection keeps the queries it prepared (sqlx's statement cache). sqlx keeps 100 per
connection, fewer than the panel's distinct queries between two uses of a connection, so a page's
queries were often evicted and planned again: the server list took about 10 ms instead of about
3 ms, planning (3 ms) costing ten times its execution. `shared/src/database.rs` keeps 512.

## Measured

`docs/benchmarks.md` has the numbers against the Pelican panel.

## Check

- Open the dashboard with the browser's network tab: no request for `/api/settings`,
  `/api/client/account`, the balance or `/api/client/servers`; the list is there.
- Open a server's console: the only API requests are the websocket token and the announcements.
- Change the server list's page size, reload: the list asks the server itself (or comes along with
  the page if the setting reached the account), with the chosen size.
- Impersonate a user (Admin › Users): the panel shows that user's data, not yours.
- Sign out in another tab, reload: the sign-in page, no error.
