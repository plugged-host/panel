# Credits, tokens, memberships and the Resource Store

Two currencies (`shared/src/economy.rs`, `shared/src/earn.rs`, `shared/src/pricing.rs`):

- **Credits** buy CPU, memory and disk in the **Resource Store**, for good, and the second
  membership. Without noprofit.space they are the panel's own; with it they are the account's
  noprofit.space credits and the store is noprofit.space's.
- **Tokens** are earned through sponsored links (Linkvertise, Vylo) and buy the memberships. They
  are always the panel's own.

Two memberships, always the panel's own. Their names are settings (plugged.host calls them Plus and
PlusPlus); in code, tables and routes they are `plus` and `plusplus`:

| | First (`plus`) | Second (`plusplus`) |
|---|---|---|
| Bought with | tokens, once, kept for good | tokens or credits, for some days; needs the first; buying again adds to the time left |
| Location tier it opens | `plus` | `premium` (and `plus`) |
| Start queue priority | 1 | 2 |
| Burst cores on every server | 1 | 2 (the total, not 1 + 2) |

A location's tier is set on the location ([node-telemetry.md](node-telemetry.md)); a tier the
economy does not know stays closed to players (`server_creation::tier_unlocked`). Admins place
servers anywhere. The start queue, ads and limbo use the memberships when they arrive (later
batches of this phase).

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

## The Resource Store

Account › Resource Store shows the balance, one section per resource with what the account has and
its offers (1, 2 and 4 steps), and the last 10 credit transactions. A purchase is confirmed in a
dialog and sent with the price the player saw: a price that moved since buys nothing (409, the page
shows the new prices).

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
the store says it is closed.

## Tokens

Account › Tokens shows the balance, what was earned so far, the ways to earn and the last 10 token
transactions. **Daily tokens** (a setting, 0 by default) are given once a day, the first time the
account looks after midnight UTC.

Earning works with claims (`user_token_claims`):

1. **Earn** opens a claim: a 64-character nonce bound to the account, and returns where to go. The
   provider sends the player back to `/account/tokens?claim=<nonce>&…` with its proof.
2. The page posts the claim and the other parameters to `/redeem`. Everything local is checked
   first: the claim is this account's, still pending and younger than **Minutes to Finish a Link**,
   the proof has the right shape and was not used before, today's cap is not reached. The provider
   is asked last, once, since its check may use the proof up.
3. Granted: the claim and the tokens change in one transaction; the unique index on
   `(provider, token_hash)` stops one proof paying twice even when two requests race.

Opening claims is throttled per account and provider (3 × the daily cap per hour, at least 5).

| Provider | How it goes | How the proof is checked |
|---|---|---|
| Linkvertise, link | The settings' Linkvertise link, with `{return}` or `{return_b64}` replaced by the way back | Linkvertise's anti-bypass API with the API token |
| Linkvertise, Full Script | With a publisher id the Tokens page loads `linkvertise.js`, which turns the page's own earn link (`…&lvr=1`, matching the whitelist) into a Linkvertise link. The claim is opened when the page loads and reused on reloads | The same API |
| Vylo | A short link made for every trip through Vylo's API, to the way back plus `vt=<proof>` | The proof is an HMAC of the nonce with the panel's key, so only a trip through the link has it; a trip faster than **Shortest Trip** does not count |

The page's `script-src` allows `https://publisher.linkvertise.com` only while Linkvertise is on with
a publisher id and a token (`backend/src/lib.rs`).

## Memberships

Account › Membership is laid out like the Pelican panel's Plus page: the two memberships side by
side (pitch, price, term, what each gives, the buy buttons), a table of what each adds over the free
tier (locations, start queue, in-game ads, panel ads, CPU, neo usage, the devbox while devboxes are
on), and the questions people ask before buying. The second membership's term reads like Pelican's
("a week, or credits for two weeks"), from the two offers' days. Buying locks the membership row;
the tokens or credits and the membership change together or not at all. With noprofit.space,
credits for the second membership are taken by noprofit.space (`/credits/spend`) with an
idempotency key per purchase, so a retry after a failure is not charged twice.

