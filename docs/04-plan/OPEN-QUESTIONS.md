# Open questions (decisions the owner needs to make)

When one is decided, record the answer here (with the date), then update the affected doc or write an ADR.

| # | Question | Options / recommendation | Blocks | Decision |
|---|---|---|---|---|
| Q1 | **Licence?** | (a) **MIT or Apache-2.0**: max adoption, matches Plexo's openness, and SignPath Foundation (free Windows signing) requires an OSI licence. (b) **GPL-3.0**: forks must stay open. (c) Closed source with a free tier. *Recommendation: Apache-2.0 for the core and app (patent grant); keep the backend private if uploads become a paid service.* | 0.7 | – |
| Q2 | **GitHub location?** | A personal account or an org (`fuselane` / `fuselane-app`). Public from day one, or private until the beta? *Recommendation: an org named `fuselane-app`, private until P4, then public.* | 0.7 | – |
| Q3 | **Signing budget?** | Apple Developer $99/yr (required for notarization). Windows: Microsoft Artifact Signing (about $10/mo, eligibility rules apply) or SignPath Foundation (free for open source, needs an OSI licence + public repo). | 0.13, P4 | – |
| Q4 | **Domain?** | `fuselane.app` (nothing answered there when checked); alternatives `getfuselane.app`, `fuselane.dev`. Confirm at a registrar. | 0.13, P6 | – |
| Q5 | **Business model for uploads?** | Free tier (for example 10 GB per link, 7-day expiry) + a paid tier (bigger, longer, branding) to cover R2 storage; or free only with bring-your-own bucket. | P6 | – |
| Q6 | **Encryption and abuse policy?** | E2E encryption on by default (privacy, can't scan) vs optional (can scan unencrypted). It affects legal exposure. | P6 | – |
| Q7 | **Accounts?** | Device key only (no sign-up) for MVP vs email accounts from the start. *Recommendation: device key first.* | P6 | – |
| Q8 | **Crash reporting provider?** | Self-hosted (GlitchTip) vs Sentry free tier vs none. Must be opt-in. | P4 | – |
| Q9 | **Minimum OS versions?** | Proposed: macOS 13.3, Windows 10 21H2, Linux kernel 5.7 / Ubuntu 22.04 (see PLATFORMS.md) | 0.8 | – |
| Q10 | **Test hardware available?** | Which machines and phones do we have for the L8 matrix (Mac model, Windows PC, Linux box, iPhone, Android, 5G SIM)? | P1 | – |
| Q11 | **Visual direction for the UI?** | Pick a reference from `~/.claude/design-md` or describe a direction before P3 | P3 | – |
