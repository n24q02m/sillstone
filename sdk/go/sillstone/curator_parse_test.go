package sillstone

import (
	"encoding/binary"
	"strings"
	"testing"
)

// The signature and describe streams are a public wire format (see
// crates/curator/src/reference.rs). ParseSignature and ParseDescribe accept
// bytes from any source, so a malformed stream must come back as an error,
// never as a panic or a half-filled value. These streams are built by hand;
// the well-formed ones produced by the native core are replayed in
// reference_test.go.

func be64Bytes(v uint64) []byte {
	var b [8]byte
	binary.BigEndian.PutUint64(b[:], v)
	return b[:]
}

func be32Bytes(v uint32) []byte {
	var b [4]byte
	binary.BigEndian.PutUint32(b[:], v)
	return b[:]
}

func cat(parts ...[]byte) []byte {
	var out []byte
	for _, p := range parts {
		out = append(out, p...)
	}
	return out
}

func wantErrContaining(t *testing.T, err error, fragment string) {
	t.Helper()
	if err == nil {
		t.Fatalf("want an error containing %q, got nil", fragment)
	}
	if !strings.Contains(err.Error(), fragment) {
		t.Fatalf("error %q does not contain %q", err.Error(), fragment)
	}
}

func TestParseSignatureRefusesMalformedStreams(t *testing.T) {
	cases := []struct {
		name     string
		raw      []byte
		fragment string
	}{
		{"empty", nil, "signature stream is empty"},
		{"unknown tag", []byte{5}, "unknown signature tag 5"},
		{"image too short", []byte{0, 1, 2, 3}, "image stream is not tag + u64"},
		{"image too long", cat([]byte{0}, be64Bytes(1), []byte{9}), "image stream is not tag + u64"},
		{"audio header cut", cat([]byte{1}, be64Bytes(0), []byte{0, 0, 0}), "audio stream is shorter than its 13-byte header"},
		{"audio count above payload", cat([]byte{1}, be64Bytes(6), be32Bytes(1)), "audio peak count disagrees with the payload"},
		{"audio partial peak", cat([]byte{1}, be64Bytes(7), be32Bytes(1), make([]byte, 7)), "audio peak count disagrees with the payload"},
		{"text header cut", []byte{2, 0, 0, 0}, "text stream is shorter than its 9-byte header"},
		{"text count above payload", cat([]byte{2}, be64Bytes(8)), "text word count disagrees with the payload"},
		{"text partial word", cat([]byte{2}, be64Bytes(5), make([]byte, 5)), "text word count disagrees with the payload"},
		{"binary header cut", []byte{3}, "binary stream is shorter than its 9-byte header"},
		{"binary count above payload", cat([]byte{3}, be64Bytes(32)), "binary chunk count disagrees with the payload"},
		{"binary partial digest", cat([]byte{3}, be64Bytes(33), make([]byte, 33)), "binary chunk count disagrees with the payload"},
		{"video header cut", cat([]byte{4}, make([]byte, 19)), "video stream is shorter than its 21-byte header"},
		{"video frames above payload", cat([]byte{4}, be32Bytes(16), make([]byte, 16), make([]byte, 8)), "video frame count disagrees with the payload"},
		{"video partial frame hash", cat([]byte{4}, be32Bytes(4), make([]byte, 16), make([]byte, 8)), "video frame count disagrees with the payload"},
		{"video partial minhash word", cat([]byte{4}, be32Bytes(8), make([]byte, 16), make([]byte, 11)), "video minhash payload is not whole words"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			sig, err := ParseSignature(tc.raw)
			if sig != nil {
				t.Fatalf("got a signature %+v for a malformed stream", sig)
			}
			wantErrContaining(t, err, tc.fragment)
		})
	}
}

