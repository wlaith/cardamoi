# cardamoi brand — rules for building UI

Read this file before writing or changing any UI, copy, icon or asset in this project. It is the source of truth for how cardamoi looks and sounds. When something here conflicts with a library default, this file wins.

cardamoi is open-source desktop software that keeps a person's films and shows in one library on their own machine. The look is editorial and plain: flat fields of colour, square edges, round "seeds", and type that does the work. No gradients, no shadows, no rounded cards.

## Files in this folder

| Path | What it is | How to use it |
|---|---|---|
| `tokens/tokens.css` | All colours, spacing, radii, type styles as CSS variables, both themes | Import once at the app root, after `fonts/fonts.css` |
| `tokens/tokens.json` | The same tokens as data | For scripts, native code, or generating other formats |
| `tokens/tailwind.preset.js` | Tailwind v3 preset mapped to the CSS variables | `presets: [require('./brand/tokens/tailwind.preset.js')]` |
| `tokens/tailwind-v4.css` | Tailwind v4 `@theme` mapped to the CSS variables | Import after `tokens.css` |
| `fonts/` | Host Grotesk and IBM Plex Mono as woff2, plus `fonts.css` | Bundle with the app. Never load fonts from a CDN; the app must work offline |
| `assets/logo/` | Mark, globe emblem, wordmark and lockups as SVG | Use the files as they are; never redraw or retype |
| `assets/app-icon/` | App icons (PNG, `.ico`, `.icns`), favicons | Wire into the desktop build and the website |
| `assets/social/` | Link preview, GitHub preview, README banners | For the repo and website |
| `assets/motion/` | The globe loader as Lottie JSON, `.lottie` and GIF | For loading states (see Loader) |
| `examples/components.html` | Working reference for every component below | Open in a browser; port the CSS rules, keep token names |

## Themes

- Two themes: **Ink** (dark) and **Paper** (light). Set `data-theme="ink"` or `data-theme="paper"` on `<html>`.
- The **app defaults to Ink**. Documentation and the website default to Paper. With no attribute, the OS preference decides.
- Never write `dark:` variants or theme-specific colours in components. Use the `--c-*` variables; they switch with the theme.
- Naming trap: `--c-ink` is the **text colour token**, not the theme. In the Ink theme it is a light colour.

## Colour

Use only these tokens. Never type a hex value in a component. The `--brand-*` variables are fixed inks for logos and artwork only.

| Token | Use for | Never use for |
|---|---|---|
| `--c-surface` | Page and window background | |
| `--c-surface-raised` | Panels, inputs, poster placeholders, dialogs | Shadows; separation comes from this step plus `--c-line` |
| `--c-ink` | Body text, headings, icons | |
| `--c-ink-muted` | Secondary text, metadata, captions, placeholder text | Anything the person must read to act |
| `--c-line` | 1px hairlines, borders, empty progress tracks | The only signal of a state change |
| `--c-red` | The ONE primary action per view, playback and download progress, "new" badges, the active nav marker | Body text on light grounds, decoration, errors that aren't the main action |
| `--c-on-red` | Text and icons on `--c-red` | |
| `--c-red-deep` | Hover state of red buttons; tone-on-tone artwork on red | Readable text |
| `--c-red-text` | Red text and links on `surface` / `surface-raised` / `red-tint` | Fills |
| `--c-red-tint` | Selected row, active nav item, highlighted tile | Status colour |
| `--c-ultramarine`, `--c-on-ultramarine` | Links in long text, editorial callouts | Primary buttons |
| `--c-husk` | Rare relief block (empty-state art, one callout) | Status colour, text background for long copy |
| `--c-focus-ring` | Keyboard focus outline, 2px, offset 2px | Anything else |

Proportion on any screen: mostly `surface`, a little `red`, almost no `ultramarine` or `husk`. If a screen has two red buttons, one of them is wrong.

Contrast is pre-checked: every text token passes WCAG AA (4.5:1) on the grounds listed above in both themes. White text on `--c-red` fails; always use `--c-on-red`.

Status colours (errors, warnings, success) are not part of the brand palette yet. Until they are, show state with text and an icon, using `--c-red-text` for errors only.

## Type

Two families, both bundled in `fonts/`:
- **Host Grotesk** (`--font-sans`) for everything people read.
- **IBM Plex Mono** (`--font-mono`) for labels, numbers and data.

Use the type classes in `tokens.css` (or the matching Tailwind `text-*` sizes). Do not invent sizes.

