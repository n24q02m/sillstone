package sillstone

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"testing"
)

type goldenImage struct {
	Width  int    `json:"width"`
	Height int    `json:"height"`
	Mode   string `json:"mode"`
	Hex    string `json:"hex"`
}

type golden struct {
	Image     goldenImage            `json:"image"`
	ImageRGBA goldenImage            `json:"image_rgba"`
	Ops       map[string]goldenImage `json:"ops"`
	Hashes    map[string]string      `json:"hashes"`
	QoiRGB    string                 `json:"qoi_rgb"`
	QoiRGBA   string                 `json:"qoi_rgba"`
	BmpRGB    string                 `json:"bmp_rgb"`
	Tensor    struct {
		A struct {
			Shape []int
			Data  []float64
		} `json:"a"`
		A2 struct {
			Shape []int
			Data  []float64
		} `json:"a2"`
		B struct {
			Shape []int
			Data  []float64
		} `json:"b"`
		Matmul []float64 `json:"matmul"`
		Add    []float64 `json:"add"`
		Sum    float64   `json:"sum"`
	} `json:"tensor"`
}

func loadGolden(t *testing.T) *golden {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join("..", "..", "..", "tests", "fixtures", "facade", "golden.json"))
	if err != nil {
		t.Fatal(err)
	}
	var g golden
	if err := json.Unmarshal(raw, &g); err != nil {
		t.Fatal(err)
	}
	return &g
}

func (g goldenImage) image(t *testing.T) *Image {
	t.Helper()
	data, err := hex.DecodeString(g.Hex)
	if err != nil {
		t.Fatal(err)
	}
	return &Image{Width: g.Width, Height: g.Height, Mode: g.Mode, Data: data}
}

func sameImage(t *testing.T, name string, got *Image, want goldenImage) {
	t.Helper()
	if got.Width != want.Width || got.Height != want.Height || got.Mode != want.Mode {
		t.Fatalf("%s: got %dx%d %s, want %dx%d %s", name, got.Width, got.Height, got.Mode, want.Width, want.Height, want.Mode)
	}
	if hex.EncodeToString(got.Data) != want.Hex {
		t.Fatalf("%s: pixels differ from the Python golden vector", name)
	}
}

// TestImageOpsMatchPythonGolden replays every golden operation: Python (the
// oracle) produced the vectors through the same native core.
func TestImageOpsMatchPythonGolden(t *testing.T) {
	g := loadGolden(t)
	src := g.Image.image(t)
	rgba := g.ImageRGBA.image(t)

	resample := map[string]uint32{"nearest": ResampleNearest, "box": ResampleBox, "bilinear": ResampleBilinear, "bicubic": ResampleBicubic, "lanczos": ResampleLanczos}
	for name, want := range map[string][2]int{"bicubic_14x11": {14, 11}, "bicubic_5x4": {5, 4}, "bilinear_5x4": {5, 4}, "box_5x4": {5, 4}, "lanczos_5x4": {5, 4}, "nearest_5x4": {5, 4}} {
		var kind string
		for k := range resample {
			if len(name) > len(k) && name[:len(k)] == k {
				kind = k
			}
		}
		out, err := src.Resize(want[0], want[1], resample[kind])
		if err != nil {
			t.Fatal(err)
		}
		sameImage(t, "resize_"+name, out, g.Ops["resize_"+name])
	}
	out, err := rgba.Resize(3, 8, ResampleBilinear)
	if err != nil {
		t.Fatal(err)
	}
	sameImage(t, "resize_rgba_bilinear_3x8", out, g.Ops["resize_rgba_bilinear_3x8"])

	if out, err = src.Crop(2, 1, 8, 6); err != nil {
		t.Fatal(err)
	}
	sameImage(t, "crop", out, g.Ops["crop_2_1_8_6"])

	for name, method := range map[string]uint32{
		"flip_left_right": TransposeFlipLeftRight, "flip_top_bottom": TransposeFlipTopBottom,
		"rotate_90": TransposeRotate90, "rotate_180": TransposeRotate180, "rotate_270": TransposeRotate270,
		"transpose": TransposeMainDiagonal, "transverse": TransposeAntiDiagonal,
	} {
		if out, err = src.Transpose(method); err != nil {
			t.Fatal(err)
		}
		sameImage(t, "transpose_"+name, out, g.Ops["transpose_"+name])
	}

	for name, c := range map[string]struct {
		angle    float64
		filter   uint32
		expand   bool
		rotateID string
	}{
		"rotate_30_bilinear":        {30, ResampleBilinear, false, ""},
		"rotate_30_bilinear_expand": {30, ResampleBilinear, true, ""},
		"rotate_45_nearest_expand":  {45, ResampleNearest, true, ""},
		"rotate_neg20_nearest":      {-20, ResampleNearest, false, ""},
		"rotate_90":                 {90, ResampleNearest, false, ""},
	} {
		if out, err = src.Rotate(c.angle, c.filter, c.expand); err != nil {
			t.Fatal(err)
		}
		sameImage(t, name, out, g.Ops[name])
	}

	for name, f := range map[string]uint32{"blur": FilterBlur, "sharpen": FilterSharpen, "contour": FilterContour, "detail": FilterDetail, "edge_enhance": FilterEdgeEnhance, "find_edges": FilterFindEdges, "emboss": FilterEmboss} {
		if out, err = src.Filter(f); err != nil {
			t.Fatal(err)
		}
		sameImage(t, "filter_"+name, out, g.Ops["filter_"+name])
	}

	if out, err = src.Brightness(1.5); err != nil {
		t.Fatal(err)
	}
	sameImage(t, "brightness_1_5", out, g.Ops["brightness_1_5"])
	if out, err = src.Invert(); err != nil {
		t.Fatal(err)
	}
	sameImage(t, "invert", out, g.Ops["invert"])

	for name, c := range map[string]struct {
		from *Image
		mode string
	}{"convert_rgb_to_l": {src, "L"}, "convert_rgb_to_rgba": {src, "RGBA"}, "convert_rgba_to_rgb": {rgba, "RGB"}} {
		if out, err = c.from.Convert(c.mode); err != nil {
			t.Fatal(err)
		}
		sameImage(t, name, out, g.Ops[name])
	}
}

