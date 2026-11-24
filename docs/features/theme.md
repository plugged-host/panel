# Theme v2

The panel looks like plugged.host's Pelican panel (its "Theme v2") in the plugged.host brand kit's
colours: a Socket-blue accent on plain grey surfaces, Space Grotesk for text and JetBrains Mono
for numbers and code, Tabler's outline icons, a flat sidebar with grouped rows, and the user's
avatar row at its foot. It is dark only.

## Brand

The brand kit (the Socket mark, the logo files, the colour scales) is kept outside this repository;
the panel carries copies of what it shows. Its values here:

| | Value | Where |
|---|---|---|
| Accent fill (white text 5.1:1) and hover | `#3a5bff`, `#2449d8` | `--theme-accent`, `--theme-accent-hover`, Mantine `blue.6` / `blue.7` |
| Accent as text or ring on dark | `#688cfc` (blue 400), glow `#95b2fd` | `--theme-accent-bright`, `--theme-accent-glow`, Mantine `blue.4` |
| Surfaces (plain cool greys, never tinted toward the blue) | floor `#0b0c0e` to `#111315`, surface `#181a1c` (sidebar, menus, modals), surface-2 `#292c2f` (plain buttons), line `#414448` (their hover) | `--theme-bg-*`, `--theme-surface*`, `--theme-line`, `--theme-sidebar`, Mantine `dark` |
| Text | `#f8f9fa`, `#cbced1`, `#a2a5a9`, `#777b7f` | `--theme-text*`, Mantine `dark.0-3` |
| Status text on dark, on its soft background | success `#1fb57a` on `#092e1e`, warning `#f2a93b` on `#352205`, danger `#fd5973` on `#43141b` (all 4.5:1 or more); the base colours `#1fb57a`, `#f2a93b`, `#ef4565` fill | `--theme-{success,warning,danger}{,-soft}`, Mantine `green`, `yellow`, `red` (base at 6, text at 4; their light variant is the soft pair), `--color-server-status-*` |
| Radii | 6px small parts, 10px buttons and inputs, 16px cards | Mantine `radius`, `defaultRadius`, Button's default |

Blue is the accent only: the primary button, links, the active row or tab, the logo. Surfaces,
borders and text stay grey, and plain (default-variant) buttons are flat surface-2 with no edge or
ring. The signal lime of the kit (`#d2f55a`, one highlight per screen) is not used yet. Cards keep
Theme v2's white/5 fill without a border (the kit's cards have a 1px line).

Logo files in `frontend/public/`:

| File | Used for |
|---|---|
| `plugged.host.png` (256 px app tile) | the default `app::icon`: the sign-in page without a banner, avatars without a picture, the Download app page, the setup wizard |
| `favicon-16x16.png`, `favicon-32x32.png`, `favicon.ico` | the kit's favicon cut (heavier slots for 16-32 px); `App.tsx` replaces them only when `app::icon` is not the default |
| `apple-touch-icon.png` (full bleed), `android-chrome-*.png` | home-screen icons, `site.webmanifest`, the default Open Graph image |
| `plugged.host-logo-on-ink.svg` | the lockup on dark: set it as **Banner** (Admin › Settings › General › Application), the sidebar's logo and the sign-in page's |
| `plugged.host-logo.png` (2x, 64 px tall) and `plugged.host-logo.svg` | the lockup on light: set the PNG as **Banner (Light Mode)**; the mails show it in their header (mail clients show no SVG) |

The wordmark is outlined in the files: never retype "plugged.host" as text in a logo spot. The
rasters are made from the kit's SVGs with headless Chromium (no renderer is needed at build time).
The panel's restart page (`plugged-host/maintenance.html`) carries the on-ink lockup inline.

### Your own brand

The panel is open source and every brand spot is a setting, so a host running it under its own
name changes no code:

