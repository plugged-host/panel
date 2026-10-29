# Credits, tokens, memberships and the Resource Store

Two currencies (`shared/src/economy.rs`, `shared/src/earn.rs`, `shared/src/pricing.rs`):

- **Credits** buy CPU, memory and disk in the **Resource Store**, for good, and the second
  membership. Without noprofit.space they are the panel's own; with it they are the account's
  noprofit.space credits and the store is noprofit.space's.
- **Tokens** are earned through a survey wall (CPX Research) and sponsored links (Linkvertise, Vylo,
  Cuty, ShrinkEarn, ShrinkMe and AdFoc.us) and buy the memberships and the panel itself. They are
  always the panel's own.

Two memberships, always the panel's own. Their names are settings (plugged.host calls them Plus and
Premium; Premium was called PlusPlus until 3.0.4); in code, tables and routes they are `plus` and
`plusplus`. Both are bought for some days at a time (plugged.host: a month), buying again adds to
the time left, and nothing renews by itself unless its member switches on [auto-renew](#auto-renew).
Neither needs the other: the second gives everything the first does.

| | First (`plus`) | Second (`plusplus`) |
|---|---|---|
| Bought with | tokens (`plus_tokens` for `plus_days`) | tokens, or credits when `plusplus_credits` is above 0 |
| Location tier it opens | `plus` | `premium` and `plus` |
| Start queue priority | 1 | 2 |
| Burst cores on every server | 1 | 2 (the total, not 1 + 2) |
| Ads ([ads.md](ads.md#who-sees-ads), while Fewer Ads for Members is on) | only the waiting room before a start: no banners, popups or in-game ads | none, and no waiting room |
| Archives, unpacking and backups | faster (more threads on the node, [wings.md](../wings.md)) | faster still |
| Panel access | included | included |
| Idle servers ([limbo.md](limbo.md)) | stopped, parked and deleted like everyone's (Spare Members off) | the same |

A location's tier is set on the location ([node-telemetry.md](node-telemetry.md)); a tier the
economy does not know stays closed to players (`server_creation::tier_unlocked`). Admins place
servers anywhere.

Until 3.0.4 the first membership was bought once and kept for good (`plus_since` only). The
`monthly_plus_data` migration gave every such account 14 more days (`plus_until`), reset the
membership prices to the new defaults (the first 15 tokens for 30 days, the second 30 tokens for
30 days and not for credits), renamed a second membership still called PlusPlus to Premium, and
turned Spare Members off.

## Burst cores

CPU above a server's paid limit, 100 % per core, that the server uses while its node has CPU to
spare: a game's startup or a busy evening runs faster without buying more. Every server of the
account gets them (servers with unlimited CPU get nothing added). They are outside the allowance,
the store and its price curve; Admin › Nodes and the allowance count the paid CPU only.

- The account's count is `users.burst_cores`, kept by `economy::sync_burst`: right after a
  membership is bought or an admin changes one, and every minute for all accounts (a PlusPlus that
  ran out drops to Plus's count; changed settings reach everyone). A changed count syncs the
  account's servers to Wings, which applies the new limit to running containers.
- Wings gets `cpu_limit` = paid + 100 × burst cores, so stock Wings already lets a server burst. The
  Wings fork also takes `build.cpu_weight` (Docker CPU shares, 1024 per paid core, from the paid
  limit alone), so when the node is busy paid CPU wins over burst, and `build.cpu_burst`, which its
  public resources leave out of the allocated CPU ([wings.md](../wings.md)).
- Upstream's CFS burst (`docker.cfs_burst`) and startup boost are separate and stay as they are.
- The server card, the console's CPU card and the CPU graph show "paid + burst"
  (`limits.burst_cpu` on the server).

### Faster archives and backups

Archiving, unpacking and backups run in Wings on the node, outside the server's container, so the
server's CPU limit never applied to them; the node's thread counts did. The Wings fork reads the
`build.cpu_burst` the panel already sends for burst cores and gives a member's server
`1 + cpu_burst / 100` times those counts, up to the node's cores ([wings.md](../wings.md)). With
the node defaults (2 threads to archive, 4 to unpack, 4 per backup):

| | Free | Plus (1 burst core) | Premium (2) |
|---|---|---|---|
| Archive (zip, tar, 7z) | 2 | 4 | 6 |
| Unpack, Wings-side backups | 4 | 8 | 8 on an 8-core node, 12 on 16 cores |
| restic backups: files read at once | 2, at a lower CPU priority | 4 | 6 |

Nothing to set in the panel: it follows the burst cores and changes with them on the next sync.
The lower priority only matters while the node is busy, so members' backups go first; on a quiet
node a free server's backup is as fast as before. Separately, for every server, a ZIP entry of
64 MiB or more (a world database, a big log) is now deflated on all of the archive's threads
instead of one, which is what made big zips slow. A node turns all of it off with
`api.burst_operations: false` (the big-entry change stays).

## The Resource Store

Account › Resource Store is laid out like the Pelican panel's, top to bottom:

1. **Unlock more resources (N)**: a bar, closed until opened (the choice is kept in the browser),
   with the upgrades the account has not claimed yet, each a row with a gray button: linking Discord
   or joining the rest of the Discord servers for what they add, credits per invited friend (and per
   boost), and what the next step of memory costs. The bar is gone once the list is empty. The
   Discord rows need noprofit.space's store to send the account's Discord rewards (`discord`,
   [noprofit.md](../noprofit.md#wire-format)); without noprofit.space only the memory row is offered.
2. **Credits**: the balance, what credits buy and, with noprofit.space, how to earn them on Discord
   with a button to noprofit.space's Discord page.
3. **Your resources** (only with a plan): one row per resource with one meter for three numbers:
   the solid part is what the account's servers use, the tinted part the rest of its limit, the
   track runs to the soft limit where prices get steep (further when the account is past it). Over
   the limit the number and the meter turn red and the row says how much to shrink. When noprofit.space's
   plan shares a pool with the account's other services (`total_*`, `elsewhere_*`), the meter shows
   the whole pool with what they use counted as used, as the Pelican panel and the sidebar do
   (`allowance::with_pool`); what is left is still this panel's share less its servers. Beside it the offers (1, 2 and 4 steps)
   as gray buttons with their price; under it how the price changes from here. While the store is
   closed the meters stay, without offers.
4. **Credit history**: the last 10 transactions as rows (what, when, the balance after, the amount).

A purchase is confirmed ("Are you sure you want to purchase +50% CPU for 100 credits?") and sent
with the price the player saw: a price that moved since buys nothing (409, the page shows the new
prices). Either way a toast says how it went.

### Sales

Admin › Settings › Economy › Resource Store › Sale takes a percentage off every price in the store
for a while: **Discount** (0 to 100 %, 0 is off), **Name** (what the store calls it, such as
"Halloween special"; empty: "Sale"), **Starts** and **Ends** (in the admin's browser time zone;
either may be empty: right away, and until the discount is set back to 0) and **Covers** (the
Resource Store, the memberships, or both; the Resource Store by default, as before the choice
existed). It works with and without noprofit.space. While it covers the memberships it works there
as in the store ([membership sales](#membership-sales)).

- The price of an offer is `round(full × (100 − discount) / 100)`, half up
  (`pricing::discounted`); an unaffordable price stays unaffordable. 100 % makes the offers free
  (the history shows a 0-credit purchase).
- While it runs, a banner in the accent colour heads the store page (the name as its title, "25% off
  everything in the store until 2 Nov 2026, 22:00.") and every offer shows its full price struck
  through in front of the sale's.
- The window is checked on every store request and purchase (`AppSettingsEconomy::store_discount_at`,
  0 while the sale covers the memberships only):
  a price shown before the sale started or after it ended has moved, so the purchase answers 409
  and the page shows the prices as they are now.
- Without noprofit.space the panel applies it; the ledger row's data keeps `full_cost` and
  `discount`. With noprofit.space the panel sends `discount` with its store and purchase requests
  and noprofit.space prices and charges with it ([noprofit.md](../noprofit.md#wire-format)); the
  sale is shown only when noprofit.space answers with the same `discount`, so an accounts app that
  does not know it simply sells at full price.

**Without noprofit.space** the account has the games allowance (Admin › Settings › Hosting › Games)
plus what it bought (`users.extra_cpu`, `extra_memory`, `extra_disk`); a resource the allowance does
not limit is not sold. Prices follow one curve per resource, the same as noprofit.space's
(`shared/src/pricing.rs`): from the lowest price up to the step price at the limit, then climbing to
the soft limit, then doubling. A price is the area under the curve, so four steps at once cost what
four single ones would. A purchase locks the user row, checks the price and the balance, and writes
the credits, the resources and the ledger row together.

**With noprofit.space** the page shows noprofit.space's store for the account (its credits,
catalog and history) and buys through it; the account's plan grows there, and the panel pulls the
account again right after, so the new allowance counts at once. When noprofit.space does not answer
the page shows the credits the panel last synced and the meters, and offers nothing. An account
without a plan sees no resources at all.

## Tokens

Account › Tokens is laid out like the Pelican panel's: the balance and what tokens pay for, the
ways to earn (one row each: the reward, today's count against the cap, and "Earn with …"; the first
one's button is the page's filled one, a capped one says "Come back tomorrow", a Full Script link
that could not be made "Unavailable right now"), and the recent activity (the last 10 token
transactions, only when there are any). Static links also get a gray button in the page header. The
way back from a link ends in a toast with the result. **Daily tokens** (a setting, 0 by default) are
given once a day, the first time the account looks after midnight UTC.

The survey wall, when it is set up, is the first row, so its button is the filled one: it pays the
most. Its row says it "Pays what each survey shows", today's finished surveys against the cap ("0 of
1 today"), that the tokens arrive once CPX Research confirms the survey (usually within minutes) and
that one survey a day pays while a screen-out pays a small bonus and does not count. "Take a survey"
opens the wall in a new tab (in the same tab when the browser blocks it); the Tokens page and the
sidebar's balance load again each time the page is shown again, so the tokens show up when the user
comes back ([CPX Research](#cpx-research)).

Earning through a link works with claims (`user_token_claims`):

1. **Earn** opens a claim: a 64-character nonce bound to the account, and returns where to go. The
   provider sends the player back to `/account/tokens?claim=<nonce>&…` with its proof.
2. The page posts the claim and the other parameters to `/redeem`. Everything local is checked
   first: the claim is this account's, still pending and younger than **Minutes to Finish a Link**,
   the proof has the right shape and was not used before, today's cap is not reached. The provider
   is asked last, once, since its check may use the proof up.
3. Granted: the claim and the tokens change in one transaction; the unique index on
   `(provider, token_hash)` stops one proof paying twice even when two requests race.

Opening claims is throttled per account and provider (3 × the daily cap per hour, at least 5).

### What a link pays

- **Fractions.** A provider's **Tokens per Link** takes up to 2 decimals (`1.5`). Balances stay whole
  tokens: each link adds its reward to the account's saved part (`users.token_fraction`, in
  hundredths) and pays the whole tokens in it, so 1.5 pays 1, 2, 1, 2 … (`earn::split_reward`). The
  Tokens page shows a saved part ("0.5 tokens saved toward your next token"); a link that pays no
  whole token yet still counts toward the daily cap.
- **Less for each further link of the day.** Advertisers pay less for a person they already showed
  ads to, so the n-th link of the day on one provider pays **Less per Further Link** percent less than
  the one before (15 by default), never under **Lowest Reward** percent of the full reward (25)
  (`earn::link_reward`, integer steps rounded once). At 1.5 tokens: 1.5, 1.28, 1.08, 0.92, 0.78, 0.67,
  0.57, 0.48, 0.41, then 0.38. Days start at midnight UTC, per provider. The Tokens page shows what
  the next link pays, with a note while it is under the full reward; 0 pays every link in full.

| Provider | How it goes | How the proof is checked |
|---|---|---|
| CPX Research (a survey wall, not a link) | **Earn** answers the wall, `https://offers.cpx-research.com/index.php?app_id=<id>&ext_user_id=<the user's uuid>&secure_hash=<md5("<uuid>-<secure hash>")>&username=&email=&subid_1=&subid_2=` (no username or email is sent), and opens no claim (`nonce` is empty), throttled like a claim. No Tokens per Link, no Less per Further Link: each survey pays what the wall shows | CPX's postback, server to server, signed with `md5("<trans_id>-<secure hash>")` ([below](#cpx-research)) |
| Linkvertise, link | The settings' Linkvertise link, with `{return}` or `{return_b64}` replaced by the way back | Linkvertise's anti-bypass API with the API token |
| Linkvertise, Full Script | With a publisher id the Tokens page loads `linkvertise.js`, which turns the page's own earn link (`…&lvr=1`, matching the whitelist) into a Linkvertise link. The claim is opened when the page loads and reused on reloads | The same API |
| Vylo | A short link made for every trip through Vylo's API, to the way back plus `vt=<proof>` | The proof is an HMAC of the nonce with the panel's key, so only a trip through the link has it; a trip faster than **Shortest Trip** does not count |
| Cuty, ShrinkEarn, ShrinkMe | The same as Vylo, through `https://{cuty.io,shrinkearn.com,shrinkme.io}/api?api=<key>&url=…` | The same HMAC, but it names the provider: a proof from one shortener cannot settle another's claim; each has its own **Shortest Trip** |
| AdFoc.us | The same, through `https://adfoc.us/api/?key=<key>&url=…`, which answers the short link as plain text (`0` when it refuses). It refuses an encoded `url` and may cut a raw one at `&`, so the way back carries the proof in the claim: `?claim=<nonce>.<proof>` | The same HMAC |

The page's `script-src` allows `https://publisher.linkvertise.com` only while Linkvertise is on with
a publisher id and a token (`backend/src/lib.rs`).

### CPX Research

A survey wall (`earn::survey_postback`, `GET /api/earn/cpx-research`). CPX works out what each
survey pays in tokens from the app's **Currency Factor** and shows it on the wall; when a survey is
done or taken back, CPX's servers call the panel's postback URL. Set up in CPX's publisher
dashboard, on the app's page:

- **Postback Settings**: the URL Admin › Settings › Economy › Tokens shows (click to copy):

  ```text
  https://panel.example.com/api/earn/cpx-research?status={status}&trans_id={trans_id}&user_id={user_id}&amount_local={amount_local}&type={type}&ip_click={ip_click}&hash={secure_hash}
  ```

  The postback names the account (`user_id`, the wall's `ext_user_id`), so no claim is needed.
  `secure_hash` for `hash` and `ext_user_id` for `user_id` are read too. CPX's servers must reach
  the panel: a proxy, CDN or firewall in front of it must let `/api/earn/cpx-research` through
  (CPX's dashboard lists the addresses its postbacks come from).
- **Reward Settings**: the currency's name, Token (plural Tokens), and the Currency Factor, the
  tokens a dollar of the publisher's earnings is worth. That decides what a survey pays; the panel
  pays `amount_local` as CPX sends it.

What the panel does with a postback:

1. **Refused** (403, logged as a warning) while tokens or CPX Research are off or not set up (on,
   App ID and Secure Hash), or when `hash` is not `md5("<trans_id>-<secure hash>")` (compared in
   constant time). CPX signs only the transaction id, so everything else in it is trusted only once
   per transaction.
2. **`status=1`** (credited): `amount_local` in hundredths, rounded down (`earn::survey_amount`, at
   most 10 000 tokens a postback). Inside one transaction with the user row locked (`FOR UPDATE`),
   so two postbacks at once cannot both take the day's last survey, it is kept in
   `user_token_claims` (provider `cpx_research`, `token_hash` = the transaction id's hash, `ip` =
   `ip_click`) and paid like a link: the whole tokens now, the rest saved on the account
   (`users.token_fraction`), in the ledger as `earn:cpx_research` ("Earned with CPX Research"). The
   unique index on `(provider, token_hash)` makes a repeated postback pay nothing.
   - **The daily cap** (**Surveys per Day**, 1 by default; days start at midnight UTC) counts only
     finished surveys. A granted claim keeps CPX's `type` in `reason` (`complete`, `out` or
     `bonus`), and the cap counts granted `complete` claims of the day. `out` (a qualified
     screen-out's small bonus) and `bonus` (a rating's, say) are paid and never use up the day, so a
     screen-out does not lock the user out. A `type` that is missing or that CPX does not document
     counts as `complete`, so the cap holds whatever the postback URL sends.
   - A `complete` over the cap is kept as `rejected` with reason `daily_cap` and pays nothing; an
     amount of 0 or one that is not a number is kept as `rejected` (`no_amount`). Both are
     answered as handled.
3. **`status=2`** (reversed, for fraud found later, say): the transaction's claim becomes
   `reversed`, and the whole tokens it paid are taken back, never more than the balance (what was
   spent stays spent; `earn::survey_taken_back`), in the ledger as `earn_reversal` ("Reversed: CPX
   Research"); `tokens_earned` goes down by what it paid. The part of a token it added to the saved
   part stays. A reversed survey no longer counts toward its day. A transaction never credited is
   kept as `reversed` (reason `before_credit`), so its credit coming late pays nothing.
4. Everything else it signed is answered `200` with the body `1`: a repeated postback, a reversal
   of something that paid nothing, an unknown user (logged as a warning), another `status`. A
   database error, or a credit that came in while its reversal was being kept, answers `500`, so
   CPX may send it again.

A grant is logged in the account's activity as `account:tokens.earn` (`provider`, `amount`, `type`),
at `ip_click`.

### Daily streak

`shared/src/streak.rs`. Under the balance, "Daily streak" with **Claim today's tokens**, "Day 3 · 0.5
tokens" and a dot per day of the week (claimed ones in the accent, today's ringed while it waits).
While today's tokens wait, the claim button is the page's filled one and the ways to earn are all
gray; after the claim the row says "Day 3 · 0.5 tokens · come back tomorrow for 0.5" and the first
way to earn is filled again. The sidebar's tokens row has a small accent dot while a claim waits
(`streak_claimable` in `GET /balance`).

- One claim per UTC day. A claim the day after the last one is the next day of the streak; a missed
  day starts again at day 1 (`users.streak_day`, `streak_claimed`).
- Day 1 to 7 pay 0.25, 0.25, 0.5, 0.5, 0.75, 0.75 and 1 token, then 1 a day (**Tokens per Day**, up to
  14 amounts; the last repeats). The amount goes through the saved part like a link's
  (`earn::split_reward`): 0.25 adds a quarter token, and whole tokens are paid when they add up.
- The account row is locked while a claim is written, so two clicks pay once (409 "today's tokens
  are claimed already"). Ledger type `streak` ("Day 3"), activity `account:tokens.streak`.
- **Daily Tokens** (above) is separate and unchanged.

### Double tokens

A window when every sponsored link and every CPX Research survey pays more: **Bonus** percent (100
doubles them, 0 is off), a **Name**, **Starts** and **Ends** (either may be empty: right away, and
until the bonus is set back to 0), on Admin › Settings › Economy › Tokens, like the store's sale.

- `economy::token_bonus_at(&settings.economy, now)` answers the window running at `now` (`{percent,
  name, starts, ends}`, `None` outside it or while tokens are off); `AppSettingsEconomy::token_bonus_at`
  answers its percent. rux's announcement of a starting window uses it ([rux.md](rux.md)).
- The bonus is applied when the tokens are paid (a link's redemption, a survey's postback), to the
  reward in hundredths before the split into whole tokens and the saved part (`earn::boosted`, half
  up): 1.5 tokens doubled pays 3. The daily caps count links and surveys as before. The ledger row's
  data keeps `bonus` (the percent).
- While it runs, an accent banner heads the Tokens page ("Double tokens until Sun 22:00", the name as
  its title when it has one; a later end shows the date), and every row shows what it pays now:
  the links' rewards include the bonus, the survey row says "Every survey pays 100% more right now."
- The streak, the first-week tasks and referrals pay what they say, without the bonus.

### Getting started

`shared/src/starter_tasks.rs`. For **Days Shown** (14) after sign-up, a "Getting started" list under
the balance: link your Discord account (while Admin › Settings › Accounts › Discord is on), create a
server, start your server, earn tokens once with a link or a survey. Each row says "+1 token"
(**Tokens per Task**) and has a gray **Claim** when it is done, else a gray button to where it is
done (Account › Discord Rewards, the create page, the server list, the ways to earn below).

- **Claim** checks the task again: a linked Discord account ([discord.md](discord.md)), a server the
  account owns, a start of one of its servers (its power button, or a start the queue or the waiting
  room let through, in the server's activity), a granted link or survey.
- Each pays once per account: the ledger row (`starter:<task>`, "Create a server") is the record,
  written with the account row locked (409 "this task is paid already").
- The list is gone once every task is claimed or the days passed. Activity `account:tokens.task`.

### Referrals

A section at the end of the Tokens page: the account's link with a copy button, how it pays in one
line, and the friends who signed up with it. [referrals.md](referrals.md) has the rules.

## Memberships

Account › Membership is laid out like the Pelican panel's Plus page: a note while one runs ("Premium
is active, it runs until …"), the two memberships side by side (pitch, price, term, what each
gives, a buy button per offer: "30 days · 15 tokens"), a table of what each adds over the free tier
(locations, start queue, in-game ads, panel ads, the waiting room while it is on, CPU, archives and
backups, neo usage, the devbox while devboxes are on, panel access while it costs tokens), and the
questions people ask before buying (one says idle servers still stop, while limbo spares no
members). Terms read like Pelican's ("a month", "a week, or credits for two weeks"), from the
offers' days. The first button is filled until the account has a membership, then the second's.
An offer the balance does not cover is not a dead button: it says what is missing ("6 more tokens
to go") and leads to the Tokens page (the Resource Store for credits). A purchase is sent with the
price the user saw (`price`): one that moved since (a sale started or ended) buys nothing (409, the
page loads the new prices), as in the store. Buying locks the membership row; the tokens or credits
and the membership change together or not at all (`economy::buy_membership`, shared by purchases,
gifts and renewals). With noprofit.space, credits for the second membership are taken by
noprofit.space (`/credits/spend`) with an idempotency key per purchase, so a retry after a failure
is not charged twice.

### Membership sales

While the [sale](#sales) covers the memberships (**Covers**: the memberships, or both), the
Membership page opens with the same accent banner as the store ("25% off every membership until 2
Nov 2026, 22:00."), each plan's price and offer shows the full price struck through in front of the
sale's, and purchases, [gifts](#gifts) and [automatic renewals](#auto-renew) charge
`pricing::discounted(full, percent)`, half up (15 → 11 at 25 %), always worked out by the panel
(`AppSettingsEconomy::membership_discount_at`, `economy::membership_offer`). With noprofit.space the
second membership's credit price gets it too: the panel sends the discounted amount to
`/credits/spend`. The ledger row's data keeps `cost`, `full_cost` and `discount`.

### Auto-renew

Opt-in per membership, off until its member switches it on: while a membership runs, its card on
the Membership page has a **Renew automatically** switch ("As it ends, it renews at its price then
(15 tokens today), if your balance covers it."). The `settle_memberships` task (every minute) first
renews every membership switched on that ends within the next 10 minutes, or ended less than an
hour ago (the task could not run then), before it settles lapses, so a renewed membership never
lapses:

- It is bought like a purchase, in one transaction, at the price it has then (a sale applied), with
  tokens (the second with credits when only credits buy it), and only while it is still switched on
  and still ends when the task read it (bought or renewed meanwhile: nothing). The ledger row says
  "Premium (30 days), renewed automatically" (`data.automatic`).
- When the balance is short, it is tried once per end (`plus_renew_failed`, `plusplus_renew_failed`
  keep the end that failed) and the member gets `membershipRenewFailed` ("Couldn’t renew Plus: 6
  more tokens needed") once; then the lapse takes its course. Switching the switch off and on again
  clears it, so the next minutes try again. A suspended account is not charged.
- Admin › Settings › Economy › Memberships › Auto-Renew off: no switches and no renewals; the
  members' choices are kept.

### Ends-soon notices

**Hours Before the End** (48) before a membership ends, the member gets one `membershipEndsSoon` per
end (`plus_reminded`, `plusplus_reminded` keep the end it was sent for): the date, whether it renews
by itself and whether the balance covers its price now, with a **Renew** link to the Membership
page. The first membership is not mentioned while the second runs past its end. The same goes for
paid panel access (`panelAccessEndsSoon`, `users.panel_access_reminded`, with a link to Account ›
Panel Access; not for admins, suspended accounts or accounts whose membership runs past the end;
`panel_access::remind_ending`). Both are sent by the `settle_memberships` task.

### Gifts

**Gift to a friend**, under the plans: a username or email (exactly, case aside; an existing
account that is not suspended and not the buyer's own) and the membership with its term ("Plus · 30
days · 15 tokens"). The buyer pays the normal price in tokens, the sale's while one covers the
memberships, after a confirmation; only memberships are given, never tokens. The recipient's
membership is extended like after a purchase (burst cores, the start queue and the user cache
follow), they get `membershipGifted` naming the buyer's username, and both get a ledger row
(`membership_gift`: the buyer's tokens; `membership_gift_received`: a row of 0 tokens for the
recipient) and an activity entry (`account:membership.gift` for the buyer, `account:membership.gifted`
for the recipient). **Gifts per Day** (10) limits each buyer over the last 24 hours (counted from the
ledger with the buyer's row locked, so two at once cannot both pass); looking up who a gift is for
is limited to 30 an hour per account, and the balance is checked before anyone is looked up, so an
account that cannot pay learns nothing about which names exist. The page says how many gifts are
left today.

When a membership runs out, the `settle_memberships` task (every minute):

1. works out the location tiers the account lost (`lost_tiers`: `premium` once the second ran out,
   `plus` too once neither runs) and queues an admin move of every server on such a location to a
   location the owner may use and that takes servers: the same country first, then `plus`
   locations while the first still runs, then the fastest ([transfers.md](transfers.md));
2. counts the lapse as settled (`plus_settled`, `plusplus_settled`) once no server is left on a lost
   location; until then it comes back, so a full location is tried again. Buying again before that
   stops it. The owner gets `membershipEnded` naming the membership.

### Memberships across plugged.host and switched.cloud

The **membership bundle** (`shared/src/membership_bundle.rs`) goes both ways: plugged.host's
Premium includes switched.cloud Plus, and switched.cloud Premium includes plugged.host Plus.
switched.cloud knows its users by their noprofit.space account, so the bundle needs
noprofit.space; without it, or until Admin › Settings › Economy › Memberships › Included With the
Second Membership is switched on with the other panel's URL and a shared secret of at least 32
characters (the same on both panels; it signs both directions), nothing is sent, taken or shown.

**Sending** (Premium here → Plus there):

- The `sync_membership_bundle` task (every minute) sends the other panel the end of the second
  membership (`plusplus_until`) of every account linked to noprofit.space whose end differs from
  what the other panel last confirmed (`user_memberships.bundle_synced_until`), however it
  changed: bought, given, renewed by itself, set or ended by an admin. Once a day it sends every
  one that still runs again (`bundle_synced` older than a day), so a lost answer heals by itself.
  An ended membership goes as its past end, or `null` once an admin cleared it, and the other
  panel ends its Plus too.
- **Delivered** means a 2xx answer: only then are the batch's rows marked (with the end that was
  sent, so a change made meanwhile goes the next minute). Anything else (the other panel down, a
  wrong secret, its side switched off) is sent again the next minute and logged as the task's error.
- The Membership page, while the bundle runs: the second membership's perks end with "{name}
  included" (the Name setting, "switched.cloud Plus included") and the table gets a row for it,
  included with the second membership only (`bundle_name` in `GET /api/client/account/membership`,
  `null` while the bundle is off).

**Receiving** (Premium there → Plus here), `POST /api/memberships/bundle` (no session, 60 requests
a minute per address):

- 404 while the bundle is off or not set up here, 401 for a wrong signature or a timestamp more than
  300 seconds off (compared in constant time), 422 for a body it cannot read, else `{"applied": n,
  "pending": m}`.
- A future `premium_until` sets `user_memberships.bundle_plus_until` of the account's user; `null`
  or a past date ends a running one now and leaves an ended one as it was. An account with no
  user here yet waits in `pending_membership_bundles` and is applied within a minute of a user
  being linked to it (by the same task); one that ended meanwhile is dropped.
- That Plus is Plus everywhere (`Membership::has_plus`, the users' `has_plus`): the `plus`
  locations, queue priority 1, the first membership's burst cores, the ads audience (`room`), no
  panel access fee, Spare Members, the devbox, neo and rux. When it ends, `settle_memberships`
  settles it like a lapse of Plus (servers leave the `plus` locations unless Plus bought here
  still runs; `plus_settled`). Plus bought here neither gets an ends-soon notice nor renews by
  itself while the bundle's Plus runs past its end; buying Plus adds to what was bought, as before.
- The Membership page says "Plus is included with your switched.cloud Premium until …" (the
  setting Their Second Membership's Name, `bundle_source_name`) and fills the second
  membership's button instead of the first's; Admin › Users › a user › Economy shows it read-only.
- Switched off on either side, nothing more is sent or taken: Plus given already runs until the
  last end received.

The wire format, the same both ways (at most 100 accounts a request from here, 500 accepted;
`account_id` is the noprofit.space account id, `user_noprofit_accounts.account_id`):

```text
POST {panel URL}/api/partner/plugged-host/membership   (to switched.cloud)
POST {panel URL}/api/memberships/bundle                (to this panel)
Content-Type: application/json
X-Timestamp: <unix seconds>
X-Signature: hex(hmac_sha256(<shared secret>, "<X-Timestamp>.<raw body>"))

{"memberships":[{"account_id":42,"premium_until":"2026-11-28T19:00:00Z"},{"account_id":7,"premium_until":null}]}
```

switched.cloud answers the same way (`pending`: accounts that have not signed in there yet; it
keeps their end until they do). The signature's test vector is in the module's tests and in
switched.cloud's `src/plans/bundle.rs`.

## The sidebar

Over the sidebar's footer (dashboard and server), Theme v2's card: one row per limited resource of
the account (used / limit, red at the limit), and a row with the tokens and credits. Usage and
credits lead to the Resource Store, tokens to the Tokens page; a part whose page is off is plain
text, and a resource without a limit or a currency that is off is left out. Under the balances, a
line counts the tokens toward the cheapest membership bought with tokens that the account does not
have ("9 / 15 to Plus", over a thin bar; "Enough for Plus" once the balance covers it), linking to
the Membership page; none with the second membership (it gives everything) or with memberships or
tokens off (`membership_goal` in the balance, `economy::membership_goal`). It reads
`GET /api/client/account/balance` once a minute and after a purchase, a redeemed link or a
membership change. Suspended users don't see it.

## Admins

Admin › Users › a user › Economy (`users.economy`): credits, tokens, what was earned, both
memberships, what the panel's store sold, and both histories. Add or take away credits and tokens
with a reason the user sees, set the end of either membership or end it now. Admin
activity `user:economy`. With noprofit.space connected a credit change is sent to the user's account
there (type `admin`, the reason as its description; [noprofit.md](../noprofit.md#what-changes-while-it-is-connected)),
and the credit history is the changes made here that noprofit.space applied. That needs its account
sync on (the API secret) and the user linked; otherwise the credits field is hidden and the page
says to change credits on noprofit.space. A deduction may not take the balance (the last snapshot
plus the changes not sent yet) below zero.

## Panel access

The panel itself can cost tokens (`shared/src/panel_access.rs`, `users.panel_access_until`). One
payment buys **Panel Access Days** (30 by default) for **Panel Access Tokens** (5 by default), added
to what is left, so paying early loses nothing. Administrators and both memberships never pay.

The client API's auth middleware checks it after suspension, two-factor and email verification and
before a route sees the request: a blocked account gets `402 Payment Required` with
`X-Panel-Access: required`. It may still use `/api/client/account` (profile, settings, tokens and
earning, balance, memberships, panel access, logout) and `/api/client/permissions`; every server
route waits for a payment. The frontend's Axios interceptor turns the 402 into a redirect to
Account › Panel Access, remembering the page to return to. Buying invalidates the user cache so the
next request sees the new date.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/account/credits` | `economy.read` | `{store}`: `source` (`panel`/`noprofit`), `available`, `credits`, `catalog: [{resource, step, total, limit, soft_limit, offers: [{steps, amount, price, full_price}]}]`, `discount` (`{percent, name, ends}` while a sale runs, else `null`), `transactions`, `has_plan`, `usage: [{resource, used, limit}]` (the meters), `rewards_url` (noprofit.space's Discord page or `null`), `perks: [{id: discord_link\|discord_join, gain, url} \| {id: discord_earn, per_invite, per_boost, url} \| {id: store, price, amount}]` |
| `POST /api/client/account/credits/store` | `economy.purchase` | `{resource, steps (1–50), price}` → `{store}`; 409 when the price changed or the balance is short, 422 when noprofit.space refused |
| `GET /api/client/account/tokens` | `economy.read` | `{tokens}`: balance, earned, `claim_minutes`, `providers: [{provider, reward, next_reward, daily_cap, granted_today, survey, full_script, publisher, whitelist}]` (`survey`: CPX Research, `reward` 0, `granted_today` its finished surveys), `transactions` |
| `POST /api/client/account/tokens/earn` | `economy.earn` | `{provider}` → `{earn: {url, nonce}}` (`cpx_research`: the wall, `nonce` empty); 409 capped, 429 throttled, 502 the provider made no link |
| `POST /api/client/account/tokens/redeem` | `economy.earn` | `{claim, params}` → `{redemption: {status, amount, provider}}`, status `granted`, `settled`, `no_claim`, `expired`, `no_token`, `replay`, `capped`, `invalid` |
| `GET /api/client/account/tokens` (more) | `economy.read` | also `bonus` (`{percent, name, starts, ends}` while a double-token window runs, the providers' rewards with it), `streak` (`{day, reward, next_reward, claimable}`), `starter` (`{reward, ends, tasks: [{task, done, claimed}]}`), each `null` while off, and `referrals` (on or off) |
| `POST /api/client/account/tokens/streak` | `economy.earn` | → `{claim: {day, reward, amount, streak}}`; 409 claimed today already |
| `POST /api/client/account/tokens/tasks` | `economy.earn` | `{task: discord\|create_server\|start_server\|earn}` → `{claim: {task, amount}}`; 400 not offered, 409 not done, paid already or the days passed |
| `GET /api/client/account/tokens/referrals` | `economy.read` | `{referrals}` ([referrals.md](referrals.md#routes)), `null` while off |
| `GET /api/client/account/balance` | `economy.read` | `{balance}`: `credits` and `tokens` (`null` when off), `allowance` (the create page's), `membership_goal` (`{membership, name, cost}`: the cheapest membership the account does not have, or `null`), `streak_claimable` (today's [streak](#daily-streak) tokens wait) |
| `GET /api/client/account/membership` | `economy.read` | `{economy}`: balances, both memberships (`plus_until`, `plusplus_until` while they run), their prices (`plus_cost`, `plusplus_offers[].cost`, the sale applied, and `plus_full_cost`, `full_cost` without it) and days, `discount` (`{percent, name, ends}` while a sale covers the memberships), `auto_renew_enabled`, `plus_auto_renew`, `plusplus_auto_renew`, `gifts_enabled`, `gifts_left`, the burst cores of each and the account's now, the queue priority, `panel_access`, `waiting_room`, `idle_stop`, `idle_delete` (what the page's table and questions say) |
| `POST /api/client/account/membership/plus` | `economy.purchase` | `{price?}` (the price shown) → `{economy}`, `plus_days` more; 409 short or the price moved |
| `POST /api/client/account/membership/plusplus` | `economy.purchase` | `{pay: tokens\|credits, price?}` → `{economy}`; 409 short, not for sale that way or the price moved |
| `POST /api/client/account/membership/auto-renew` | `economy.purchase` | `{membership: plus\|plusplus, enabled}` → `{economy}`; 400 when switching on a membership that does not run, 403 while auto-renew is off |
| `POST /api/client/account/membership/gift` | `economy.purchase` | `{recipient (username or email), membership: plus\|plusplus, price?}` → `{economy}` (the buyer's); 400 the buyer's own account, 403 gifts off, 404 no such account, 409 short or the price moved, 429 the day's gifts or the lookups used up |
| `GET /api/client/account/panel-access` | `economy.read` | `{panel_access}`: `enabled`, `required`, `exempt`, `tokens`, `days`, `until` |
| `POST /api/client/account/panel-access` | `economy.purchase` | `{panel_access}` after charging the tokens; 409 short |
| `GET /api/admin/users/{user}/economy` | `users.economy` | The user's economy and both histories |
| `PUT /api/admin/users/{user}/economy` | `users.economy` | `{credits?, tokens?, reason?, plus_until?, plusplus_until?}` (`null` ends it now) |
| `POST /api/memberships/bundle` | none (public; signed with the bundle's shared secret, [the bundle](#memberships-across-pluggedhost-and-switchedcloud)) | `{memberships: [{account_id, premium_until}]}` → `{applied, pending}`; 401 a wrong signature, 404 off, 422 unreadable |
| `GET /api/earn/cpx-research` | none (public; signed by CPX, [CPX Research](#cpx-research)) | CPX Research's postback: `status`, `trans_id`, `user_id`, `amount_local`, `type`, `ip_click`, `hash` → `200` `1` when handled, `403` off, not set up or a wrong hash |

User activity: `account:store.purchase`, `account:membership.plus`, `account:membership.plusplus`,
`account:membership.auto-renew`, `account:membership.gift` (the buyer), `account:membership.gifted`
(the recipient: `from`, `membership`, `days`), `account:tokens.earn`, `account:tokens.streak`
(`day`, `amount`), `account:tokens.task` (`task`,
`amount`).

## Tables

| Table / columns | What it holds |
|---|---|
| `users.credits`, `users.extra_cpu`, `extra_memory`, `extra_disk` | The panel's credits and what its store sold (without noprofit.space) |
| `users.tokens`, `tokens_earned`, `tokens_daily_granted` | Tokens, all earned so far, the day the daily tokens were given |
| `users.panel_access_until` | When the paid panel access runs out (empty: never paid) |
| `user_credit_transactions`, `user_token_transactions` | The ledgers: amount, balance after, type, description, data |
| `user_token_claims` | Earn trips: nonce, provider, proof hash, status, reason, amount. CPX Research's postbacks too: the transaction id's hash as the proof, a random nonce nobody uses, status `granted`, `rejected` or `reversed`, `reason` the survey's `type` (`complete`, `out`, `bonus`) or why it was not paid |
| `user_memberships` | `plus_since` (first bought), `plus_until`, `plusplus_until`, the purchases of each and `plus_settled`, `plusplus_settled` of the last lapses |
| `user_memberships.plus_auto_renew`, `plusplus_auto_renew` | The member's [auto-renew](#auto-renew) switches (off by default) |
| `user_memberships.plus_reminded`, `plusplus_reminded`, `plus_renew_failed`, `plusplus_renew_failed` | The end each [ends-soon notice](#ends-soon-notices) and each failed renewal was for, so each goes out once |
| `users.panel_access_reminded` | The end of paid panel access its ends-soon notice was for (`users.panel_access_until` is indexed for it) |
| `user_memberships.bundle_synced_until`, `bundle_synced` | The second membership's end the membership bundle's other panel last confirmed, and when (empty: never sent; [the bundle](#memberships-across-pluggedhost-and-switchedcloud)) |
| `user_memberships.bundle_plus_until`, `pending_membership_bundles` | Plus from the bundle's other panel until then, and for accounts with no user here yet (`account_id`, `plus_until`) |
| `users.burst_cores` | The burst cores the account's servers get now (kept by `sync_burst`) |
| `users.streak_day`, `streak_claimed` | The [daily streak](#daily-streak): the day of the streak last claimed and the (UTC) date |
| `users.referral_code`, `referred_by`, `referral_paid` | [Referrals](referrals.md#tables) |

Ledger types on the Tokens page's features: `streak`, `starter:<task>` and `referral`; the history
names them "Daily streak", "Getting started" and "Referral".

## Settings

Admin › Settings › Economy, in three tabs: Resource Store (credits, starting credits, prices),
Tokens (daily tokens, panel access, CPX Research, Linkvertise, Vylo, Cuty, ShrinkEarn, ShrinkMe,
AdFoc.us, then Daily Streak, Double Tokens, Getting Started and Referrals) and
Memberships. CPX Research's part shows the postback URL to paste into CPX (built from
the panel's application URL, click to copy) and what to set in CPX's Reward Settings.
A tab saves only its own fields.

| Field | Key | Default |
|---|---|---|
| Credits | `economy::credits_enabled` | on |
| Starting Credits (once per account, without noprofit.space) | `economy::starting_credits` | 0 |
| Tokens | `economy::tokens_enabled` | on |
| Daily Tokens | `economy::daily_tokens` | 0 |
| Panel access: on, tokens, days | `economy::panel_access_{enabled,tokens,days}` | on, 5, 30 |
| Profile badges: on, tokens per badge ([promos.md](promos.md#profile-badges)) | `economy::badges_enabled`, `badge_tokens` | on, 5 |
| Memberships | `economy::membership_enabled` | on |
| Names | `economy::plus_name`, `plusplus_name` | Plus, Premium |
| First membership: tokens and days | `economy::plus_tokens`, `plus_days` | 15, 30 |
| Second: tokens and days | `economy::plusplus_tokens`, `plusplus_token_days` | 30, 30 |
| Second: credits (0 = not for credits) and days | `economy::plusplus_credits`, `plusplus_credit_days` | 0, 30 |
| Burst cores: first, second (the total) | `economy::plus_burst_cores`, `plusplus_burst_cores` (0 to 16) | 1, 2 |
| Ends-soon notice: on, hours before the end (1 to 720) | `economy::ends_soon_enabled`, `ends_soon_hours` | on, 48 |
| Auto-renew (the members' switches; each is off until switched on) | `economy::auto_renew_enabled` | on |
| Gifts: on, per buyer and 24 hours (1 to 1000) | `economy::gifts_enabled`, `gifts_per_day` | on, 10 |
| Membership bundle: on, panel URL (https), name of what Premium includes there, name of their membership that includes Plus here, shared secret (encrypted, at least 32 characters) | `economy::bundle_{enabled,url,name,source_name,secret}` | off, `https://panel.switched.cloud`, switched.cloud Plus, switched.cloud Premium, – |
| Store steps and prices | `economy::{cpu,memory,disk}_{step,price}` | 50 % / 100, 1024 MiB / 150, 5120 MiB / 75 |
| Store limits | `economy::limit_{cpu,memory,disk}` | 600 %, 24576 MiB, 102400 MiB |
| Curve | `economy::price_floor`, `soft_limit`, `soft_multiplier`, `doubling` | 50 %, 125 %, 4 ×, 5 % |
| Sale: discount, starts, ends, name, covers (`store`, `memberships`, `both`) | `economy::store_discount` (0 to 100), `store_discount_starts`, `store_discount_ends` (RFC 3339, empty: open), `store_discount_name`, `store_discount_covers` | 0, –, –, –, `store` |
| Minutes to Finish a Link | `economy::claim_minutes` | 30 |
| CPX Research: on, App ID (digits), Secure Hash (encrypted), Surveys per Day (finished surveys, 0 = no cap) | `economy::cpx_research_{enabled,app_id,secret,daily_cap}` | off, –, –, 1 |
| Linkvertise | `economy::linkvertise_{enabled,token (encrypted),publisher,whitelist,link,reward,daily_cap}` | off, –, –, `lvr=`, –, 1, 5 |
| Vylo | `economy::vylo_{enabled,api_key (encrypted),reward,daily_cap,min_seconds}` | off, –, 1, 5, 10 |
| Cuty | `economy::cuty_{enabled,api_key (encrypted),reward,daily_cap,min_seconds}` | off, –, 1, 5, 10 |
| ShrinkEarn | `economy::shrinkearn_{enabled,api_key (encrypted),reward,daily_cap,min_seconds}` | off, –, 1, 5, 10 |
| ShrinkMe | `economy::shrinkme_{enabled,api_key (encrypted),reward,daily_cap,min_seconds}` | off, –, 1, 5, 10 |
| AdFoc.us | `economy::adfocus_{enabled,api_key (encrypted),reward,daily_cap,min_seconds}` | off, –, 1, 5, 10 |
| Daily streak: on, Tokens per Day (1 to 14 amounts from 0.01 to 100, comma separated; the last repeats) | `economy::streak_enabled`, `streak_rewards` | on, `0.25,0.25,0.5,0.5,0.75,0.75,1` |
| Double tokens: bonus (0 to 1000 %, 0 = off), name, starts, ends (RFC 3339, empty: open) | `economy::token_bonus_percent`, `token_bonus_name`, `token_bonus_starts`, `token_bonus_ends` | 0, –, –, – |
| Getting started: on, Days Shown (0 hides it), Tokens per Task | `economy::starter_enabled`, `starter_days`, `starter_reward` | on, 14, 1 |
| Referrals: on, Tokens per Friend, Friend Earns, Paid per Month (0 = no limit) | `economy::referrals_enabled`, `referral_reward`, `referral_threshold`, `referral_monthly_cap` | on, 5, 10, 10 |

Turning credits, tokens or memberships off hides their page; balances and memberships are kept.

## Without noprofit.space

Credits, the store and its prices are the panel's own, as above. With noprofit.space the panel needs
four routes of the accounts app ([noprofit.md](../noprofit.md#wire-format)): `/store`, `/purchase`,
`/credits/spend` and `/credits/apply` (credits an admin or a Discord invite gives or takes).

## Check

- Admin › Settings › Economy: save each of the three tabs with the defaults; the Account menu shows
  Resource Store, Tokens and Membership.
- Daily streak: a new account's Tokens page shows "Daily streak", "Day 1 · 0.25 tokens", the first
  dot ringed, the filled **Claim today's tokens** and every way to earn in gray; the sidebar's tokens
  row has an accent dot. Claim: a toast "+0.25 tokens for day 1", "0.25 tokens saved toward your next
  token", the row says "come back tomorrow for 0.25", the first way to earn is filled again and the dot
  in the sidebar is gone within a minute. Claiming again (a second tab) answers "today's tokens are
  claimed already". The next UTC day it is day 2; skip a day and it is day 1 again.
- Double tokens: Admin › Settings › Economy › Tokens › Double Tokens › Bonus 100, Ends in two days at
  22:00, save. The Tokens page has the banner "Double tokens until <weekday> 22:00", each link's row
  shows twice its reward and the survey row "Every survey pays 100% more right now."; a link of 1.5
  pays 3 tokens and its ledger data has `bonus: 100`. A CPX postback of 12.5 pays 25. Set Starts to
  tomorrow: no banner, normal rewards. Back to 0: nothing changes anywhere.
- Getting started: a new account sees the list with four rows (three with Discord off). Create a
  server, come back: its row has **Claim**; claim it: "+1 token", the row says Claimed. Start the
  server (through the queue or the waiting room too): its row can be claimed. Claim the same task in
  two tabs: one pays. Set Days Shown to 0: the list is gone.
- Without noprofit.space, with a CPU allowance in Admin › Settings › Hosting › Games: Admin › Users
  › a user › Economy › add 500 credits. Resource Store: +50 % CPU costs 50 credits; buy it; the
  balance, the history and the create page's allowance show it.
- A resource without a limit in the games settings shows its meter without offers.
- Sale: Resource Store tab › Discount 25, Name "Halloween special", Ends tomorrow 22:00, save. A
  banner over the store names the sale and its end; each offer shows the full price struck through
  and a quarter less (50 → 38). Buy one: the toast and the history say 38. Set Starts to tomorrow:
  the store is back at full price until then. Back to 0: no sale anywhere.
- Resource Store: "Unlock more resources (1)" is closed; open it, reload: still open. Its row says
  what 1 GiB of memory costs and "Open store" scrolls to the resources. Each meter's solid part
  follows the servers' use; a server bigger than the limit turns the row red.
- Admin: add 60 tokens. Membership: buy the first one: "Plus is active, it runs until" a date 30
  days out, a `plus` location opens on the create and transfer pages; buy it again: the date moves
  30 days further. As another user, buy only the second: it runs for 30 days, `premium` and `plus`
  locations open, its servers burst +200 %; the CPU allowance stays the same (memberships add
  burst cores, not allowance). With ads on and Fewer Ads for Members on, a first-membership
  account sees no banners on any page and no popups, and still waits in the waiting room before a
  start; a second-membership account sees nothing.
- Admin › Economy › End the second membership now: within a minute, a server on a premium location
  starts moving (its Transfer tab). End the first one now on an account with a server on a plus
  location and no second membership: that server moves to a free location, and the account gets
  "Plus has ended".
- After the 3.0.4 migration: an account that had Plus for good shows "Plus is active" until 14
  days after the release; the Memberships tab shows 15 tokens / 30 days and 30 tokens / 30 days,
  credits 0, the second named Premium.
- Linkvertise, Vylo, Cuty, ShrinkEarn, ShrinkMe or AdFoc.us set up with real keys: Earn with …, finish the
  link, come back: a toast "Added: +1 tokens"; reloading the page that came back does not add more.
- CPX Research: Admin › Settings › Economy › Tokens › turn it on, enter the App ID and the Secure
  Hash, save; click the postback URL (copied) and paste it into the app's Postback Settings at CPX,
  name the currency Token/Tokens in its Reward Settings. The Tokens page lists CPX Research first
  with the filled "Take a survey", "Pays what each survey shows" and "0 of 1 today". The button opens
  the wall in a new tab, signed for the account. With CPX's "Test postback" (or a hand-made request
  with `hash` = `md5("<trans_id>-<secure hash>")`): `status=1&type=complete&amount_local=12.5` adds
  12 tokens and saves 0.5, the row says "1 of 1 today" and "Come back tomorrow" once the Tokens tab
  is shown again; the same request again adds nothing; another `complete` the same day is kept as
  `daily_cap` and pays nothing; `type=out` still pays. `status=2` with the first `trans_id` takes the
  12 tokens back ("Reversed: CPX Research"), down to 0 at most. A wrong `hash` answers 403.
- Panel access: as a user without a membership, open a server page: it lands on Account › Panel
  Access; the Tokens page still opens. Pay: the server page opens and the sidebar shows the new
  date. A Plus account never sees the paywall.
- With noprofit.space: the store shows noprofit.space's balance and prices; a purchase there raises
  the plan at once (create page allowance).
- Burst cores: with the first membership, a server with 200 % CPU shows "200% + 100% burst" on its
  card and console, and its CPU graph tops out at 300 %. A busy loop in the server reaches about 300 %
  while the node is quiet. On the node, `docker inspect <uuid> --format '{{.HostConfig.CpuShares}}'`
  says 2048 (the paid 200 %). End the second membership in Admin › Users › Economy: within a minute
  a server that had 2 burst cores shows 1, without a restart.
- The sidebar card: with a RAM limit, the RAM row shows what the servers use; buying in the Resource
  Store or redeeming a link updates it at once; with tokens off the tokens pill is gone.
- "N more to go": an account with 9 tokens: the Membership page's Plus button says "6 more tokens
  to go" and opens the Tokens page; the sidebar shows "9 / 15 to Plus" with a bar 60 % full, linking
  to the Membership page. With 15 tokens: "Enough for Plus". With Plus: "… / 30 to Premium"; with
  Premium (or memberships off): no line.
- Membership sale: Resource Store tab › Sale › Discount 25, Covers: the memberships, save. The store
  is at full price; the Membership page has the banner and "~~15~~ 11 tokens" for Plus, "~~30~~ 23"
  for Premium. Buy Plus: the history says 11. Open the page, change the discount to 50 in another
  tab, then buy: "the price changed" and the page shows the new prices. Covers: both: the store and
  the memberships are on sale; back to the store: only the store.
- Auto-renew: with Plus, switch on Renew automatically (a toast says so). Admin › Users › Economy ›
  set Plus to end in 5 minutes: the summary says "renews automatically"; within a minute the end
  moves 30 days on and the token history says "Plus (30 days), renewed automatically". With too few
  tokens instead: one notification "Couldn’t renew Plus" with "6 more tokens needed", and Plus ends.
- Ends-soon: set Plus to end in 47 hours: within a minute one notification "Plus ends soon" with the
  date, "It does not renew by itself …" and a Renew link to the Membership page; nothing more the
  next minutes; set another end within 48 hours: one more, for the new end. Paid panel access ending
  within 48 hours: "Panel
  access ends soon" with a link to Account › Panel Access.
- Gifts: Gift to a friend › another account's username, Plus › Send gift › Send: 15 tokens are gone
  ("Plus (30 days) for bob"), bob's Plus runs 30 days longer, bob gets "alice gave you Plus" and a
  0-token row "Plus (30 days) from alice", both activity logs show it. Your own username: refused;
  an unknown one: "no account has that username or email"; the 11th gift within a day: refused
  (Gifts per Day 10).
- The membership bundle (noprofit.space connected; switched.cloud's side set up with the same
  secret): Memberships tab › Included With the Second Membership › on, the URL of a switched.cloud
  panel, the secret, save (a secret under 32 characters is refused). The Membership page lists
  "switched.cloud Plus included" under Premium and a "switched.cloud Plus" row in the table. Give a
  linked account Premium in Admin › Users › Economy: within a minute switched.cloud's Admin › Users
  shows its Plus with plugged.host Premium. End it now: within a minute that Plus ends there too.
  A wrong secret on either side: the task logs the 401 every minute and marks nothing as sent.
  The other way: give a linked account Premium on switched.cloud: within a minute the Membership
  page here says "Plus is included with your switched.cloud Premium until …", a `plus` location
  opens and its servers get the first membership's burst cores; Admin › Users › Economy shows the
  date. End it there: within a minute Plus ends here and servers on `plus` locations move off.
