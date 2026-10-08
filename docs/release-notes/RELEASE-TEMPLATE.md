Fuselane downloads one file over every network you have at once (Wi-Fi, Ethernet, a USB-tethered phone) and fuses the parts into one file.

This is a **beta**. Please report problems at https://github.com/ArshPunisher/fuselane/issues.

## Install

| System | File |
|---|---|
| macOS 13.3+ (Apple silicon and Intel) | `Fuselane_<version>_macos-universal.dmg`, or run the install script below |
| Windows 10/11 (x64) | `Fuselane_<version>_windows-x64-setup.exe` |
| Linux x64 | `.AppImage`, `.deb` or `.rpm` ending in `linux-x64` |
| Linux arm64 | `.deb` or `.rpm` ending in `linux-arm64` |
| Command line | `fuselane-cli_<version>_<system>` |

**macOS:** the app is open source and ad-hoc signed, not notarized (no paid Apple account, by policy). The install script does the one-time setup for you:

```sh
curl -fsSL https://raw.githubusercontent.com/ArshPunisher/fuselane/main/packaging/macos/install.sh | sh
```

**Windows:** until code signing is approved, SmartScreen may say "Windows protected your PC". Choose **More info → Run anyway**.

## Verify

Every file's SHA-256 is in `SHA256SUMS`. For example: `shasum -a 256 -c SHA256SUMS --ignore-missing`.
