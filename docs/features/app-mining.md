# The app's AFK mining page

The desktop app cannot use Linkvertise, CPX Research or the shorteners: those are browser redirects.
Its Tokens page blurs them and points to the panel instead, and offers **AFK Mining** as the native
way to earn.

The app keeps a local session while it is open. It claims once a minute; the panel pays whole
tokens for the time since the last `mining` row in `user_token_transactions`, but never more than
two minutes per claim, so closing the app stops the clock. Nothing is earned offline.

| Route | What |
|---|---|
| `GET /api/client/account/mining` | Balance, rate, daily cap, last claim and the next claim |
| `POST /api/client/account/mining/start` | Writes a zero-token baseline row and returns the status |
| `POST /api/client/account/mining/claim` | Pays the elapsed whole tokens and returns the status |

`PLUGGED_MINING_TOKENS_PER_HOUR` (default 6) and `PLUGGED_MINING_DAILY_CAP` (default 24) tune it.
The ledger type is `mining`, so the Tokens page's history shows it like every other earning. The
endpoint is available to every signed-in account; the app itself is admin-only on dev for now.
