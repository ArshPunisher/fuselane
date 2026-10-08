# Naming research: multi-connection bonding app (download + upload + share links)

> **Decision (2026-10-08): the product is named Fuselane.** Sangam, Pluro and the others were rejected (Pluro because many websites already use it). This file is kept as history.

Checked 2026-10-08. Method: `registry.npmjs.org/<name>` and `crates.io/api/v1/crates/<name>` (200 = taken, 404 = free), `github.com/<name>` (user or org exists), HTTPS probe of `<name>.app`, `get<name>.app`, `<name>.dev`, `<name>.io` (code 000 = no HTTPS site answered, which suggests the domain is free or parked without TLS; it does not prove it can be registered), plus web searches for existing products. Trademark notes are informal, not legal clearance. I could not query the Chrome Web Store directly; the CWS column reflects web-search results only.

Main competitors to stay clear of: **Speedify** (formerly Connectify), **Connectify Dispatch**, dispatch-proxy, Peplink SpeedFusion, WeTransfer.

## Candidate table

| Name | Meaning | npm | crates | GitHub handle | Domains (.app / get.app / .dev / .io) | Known conflicts / TM risk | Verdict |
|---|---|---|---|---|---|---|---|
| **Sangam** | Sanskrit: confluence of rivers | free | free | taken (user) | parked (Hostinger) / none / for sale (Spaceship) / site | Sangam.com is a Shaadi.com matrimonial brand (India, other class). No networking or file-transfer software found | Strong |
| **Triveni** | Sanskrit: meeting of *three* rivers (fits Wi-Fi + tether + Ethernet) | taken | free | taken | site / none / site / site | Triveni Turbines and Triveni Engineering (Indian industrials, other class); a poetry form. No software conflict | Strong |
| Braid | strands woven together | taken | taken | taken | none / site / site / none | Braid HTTP protocol (braid.org), Braid video game, rafd/braid chat app | Crowded |
| Weft | crosswise woven threads | taken | taken | taken | none / site / none / none | Weft textile software (Cornell spinout), stevekinney/weft workflow engine | Medium |
| Lanes | parallel lanes | taken | taken | taken | none / none / redirect / site | Generic dictionary word, so it is hard to register as a trademark and hard to search for | Weak |
| Plait | braid | taken | taken | taken | none / site / none / site | Plait (Rust/Ruby libraries); getplait.app is live | Medium |
| Tributary | stream feeding a river | taken | taken | taken | site / none / none / redirect | Tributary data tools, tributary.io; long (4 syllables) | Medium |
| Twine | twisted strands | taken | taken | taken | none / site / none / site | **Twine** (interactive fiction tool) and **twine** (Python upload tool). High developer confusion | Avoid |
| Hawser | thick ship rope | free | taken | taken | none / redirect / none / site | Obscure word that people may misspell or mispronounce | Weak |
| Strand | single thread | taken | taken | taken | none / none / site / none | Strand Software, NYT "Strands" game; the meaning is the opposite of "many joined into one" | Avoid |
| Confluo | Latin: flow together | free | free | taken | none on all four | **Confluo** is a known UC Berkeley RISELab project (ucbrise/confluo, real-time monitoring). Close to *Confluent* (Kafka) | Medium-risk |
| Bondwave | bond + wave | free | free | taken | none on all four | **BondWave** is a US fintech company (bond-trading software). TM risk in software classes | Avoid |
| Tandem | together | taken | taken | taken | site on .app / get.app / .dev | Tandem language app, Tandem Diabetes, and others. Very crowded | Avoid |
| Duplo-net | double net | n/a | n/a | n/a | n/a | Collides with **LEGO DUPLO**, a famous trademark | Avoid |
| Skein | bundle of yarn; a flock of geese in flight | taken | taken | taken | none / **site (Skein journal app)** / site / none | Skein hash function, Skein journaling app, rust-adventure/skein (324 stars), jcrist/skein (YARN deploys) | Medium |
| Plyo | ply = strands twisted into yarn | taken | free | taken | redirect / none / site / redirect | Plyo is also a fitness term (plyometrics) | Weak |
| Splice | join two ropes | taken | taken | taken | redirect / redirect / none / none | **Splice** music platform (large brand) | Avoid |
| Yoke | joins two oxen to pull together | taken | taken | taken | none / redirect / none / none | Several minor uses. The word suggests a burden or harness | Medium |
| Tressa | from tress (braid) | taken | free | taken | none on all four | Mostly used as a person's name | Medium |
| Kumi | from *kumihimo*, Japanese cord braiding | taken | free | taken | rate-limited / none / site / none | Kumi is a common name and brand | Medium |
| **Fuselane** | fuse + lanes | **free** | **free** | **free** | **none on all four** | No product found in search. A coined word, so it can be trademarked | Strong |
| Merjo | merge + flow | free | free | taken | none on all four | No conflicts found. Weaker meaning | OK |
| Prayag | Sanskrit: the Triveni Sangam site | taken | free | taken | none / none / redirect / site | Place name (Prayagraj); has religious significance | Medium |
| **Duolane** | two-plus lanes | free | free | taken | none on all four | Duolingo-like "Duo" prefix. Implies only two connections | OK |
| **Plyline** | ply + line: strands twisted into one line | **free** | **free** | **free** | **none on all four** | No conflicts found; "polyline" (graphics term) is a near-homophone | Strong |
| Laneweave | lanes + weave | free | free | free | none on all four | Clean but long (3 syllables, 9 letters) | OK |
| Cordel | cord | free | free | taken | site / none / none / none | Cordel (Brazilian folk-literature genre); cordel.app is live | Medium |
| Braidr | braid + r | free | free | free | site / none / none / none | braidr.app is live; the dropped-vowel style looks dated | Weak |
| Multilane | multi lanes | taken | free | taken | site / none / site / none | MultiLane Inc. (high-speed I/O test equipment, networking-adjacent) | Avoid |

