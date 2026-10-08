#!/usr/bin/env python3
"""Writes the updater feed (latest.json) for one release.

Usage: update_feed.py VERSION BASE_URL SIG_DIR [NOTES_FILE] > latest.json

SIG_DIR holds the release's .sig files. Artifacts are matched by their exact
names (L-78: rc.1 must never pick up rc.10's files), and a missing signature
for a supported platform is an error, never a silently thinner feed.
"""
import json
import pathlib
import sys
from datetime import datetime, timezone

# Updater platform -> artifact name pattern (per release.yml's naming, L-74).
PLATFORMS = {
    "darwin-aarch64": "Fuselane_{v}_macos-universal.app.tar.gz",
    "darwin-x86_64": "Fuselane_{v}_macos-universal.app.tar.gz",
    "windows-x86_64": "Fuselane_{v}_windows-x64-setup.exe",
    "linux-x86_64": "Fuselane_{v}_linux-x64.AppImage",
}


def feed(version: str, base_url: str, sig_dir: pathlib.Path, notes: str = "") -> dict:
    if not version or "/" in version or version.startswith("v"):
        raise ValueError(f"bad version {version!r}: pass it without the leading v")
    platforms = {}
    for platform, pattern in PLATFORMS.items():
        name = pattern.format(v=version)
        sig = sig_dir / f"{name}.sig"
        if not sig.is_file():
            raise FileNotFoundError(f"missing signature {sig.name} for {platform}")
        signature = sig.read_text().strip()
        if not signature:
            raise ValueError(f"empty signature {sig.name}")
        platforms[platform] = {
            "signature": signature,
            "url": f"{base_url.rstrip('/')}/{name}",
        }
    return {
        "version": version,
        "notes": notes,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }


def main(argv: list[str]) -> int:
    if len(argv) not in (4, 5):
        print(__doc__, file=sys.stderr)
        return 2
    notes = pathlib.Path(argv[4]).read_text() if len(argv) == 5 else ""
    try:
        out = feed(argv[1], argv[2], pathlib.Path(argv[3]), notes)
    except (ValueError, FileNotFoundError) as e:
        print(f"update_feed: {e}", file=sys.stderr)
        return 1
    json.dump(out, sys.stdout, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
