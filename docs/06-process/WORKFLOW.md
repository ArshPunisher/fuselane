# How we work

## 1. The loop for every step

```text
1. Read   STATUS.md → the next step in STEPS.md → the design doc for the area → the L-xx rules it names
2. Plan   If the step changes a design decision: write or update the design doc / an ADR first
3. Test   Write the failing test first (unit/property for pure logic; testkit scenario for behaviour)
4. Build  The smallest change that makes it pass, on all three OSes
5. Check  fmt, clippy, tests locally; push; CI green on macOS, Windows, Linux
6. Commit Small, focused commits (see §3), each one building and passing
7. Record Tick the step in STEPS.md (and PARITY-CHECKLIST.md if it applies), update STATUS.md
```

**Never** write code ahead of the design. **Never** tick a box without a test. **Never** copy Plexo code (ADR 0005).

## 2. Branches

- `main` is always releasable: protected, CI required, no force-push.
- Work branches: `feat/<area>-<thing>`, `fix/<area>-<thing>`, `docs/<thing>`, `spike/sN-<thing>` (spikes are never merged; only their write-ups are).
- Short-lived branches (≤ 2–3 days), merged by squash or rebase so the history stays readable.
- While it's a solo project in P0, docs may be committed straight to `main`.

## 3. Commits

- **Conventional commits:** `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `build`, `ci`, `perf`, with a scope: `feat(engine-http): …`, `fix(netif): …`, `docs(plan): …`.
- **One logical change per commit.** Stage only the files or hunks for that change. Each commit builds and passes tests.
- The body explains **why**, not what.
- **Author is the owner only.** No `Co-Authored-By`, no "Generated with…", no AI attribution in commits or PR descriptions.
- Never commit secrets, `.env` files, certificates or keys.
- "commit" = commit only. "commit and push" = commit, then push the current branch. Never push to another branch without asking.

## 4. Pull requests

The template asks for:
- What and why (link to the STEPS.md item)
- `L-xx` rules relied on, and `EC-xxx` tests added
- OSes tested locally, and screenshots for UI changes (including the minimum window size)
- Docs updated (design doc / ADR / parity / STATUS)

## 5. Definition of done

- [ ] Tests at the right level, passing on all three OSes in CI
- [ ] No new warnings; clippy and eslint clean
- [ ] Errors go through the catalogue (no raw messages to users; backend uses correct status codes)
- [ ] Docs and checklists updated
- [ ] For UI: design-taste-frontend followed, web-design-guidelines review done, checked in a real window with playwright-cli where possible

## 6. Versioning and releases

- **SemVer**, with pre-releases `0.x.y-beta.N`. Version comparison is tested (L-76).
- The desktop app and CLI share one version. The extension and backend have their own.
- Release = tag `vX.Y.Z` on `main` → the release workflow builds, signs and publishes, and writes the updater feed. Never build releases on a laptop (L-73).
- A changelog is written from conventional commits (git-cliff) and edited by hand.

## 7. Working with Claude across sessions

- Claude reads `CLAUDE.md` automatically, then `docs/04-plan/STATUS.md`.
- At the end of every session Claude updates STATUS.md (what was done, what's next, what's blocked) and ticks STEPS.md.
- Decisions taken in conversation get written down (OPEN-QUESTIONS.md answer or an ADR) **in the same session**.
