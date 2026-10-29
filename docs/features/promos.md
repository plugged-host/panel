# Promotions and profile badges

Where users feel a limit, the panel says once, plainly, what lifts it: a membership popup, the
house promo clips among the ads, one line with a link on the pages where a limit stops someone, and
profile badges bought with tokens. The waiting room's day pass is in [ads.md](ads.md#day-pass);
neo's suggestions in [neo.md](neo.md#the-prompt). Code: `shared/src/promos.rs`,
`shared/src/badges.rs`, `frontend/src/elements/{PromoPopup,ProfileBadge}.tsx`,
`frontend/src/elements/ads/PromoClip.tsx`.

## The popup

A card for an account without a membership that sees ads (`audience` `everyone`,
[ads.md](ads.md#who-sees-ads)): the membership it is closest to, its perks and how far its tokens
are.

- **Which membership**: the dearest one its tokens already buy, else the cheapest one sold for
  tokens (`promos::pick`), at today's price (a membership sale applied, the full price struck
  through).
- **What it says**: "Get more with Plus", "15 tokens for 30 days", the perks from the settings,
  only what is on (the tier's locations while public ones exist, the start queue's priority, burst
  cores, no ads or only the waiting room while Fewer Ads for Members is on, the panel included
  while it costs tokens, more neo usage while neo is offered, faster backups), then a bar "9 of 15"
  that fills, or "You have enough: get Plus now." One filled button "Get Plus" (to Account ›
  Membership) and "Not now"; Escape is "Not now".
- **When**: at most once per **Hours Between Popups** (24) per account, on any device: the panel
  keeps when it was last shown (`users.promo_shown`) and claims it with one conditional update, so
  two tabs never both show it. Never on the account's first day, never for admins, suspended or
  impersonated accounts, never on the membership or tokens pages, the start queue or the waiting
  room, and never while the welcome tour runs ([welcome-tour.md](welcome-tour.md)). The browser
  asks once per page load, 8 seconds after it settles.
- **Look**: Theme v2's modal surface and elevation, flat. It springs up from the window's end
  corner, above neo's launcher, and fades in; the perks follow one by one, the number counts up
  and the bar fills. On phones it is a sheet along the window's foot. With
  `prefers-reduced-motion` nothing moves.

## Promo clips

The house clips in `frontend/public/promo/` (`<name>.mp4`, `.webm`, `.jpg` poster, listed with
their button and link in `index.json`: `[{name, cta, href}]`) are one more creative in the ad
slots ([ads.md](ads.md#slots)), to the same audience as every other unit:

- A waiting room slot ([start queue](start-queue.md) and waiting room) shows the clip as a video,
  16:9, muted, looping, inline, its button over its foot; a page rectangle (300×250) shows its
  poster as a banner to the same link. The strips and the foot never get one.
- A slot that can take one shows a clip instead of its unit **Share of the Slots** percent of the
  time (25), and a slot with nothing else to show gets one; each clip at most once per page. Which
  slot shows one is rolled per slot from the visit, so it holds while the page stays; the clips'
  order changes with every visit.
- Never to someone without ads, never `go-plus` to someone with a membership, never a clip whose
  page (memberships, tokens) is off. A clip whose files do not load folds away; a missing
  `index.json` shows none. Links open in a new tab, since the waiting room holds its page.
- Known clips get their button in the reader's language (`skip-the-wait`: "Get Premium",
  `go-plus`: "Get Plus", `free-tokens`: "Earn tokens"); another clip keeps its own text.

## Offers

One line and one link, while **Offers Where a Limit Is Felt** is on and the user does not have
what it offers:

| Where | When | Says |
|---|---|---|
| Start queue | memberships on, the user lacks the second | "Premium goes first in this line, ahead of everyone. See Premium" (under the first membership's offer, or under a Plus member's priority note) |
| Create Server | credits on, the allowance stops the deploy (a resource used up, or the size over what is left; not the server count) | "Get more in the Resource Store" beside the reason |
| Account › Panel Access | memberships on, the user has to pay | "Plus and Premium include the panel. Get Plus instead" |

Idle stops, parked servers and per-server limits have their own offers
([server upgrades](server-upgrades.md)). Tokens never turn into credits, so no offer suggests it.

## Profile badges

A few preset badges (Supporter, Early Bird, Night Owl, OG, Builder, Explorer), each bought for
good with **Tokens per Badge** (5) on the account page (its Badges card). The account picks one to
show, or none; the first one bought is shown at once. The badge is a flat Mantine badge in one of
the theme's colours next to the name in the sidebar's account row, the server's subusers list and
its chat. Bought badges stay shown when selling them is turned off.

A purchase locks the user row, checks the price the user saw (409 when it moved), the balance and
that the badge is not owned yet (409), and writes the tokens, the ledger row (`badge`, "Profile
badge") and the badge together.

## Routes

| Route | Permission | Does |
|---|---|---|
| `POST /api/client/account/promo` | `economy.read` | `{promo}`: the popup when it is due, marked shown (`membership`, `name`, `cost`, `full_cost`, `days`, `tokens`, `perks: [{id: locations\|queue (first)\|burst_cores (cores)\|no_ads\|fewer_ads\|panel_access\|neo (percent)\|faster_backups}]`), else `null` |
| `GET /api/client/account/badges` | `economy.read` | `{badges}`: `enabled`, `tokens`, `available`, `owned`, `shown`, `balance` |
| `POST /api/client/account/badges` | `economy.purchase` | `{badge, price}` → `{badges}`; 409 short, owned or the price moved, 400 unknown, 403 off |
| `PUT /api/client/account/badges` | `economy.read` | `{badge}` (one owned, or `null`) → `{badges}` |

User activity: `account:badge.purchase`. The user objects (`ApiUser`, `ApiFullUser`) carry
`badge`.

## Tables

| Column | Holds |
|---|---|
| `users.promo_shown` | When the popup was last shown |
| `users.badges` | The badges bought (`varchar(32)[]`, empty by default) |
| `users.badge` | The one shown, or empty |

## Settings

| Field | Key | Default |
|---|---|---|
| Membership Popup | `ads::promo_enabled` | on |
| Hours Between Popups (1 to 720) | `ads::promo_hours` | 24 |
| Offers Where a Limit Is Felt | `ads::offers_enabled` | on |
| Promo Clips in the Ad Slots | `ads::promo_videos` | on |
| Share of the Slots (%, 0 to 100) | `ads::promo_video_share` | 25 |
| Sell Profile Badges | `economy::badges_enabled` | on |
| Tokens per Badge | `economy::badge_tokens` | 5 |

The first five are on Admin › Settings › Ads › Promotions, the badges on Admin › Settings ›
Economy › Tokens. The popup needs memberships and tokens on, the clips need ads on.

## Check

- An account two days old without a membership, ads on with a zone: within 10 seconds of opening
  the server list the card slides up at the corner above "Ask neo", its perks one by one, "9 of 15"
  with the bar filling (with 9 tokens). "Not now" closes it; reloading, or another browser, shows
  none for 24 hours. With 15 tokens it says "You have enough: get Plus now"; with 30, Premium.
  "Get Plus" opens Account › Membership.
- No popup on an account created today, for an admin, on Account › Tokens or Membership, on a
  waiting page, while the welcome tour runs, or for a Plus member. On a phone it is a sheet at the
  foot; with reduced motion it simply appears.
- Waiting room with one zone: about one start in four shows a clip in the first slot instead,
  playing muted, its button over its foot; with no zone at all the room shows a clip. A Plus member
  never sees "go-plus". Share 0: no clip. Rename `index.json`: no clip and nothing broken.
- The server list's 300×250 rectangle with no zone shows a clip's poster linking to its page.
- Start queue as a free user: "Premium goes first in this line" with its link under the Plus offer;
  as a Plus member under the priority note; as Premium nothing.
- Create Server with the memory allowance used up: "Get more in the Resource Store" beside the
  reason; with credits off it is gone.
- Panel Access as a user who has to pay: "Plus and Premium include the panel. Get Plus instead".
- Offers off: none of the three lines.
- Account page › Badges with 10 tokens: buy Supporter (5 tokens): it shows next to the name in the
  sidebar at once; buy Night Owl and Show it; on a server shared with someone, their subusers list
  and the chat show it. "Show no badge" removes it everywhere. Buying the same badge again answers
  409.
