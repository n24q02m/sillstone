package sillstone

import (
	"bytes"
	"image"
	"image/color"
	"testing"
)

// The conversions between Image and the standard library image types,
// checked against the standard library's own colour models. Every pixel is
// opaque: image.RGBA stores alpha-premultiplied samples, and for opaque
// pixels those equal the straight samples Image holds.

func TestNewImageSizesTheBufferForTheMode(t *testing.T) {
	for mode, ch := range map[string]int{"L": 1, "RGB": 3, "RGBA": 4} {
		img := NewImage(mode, 5, 3)
		if img.Mode != mode || img.Width != 5 || img.Height != 3 || len(img.Data) != 5*3*ch {
			t.Fatalf("NewImage(%q, 5, 3) = %s with %d bytes, want %d", mode, img, len(img.Data), 5*3*ch)
		}
	}
}

func opaquePattern(r image.Rectangle) *image.RGBA {
	src := image.NewRGBA(r)
	for y := r.Min.Y; y < r.Max.Y; y++ {
		for x := r.Min.X; x < r.Max.X; x++ {
			src.SetRGBA(x, y, color.RGBA{R: uint8(x * 40), G: uint8(y * 30), B: uint8(x*y + 5), A: 255})
		}
	}
	return src
}

func TestFromImageReadsTheSourceBoundsRowByRow(t *testing.T) {
	// A source whose bounds do not start at the origin.
	src := opaquePattern(image.Rect(2, 3, 5, 7))
	img := FromImage(src)
	if img.Mode != "RGBA" || img.Width != 3 || img.Height != 4 {
		t.Fatalf("FromImage = %s, want a 3x4 RGBA image", img)
	}
	if !bytes.Equal(img.Data, src.Pix) {
		t.Fatalf("pixels %x, want %x", img.Data, src.Pix)
	}

	gray := image.NewGray(image.Rect(0, 0, 2, 1))
	gray.Pix = []byte{17, 200}
	if got := FromImage(gray).Data; !bytes.Equal(got, []byte{17, 17, 17, 255, 200, 200, 200, 255}) {
		t.Fatalf("gray source gives %v", got)
	}
}

func TestToRGBAExpandsEveryMode(t *testing.T) {
	rgba := &Image{Width: 2, Height: 1, Mode: "RGBA", Data: []byte{1, 2, 3, 255, 4, 5, 6, 255}}
	rgb := &Image{Width: 2, Height: 1, Mode: "RGB", Data: []byte{1, 2, 3, 4, 5, 6}}
	gray := &Image{Width: 2, Height: 1, Mode: "L", Data: []byte{9, 250}}
	cases := []struct {
		img  *Image
		want []byte
	}{
		{rgba, []byte{1, 2, 3, 255, 4, 5, 6, 255}},
		{rgb, []byte{1, 2, 3, 255, 4, 5, 6, 255}},
		{gray, []byte{9, 9, 9, 255, 250, 250, 250, 255}},
	}
	for _, tc := range cases {
		out := tc.img.ToRGBA()
		if out.Bounds() != image.Rect(0, 0, 2, 1) || !bytes.Equal(out.Pix, tc.want) {
			t.Fatalf("%s: ToRGBA = %v %v, want %v", tc.img.Mode, out.Bounds(), out.Pix, tc.want)
		}
		// Each pixel must equal the standard library's conversion of ColorAt.
		for x := 0; x < 2; x++ {
			if got, want := out.RGBAAt(x, 0), color.RGBAModel.Convert(tc.img.ColorAt(x, 0)); got != want {
				t.Fatalf("%s: pixel %d = %v, ColorAt converts to %v", tc.img.Mode, x, got, want)
			}
		}
	}

	src := opaquePattern(image.Rect(0, 0, 4, 3))
	if back := FromImage(src).ToRGBA(); !bytes.Equal(back.Pix, src.Pix) {
		t.Fatal("FromImage then ToRGBA does not return the source pixels")
	}
}

func TestColorAtReadsEachModeAndIsZeroOutside(t *testing.T) {
	rgba := &Image{Width: 1, Height: 1, Mode: "RGBA", Data: []byte{10, 20, 30, 40}}
	rgb := &Image{Width: 1, Height: 1, Mode: "RGB", Data: []byte{10, 20, 30}}
	gray := &Image{Width: 1, Height: 1, Mode: "L", Data: []byte{77}}
	if got := rgba.ColorAt(0, 0); got != (color.RGBA{R: 10, G: 20, B: 30, A: 40}) {
		t.Fatalf("RGBA pixel %v", got)
	}
	if got := rgb.ColorAt(0, 0); got != (color.RGBA{R: 10, G: 20, B: 30, A: 255}) {
		t.Fatalf("RGB pixel %v", got)
	}
	if got := gray.ColorAt(0, 0); got != (color.Gray{Y: 77}) {
		t.Fatalf("L pixel %v", got)
	}
	for _, p := range [][2]int{{-1, 0}, {1, 0}, {0, -1}, {0, 1}} {
		if got := rgb.ColorAt(p[0], p[1]); got != (color.RGBA{}) {
			t.Fatalf("ColorAt(%d, %d) outside a 1x1 image = %v, want the zero colour", p[0], p[1], got)
		}
	}
}

func TestLayoutCodeAndString(t *testing.T) {
	// The C ABI layout codes: SILLSTONE_LAYOUT_GRAY8 = 0, RGB8 = 2, RGBA8 = 4.
	for mode, want := range map[string]uint32{"L": 0, "RGB": 2, "RGBA": 4} {
		if got := NewImage(mode, 1, 1).LayoutCode(); got != want {
			t.Fatalf("LayoutCode(%q) = %d, want %d", mode, got, want)
		}
	}
	if got := NewImage("RGB", 9, 7).String(); got != "Sillstone.Image(mode=RGB, width=9, height=7)" {
		t.Fatalf("String() = %q", got)
	}
}
