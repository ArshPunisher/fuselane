# Design system: "Lanes → Fuse"

Status: **direction approved by the owner (2026-10-08): "fully responsive UI with amazing colour, aesthetic, graphics and motion"**. This is the source of truth for every surface: the desktop app, extension popup, share page, landing site. Motion has its own doc: [MOTION.md](MOTION.md). Implementation lands in `packages/ui` during P3.

Rules from the owner's global setup apply to every UI change: load the **design-taste-frontend** skill first, review with **web-design-guidelines** afterwards, and check in a real browser with **playwright-cli** (including the smallest window size and 375px for web pages).

---

## 1. Design read and dials

| Surface | Design read | VARIANCE | MOTION | DENSITY |
|---|---|---|---|---|
| Desktop app | A premium utility for people who watch their transfers, with a Raycast-grade dark-tool chrome plus one living data visualisation that is the product's signature | 5 | 6 | 6 |
| Extension popup | A tiny companion; instant, calm | 3 | 3 | 5 |
| Share page | A trust-first receiving page for someone who never heard of Fuselane | 4 | 4 | 3 |
| Landing site | A consumer-tech launch page with an interactive real product component as the hero | 8 | 7 | 3 |

References (inspiration, not copies): Raycast (surface ladder, hairlines, restraint), Warp (dark tool polish), Linear (density and calm). Avoid the LLM defaults: AI-purple glow, centred hero over a mesh blob, three equal cards, Inter on slate-900.

## 2. The concept

**Each network is a lane of moving light. Where the lanes meet, they fuse into one warm current.**

- Lanes are **cool** colours (cyan, chartreuse, periwinkle…): many separate sources.
- The fused result is the single **warm brand accent, "Fuse"** (molten orange): one fast stream.
- So the colour itself tells the story: cool lanes in, warm current out. The brand accent is used only for "the combined thing" (total speed, the primary action, the fused stream, the completed file). That is how the one-accent rule is kept.

## 3. Colour tokens (OKLCH, with sRGB hex; contrast verified)

Dark is the hero theme; light is first-class (not an afterthought). Default follows the OS; the user can pick Light, Dark or System.

### Neutrals: "Graphite" (one cool family, never mixed with warm greys)

| Token | Dark | Light |
|---|---|---|
| `canvas` | `oklch(0.155 0.010 260)` #0a0c11 | `oklch(0.985 0.003 260)` #f9fafc |
| `surface-1` (panels) | `oklch(0.185 0.011 260)` #101318 | `oklch(0.995 0.002 260)` #fdfdff |
| `surface-2` (raised rows, inputs) | `oklch(0.215 0.012 260)` #161a1f | `oklch(0.965 0.004 260)` #f2f3f6 |
| `surface-3` (hover, popovers) | `oklch(0.255 0.013 260)` #1f2329 | `oklch(0.935 0.006 260)` #e7eaee |
| `hairline` | `oklch(1 0 0 / 0.08)` | `oklch(0.21 0.015 260 / 0.10)` |
| `hairline-strong` | `oklch(1 0 0 / 0.16)` | `oklch(0.21 0.015 260 / 0.18)` |
| `ink` (headings, numbers) | `oklch(0.965 0.004 260)` #f2f3f6 (17.6:1) | `oklch(0.21 0.015 260)` #14181f (17.0:1) |
| `body` | `oklch(0.82 0.008 260)` #c1c4c9 (11.2:1) | `oklch(0.36 0.015 260)` #393d45 (10.4:1) |
| `mute` | `oklch(0.66 0.012 260)` #8e929a (5.6:1 on surface-2) | `oklch(0.50 0.015 260)` #5e646c (5.4:1 on surface-2) |

No pure `#000` and no pure `#fff` anywhere.

### Brand accent: "Fuse"

| Token | Dark | Light |
|---|---|---|
| `fuse` | `oklch(0.74 0.17 50)` #fd8537 (8.0:1 on canvas) | `oklch(0.56 0.165 42)` #c04909 (4.8:1) |
| `fuse-ink` (text on a fuse fill) | `canvas` (8.0:1) | `oklch(0.99 0 0)` |
| `fuse-soft` (tinted fills) | `fuse / 0.14` | `fuse / 0.10` |