func TestParseSignatureDecodesHandBuiltStreams(t *testing.T) {
	t.Run("audio", func(t *testing.T) {
		// One peak: t = 0x01020304, f = 0x0506, both little-endian.
		payload := []byte{0x04, 0x03, 0x02, 0x01, 0x06, 0x05}
		sig, err := ParseSignature(cat([]byte{1}, be64Bytes(6), be32Bytes(77), payload))
		if err != nil {
			t.Fatal(err)
		}
		if sig.Modality != "audio" || sig.PeakCount != 6 || sig.Frames != 77 {
			t.Fatalf("header decoded as %+v", sig)
		}
		if len(sig.Peaks) != 1 || sig.Peaks[0] != (Peak{T: 0x01020304, F: 0x0506}) {
			t.Fatalf("peaks %+v", sig.Peaks)
		}
	})
	t.Run("video", func(t *testing.T) {
		frame := []byte{1, 0, 0, 0, 0, 0, 0, 0}
		word := []byte{2, 0, 0, 0, 0, 0, 0, 0}
		raw := cat([]byte{4}, be32Bytes(8), be32Bytes(64), be32Bytes(48), be64Bytes(0x4000000000000000), frame, word)
		sig, err := ParseSignature(raw)
		if err != nil {
			t.Fatal(err)
		}
		if sig.Width != 64 || sig.Height != 48 || sig.DurationBits != 0x4000000000000000 {
			t.Fatalf("header decoded as %+v", sig)
		}
		if len(sig.FrameHashes) != 1 || sig.FrameHashes[0] != 1 || len(sig.Minhash) != 1 || sig.Minhash[0] != 2 {
			t.Fatalf("payload decoded as frames %v minhash %v", sig.FrameHashes, sig.Minhash)
		}
	})
}

// describeHeader is the 34-byte head of a describe stream: format code,
// modality code and a 32-byte tier-1 digest.
func describeHeader(format, modality byte) []byte {
	return cat([]byte{format, modality}, make([]byte, 32))
}

func TestParseDescribeRefusesMalformedStreams(t *testing.T) {
	imageSig := cat([]byte{0}, be64Bytes(0xfeed))
	cases := []struct {
		name     string
		raw      []byte
		fragment string
	}{
		{"header cut", make([]byte, 34), "describe stream is shorter than its 35-byte header"},
		{"unknown format code", cat(describeHeader(11, 0), imageSig, make([]byte, 16)), "unknown describe codes 11/0"},
		{"unknown modality code", cat(describeHeader(0, 5), imageSig, make([]byte, 16)), "unknown describe codes 0/5"},
		{"bad embedded signature", cat(describeHeader(0, 0), []byte{0, 1, 2}, make([]byte, 16)), "image stream is not tag + u64"},
		{"no room for the signature", cat(describeHeader(0, 0), make([]byte, 16)), "signature stream is empty"},
		// Streams shorter than header + facts block: the facts block would
		// start inside the 34-byte header.
		{"image facts cut", cat(describeHeader(0, 0), make([]byte, 15)), "too short for the 16-byte image facts block"},
		{"audio facts cut", cat(describeHeader(6, 1), make([]byte, 17)), "too short for the 18-byte audio facts block"},
		{"text facts cut", cat(describeHeader(10, 2), make([]byte, 1)), "too short for the 16-byte text facts block"},
		{"binary facts cut", cat(describeHeader(3, 3), make([]byte, 15)), "too short for the 16-byte binary facts block"},
		{"video facts cut", cat(describeHeader(4, 4), make([]byte, 1)), "too short for the 24-byte video facts block"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			d, err := ParseDescribe(tc.raw)
			if d != nil {
				t.Fatalf("got a description %+v for a malformed stream", d)
			}
			wantErrContaining(t, err, tc.fragment)
		})
	}
}

func TestFfiErrorNamesTheOperationAndTheStatus(t *testing.T) {
	c := buildCorpus(t)
	// A cross-modality pair is the pinned SILLSTONE_E_REJECTED refusal.
	_, err := MatchSignatures(c.png48, c.proseA)
	if err == nil || err.Error() != "sillstone_curator_match failed: input rejected (status -2)" {
		t.Fatalf("cross-modality match error = %v", err)
	}
	cases := map[int32]string{
		StatusInvalid: "op failed: invalid argument (status -1)",
		-7:            "op failed: unknown failure (status -7)",
	}
	for status, want := range cases {
		if got := (&FfiError{Op: "op", Status: status}).Error(); got != want {
			t.Fatalf("status %d: %q, want %q", status, got, want)
		}
	}
}