| Class | Size / line | Weight | Use |
|---|---|---|---|
| `.t-display` | 88 / 80, −0.04em | 600 | Splash and onboarding only, lowercase |
| `.t-headline` | 48 / 48, −0.03em | 600 | Page titles, lowercase |
| `.t-title` | 24 / 28, −0.01em | 600 | Section titles, dialog titles, media detail titles |
| `.t-body` | 16 / 24 | 400 | Running text, descriptions |
| `.t-body-sm` | 14 / 20 | 400 | UI text, list rows, settings |
| `.t-label` | 12 / 16, +0.06em, UPPERCASE | 500 mono | Eyebrows, tags, section meta, badges |
| `.t-data` | 13 / 20, tabular | 400 mono | Years, runtimes, file sizes, counts, timecodes, episode numbers |

- Media titles in tiles are 14px / 600 sans.
- Every number shown to the person is in mono (`.t-data` or `.t-label`).
- The signature pairing: one large sans line plus one small mono line, e.g. a `.t-headline` title with a `.t-label` count aligned to the right of the same row.

## Spacing, radius, borders

- Spacing tokens are a Fibonacci scale: `--space-5`, `-8`, `-13`, `-21`, `-34`, `-55`, `-89`. Use nothing else. Tailwind: `p-13`, `gap-21`, and so on.
- Typical use: 8 inside tight stacks, 13 between tiles and inside controls, 21 card padding and mobile gutters, 34 desktop gutters, 55 between page sections.
- Radius: `0` for everything, `--radius-2` only on text inputs and tags, `--radius-full` for round things (seed dots, avatars). Nothing else is rounded.
- No `box-shadow` anywhere. Separate with `--c-surface-raised` and 1px `--c-line`.
- Control height is 40px. Icon buttons are 40 × 40.

## Components

Each one is implemented in `examples/components.html`. Port it; don't redesign it.

- **Button.** Square, 40px tall, `--space-21` side padding, 14px / 600 sans.
  - Primary: `--c-red` fill, `--c-on-red` text; hover `--c-red-deep` fill with `--brand-paper` text. One per view.
  - Secondary: transparent, 1px `--c-ink` border; hover inverts to an `--c-ink` fill.
  - Ghost: text only; hover underline.
  - Disabled: 40% opacity.
  - Labels are verbs: "Play", "Add to library", "Remove".
- **Input.** 40px, `--c-surface-raised` fill, 1px `--c-line` border, `--radius-2`. On focus the border becomes `--c-focus-ring`. The label above it is `.t-label` in `--c-ink-muted`.
- **Tag.** `.t-label`, 21px tall, 1px `--c-line` border, `--radius-2`, `--c-ink-muted` text. For quality and format: `4K`, `HDR`, `EN SUBS`.
- **Media tile.**
  - Poster at 2:3 on `--c-surface-raised`, square corners.
  - Title below in 14px / 600; one meta line in `.t-data` muted (`1927 · 2 h 33 min`).
  - Hover: a 1px `--c-ink` outline, offset 3px. No scaling, no shadow, no overlay gradient.
  - Grid: `repeat(auto-fill, minmax(144px, 1fr))`, gaps 21 vertical and 13 horizontal.
- **Slot rule (progress bar).** 4px tall, square ends, `--c-line` track, `--c-red` fill. On a poster it sits flush on the bottom edge. Named after the slot in the logo.
- **Seed dots (episodes).** 34px circles with a mono 2-digit number. Watched: `--c-ink` fill. Up next: `--c-red` fill. Unwatched: 1.5px `--c-ink-muted` outline. Each dot carries an `aria-label` with its state.
- **Navigation.** Text rows in `.t-body-sm`, muted until hovered. The active item has a `--c-red-tint` background and a 3px `--c-red` bar on its left edge. Counts are right-aligned in mono.
- **Empty state.** On `--c-surface-raised`: the globe emblem at 144px, a `.t-title` line, one sentence, and one primary button.

## Motion

- Short and functional only: `--dur-fast` (120ms) for hover colour changes, `--dur` (200ms) for panels, `--ease` for both.
- No bounces, no parallax, no scale-on-hover.
- `tokens.css` already disables motion under `prefers-reduced-motion`.

## Loader

The loading animation is the globe emblem turning: the meridians sweep west to east at 30° per second while the ring, latitude lines and the C stay still. It loops seamlessly every second and starts on the exact pose of the static emblem.

- Files in `assets/motion/`: `cardamoi-loader-{ink,paper,red}.json` (Lottie, 500 × 500, 60 fps, 60 frames), the same as `.lottie` (smaller, for dotLottie players), and two GIFs for places that can't run Lottie (README, email).
- Pick the file by ground: `-paper` on the Ink theme, `-ink` on the Paper theme and on `--c-red` fields, `-red` for the signature look on Ink.
- Play it with `lottie-web` (`renderer: 'svg'`, `loop: true`) or `@lottiefiles/dotlottie-web`. Size it with its container; keep it square.
- Use it for waits longer than about 500ms: first launch, library scans, fetching metadata. Show it at 48px or larger. For anything smaller, or for a known amount of work, use the slot-rule progress bar instead.
- Give the container `role="img"` and `aria-label="Loading"`, or put the status text ("Scanning your library") next to it.
- Under `prefers-reduced-motion: reduce`, don't play it: show `assets/logo/cardamoi-globe-*.svg` in the same place.
- Never recolour it with CSS filters, rotate the whole emblem, or change its speed per screen. `make_loader.py` regenerates the files if the emblem changes.