### Lanes (network colours; 8 swatches, assigned by kind first, user can change)

| Lane | Default for | Dark | Light (text-safe) |
|---|---|---|---|
| **Tide** | Wi-Fi | `oklch(0.78 0.12 215)` #43cae7 | `oklch(0.52 0.095 222)` #057490 |
| **Volt** | USB tether / phone | `oklch(0.85 0.17 128)` #ade25f | `oklch(0.52 0.13 130)` #4f7715 |
| **Iris** | Ethernet | `oklch(0.72 0.13 280)` #959af4 | `oklch(0.52 0.16 280)` #5a58c2 |
| **Rose** | Cellular / WWAN | `oklch(0.74 0.14 350)` #ec84b7 | `oklch(0.56 0.17 355)` #bc3e79 |
| **Mint** | 2nd of a kind | `oklch(0.81 0.12 172)` #61dab8 | `oklch(0.52 0.095 172)` #157a63 |
| **Sky** | 3rd of a kind | `oklch(0.74 0.12 245)` #65b2f1 | `oklch(0.52 0.14 250)` #116bb5 |
| **Lilac** | other | `oklch(0.76 0.11 312)` #c99de4 | `oklch(0.54 0.14 312)` #8a53a9 |
| **Steel** | unknown / VPN | `oklch(0.72 0.04 250)` #92a7bd | `oklch(0.50 0.04 250)` #52657a |

All dark lanes are ≥ 7.6:1 on canvas; all light lanes are ≥ 4.7:1. Lanes are **never** warm orange, so they never compete with Fuse. Colour is never the only signal: every lane also carries its **kind icon** and, in charts, its own **dash pattern** (colour-blind safety).

### Semantic

| Token | Dark | Light |
|---|---|---|
| `success` | `oklch(0.79 0.15 152)` #65d68a | `oklch(0.52 0.13 152)` #137d41 |
| `warning` | `oklch(0.86 0.15 92)` #f4cd4b | `oklch(0.54 0.115 72)` (≥ 4.5:1) |
| `danger` | `oklch(0.70 0.19 18)` #fe6270 | `oklch(0.56 0.20 22)` #d02a3a |

Semantic colours always come with an icon and words; never a bare coloured dot.

## 4. Typography

- **Geist Sans** (variable, OFL, self-hosted) for all UI; **Geist Mono** for **every number** (speeds, sizes, percentages, ETAs) with `font-variant-numeric: tabular-nums` so live numbers never jitter.
- Scale (app): 12 / 13 / 14 (body) / 16 / 20 / 28 / 40 (hero speed). Landing: up to 64 for 3–5 word headlines.
- Weights: 400 body, 500 labels, 600 headings, 650 for the hero speed number. Emphasis uses weight or italic of the same family, never a second family.
- Truncated text uses `leading-normal` or looser (L-81). Italic display words with descenders get `leading-[1.1]` plus bottom room.

## 5. Shape, depth, spacing

- **Radius rule (documented, used everywhere):** controls and inputs 8px · panels, cards, dialogs 14px · chips, toggles, lane pills full-pill. Nothing else.
- **Depth:** the surface ladder plus hairlines, not shadows. Floating layers (popover, dialog, toast) get a tinted shadow `0 12px 40px oklch(0.155 0.01 260 / 0.45)` (dark) and an inner top highlight `inset 0 1px 0 oklch(1 0 0 / 0.06)`.
- **Glass:** only on the floating title bar and popovers on macOS and Windows; **solid fallback on Linux/WebKitGTK and under `prefers-reduced-transparency`**.
- **Spacing:** 4px grid. App density: row height 52px regular, 44px compact; section gaps 24–32px.
- **Z-index scale (only these):** base 0 · sticky 10 · popover 20 · dialog 30 · toast 40 · title bar 50.

## 6. Icons and imagery

