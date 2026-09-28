import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch
import urllib.error

spec = importlib.util.spec_from_file_location("cache_config", Path(__file__).with_name("warbler_cache.py"))
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)


class CacheRetryTest(unittest.TestCase):
    def test_outage_is_bounded(self):
        with patch.object(cache.urllib.request, "urlopen", side_effect=urllib.error.URLError("offline")) as request, patch.object(cache.time, "sleep") as sleep:
            self.assertIsNone(cache.open_with_retries(None, None))
            self.assertEqual(request.call_count, 3)
            self.assertEqual(sleep.call_count, 2)

    def test_transient_failure_recovers(self):
        response = object()
        with patch.object(cache.urllib.request, "urlopen", side_effect=[TimeoutError(), response]) as request, patch.object(cache.time, "sleep"):
            self.assertIs(cache.open_with_retries(None, None), response)
            self.assertEqual(request.call_count, 2)

    def test_authorization_denial_is_preserved(self):
        error = urllib.error.HTTPError("https://cache", 401, "Unauthorized", {}, None)
        with patch.object(cache.urllib.request, "urlopen", side_effect=error) as request:
            with self.assertRaises(urllib.error.HTTPError):
                cache.open_with_retries(None, None)
            self.assertEqual(request.call_count, 1)


if __name__ == "__main__":
    unittest.main()
