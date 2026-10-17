package sillstone

import (
	"math"
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

func TestDst(t *testing.T) {
	x := []float64{1.0, 2.0, 3.0, 4.0}
	d1 := SillstoneMathDst1(x)
	if len(d1) != 4 {
		t.Fatalf("dst1 len %d != 4", len(d1))
	}
	rec1 := SillstoneMathDst1(d1)
	for i := range x {
		if math.Abs(x[i]-rec1[i]) > 1e-10 {
			t.Fatalf("dst1 self-inversion mismatch at %d: %f != %f", i, x[i], rec1[i])
		}
	}

	d2 := SillstoneMathDst2(x)
	rec2 := SillstoneMathDst3(d2)
	for i := range x {
		if math.Abs(x[i]-rec2[i]) > 1e-10 {
			t.Fatalf("dst2/dst3 inversion mismatch at %d: %f != %f", i, x[i], rec2[i])
		}
	}
}

func TestUnicode(t *testing.T) {
	fam := "👨‍👩‍👧‍👦"
	cnt := SillstoneUnicodeGraphemeCount([]byte(fam))
	if cnt != 1 {
		t.Fatalf("family emoji grapheme count = %d; want 1", cnt)
	}
	hello := SillstoneUnicodeGraphemeCount([]byte("Hello"))
	if hello != 5 {
		t.Fatalf("hello grapheme count = %d; want 5", hello)
	}
	cat := SillstoneUnicodeCharCategory('A')
	if cat == 255 {
		t.Fatalf("category of 'A' unexpected")
	}
}
