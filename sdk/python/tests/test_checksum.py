import unittest
import sys
import os

# Add sdk/python to sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

import sillstone


class TestSillstonePython(unittest.TestCase):
    def test_version(self):
        v = sillstone.version()
        self.assertEqual(v, "0.2.0")

    def test_checksum_sample(self):
        data = b"sillstone pure rust zero dep"
        res = sillstone.buffer_checksum(data)
        # Expected FNV-1a 64-bit checksum
        # Compute reference in pure Python
        h = 0xcbf29ce484222325
        for b in data:
            h ^= b
            h = (h * 0x0100000001b3) & 0xffffffffffffffff
        self.assertEqual(res, h)

    def test_digests(self):
        import hashlib
        data = b"abc"
        self.assertEqual(sillstone.sha256(data).hex(), hashlib.sha256(data).hexdigest())
        self.assertEqual(sillstone.sha3_256(data).hex(), hashlib.sha3_256(data).hexdigest())
        self.assertEqual(sillstone.sha3_512(data).hex(), hashlib.sha3_512(data).hexdigest())
        self.assertEqual(sillstone.blake2b_512(data).hex(), hashlib.blake2b(data).hexdigest())
        # BLAKE3 "abc" reference
        self.assertEqual(sillstone.blake3(b"").hex(), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262")


if __name__ == "__main__":
    unittest.main()
