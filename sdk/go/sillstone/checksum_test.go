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
