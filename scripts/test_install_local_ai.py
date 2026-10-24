"""No-network checks for the optional artifact downloader's redirect boundary."""
from pathlib import Path
import runpy
import unittest
from urllib.error import URLError
from urllib.request import Request

HELPER = runpy.run_path(str(Path(__file__).with_name("install-local-ai.py")))


class ArtifactRedirectTests(unittest.TestCase):
    def test_https_downgrade_is_rejected_before_request_creation(self):
        handler = HELPER["HTTPSOnlyRedirect"]()
        source = Request("https://example.invalid/artifact")
        for destination in ("http://cdn.example.invalid/artifact", "ftp://cdn.example.invalid/artifact"):
            with self.subTest(destination=destination), self.assertRaises(URLError):
                handler.redirect_request(source, None, 302, "Found", {}, destination)

    def test_https_redirect_preserves_normal_request_handling(self):
        handler = HELPER["HTTPSOnlyRedirect"]()
        destination = "https://cdn.example.invalid/artifact"
        redirected = handler.redirect_request(Request("https://example.invalid/artifact"), None, 302, "Found", {}, destination)
        self.assertIsInstance(redirected, Request)
        self.assertEqual(redirected.full_url, destination)


if __name__ == "__main__":
    unittest.main()