When the second membership runs out, the `settle_memberships` task (every minute):

1. queues an admin move of every server on a `premium` location to a location the owner may use
   and that takes servers: the same country first, then `plus` locations, then the fastest
   ([transfers.md](transfers.md));
2. counts the lapse as settled once no server is left on a premium location; until then it comes
   back, so a full location is tried again. Buying again before that stops it.

## The sidebar

Over the sidebar's footer (dashboard and server), Theme v2's card: one row per limited resource of
the account (used / limit, red at the limit), and a row with the tokens and credits. Usage and
credits lead to the Resource Store, tokens to the Tokens page; a part whose page is off is plain
text, and a resource without a limit or a currency that is off is left out. It reads
`GET /api/client/account/balance` once a minute and after a purchase, a redeemed link or a
membership change. Suspended users don't see it.

## Admins

Admin › Users › a user › Economy (`users.economy`): credits, tokens, what was earned, both
memberships, what the panel's store sold, and both histories. Add or take away credits (without
noprofit.space) and tokens with a reason the user sees, give or take the first membership, set or
end the second. Admin activity `user:economy`.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /api/client/account/credits` | `economy.read` | `{store}`: `source` (`panel`/`noprofit`), `available`, `credits`, `catalog: [{resource, step, total, limit, soft_limit, offers: [{steps, amount, price}]}]`, `transactions` |
| `POST /api/client/account/credits/store` | `economy.purchase` | `{resource, steps (1–50), price}` → `{store}`; 409 when the price changed or the balance is short, 422 when noprofit.space refused |
| `GET /api/client/account/tokens` | `economy.read` | `{tokens}`: balance, earned, `claim_minutes`, `providers: [{provider, reward, daily_cap, granted_today, full_script, publisher, whitelist}]`, `transactions` |
| `POST /api/client/account/tokens/earn` | `economy.earn` | `{provider}` → `{earn: {url, nonce}}`; 409 capped, 429 throttled, 502 the provider made no link |
| `POST /api/client/account/tokens/redeem` | `economy.earn` | `{claim, params}` → `{redemption: {status, amount, provider}}`, status `granted`, `settled`, `no_claim`, `expired`, `no_token`, `replay`, `capped`, `invalid` |
| `GET /api/client/account/balance` | `economy.read` | `{balance}`: `credits` and `tokens` (`null` when off), `allowance` (the create page's) |
| `GET /api/client/account/membership` | `economy.read` | `{economy}`: balances, both memberships, their prices, the burst cores of each and the account's now, the queue priority |
| `POST /api/client/account/membership/plus` | `economy.purchase` | `{economy}`; 409 owned or short |
| `POST /api/client/account/membership/plusplus` | `economy.purchase` | `{pay: tokens\|credits}` → `{economy}`; 403 without the first membership |
| `GET /api/admin/users/{user}/economy` | `users.economy` | The user's economy and both histories |
| `PUT /api/admin/users/{user}/economy` | `users.economy` | `{credits?, tokens?, reason?, plus?, plusplus_until?}` (`null` ends it now) |

User activity: `account:store.purchase`, `account:membership.plus`, `account:membership.plusplus`,
`account:tokens.earn`.

## Tables

| Table / columns | What it holds |
|---|---|
| `users.credits`, `users.extra_cpu`, `extra_memory`, `extra_disk` | The panel's credits and what its store sold (without noprofit.space) |
| `users.tokens`, `tokens_earned`, `tokens_daily_granted` | Tokens, all earned so far, the day the daily tokens were given |
| `user_credit_transactions`, `user_token_transactions` | The ledgers: amount, balance after, type, description, data |
| `user_token_claims` | Earn trips: nonce, provider, proof hash, status, reason, amount |
| `user_memberships` | `plus_since`, `plusplus_until`, purchases and `plusplus_settled` of the last lapse |
| `users.burst_cores` | The burst cores the account's servers get now (kept by `sync_burst`) |

## Settings

Admin › Settings › Economy, in three tabs: Resource Store (credits, starting credits, prices),
Tokens (daily tokens, Linkvertise, Vylo) and Memberships. A tab saves only its own fields.

| Field | Key | Default |
|---|---|---|
| Credits | `economy::credits_enabled` | on |
| Starting Credits (once per account, without noprofit.space) | `economy::starting_credits` | 0 |
| Tokens | `economy::tokens_enabled` | on |
| Daily Tokens | `economy::daily_tokens` | 0 |
| Memberships | `economy::membership_enabled` | on |
| Names | `economy::plus_name`, `plusplus_name` | Plus, PlusPlus |
| First membership: tokens | `economy::plus_tokens` | 25 |
| Second: tokens and days | `economy::plusplus_tokens`, `plusplus_token_days` | 10, 7 |
| Second: credits (0 = not for credits) and days | `economy::plusplus_credits`, `plusplus_credit_days` | 150, 14 |
| Burst cores: first, second (the total) | `economy::plus_burst_cores`, `plusplus_burst_cores` (0 to 16) | 1, 2 |
| Store steps and prices | `economy::{cpu,memory,disk}_{step,price}` | 50 % / 100, 1024 MiB / 150, 5120 MiB / 75 |
| Store limits | `economy::limit_{cpu,memory,disk}` | 600 %, 24576 MiB, 102400 MiB |
| Curve | `economy::price_floor`, `soft_limit`, `soft_multiplier`, `doubling` | 50 %, 125 %, 4 ×, 5 % |
| Minutes to Finish a Link | `economy::claim_minutes` | 30 |
| Linkvertise | `economy::linkvertise_{enabled,token (encrypted),publisher,whitelist,link,reward,daily_cap}` | off, –, –, `lvr=`, –, 1, 5 |
| Vylo | `economy::vylo_{enabled,api_key (encrypted),reward,daily_cap,min_seconds}` | off, –, 1, 5, 10 |

Turning credits, tokens or memberships off hides their page; balances and memberships are kept.

## Without noprofit.space

Credits, the store and its prices are the panel's own, as above. With noprofit.space the panel needs
three routes of the accounts app ([noprofit.md](../noprofit.md#wire-format)): `/store`, `/purchase`
and `/credits/spend`.

## Check

- Admin › Settings › Economy: save each of the three tabs with the defaults; the Account menu shows
  Resource Store, Tokens and Membership.
- Without noprofit.space, with a CPU allowance in Admin › Settings › Hosting › Games: Admin › Users
  › a user › Economy › add 500 credits. Resource Store: +50 % CPU costs 50 credits; buy it; the
  balance, the history and the create page's allowance show it.
- A resource without a limit in the games settings says it is not sold.
- Admin: add 30 tokens. Membership: buy the first one; a `plus` location opens on the create and
  transfer pages. Buy the second with tokens: it runs for 7 days, a `premium` location opens; the
  CPU allowance stays the same (the second membership adds burst cores, not allowance).
- Admin › Economy › End the second membership now: within a minute, a server on a premium location
  starts moving (its Transfer tab).
- Linkvertise or Vylo set up with real keys: Earn, finish the link, come back: "1 tokens added";
  reloading the page that came back does not add more.
- With noprofit.space: the store shows noprofit.space's balance and prices; a purchase there raises
  the plan at once (create page allowance).
- Burst cores: with the first membership, a server with 200 % CPU shows "200% + 100% burst" on its
  card and console, and its CPU graph tops out at 300 %. A busy loop in the server reaches about 300 %
  while the node is quiet. On the node, `docker inspect <uuid> --format '{{.HostConfig.CpuShares}}'`
  says 2048 (the paid 200 %). End the second membership in Admin › Users › Economy: within a minute
  a server that had 2 burst cores shows 1, without a restart.
- The sidebar card: with a RAM limit, the RAM row shows what the servers use; buying in the Resource
  Store or redeeming a link updates it at once; with tokens off the tokens pill is gone.
