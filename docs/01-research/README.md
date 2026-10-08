# Research

Research done on 2026-10-08, before any code. Treat these files as **reference**: the decisions made from them live in `../02-product`, `../03-architecture` and `../adr`.

| File | What it is | When to read it |
|---|---|---|
| [PLEXO-ANALYSIS.md](PLEXO-ANALYSIS.md) | First-pass analysis of Plexo: stack, platforms, flaws, ideas | Orientation |
| [plexo-forensics.md](plexo-forensics.md) | About 34k words: every Plexo feature, about 60 fix commits with root causes, the error taxonomy, 120 edge cases, every constant, every test, weaknesses | Before building any engine, storage, networking or test piece. Search it for the area you're working on. |
| [market-research.md](market-research.md) | Plexo's GitHub issues, 18 competitors, user pain points (creators, Indian data caps), top 15 opportunities | Product and priority decisions |
| [tech-research.md](tech-research.md) | Current versions and APIs: Tauri 2, socket2, per-OS binding, DNS, librqbit vs libtorrent, S3/R2 multipart, WXT, native messaging, signing, Android, test tooling | Before choosing or upgrading a library |
| [naming.md](naming.md) | 29 names checked for conflicts | History only. The name is **Fuselane**. |

Plexo citations (`file:line`, commit hashes) refer to its repository at commit `21ca1ef` (rc.14). A local copy was studied at `~/Documents/sample/plexo`.

Items marked **(verify)** in tech-research are unconfirmed. Check them in the relevant spike before relying on them.
