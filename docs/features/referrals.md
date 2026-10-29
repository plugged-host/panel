# Referrals

Every account has a referral link. A friend who signs up through it and earns their first tokens
pays its owner tokens once (`shared/src/referrals.rs`, `backend/src/routes/referral.rs`,
`frontend/src/pages/dashboard/tokens/TokenReferrals.tsx`). Off with tokens or with **Referrals**
off in Admin › Settings › Economy › Tokens.

## The link

`https://panel.example.com/r/<code>`: the code is 8 lowercase letters and digits, made the first
time the account's Tokens page asks for it (`users.referral_code`, unique). Opening the link keeps
the code in an HTTP-only cookie (`referral`, path `/`, 30 days) and sends the browser to sign-in
(`/auth/login`, which with noprofit.space goes straight on to noprofit.space). An unknown code, or
referrals off, only goes to sign-in.

## Who counts

[`record`](../../shared/src/referrals.rs) runs where a new panel user is made, reads the cookie and
removes it:

- the first noprofit.space sign-in that makes a user (`backend/src/routes/api/auth/noprofit.rs`, not
  one that links an existing user by email);
- registration (`/api/auth/register`) and a sign-up through one of the panel's OAuth providers.

It records `users.referred_by` only when referrals are on, the code belongs to an account, that
account is not the new one, the new user was made in the last hour and has no referrer yet, and the
sign-up's address is not the address the referrer was last seen at (their newest session). A
problem is logged and never fails the sign-up. An existing account never gets a referrer.

## The reward

The `pay_referrals` task (every minute, primary instance) pays the referrer of each friend whose
`tokens_earned` reached **Friend Earns** (10): **Tokens per Friend** (5), once per friend
(`users.referral_paid`). Every token that comes in counts toward `tokens_earned`: links, surveys,
the daily streak, the first-week tasks (not an admin's change). A survey taken back after the payout
does not take the referral back.

- At most **Paid per Month** (10) friends are paid per referrer in any 30 days; a friend over it
  waits until the window frees up (referrers at their limit are left out of a run, so they never
  hold up others). 0 has no limit.
- The referrer's row is locked while one is paid, and the friend's row is marked in the same
  transaction as the ledger row (`referral`, the friend's username), so nobody is paid twice.
- The referrer gets the notification `referralPaid`: "Your friend <username> earned their first 10
  tokens: +5 tokens" ([notifications.md](notifications.md)).

## The page

The end of Account › Tokens: "Invite friends", the rule in one line ("When a friend signs up with
your link and earns their first 10 tokens, you get +5 tokens."), the link with a copy button, the
monthly limit, and the newest 50 friends, each with "3 of 10 tokens earned" or "Paid +5".

Admin › Users › a user › Economy says who brought the account ("Brought by <username>.") and how many
it brought and was paid for.

## Routes

| Route | Permission | Does |
|---|---|---|
| `GET /r/{code}` | none (public) | Sets the `referral` cookie for a known code, `307` to `/auth/login` |
| `GET /api/client/account/tokens/referrals` | `economy.read` | `{referrals}`: `code`, `url`, `reward`, `threshold`, `monthly_cap`, `paid_this_month`, `total`, `friends: [{username, tokens_earned, paid}]` (the newest 50), or `null` while off |
| `GET /api/admin/users/{user}/economy` | `users.economy` | also `referrals: {referred_by: {uuid, username} \| null, referred, paid}` |

## Tables

| Table / columns | What it holds |
|---|---|
| `users.referral_code` | The account's code (unique index `users_referral_code_idx`), made on first use |
| `users.referred_by` | Who brought the account (set to null when that account is deleted; index `users_referred_by_idx`) |
| `users.referral_paid` | When the referrer was paid for this account |

## Settings

Admin › Settings › Economy › Tokens › Referrals.

| Field | Key | Default |
|---|---|---|
| Referrals | `economy::referrals_enabled` | on |
| Tokens per Friend | `economy::referral_reward` | 5 |
| Friend Earns (tokens before it pays) | `economy::referral_threshold` | 10 |
| Paid per Month (friends per referrer in any 30 days, 0 = no limit) | `economy::referral_monthly_cap` | 10 |

## Without noprofit.space

The same: registration and OAuth sign-ups read the cookie. With noprofit.space only a first sign-in
that makes a new panel user counts; noprofit.space's own sign-up page is not involved.

## Check

- Account › Tokens: "Invite friends" with a link `https://<panel>/r/<8 characters>` and a copy
  button; reloading shows the same link.
- In a private window, open the link: the browser lands on sign-in with a `referral` cookie for 30
  days. Sign up (or sign in with a new noprofit.space account): the referrer's page lists the friend
  with "0 of 10 tokens earned"; Admin › Users › the friend › Economy says "Brought by <referrer>.".
- Sign up from the referrer's own network (same address as their last session): nobody is
  recorded. Signing in with an existing account through the link records nothing.
- Give the friend 10 earned tokens (a link, a survey, the streak): within a minute the referrer gets
  +5 tokens, the ledger row "Referral: <friend>" and the notification; the row says "Paid +5". Earning
  more pays nothing again.
- Set Paid per Month to 1 and bring two friends to 10 tokens: one is paid, the other waits.
- Referrals off: the section is gone, `/r/<code>` only goes to sign-in and nothing is paid.