| Spot | Setting (Admin › Settings › General) |
|---|---|
| Name (page titles, mails, the sign-in page without a banner) | Application › Name |
| Sidebar and sign-in logo | Application › Banner (dark); without one, the icon |
| Mail header | Application › Banner (Light Mode), a PNG; without one, the name as text |
| Favicon, the sign-in icon, avatars without a picture | Application › Icon; any value but the default replaces the favicons too |
| Accent (buttons, links, active rows, mail buttons) | Metadata › Theme Color |
| Open Graph image | Metadata › Preview Image (default: `android-chrome-512x512.png`) |

The files in `frontend/public/` are only plugged.host's defaults: a fork may replace them, but
settings are enough. The ink surfaces, the status colours and the radii are fixed parts of the
theme, as Theme v2's were.

## Where it lives

| Part | File |
|---|---|
| Mantine palette, component defaults (radius, buttons, tabs, modals, badges) | `frontend/src/theme.ts`, merged into upstream's base theme in `main.tsx` |
| Tokens and everything Mantine's theme object cannot say: surfaces, the page floor, sidebar, inputs, tables, cards, the server list, the console | `frontend/src/app.css`, section "plugged.host Theme v2" |
| Accent colour | `frontend/src/lib/accent.ts`, applied in `App.tsx` |
| Seasonal themes (Halloween, Christmas) | `frontend/src/lib/seasonalTheme.ts`, applied in `App.tsx`; drawings in `frontend/src/elements/navigation/SeasonalDecor.tsx`, `.theme-season*` in `app.css` |
| Fonts | `frontend/src/assets/fonts/` (self-hosted, Latin + Latin Extended) |
| Icons | `frontend/src/lib/icons/` + `frontend/vite-plugins/tabler-icons.ts` |
| Sidebar (logo block, groups, avatar row, mobile toggle) | `frontend/src/elements/navigation/Sidebar.tsx` |
| Dashboard sidebar rows: Pelican's order (Home, then Paid while it shows (as on switched.cloud), Resource Store, the first membership, Tokens, Backups), then the account's pages folded into an "Account" row with an arrow, Admin last | `frontend/src/routers/DashboardRouter.tsx`, the folding row `frontend/src/elements/navigation/SidebarGroup.tsx` (`Sidebar.Group`) |
| Server list cards | `frontend/src/pages/dashboard/home/ServerItem.tsx` (container queries: one card per row on narrow screens, as many as fit on wide ones) |
| Server list page, as Pelican's (no All/Grouped tabs; a toolbar row with Create server at its start and the search at its end; the pagination with a per-page choice underneath; bulk selection with S + click only) | `frontend/src/pages/dashboard/home/DashboardHomeAll.tsx`, `.theme-toolbar*`, `.theme-list-foot` in `app.css` |
| A server's condition (label and status-bar colour, Pelican's `Server::condition`) and the card's numbers (70 % / 90 % marks, "1.2 / 2 GiB", "2h 14m") | `frontend/src/lib/domain/serverCondition.ts`, `frontend/src/lib/format/serverUsage.ts` |
| Console, as Pelican's: title "Console" with the power buttons (icon squares on phones), the log check above it, six stat blocks (Name, Status, Address, CPU, Memory, Disk), the terminal without a header row (its tools are quick actions; the popout keeps the row), the CPU / Memory / Network charts only while the server runs | `frontend/src/pages/server/console/{ServerConsole.tsx,stats/ServerOverview.tsx,stats/ServerStats.tsx,terminal/Console.tsx,terminal/ConsoleSearch.tsx,terminal/ConsoleCommandInput.tsx}`, `.theme-console*` in `app.css` |
| Choice cards, the Pelican panel's CardSelect (Create Server, the Minecraft setup, moving a server): a sunken well in a section card, the bright accent border when selected, an optional radio, picture, corner meter or centred layout | `frontend/src/elements/input/Choice.tsx`, `.theme-choice*` in `app.css` |
| Section descriptions under a card's heading (Filament's) | `frontend/src/elements/data-display/TitleCard.tsx` (`description`) |

