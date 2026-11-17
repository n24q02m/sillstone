package sillstone

import (
	"bytes"
	"crypto/sha256"
	"crypto/sha3"
	"encoding/hex"
	"hash/fnv"
	"testing"
)

// The digest wrappers checked against independent implementations: the Go
// standard library for SHA-2, SHA-3 and FNV-1a, and published vectors where
// the standard library has no implementation.

func digestInputs() map[string][]byte {
	long := make([]byte, 1000)
	for i := range long {
		long[i] = byte(i * 7)
	}
	return map[string][]byte{"empty": {}, "nil": nil, "abc": []byte("abc"), "1000 bytes": long}
}

func TestShaDigestsMatchTheStandardLibrary(t *testing.T) {
	for name, in := range digestInputs() {
		t.Run(name, func(t *testing.T) {
			s2 := sha256.Sum256(in)
			if got := SillstoneDigestSha256(in); !bytes.Equal(got, s2[:]) {
				t.Fatalf("sha256 %x, want %x", got, s2)
			}
			s3 := sha3.Sum256(in)
			if got := SillstoneDigestSha3256(in); !bytes.Equal(got, s3[:]) {
				t.Fatalf("sha3-256 %x, want %x", got, s3)
			}
			s5 := sha3.Sum512(in)
			if got := SillstoneDigestSha3512(in); !bytes.Equal(got, s5[:]) {
				t.Fatalf("sha3-512 %x, want %x", got, s5)
			}
		})
	}
}

func TestBlake2b512MatchesPublishedVectors(t *testing.T) {
	cases := map[string]string{
		// RFC 7693, Appendix A.
		"abc": "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1" +
			"7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923",
		// The unkeyed BLAKE2b-512 of the empty string.
		"": "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419" +
			"d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce",
	}
	for in, want := range cases {
		if got := hex.EncodeToString(SillstoneDigestBlake2b512([]byte(in))); got != want {
			t.Fatalf("blake2b-512(%q) = %s, want %s", in, got, want)
		}
	}
}

func TestBlake3MatchesTheSpecificationVector(t *testing.T) {
	// The BLAKE3 test vector for a one-byte input (0x00), the same one
	// crates/digest/tests/wave2_vectors.rs checks.
	const want = "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"
	if got := hex.EncodeToString(SillstoneDigestBlake3([]byte{0})); got != want {
		t.Fatalf("blake3([0]) = %s, want %s", got, want)
	}
}

func TestChecksumOfEmptyInputIsTheFnvOffsetBasis(t *testing.T) {
	want := fnv.New64a().Sum64()
	for _, in := range [][]byte{nil, {}} {
		if got := SillstoneBufferChecksum(in); got != want {
			t.Fatalf("checksum of %d bytes = %#x, want %#x", len(in), got, want)
		}
	}
}

// The generated curator wrappers drop the status code, so they must agree
// with the checked functions in curator.go on every accepted input and
// return nil where the checked functions report a refusal.
func TestGeneratedCuratorWrappersAgreeWithTheCheckedAPI(t *testing.T) {
	c := buildCorpus(t)
	inputs := map[string][]byte{
		"png": c.png48, "wav": c.wav, "text": c.proseA, "binary": c.blob, "mp4": c.mp4, "pdf": c.pdf,
	}
	for name, data := range inputs {
		t.Run(name, func(t *testing.T) {
			det, err := Detect(data)
			if err != nil {
				t.Fatal(err)
			}
			format, modality, pending := SillstoneCuratorDetect(data)
			if Formats[format] != det.Format || Modalities[modality] != det.Modality || (pending == 1) != det.Pending {
				t.Fatalf("raw detect %d/%d/%d, checked %+v", format, modality, pending, det)
			}

			hash, err := ContentHash(data)
			if err != nil {
				t.Fatal(err)
			}
			if raw := SillstoneCuratorContentHash(data); !bytes.Equal(raw, hash) {
				t.Fatalf("raw content hash %x, checked %x", raw, hash)
			}

			sig, err := SignatureStream(data)
			if err != nil {
				t.Fatal(err)
			}
			if raw := SillstoneCuratorSignature(data); !bytes.Equal(raw, sig) {
				t.Fatal("raw signature stream differs from SignatureStream")
			}

			desc, err := DescribeStream(data)
			if err != nil {
				t.Fatal(err)
			}
			if raw := SillstoneCuratorDescribe(data); !bytes.Equal(raw, desc) {
				t.Fatal("raw describe stream differs from DescribeStream")
			}
		})
	}

	match, err := MatchSignatures(c.blob, c.blobShifted)
	if err != nil {
		t.Fatal(err)
	}
	tag, slots, matched := SillstoneCuratorMatch(c.blob, c.blobShifted)
	if MatchTags[tag] != match.Tag || (matched == 1) != match.Matched {
		t.Fatalf("raw match %d/%d, checked %+v", tag, matched, match)
	}
	for i := range slots {
		if slots[i] != match.Slots[i] {
			t.Fatalf("raw slot %d = %d, checked %d", i, slots[i], match.Slots[i])
		}
	}

	corrupt := append([]byte{0x89, 'P', 'N', 'G', 0x0d, 0x0a, 0x1a, 0x0a}, []byte("garbage")...)
	if raw := SillstoneCuratorSignature(corrupt); raw != nil {
		t.Fatalf("raw signature of a corrupt png = %x, want nil", raw)
	}
	if raw := SillstoneCuratorDescribe(corrupt); raw != nil {
		t.Fatalf("raw describe of a corrupt png = %x, want nil", raw)
	}

	// The C ABI accepts and ignores a null pointer, so callers can free
	// unconditionally on an error path.
	SillstoneCuratorFree(nil, 0)
}
