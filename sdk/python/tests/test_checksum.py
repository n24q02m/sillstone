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


if __name__ == "__main__":
    unittest.main()