## Logo

All files are in `assets/logo/`. Each SVG is a single solid colour; pick the file in the ink you need. Never recolour with filters, never redraw, never set the wordmark in live type.

- **Pod mark** `cardamoi-mark-{ink,paper,red,white}.svg` is the primary logo. Minimum width 24px.
- **Globe emblem** `cardamoi-globe-*.svg` is for app icons, avatars, empty states and print. Minimum 64px; below that, use the pod mark.
- **Lockups**: `cardamoi-lockup-horizontal-*` is the default (app sidebar, headers, README), minimum 18px tall. `-stacked-*` is for square spaces, minimum 42px tall. `-globe-*` is for print and merchandise.
- **Ink by ground:**
  - On `--c-surface` in Paper: `ink`.
  - On `--c-surface` in Ink: `paper`, or `red-on-ink` for the signature look.
  - On `--c-red`: `ink`.
  - Swap the file when the theme changes.
- **Clear space:** at least a quarter of the mark's height on all sides.
- **Never:** rotate, mirror, stretch, outline, add effects, close the slot, or place on a photo without a solid field behind it.

## App icons and web head

- macOS: `assets/app-icon/icon.icns` (from `icon-macos@1024.png`).
- Windows: `assets/app-icon/icon-windows.ico` (16–256).
- Linux: `icon-red@{64,128,256,512,1024}.png`.
- Tray icons and anything 48px or smaller: `icon-red-small@{16,32,48}.png`, which uses the pod mark because the globe's lines blur at small sizes.
- Website `<head>`:

```html
<link rel="icon" href="/favicon.ico" sizes="48x48">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/icon-red@256.png">
<meta name="theme-color" content="#131313" media="(prefers-color-scheme: dark)">
<meta name="theme-color" content="#f1eee8" media="(prefers-color-scheme: light)">
<meta property="og:image" content="/og-image-1200x630.png">
<meta name="twitter:card" content="summary_large_image">
```

- README top:

```html
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="brand/assets/social/readme-banner-dark-1280x320@1280.png">
  <img alt="cardamoi" src="brand/assets/social/readme-banner-light-1280x320@1280.png">
</picture>
```

## Icons

There is no custom icon set yet. Use **Lucide** with `stroke-width={2}`, `strokeLinecap="square"`, `strokeLinejoin="miter"`, at 16, 20 or 24px, coloured with `currentColor`. Pair every icon-only button with an `aria-label`. Don't mix in other icon sets.

## Writing (UI copy)

- The product name is always lowercase: `cardamoi`, including at the start of a sentence.
- Plain, short, friendly. Speak as someone who knows films: "Added 12 episodes of Sherlock Jr." not "Content successfully synchronized!"
- Say "you" and "your library". Use people's words: films, shows, seasons, episodes, subtitles. Never "media assets", "items", "entities" or "content".
- Buttons are verbs that say what happens: "Play", "Add folder", "Remove from library". A result echoes the verb: "Removed".
- Errors say what went wrong and what to do: "Couldn't read this folder. Check that the drive is connected."
- No exclamation marks, no emoji, no "Oops", no superlatives.
- Headlines and page titles are lowercase; everything else is sentence case.
- Numbers: `1 284 titles` (thin space thousands), `2 h 33 min`, `S02 E07`, `62 %`, `1.4 GB`.

## Accessibility

- Every interactive element shows the focus ring (`:focus-visible` is set globally in `tokens.css`; don't remove it).
- Text pairs are pre-checked for contrast. Don't introduce new text/ground combinations without checking 4.5:1.
- Never rely on colour alone: watched/unwatched episodes and download state also carry text or an `aria-label`.
- Hit targets are at least 34 × 34px.

## Before you finish a UI change

- [ ] Only `--c-*`, `--space-*`, `--radius-*` and type classes used. No raw hex, px spacing or new font sizes.
- [ ] At most one `--c-red` primary action in the view.
- [ ] Works in both `data-theme="ink"` and `data-theme="paper"`, including the logo file swap.
- [ ] No shadows, gradients or rounded corners outside the rules.
- [ ] Copy follows the writing rules, and `cardamoi` is lowercase.
- [ ] Numbers are in mono.
- [ ] Loading states use the globe loader (48px+) or the slot rule, and respect reduced motion.