## Top 5 recommendations

### 1. Sangam (best story)
- **Meaning:** the confluence of rivers. Many streams become one stronger flow, which matches the product exactly, and the name has warmth and cultural depth.
- **Memorability:** two syllables, easy to say globally (SUNG-um), and distinctive in tech.
- **Conflict risk:** low in software. npm and crates are free. Sangam.com belongs to Shaadi.com (a matrimonial site, different trademark class, but a well-known brand in India). The bare `.app`, `.dev` and `.io` are parked or for sale, so use a prefixed domain.
- **Domain:** `getsangam.app` (no site found). Alternatives are `sangam.so` and `usesangam.com`. `sangam.dev` is listed for sale on Spaceship.
- **CLI:** `sangam get <url>`, `sangam send <file>`
- **Tagline:** "Every connection, one current."
- **Share links:** `sangam.link/abc123`, or a subpath on the main domain.

### 2. Fuselane (cleanest namespace)
- **Meaning:** fuse your lanes. It is self-explanatory for speed and joining connections.
- **Memorability:** compound of two familiar words; spells the way it sounds.
- **Conflict risk:** the lowest of any candidate. npm, crates and the GitHub handle are all free, no HTTPS site answered on any of the four domains, and search found no product. A coined word is easy to trademark.
- **Domain:** `fuselane.app` (try it first) or `getfuselane.app`
- **CLI:** `fuselane` (alias `fl`)
- **Tagline:** "Fuse every connection into one fast lane."

### 3. Triveni (best fit for three connections)
- **Meaning:** Sanskrit for the meeting point of three rivers. Wi-Fi, tether and Ethernet map neatly onto three rivers. It pairs well with Sangam: "Triveni Sangam" is the actual name of that confluence.
- **Memorability:** melodic (tri-VAY-nee). The "tri" also reads as "three" to English speakers.
- **Conflict risk:** low to medium. Triveni Turbines and Triveni Engineering are large Indian industrial companies in non-software classes. The crate is free but the npm name is taken. All bare domains have sites, so use a prefix.
- **Domain:** `gettriveni.app` (no site found)
- **CLI:** `triveni` (alias `tv`)
- **Tagline:** "Three rivers. One flood of speed."

### 4. Plyline
- **Meaning:** ply means strands twisted together into one stronger yarn or rope. That is bonding, literally.
- **Memorability:** short and techy. People may hear it as "polyline", which still suggests "many lines".
- **Conflict risk:** very low. npm, crates and GitHub are all free, and no HTTPS site answered on any of the four domains.
- **Domain:** `plyline.app`
- **CLI:** `ply` (short, nice to type) or `plyline`
- **Tagline:** "Many strands. One strong line."

### 5. Skein (most elegant, but more crowded)
- **Meaning:** a bundle of yarn, and also a flock of geese flying in formation (many moving as one, fast).
- **Memorability:** one syllable (SKAYN) and beautiful, though some people will need to hear it said once.
- **Conflict risk:** medium. npm and crates are taken, `getskein.app` is a live iOS/Android journaling app, there is the Skein hash function, and there are a couple of mid-sized GitHub repos. It is usable only with a modifier.
- **Domain:** `skein.so`, `useskein.com` or `skeinnet.app`
- **CLI:** `skein`
- **Tagline:** "All your connections, wound into one."

## Summary
- **Avoid:**
  - Twine (Twine fiction tool and Python's twine)
  - Splice (music platform)
  - Tandem (crowded)
  - BondWave (fintech trademark)
  - Duplo-net (LEGO)
  - Strand (wrong meaning, crowded)
  - Multilane (MultiLane Inc.)
- **Confluo:** usable but shadowed by the Berkeley research project and by Confluent.
- **Braid, Weft, Lanes, Plait:** taken on every registry checked, with existing projects, so they would need a modifier.
- **Pick:**
  - **Sangam** for brand story.
  - **Fuselane** for a zero-conflict, instantly understood name.
  - Sangam as the brand with `sangam` as the CLI, and Triveni held as a product or tier name (for example "Sangam Triveni" mode for three connections), is a coherent family.
- **Next steps:**
  - Check actual availability at a registrar (Porkbun or Namecheap) for getsangam.app, fuselane.app and plyline.app.
  - Search USPTO, EUIPO and IP India for word marks in classes 9 and 42.
  - Search the Chrome Web Store by hand for the finalist.
