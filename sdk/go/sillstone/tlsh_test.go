package sillstone

import (
	"strings"
	"testing"
)

// SillstoneTextTlshDistanceChecked passes both lengths to the C ABI, which refuses any length
// but 72. The deprecated SillstoneTextTlshDistance has no error result, so it refuses a wrong
// length by returning 0 before the C function (which always reads 72 bytes) is reached.
func tlshSample(seed int) []byte {
	out := make([]byte, 4000)
	for i := range out {
		out[i] = byte((i*31 + seed*(i/7)) % 251)
	}
	return out
}

func TestTlshDistanceCheckedAcceptsFullDigests(t *testing.T) {
	a := SillstoneTextTlsh(tlshSample(1))
	b := SillstoneTextTlsh(tlshSample(2))
	if len(a) != 72 || len(b) != 72 {
		t.Fatalf("digest lengths %d and %d, want 72", len(a), len(b))
	}
	self, err := SillstoneTextTlshDistanceChecked(a, a)
	if err != nil || self != 0 {
		t.Fatalf("self distance = %d, %v; want 0, nil", self, err)
	}
	ab, err := SillstoneTextTlshDistanceChecked(a, b)
	if err != nil || ab == 0 {
		t.Fatalf("distance = %d, %v; want a non-zero value", ab, err)
	}
	if old := SillstoneTextTlshDistance(a, b); old != ab {
		t.Fatalf("deprecated function = %d, checked = %d; want equal", old, ab)
	}
}

func TestTlshDistanceCheckedRefusesOtherLengths(t *testing.T) {
	a := SillstoneTextTlsh(tlshSample(1))
	for _, bad := range [][]byte{a[:71], append(append([]byte{}, a...), '0'), {}, nil} {
		if _, err := SillstoneTextTlshDistanceChecked(bad, a); err == nil || !strings.Contains(err.Error(), "72 bytes") {
			t.Fatalf("first argument of %d bytes: err = %v", len(bad), err)
		}
		if _, err := SillstoneTextTlshDistanceChecked(a, bad); err == nil || !strings.Contains(err.Error(), "72 bytes") {
			t.Fatalf("second argument of %d bytes: err = %v", len(bad), err)
		}
	}
}

func TestTlshDistanceCheckedRejectsA72ByteValueThatIsNotADigest(t *testing.T) {
	a := SillstoneTextTlsh(tlshSample(1))
	if _, err := SillstoneTextTlshDistanceChecked([]byte(strings.Repeat("0", 72)), a); err == nil {
		t.Fatal("want an error for a 72-byte value that is not a TLSH digest")
	}
}

func TestTlshDistanceDeprecatedReturnsZeroOnOtherLengths(t *testing.T) {
	a := SillstoneTextTlsh(tlshSample(1))
	b := SillstoneTextTlsh(tlshSample(2))
	if SillstoneTextTlshDistance(a, b) == 0 {
		t.Fatal("a valid pair must give a non-zero distance")
	}
	for _, bad := range [][]byte{a[:71], append(append([]byte{}, a...), '0'), {}, nil} {
		if got := SillstoneTextTlshDistance(bad, b); got != 0 {
			t.Fatalf("first argument of %d bytes: got %d, want 0", len(bad), got)
		}
		if got := SillstoneTextTlshDistance(b, bad); got != 0 {
			t.Fatalf("second argument of %d bytes: got %d, want 0", len(bad), got)
		}
	}
}
