package sillstone

import (
	"bytes"
	"encoding/hex"
	"math"
	"testing"
)

// Each refusal path of the image operations: Go-side validation first, then
// the status codes the native core returns for arguments it does not accept.

func TestImageOpsValidateTheImageBeforeCallingTheCore(t *testing.T) {
	cmyk := &Image{Width: 2, Height: 2, Mode: "CMYK", Data: make([]byte, 16)}
	empty := &Image{Width: 0, Height: 3, Mode: "RGB"}
	for name, img := range map[string]*Image{"unsupported mode": cmyk, "zero width": empty} {
		t.Run(name, func(t *testing.T) {
			fragment := `unsupported image mode "CMYK"`
			if img == empty {
				fragment = "image dimensions must be positive"
			}
			if _, err := img.Invert(); err == nil {
				t.Fatal("Invert accepted an invalid image")
			} else {
				wantErrContaining(t, err, fragment)
			}
			if _, err := img.Convert("L"); err == nil {
				t.Fatal("Convert accepted an invalid image")
			} else {
				wantErrContaining(t, err, fragment)
			}
			if _, err := img.QoiEncode(); err == nil {
				t.Fatal("QoiEncode accepted an invalid image")
			} else {
				wantErrContaining(t, err, fragment)
			}
		})
	}
}

func TestImageOpsReportNativeRefusals(t *testing.T) {
	rgb := NewImage("RGB", 4, 4)
	gray := NewImage("L", 4, 4)
	cases := []struct {
		name     string
		run      func() error
		fragment string
	}{
		// sillstone_image_filter knows filter ids 0 to 6 only.
		{"unknown filter", func() error { _, err := rgb.Filter(99); return err }, "sillstone: filter failed (status -1)"},
		// sillstone_image_rotate_size refuses an angle that is not finite.
		{"NaN angle", func() error { _, err := rgb.Rotate(math.NaN(), ResampleNearest, false); return err }, "sillstone: rotate failed (status -1)"},
		{"infinite angle", func() error { _, err := rgb.Rotate(math.Inf(1), ResampleNearest, true); return err }, "sillstone: rotate failed (status -1)"},
		// QOI and the BMP encoder have no one-channel form.
		{"qoi of L", func() error { _, err := gray.QoiEncode(); return err }, "sillstone: qoi encode failed (status -1)"},
		{"bmp of L", func() error { _, err := gray.BmpEncode(); return err }, "sillstone: bmp encode failed (status -1)"},
		{"empty qoi stream", func() error { _, err := QoiDecode(nil); return err }, "sillstone: qoi decode failed"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			wantErrContaining(t, tc.run(), tc.fragment)
		})
	}
}

func TestConvertToTheSameModeReturnsAnIndependentCopy(t *testing.T) {
	src := &Image{Width: 2, Height: 1, Mode: "RGB", Data: []byte{1, 2, 3, 4, 5, 6}}
	out, err := src.Convert("RGB")
	if err != nil {
		t.Fatal(err)
	}
	if out.Mode != "RGB" || out.Width != 2 || out.Height != 1 || !bytes.Equal(out.Data, src.Data) {
		t.Fatalf("Convert to the same mode = %s %v", out, out.Data)
	}
	src.Data[0] = 99
	if out.Data[0] != 1 {
		t.Fatal("the converted image shares its pixel buffer with the source")
	}
}

func TestQoiDecodeOfTheRGBAGoldenStream(t *testing.T) {
	g := loadGolden(t)
	stream, err := hex.DecodeString(g.QoiRGBA)
	if err != nil {
		t.Fatal(err)
	}
	img, err := QoiDecode(stream)
	if err != nil {
		t.Fatal(err)
	}
	sameImage(t, "qoi_rgba decode", img, g.ImageRGBA)
}