The tokens are CSS custom properties named `--theme-*` (`--theme-accent`, `--theme-accent-bright`,
`--theme-raise`, `--theme-hairline`, `--theme-text-*` …). Use them instead of new colours; components
from `src/elements` already do.

## Accent colour

Admin › Settings › General › Metadata › Theme Color sets it: one of Tailwind's palettes (the
swatches) or any colour. The theme is two-tier: a darker fill (buttons, active rows) and a lighter
shade for text and rings on dark surfaces. `accentFor()` builds both from the chosen colour, darkens
the fill until white text on it reaches 4.5:1 contrast and lightens the text shade until it reaches
4.5:1 on the sidebar. Blue, the default (`#3a5bff`), changes nothing. A panel that saved its
settings before the Socket rebrand stores the old default, `#1d4ed8`, and keeps that blue until an
admin picks the blue swatch again. The mails' buttons take the same colour. The public settings
carry it as `app.themeColor`.

## Seasonal themes

Admin › Settings › General › Metadata › Theme, next to the theme colour: **Default**, **Halloween**,
**Christmas** or **GTA VI** (`metadata::seasonal_theme`, public as `app.seasonalTheme`). Default is Theme v2
with the theme colour; choosing it again removes everything a season adds. Saving applies it at
once for the admin; everyone else gets it on their next page load.

A season keeps the layout, type and components and changes:

