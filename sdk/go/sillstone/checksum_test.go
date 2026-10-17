package sillstone

import (
	"testing"
)

func TestChecksum(t *testing.T) {
	data := []byte("sillstone pure rust zero dep")
	got := SillstoneBufferChecksum(data)

	// Reference FNV-1a 64-bit
	var want uint64 = 0xcbf29ce484222325
	for _, b := range data {
		want ^= uint64(b)
		want *= 0x0100000001b3
	}

	if got != want {
		t.Fatalf("checksum mismatch: got %x, want %x", got, want)
	}
}

func TestVersion(t *testing.T) {
	if SillstoneVersionMajor() != 0 || SillstoneVersionMinor() != 2 || SillstoneVersionPatch() != 0 {
		t.Fatalf("unexpected version %d.%d.%d", SillstoneVersionMajor(), SillstoneVersionMinor(), SillstoneVersionPatch())
	}
}

func TestDigests(t *testing.T) {
	b3 := SillstoneDigestBlake3([]byte{})
	if len(b3) != 32 {
		t.Fatalf("blake3 length %d != 32", len(b3))
	}
	if b3[0] != 0xaf || b3[1] != 0x13 || b3[2] != 0x49 || b3[3] != 0xb9 {
		t.Fatalf("blake3 unexpected prefix: %x", b3[:4])
	}

	s256 := SillstoneDigestSha256([]byte("abc"))
	if len(s256) != 32 {
		t.Fatalf("sha256 length %d != 32", len(s256))
	}
	if s256[0] != 0xba || s256[1] != 0x78 {
		t.Fatalf("sha256 mismatch prefix: %x", s256[:4])
	}
}
