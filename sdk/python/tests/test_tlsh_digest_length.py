"""``tlsh_distance`` must refuse digests that are not 72 bytes.

The C ABI ``sillstone_text_tlsh_distance`` takes no lengths and always reads 72 bytes from
each pointer, so a shorter Python ``bytes`` object used to be read past its end. The wrapper
now calls ``sillstone_text_tlsh_distance_checked``, which takes the lengths and refuses
anything but 72 bytes.
"""

import ctypes
import os
import sys
import unittest

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

import sillstone
from sillstone._generated import (
    sillstone_text_tlsh_distance_checked_raw,
    sillstone_text_tlsh_distance_raw,
)


def _sample(seed: int) -> bytes:
    return bytes((i * 31 + seed * (i // 7)) % 251 for i in range(4000))


def _abi_distance(a: bytes, b: bytes) -> int:
    out = ctypes.c_uint32()
    rc = sillstone_text_tlsh_distance_raw(a, b, ctypes.byref(out))
    assert rc == 0
    return out.value


def _checked_abi(a: bytes, b: bytes, len_a: int | None = None, len_b: int | None = None) -> tuple[int, int]:
    out = ctypes.c_uint32(7)
    rc = sillstone_text_tlsh_distance_checked_raw(
        a, len(a) if len_a is None else len_a, b, len(b) if len_b is None else len_b, ctypes.byref(out)
    )
    return rc, out.value


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

    def test_checked_c_abi_refuses_wrong_lengths_without_touching_out(self):
        a = self.a.encode("ascii")
        b = self.b.encode("ascii")
        rc, value = _checked_abi(a, b)
        self.assertEqual(rc, 0)
        self.assertEqual(value, _abi_distance(a, b))
        # The memory holds a valid digest twice, so only the length argument can make the
        # call fail: an implementation that ignored it would return 0.
        twice = a + a
        for n in [0, 1, 71, 73, 144]:
            with self.subTest(length=n):
                self.assertEqual(_checked_abi(twice, b, len_a=n), (-1, 7))
                self.assertEqual(_checked_abi(a, twice, len_b=n), (-1, 7))

    def test_checked_c_abi_rejects_a_72_byte_value_that_is_not_a_digest(self):
        with self.assertRaises(sillstone.FfiError):
            sillstone.tlsh_distance("0" * 72, self.b)
        self.assertEqual(_checked_abi(b"0" * 72, self.b.encode("ascii")), (-1, 7))


if __name__ == "__main__":
    unittest.main()
