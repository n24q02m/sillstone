package sillstone

import (
	"strings"
	"testing"
)

// The C ABI reads exactly 72 bytes from each digest pointer, so the wrapper must refuse any other
// length instead of reading past the slice.
func tlshSample(seed int) []byte {
	out := make([]byte, 4000)
	for i := range out {
		out[i] = byte((i*31 + seed*(i/7)) % 251)
	}
	return out
}

func TestTlshDistanceAcceptsFullDigests(t *testing.T) {
	a := SillstoneTextTlsh(tlshSample(1))
	b := SillstoneTextTlsh(tlshSample(2))
	if len(a) != 72 || len(b) != 72 {
		t.Fatalf("digest lengths %d and %d, want 72", len(a), len(b))
	}
	self, err := SillstoneTextTlshDistance(a, a)
	if err != nil || self != 0 {
		t.Fatalf("self distance = %d, %v; want 0, nil", self, err)
	}
	ab, err := SillstoneTextTlshDistance(a, b)
	if err != nil || ab == 0 {
		t.Fatalf("distance = %d, %v; want a non-zero value", ab, err)
	}
}

func TestTlshDistanceRefusesOtherLengths(t *testing.T) {
	a := SillstoneTextTlsh(tlshSample(1))
	for _, bad := range [][]byte{a[:71], append(append([]byte{}, a...), '0'), {}, nil} {
		if _, err := SillstoneTextTlshDistance(bad, a); err == nil || !strings.Contains(err.Error(), "72 bytes") {
			t.Fatalf("first argument of %d bytes: err = %v", len(bad), err)
		}
		if _, err := SillstoneTextTlshDistance(a, bad); err == nil || !strings.Contains(err.Error(), "72 bytes") {
			t.Fatalf("second argument of %d bytes: err = %v", len(bad), err)
		}
	}
}
