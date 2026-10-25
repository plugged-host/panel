# Reviews

Asks players to review the host on a public review site (Trustpilot, Google and the like), the way
those sites allow it: everyone the same way at the same point, in neutral words, with nothing given
in return.

- **The prompt**: a callout above the server list (`/`, `/all`, `/grouped`): "How is {app} working
  for you?", "Tell other players how it went in a review on {site}, good or bad." and a secondary
  "Write a Review" button, which opens the review page in a new tab. Its close button and the
  button both put it away.
- **The link**: "Review {app} on {site}" in the menu under the user's name, for everyone, always
  (while reviews are on and the account is not suspended).
- **neo** ([neo.md](neo.md), in the panel chat and on Discord) knows the review page and gives it
  when someone asks where to review the host. It never brings reviews up by itself: an assistant
  that chose when to ask would ask the people it judged happy, which is review gating
  (`shared/src/neo/prompt.rs`, `ReviewFacts`).

## Who sees the prompt, and when

The prompt shows when all of these hold:

| Condition | Why |
|---|---|
| Admin › Settings › Accounts › Reviews has a review page | Off by default |
| The account is at least **Ask After** days old (3) | People need time to use the service before they are asked |
| The account has a server, its own or one shared with it | It has used the panel |
| It was never closed or followed, or that was at least **Ask Again After** days ago (180; 0: never again) | One ask per stretch of use, not a nag |
| The account has no admin permissions (`admin`, or a role with any admin permission) and nobody is impersonating it | Review sites forbid asking staff; an impersonating admin could not close it for the account anyway |
| The welcome tour's first part is not running | One thing at a time |

Nothing about how the account is doing (its servers' state, suspensions, spending, support
history) is looked at: a review site counts asking only happy customers as "review gating" and
removes the reviews. Keep it that way: no rating step before the review site, no "if you like us",
no star counts in the text, and no credits, tokens or perks for a review (not even for "any
review").

## State

Per account in the user settings (`user_settings`, `PATCH /api/client/account/settings`), key
`dashboard::review_prompt_closed`: when the prompt was last closed or followed (ISO 8601). It is
kept with the account, not only in the browser, so closing it on one device closes it on every
device. The prompt waits for the account's settings to load before it shows. It asks for one
server (`GET /api/client/servers?per_page=1`) only when every other condition already holds.

No new table, route or permission: the setting is upstream's per-user settings store, and the
prompt only reads what the account may already read.

## Settings

Admin › Settings › Accounts › Reviews (`reviews::*`):

| Field | Key | Default | |
|---|---|---|---|
| Review Page | `reviews::url` | empty (off) | The https address where a review is written (a review site's "write a review" link, for example `https://www.trustpilot.com/evaluate/panel.example.com`). Empty turns the prompt and the menu link off |
| Site Name | `reviews::site` | empty | The site's name players see; empty: the review page's host without `www.` (`trustpilot.com`) |
| Ask After (Days) | `reviews::delay_days` | 3 | 0 to 365 |
| Ask Again After (Days) | `reviews::interval_days` | 180 | 0 to 3650; 0: a closed prompt never comes back |

`GET /api/settings` carries `reviews: {url, site, delay_days, interval_days}`, `site` already
resolved (`AppSettingsReviews::site_name`). The admin API refuses a review page that is not an
https address.

## Without noprofit.space

Nothing differs.

## Check

- Admin › Settings › Accounts › Reviews: save `http://example.com`: refused; save an https review
  page and a site name. The menu under your name shows "Review {app} on {site}" at once and opens
  the page in a new tab.
- As an admin the server list shows no prompt. Sign in as a player whose account is older than
  Ask After and who has a server: the prompt shows above the server list (not on other pages), with
  one secondary button and a close button.
- Close it: it is gone after a reload, and on another browser signed in to the same account.
  "Write a Review" opens the page and puts it away the same way.
- A new account, or one without a server, sees no prompt; nor does anyone while the welcome tour's
  first part runs.
- With Ask Again After at 0, a closed prompt stays away for good. Clearing the review page removes
  the prompt and the menu link for everyone.