- **Phosphor Icons** (`@phosphor-icons/react`), "regular" weight, one family only. Network kinds: `WifiHigh`, `DeviceMobile`, `HardDrives`/`Plugs`, `CellSignalFull`, `Globe`.
- **No hand-drawn SVG icons.** The signature graphics (Lane Weave, Loom, orbs) are **data visualisations rendered from real data**, not decoration.
- **Setup guides** (tethering, the Windows Wi-Fi policy, macOS Open Anyway) use **Rough.js** sketch-style diagrams generated from data, so instructions feel friendly and distinct from live UI (inspired by the open-source effects list in the owner's motion tips).
- Landing and share page photography: generated or real product screenshots only; no div-built fake screenshots. The landing hero is the **real Lane Weave component** running a demo feed.

## 7. Signature graphics (all driven by real engine data)

| Graphic | Where | What it shows |
|---|---|---|
| **Lane Weave** | Transfer detail hero, landing hero | One ribbon per network; ribbon **thickness = live throughput**; particles = blocks in flight at true relative speed; lanes braid into the warm Fuse current that flows into the file. A lane that drops **frays and fades**; a hedge race shows **two sparks** racing on different lanes; a lane at its data limit dims with a lock glyph. |
| **Loom** (block field) | Below the Weave | The file as a woven strip: each cell is a block, filled as a thread in the colour of the lane that fetched it; in-flight cells shimmer subtly; completed rows settle once. Hover shows block details. Virtualised. |
| **Strata chart** | Detail and complete screens | Last-60 s stacked areas per lane (soft gradient fills, dash pattern per lane) with the fused total as a crisp Fuse-coloured line. |
| **Network orb** | Network rows and onboarding | Dotted orb inspired by "thinking orbs": *probing* = scattered dots searching, *connecting* = dots converging, *live* = a steady ring whose breathing rate follows throughput, *unreachable* = dots drift apart and grey out. |
| **Ignition** | Completion | Lanes converge, one warm flash travels into the file icon, then everything goes calm (≤ 900 ms, once). |
| **Launch** | Upload start | The file tile splits into part-tiles that stream off along each lane; the share-link card morphs out of the file tile when done. |

## 8. Responsive layout (the app is a resizable window, so it's designed like a responsive site)

| Width | Layout |
|---|---|
| **Compact < 640px** (min window 360 × 560) | Single column. Bottom tab bar: Transfers · Send · Networks · Settings. Detail opens as a full-height sheet. The Weave collapses to a slim 64px band. |
| **Regular 640–1023px** | Left icon rail + list; detail replaces the list with a shared-element transition (row → detail header). |
| **Wide ≥ 1024px** | Three panes: sidebar (filters, networks with live mini-lanes), list, detail. Panes resizable, sizes remembered. |
| **≥ 1440px** | Detail gets the full Weave + Strata side by side. |

Web surfaces (share page, landing): 375 → 1536px, `min-h-[100dvh]`, CSS Grid, explicit single-column collapse below 768px.

## 9. Components (in `packages/ui`, built on Base UI primitives, fully restyled)

Button (primary = Fuse fill; secondary = surface-3; ghost; destructive), IconButton, Input (label above, error below), Select, Switch, Segmented control, Checkbox (tri-state), Chip/LanePill, Tooltip, Popover, Dialog, Sheet (compact), Toast, Menu, Tabs, ProgressBar (segmented by lane), Skeleton (shaped like the real content), EmptyState, Kbd, NumberFlow (animated tabular number), Orb, Weave, Loom, Strata.

Every interactive component ships with **hover, focus-visible (2px Fuse ring with offset), active (scale 0.98), disabled (aria-disabled when it has a tooltip, L-83), loading, error** states.

## 10. Content and copy rules

- Plain language, a verb on every button ("Download", "Pause", "Copy link"); one label per intent across the whole app.
- Vocabulary: networks, streams, blocks, combined speed. "Lanes" and "fuse" are visual metaphors for marketing and graphics, not words the user must learn.
- **No em dashes or en dashes in any UI text** (use a period, comma or colon), no "Elevate / Seamless / Unleash", no fake-precise numbers on the landing page; real measured numbers only.
- Errors come from the catalogue (ERRORS.md): what happened plus what to do.

## 11. Accessibility

WCAG 2.2 AA minimum (AAA for body text where possible); 24px minimum targets; full keyboard support; screen-reader labels for every live number with polite, throttled announcements; `prefers-reduced-motion`, `prefers-reduced-transparency`, `prefers-contrast` all honoured; automated axe checks in CI; both themes checked before shipping.
