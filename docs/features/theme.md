# Theme v2

The panel looks like plugged.host's Pelican panel (its "Theme v2"): a royal-blue accent on obsidian
surfaces, Space Grotesk for text and JetBrains Mono for numbers and code, Tabler's outline icons,
a flat sidebar with grouped rows, and the user's avatar row at its foot. It is dark only.

## Where it lives

| Part | File |
|---|---|
| Mantine palette, component defaults (radius, buttons, tabs, modals, badges) | `frontend/src/theme.ts`, merged into upstream's base theme in `main.tsx` |
| Tokens and everything Mantine's theme object cannot say: surfaces, the page floor, sidebar, inputs, tables, cards, the server list, the console | `frontend/src/app.css`, section "plugged.host Theme v2" |
| Accent colour | `frontend/src/lib/accent.ts`, applied in `App.tsx` |
| Fonts | `frontend/src/assets/fonts/` (self-hosted, Latin + Latin Extended) |
| Icons | `frontend/src/lib/icons/` + `frontend/vite-plugins/tabler-icons.ts` |
| Sidebar (logo block, groups, avatar row, mobile toggle) | `frontend/src/elements/navigation/Sidebar.tsx` |
| Dashboard sidebar rows: Pelican's order (Home, Resource Store, the first membership, Tokens, Backups), then the account's pages folded into an "Account" row with an arrow, Admin last | `frontend/src/routers/DashboardRouter.tsx`, the folding row `frontend/src/elements/navigation/SidebarGroup.tsx` (`Sidebar.Group`) |
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
4.5:1 on the sidebar. Blue, the default, changes nothing. The public settings carry it as
`app.themeColor`.

## Icons

Upstream's code imports FontAwesome icons. A Vite plugin points every
`@fortawesome/react-fontawesome` import in the panel's own code at `src/lib/icons/FontAwesomeIcon.tsx`,
which draws the matching Tabler icon from the map in `src/lib/icons/tablerIcons.ts` and hands
unmapped ones to FontAwesome. New code keeps importing FontAwesome names; add the Tabler equivalent
to the map when one is missing.

## Rules the theme assumes

- Flat colours; the page floor is the only gradient.
- No decorative icons, shadows (beyond menus and modals), glass effects or background shapes.
- One filled button per section; the rest are secondary (`variant='default'`) or subtle.
- Spacing groups things; a box gets a border or a fill, never both, and never a card in a card.

## Check

- The sidebar shows the logo, grouped rows and the avatar row; on a phone it opens from the square
  button at the top.
- The dashboard sidebar lists Home, Resource Store, the membership, Tokens and Backups first, then
  "Account" with an arrow: a click folds its pages open or shut, and the choice is kept after a
  reload. On one of the account's pages it is open. Admin (admins only) is the last row.
- Admin › Settings › General › Metadata: pick a swatch, then a custom colour such as `#facc15`.
  Buttons, active rows and links follow; button text stays readable.
- The server list shows cards with live usage bars; narrowing the window reflows them.
- The server list: Create server at the start of the toolbar row, the search at its end, the cards
  under a hairline, the pagination under them. A card's power button (blue) lists only what the
  state allows; its status bar is green when running, amber while starting or stopping, red when
  offline.
- A console: "Console" with the power buttons, then Name, Status ("Running (2h 14m)"), Address, CPU,
  Memory and Disk, the terminal with its command line, and the three charts while the server runs.
  Ctrl+F (or the quick action) opens the search at the terminal's top end.
