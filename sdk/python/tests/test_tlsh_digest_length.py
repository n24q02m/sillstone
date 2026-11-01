"""``tlsh_distance`` must refuse digests that are not 72 bytes.

The C ABI ``sillstone_text_tlsh_distance`` takes no lengths and always reads 72 bytes from
each pointer, so a shorter Python ``bytes`` object used to be read past its end.
"""

import ctypes
import os
import sys
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

import sillstone
from sillstone._generated import sillstone_text_tlsh_distance_raw


def _sample(seed: int) -> bytes:
    return bytes((i * 31 + seed * (i // 7)) % 251 for i in range(4000))


def _abi_distance(a: bytes, b: bytes) -> int:
    out = ctypes.c_uint32()
    rc = sillstone_text_tlsh_distance_raw(a, b, ctypes.byref(out))
    assert rc == 0
    return out.value


class TlshDigestLengthTests(unittest.TestCase):
    def setUp(self):
        self.a = sillstone.tlsh(_sample(1))
        self.b = sillstone.tlsh(_sample(2))

    def test_valid_digests_match_the_c_abi(self):
        self.assertEqual(len(self.a), 72)
        self.assertEqual(sillstone.tlsh_distance(self.a, self.a), 0)
        want = _abi_distance(self.a.encode("ascii"), self.b.encode("ascii"))
        self.assertGreater(want, 0)
        self.assertEqual(sillstone.tlsh_distance(self.a, self.b), want)
        self.assertEqual(sillstone.tlsh_distance(self.a.encode("ascii"), self.b.encode("ascii")), want)

    def test_wrong_length_digests_are_refused(self):
        for bad in [self.a[:71], self.a + "0", "", b"", self.a.encode("ascii")[:71]]:
            with self.subTest(length=len(bad)):
                with self.assertRaisesRegex(ValueError, "72 bytes"):
                    sillstone.tlsh_distance(bad, self.b)
                with self.assertRaisesRegex(ValueError, "72 bytes"):
                    sillstone.tlsh_distance(self.a, bad)


if __name__ == "__main__":
    unittest.main()