| | Halloween | Christmas | GTA VI |
|---|---|---|---|
| Accent (over the theme colour, with `accentFor()`'s contrast guards) | Tailwind orange | Tailwind red | Tailwind pink (hot pink) |
| Surfaces (`--theme-bg-*`, `--theme-sidebar`, dropdowns, modals, the raise and hairline tints, muted and faint text, Mantine's dark ramp) | a violet tint on the obsidian | an evergreen tint | a night-indigo obsidian with neutral raise tints; the page gets a faint sunset glow at the horizon and, on wide screens, palm silhouettes in the bottom corners |
| By the sidebar's logo | two bats and a jack-o'-lantern | three snowflakes | two swaying palm trees and a striped sunset sun |
| On a shelf over the sidebar's footer | three pumpkins | a tree between two presents | a striped sunset sun between two palm trees |
| At the end of the phone's top bar | a jack-o'-lantern | a tree | a striped sunset sun |

The drawings are flat inline SVG, `aria-hidden`, without pointer events, and sit in the logo
block's padding, over the footer and beside the phone's title, never over a control. The bats
flutter, the snowflakes turn, and the palm trees sway only under `prefers-reduced-motion: no-preference`. They are the
one exception to "no decorative icons" below, asked for by the owner for these seasons. GTA VI's
drawings are original (a sunset, palms); no game artwork or logos. The root
element carries `data-season` while one is on.

A new season is an entry in `SEASONS` (`seasonalTheme.ts`: accent preset, tokens, dark ramp), its
drawings in `SeasonalDecor.tsx`, a variant of `SeasonalTheme` (`shared/src/settings/metadata.rs`,
the zod enums) and its option label.

## Icons

Upstream's code imports FontAwesome icons. A Vite plugin points every
`@fortawesome/react-fontawesome` import in the panel's own code at `src/lib/icons/FontAwesomeIcon.tsx`.
It draws the matching **Heroicons outline** SVG from `src/lib/icons/heroIcons.ts` (paths generated
into `heroIconPaths.ts` from switched.cloud's `templates/icons/o-*.svg`), falls back to the Tabler
icon in `tablerIcons.ts`, and hands unknown names to FontAwesome. New code keeps importing
FontAwesome names; add the Heroicons equivalent to `heroIcons.ts` when one is missing, otherwise the
Tabler map. Both panels now use the same outline set at 24px/1.75 stroke.

## Rules the theme assumes

- Flat colours; the page floor is the only gradient.
- No decorative icons, shadows (beyond menus and modals), glass effects or background shapes
  (except a seasonal theme's drawings, above).
- One filled button per section; the rest are secondary (`variant='default'`) or subtle.
- Spacing groups things; a box gets a border or a fill, never both, and never a card in a card.

## Check

- The sidebar shows the logo, grouped rows and the avatar row; on a phone it opens from the square
  button at the top.
- The dashboard sidebar lists Home, Paid (while a plan is on sale or the account owns a paid
  server), Resource Store, the membership, Tokens and Backups first, then
  "Account" with an arrow: a click folds its pages open or shut, and the choice is kept after a
  reload. On one of the account's pages it is open. Admin (admins only) is the last row.
- Admin › Settings › General › Metadata: pick a swatch, then a custom colour such as `#facc15`.
  Buttons, active rows and links follow; button text stays readable.
- Metadata › Theme: Halloween, save: buttons and links turn orange, the surfaces get a violet
  tint, two bats flutter by the logo with a jack-o'-lantern, three pumpkins stand on the sidebar's
  balance card, and a phone's top bar ends with a jack-o'-lantern. With reduced motion turned on in
  the system, the bats are still. Christmas: red, an evergreen tint, snowflakes, a tree and
  presents. Default: Theme v2 with the theme colour again, no drawings.
- The server list shows cards with live usage bars; narrowing the window reflows them.
- The server list: Create server at the start of the toolbar row, the search at its end, the cards
  under a hairline, the pagination under them. A card's power button (blue) lists only what the
  state allows; its status bar is green when running, amber while starting or stopping, red when
  offline.
- A console: "Console" with the power buttons, then Name, Status ("Running (2h 14m)"), Address, CPU,
  Memory and Disk, the terminal with its command line, and the three charts while the server runs.
  Ctrl+F (or the quick action) opens the search at the terminal's top end.

## Shared parity with switched.cloud (2026-11-07)

The two panels are one design: a change to one of these is a change to both.

- **Buttons** take Filament's heights — 2.75rem default, 2.125rem `sm`, 1.75rem `xs` — through
  `.theme-btn*` in `app.css` (added in `elements/buttons/Button.tsx`), with the same 1.25/.875/.625rem
  inline padding and 600 weight. Mantine's own explicit sizes still set the font size.
- **Page tabs** (`SubNavigation`) are the Theme v2 tabs from `md` up; below `md` the same sections
  become one labelled `Select` that navigates, so nothing scrolls off a phone.
- **Empty states** are the centred Filament block: a 1rem/600 heading over one .875rem muted line,
  no illustration. `EmptyState` (`flush`) and the switched `ph-eco-empty` block are the same shape.
- **Sidebar group labels** are sentence case (no uppercase tracking), and the logo block is 2rem
  top / 1rem bottom padding with a 2.5rem mark, the same as switched.
- **Header actions** use a .5rem gap; the page header's title is 1.25rem/600 and its subheading
  .875rem/1.25rem muted, as in switched.
- **Sidebar rows** share one sliding pill (220ms cubic-bezier(.16,1,.3,1)) that follows the
  hovered row and returns to the active row on leave; rows stay transparent, only the row under
  the pill reads bright, and a leading Font Awesome glyph carries the same 10px gap as a Heroicon.
- **File fields** use `elements/input/FileInput.tsx` (generic over `multiple`) instead of a one-off
  `.field`/`.input` block, so the ticket attachments match every other field.

### CSS details (2026-11-07)

The smaller values are shared too: body copy 14px; 600-weight buttons, badges and table heads; flat
buttons (no shadow) with the .5 disabled fade; 2.25rem icon buttons; 1rem dropdown items; 10%
badge/alert tints (not 15%); 5% row and card hover (not 10%); the 2px bright input focus ring; stat cards
(.8125rem muted label, 1.5rem/600 white value, .75rem muted description, 1/1.5rem padding); lists as
one `white/5` card; the table header `white/5` band, .75rem row padding and 1.5rem edge cells;
1.5rem empty states; full-height phone modals; and 44px phone inputs.
