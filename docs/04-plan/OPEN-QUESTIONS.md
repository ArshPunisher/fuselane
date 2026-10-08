# Open questions (decisions the owner needs to make)

When one is decided, record the answer here (with the date), then update the affected doc or write an ADR.

| # | Question | Options / recommendation | Blocks | Decision |
|---|---|---|---|---|
| Q1 | **Licence?** | (a) **MIT or Apache-2.0**: max adoption, matches Plexo's openness, and SignPath Foundation (free Windows signing) requires an OSI licence. (b) **GPL-3.0**: forks must stay open. (c) Closed source with a free tier. *Recommendation: Apache-2.0 for the core and app (patent grant); keep the backend private if uploads become a paid service.* | 0.7 | **2026-10-08: open source; Apache-2.0** (OSI, required by SignPath) |
| Q2 | **GitHub location?** | A personal account or an org (`fuselane` / `fuselane-app`). Public from day one, or private until the beta? | 0.7 | **2026-10-08: personal GitHub account.** Public is recommended (free unlimited CI, required by SignPath). Remote creation waits for the owner to confirm visibility. |
| Q3 | **Signing budget?** | Apple Developer $99/yr (required for notarization). Windows: Microsoft Artifact Signing (about $10/mo, eligibility rules apply) or SignPath Foundation (free for open source, needs an OSI licence + public repo). | 0.13, P4 | **2026-10-08: no paid services. SignPath Foundation for Windows; macOS ad-hoc + install script + Homebrew tap** ([ADR 0009](../adr/0009-zero-cost-policy.md)) |
| Q4 | **Domain?** | `fuselane.app` (nothing answered there when checked); alternatives `getfuselane.app`, `fuselane.dev`. Confirm at a registrar. | 0.13, P6 | **Deferred: the owner will check later.** Use free subdomains (GitHub/Cloudflare Pages, workers.dev) until then |
| Q5 | **Business model for uploads?** | Free tier limits that keep the backend inside the Cloudflare free tier, plus bring-your-own bucket for bigger needs | P6 | **2026-10-08: no paid services, so free tier with hard quotas + BYO bucket** (exact limits decided in P6) |
| Q6 | **Encryption and abuse policy?** | E2E encryption on by default (privacy, can't scan) vs optional (can scan unencrypted). It affects legal exposure. | P6 | – |
| Q7 | **Accounts?** | Device key only (no sign-up) for MVP vs email accounts from the start. *Recommendation: device key first.* | P6 | – |
| Q8 | **Crash reporting provider?** | – | P4 | **2026-10-08: none (ADR 0009).** Local logs + Copy diagnostics |
| Q9 | **Minimum OS versions?** | Proposed: macOS 13.3, Windows 10 21H2, Linux kernel 5.7 / Ubuntu 22.04 (see PLATFORMS.md) | 0.8 | – |
| Q10 | **Test hardware available?** | Which machines and phones do we have for the L8 matrix (Mac model, Windows PC, Linux box, iPhone, Android, 5G SIM)? | P1 | – |
| Q11 | **Visual direction for the UI?** | – | P3 | **2026-10-08: "fully responsive UI with amazing colour, aesthetic, graphics and motion"** → [`docs/07-design/DESIGN-SYSTEM.md`](../07-design/DESIGN-SYSTEM.md). **Fuse Core design approved by the owner after seeing the real window ("design good").** |
| Q12 | **Chrome Web Store ($5 one-time)?** | The only paid item left; needed for one-click installs on Chrome. (Google Play is moot: no Android app, ADR 0010.) | P7 | – |
| Q13 | **Make the GitHub repo public now?** | Needed for free CI on macOS/Windows/Linux and for SignPath | 0.7 | **2026-10-08: yes.** Public at https://github.com/ArshPunisher/fuselane |
