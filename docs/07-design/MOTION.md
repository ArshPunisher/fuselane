# Motion language

Companion to [DESIGN-SYSTEM.md](DESIGN-SYSTEM.md). Inspired by the owner's "Claude Motion Tips" page (its open-source effects list and its three workflow habits), adapted from video to a live app.

## 1. Principles

1. **Motion is information.** Every animation must answer: what changed, what caused it, or what is live. If it can't be explained in one sentence, it goes.
2. **Live data moves; idle things are still.** The Weave flows only while bytes flow. A paused transfer is visibly frozen. No decorative infinite loops.
3. **Physical, not mechanical.** Springs for anything the user touches; expo-out curves for entrances; nothing linear except progress tied to real bytes.
4. **Calm by default, expressive at moments.** Two "moments" are allowed to be big: Ignition (complete) and Launch (upload start). Everything else is quick and quiet.
5. **Respect the machine.** Off the main thread where possible, paused when hidden, cheaper on Linux and on battery.

## 2. Tokens

| Token | Value | Use |
|---|---|---|
| `dur-instant` | 90 ms | Press feedback, toggles |
| `dur-quick` | 160 ms | Hovers, tooltips, small state changes |
| `dur-base` | 240 ms | Popovers, list item enter/exit |
| `dur-slow` | 380 ms | Dialogs, sheets, pane transitions |
| `dur-moment` | 700–900 ms | Ignition, Launch (once) |
| `ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` | Entrances |
| `ease-in-out` | `cubic-bezier(0.65, 0, 0.35, 1)` | Moves between two resting places |
| `spring-control` | stiffness 420, damping 34 | Switches, segmented controls, checkbox |
| `spring-panel` | stiffness 220, damping 28 | Sheets, panes, shared-element row → detail |
| `spring-number` | stiffness 140, damping 22 | NumberFlow digits, bar widths |
| `stagger` | 28 ms per item, max 8 items | List reveals |

Only `transform` and `opacity` are animated in the DOM (plus canvas drawing). Never width/height/top/left.

## 3. Catalogue

| Moment | Motion | Reduced motion |
|---|---|---|
| App launch | Surfaces fade in 160 ms; lanes in the sidebar draw from left to right once (240 ms, staggered) | Instant |
| New download dialog | Scale 0.97 → 1 + fade (`spring-panel`); probe states cross-fade; network chips light up as each network passes its probe | Fade only |
| Row → detail | Shared element: the row's icon and name morph into the detail header (`layoutId`) | Cross-fade 120 ms |
| Live speed | NumberFlow digit roll at most 2 Hz with `spring-number`; ETA counts down smoothly between estimates | Numbers update without rolling |
| Weave | Canvas: ribbons ease to the new thickness each tick; particle speed proportional to each lane's real rate | Static ribbons with widths, no particles |
| Lane drops | Ribbon frays (particles scatter, alpha → 0 over 600 ms); the network row orb drifts apart | Ribbon greys out |
| Lane returns | Ribbon re-weaves from the source end | Instant |
| Hedge race | Two sparks on two lanes; the loser's spark fizzles | Badge "Backup" only |
| Block done | Loom cell settles (one 300 ms shimmer pass per completed row, not per cell) | None |
| Pause | Everything decelerates to a stop over 300 ms (not a cut) | Instant stop |
| Error | One horizontal "nudge" (6px, 2 oscillations) on the failing element + danger colour | Colour only |
| Ignition (complete) | Lanes converge → warm flash travels into the file icon → check mark draws (≤ 900 ms) | Check mark appears |
| Launch (upload) | File tile splits into part-tiles that stream off along lanes; share-link card morphs out of the tile | Fade to the share card |
| Network orb | Probing / connecting / live / unreachable states (see DESIGN-SYSTEM §7) | Static icon + label |
| Theme switch | 200 ms cross-fade of tokens (View Transitions where supported) | Instant |

## 4. Implementation

- **UI motion:** `motion` (`motion/react`) with motion values, never React state for continuous values. Components that animate are isolated leaves.
- **Weave, Loom, Orb:** **Canvas 2D**, drawn from a ring buffer of engine snapshots in a `requestAnimationFrame` loop that **never touches React state**. Device-pixel-ratio aware. Optional OffscreenCanvas in a worker where the webview supports it (WebView2, newer WebKit), with a main-thread fallback.
- **Setup diagrams:** Rough.js (MIT, tiny), rendered once (no loop).
- **Landing site only:** CSS scroll-driven animations (`animation-timeline: view()`) for "lanes fuse as you scroll", with an IntersectionObserver fallback. No `window` scroll listeners. Optional playful physics (Matter.js) only if it serves a real section and passes the performance budget.
- Libraries considered from the motion-tips list and **not** used in the app: PixiJS (too heavy for a background utility), Anime.js (Motion covers choreography), p5.brush (Rough.js is lighter for the same feel).

## 5. Performance budget

| Metric | Budget |
|---|---|
| Weave + Loom draw time | ≤ 2 ms/frame on Apple Silicon, ≤ 4 ms on WebKitGTK |
| Frame rate | 60 fps; automatically 30 fps on Linux, on battery saver, or when the user picks "Low power visuals" |
| Hidden / minimised / tray | Drawing **stops** (`visibilitychange` + Tauri window events); the engine keeps running |
| Idle CPU of the app with nothing transferring | ≈ 0% (no loops running) |
| UI updates from core | ≤ 10 Hz per job, deltas only (L-34) |

## 6. Workflow (adapted from the motion-tips habits)

1. **Structured briefs.** Every motion piece starts from a brief with XML-tagged sections, so style, references, timing and acceptance are explicit:

```xml
<motion_brief name="ignition">
  <intent>Tell the user the file is complete and safe, then get out of the way.</intent>
  <style>Warm Fuse flash on graphite; lanes converge; no confetti.</style>
  <references>DESIGN-SYSTEM §2, §7; MOTION §3 row "Ignition".</references>
  <timing>Total ≤ 900 ms: converge 0–350, flash 350–600, check draw 600–900. ease-out; spring-panel for the icon.</timing>
  <states>Before: downloading 99.x%. After: complete screen, still.</states>
  <reduced_motion>Check mark appears; no flash.</reduced_motion>
  <acceptance>Contact sheet approved in both themes at compact and wide sizes; ≤ 2 ms/frame.</acceptance>
</motion_brief>
```

2. **Build the hardest second first.** Before any screen, prototype the Weave alone in a `lab` route with a synthetic feed (2–3 lanes, one drop, one hedge race). Judge texture and movement there; only then wire it into screens. This becomes **spike S9** in Phase 1.
3. **Review keyframes as a contact sheet before merging.** A Playwright script captures each animated moment at 0 / 25 / 50 / 75 / 100% (motion values driven by a test clock), in dark and light, at compact and wide widths, and tiles them into one PNG attached to the PR. A short screen recording follows for pacing.
