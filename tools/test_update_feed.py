"""Tests for update_feed.py (run: python3 -m unittest tools/test_update_feed.py)."""
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import update_feed  # noqa: E402


def sigs(d: pathlib.Path, version: str, skip: str = "") -> None:
    for pattern in set(update_feed.PLATFORMS.values()):
        name = pattern.format(v=version)
        if name != skip:
            (d / f"{name}.sig").write_text(f"sig-of-{name}\n")


class FeedTests(unittest.TestCase):
    def test_every_platform_gets_its_exact_file_and_signature(self):
        with tempfile.TemporaryDirectory() as t:
            d = pathlib.Path(t)
            sigs(d, "0.1.0-beta.1")
            f = update_feed.feed("0.1.0-beta.1", "https://x/rel/", d, "notes")
            self.assertEqual(f["version"], "0.1.0-beta.1")
            self.assertEqual(set(f["platforms"]), set(update_feed.PLATFORMS))
            win = f["platforms"]["windows-x86_64"]
            self.assertEqual(win["url"], "https://x/rel/Fuselane_0.1.0-beta.1_windows-x64-setup.exe")
            self.assertEqual(win["signature"], "sig-of-Fuselane_0.1.0-beta.1_windows-x64-setup.exe")

    def test_rc1_never_matches_rc10_files(self):
        with tempfile.TemporaryDirectory() as t:
            d = pathlib.Path(t)
            sigs(d, "1.0.0-rc.10")
            with self.assertRaises(FileNotFoundError):
                update_feed.feed("1.0.0-rc.1", "https://x", d)

    def test_a_missing_platform_is_an_error_not_a_thinner_feed(self):
        with tempfile.TemporaryDirectory() as t:
            d = pathlib.Path(t)
            sigs(d, "0.2.0", skip="Fuselane_0.2.0_linux-x64.AppImage")
            with self.assertRaises(FileNotFoundError):
                update_feed.feed("0.2.0", "https://x", d)

    def test_bad_versions_and_empty_signatures_are_refused(self):
        with tempfile.TemporaryDirectory() as t:
            d = pathlib.Path(t)
            sigs(d, "0.3.0")
            for bad in ["", "v0.3.0", "../0.3.0"]:
                with self.assertRaises(ValueError):
                    update_feed.feed(bad, "https://x", d)
            (d / "Fuselane_0.3.0_linux-x64.AppImage.sig").write_text("  \n")
            with self.assertRaises(ValueError):
                update_feed.feed("0.3.0", "https://x", d)


if __name__ == "__main__":
    unittest.main()