func TestImageCodecsAndHashesMatchPythonGolden(t *testing.T) {
	g := loadGolden(t)
	src := g.Image.image(t)

	qoi, err := src.QoiEncode()
	if err != nil {
		t.Fatal(err)
	}
	if hex.EncodeToString(qoi) != g.QoiRGB {
		t.Fatal("qoi encode differs from the golden stream")
	}
	back, err := QoiDecode(qoi)
	if err != nil {
		t.Fatal(err)
	}
	if back.Mode != "RGB" || back.Width != src.Width || !bytes.Equal(back.Data, src.Data) {
		t.Fatal("qoi round trip is not lossless")
	}
	if qoi, err = g.ImageRGBA.image(t).QoiEncode(); err != nil || hex.EncodeToString(qoi) != g.QoiRGBA {
		t.Fatalf("rgba qoi encode differs from the golden stream (err=%v)", err)
	}
	bmp, err := src.BmpEncode()
	if err != nil || hex.EncodeToString(bmp) != g.BmpRGB {
		t.Fatalf("bmp encode differs from the golden stream (err=%v)", err)
	}

	for name, got := range map[string]uint64{"ahash": src.AHash(), "dhash": src.DHash(), "phash": src.PHash(), "whash": src.WHash(), "colorhash": src.ColorHash()} {
		want, perr := strconv.ParseUint(g.Hashes[name], 0, 64)
		if perr != nil {
			t.Fatalf("golden %s: %v", name, perr)
		}
		if got != want {
			t.Fatalf("%s = %#x, want %#x", name, got, want)
		}
	}
}

func TestImageOpsRejectBadArguments(t *testing.T) {
	img := NewImage("RGB", 4, 4)
	if _, err := img.Crop(0, 0, 5, 4); err == nil {
		t.Fatal("crop outside the image must fail")
	}
	if _, err := img.Resize(0, 3, ResampleBox); err == nil {
		t.Fatal("zero-size resize must fail")
	}
	if _, err := img.Convert("CMYK"); err == nil {
		t.Fatal("unsupported mode must fail")
	}
	if _, err := QoiDecode([]byte("not qoi")); err == nil {
		t.Fatal("garbage qoi stream must fail")
	}
	short := &Image{Width: 4, Height: 4, Mode: "RGB", Data: make([]byte, 10)}
	if _, err := short.Invert(); err == nil {
		t.Fatal("a short pixel buffer must fail")
	}
}

func TestTensorMatchesPythonGolden(t *testing.T) {
	g := loadGolden(t)
	a, _ := NewTensor(g.Tensor.A.Shape, g.Tensor.A.Data)
	a2, _ := NewTensor(g.Tensor.A2.Shape, g.Tensor.A2.Data)
	b, _ := NewTensor(g.Tensor.B.Shape, g.Tensor.B.Data)
	prod, err := a.Matmul(b)
	if err != nil {
		t.Fatal(err)
	}
	sum, err := a.Add(a2)
	if err != nil {
		t.Fatal(err)
	}
	for name, c := range map[string][2][]float64{"matmul": {prod.Data, g.Tensor.Matmul}, "add": {sum.Data, g.Tensor.Add}} {
		if len(c[0]) != len(c[1]) {
			t.Fatalf("%s: length %d, want %d", name, len(c[0]), len(c[1]))
		}
		for i := range c[0] {
			if math.Abs(c[0][i]-c[1][i]) > 1e-9 {
				t.Fatalf("%s[%d] = %v, want %v", name, i, c[0][i], c[1][i])
			}
		}
	}
	if math.Abs(a.Sum()-g.Tensor.Sum) > 1e-9 {
		t.Fatalf("sum = %v, want %v", a.Sum(), g.Tensor.Sum)
	}
	if _, err := a.Matmul(a); err == nil {
		t.Fatal("mismatched inner dimensions must fail")
	}
}
