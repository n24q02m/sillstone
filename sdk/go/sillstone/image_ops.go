package sillstone

import (
	"errors"
	"fmt"
	"unsafe"
)

// channels returns the interleaved sample count per pixel of an 8-bit mode.
func channels(mode string) (int, error) {
	switch mode {
	case "L":
		return 1, nil
	case "RGB":
		return 3, nil
	case "RGBA":
		return 4, nil
	}
	return 0, fmt.Errorf("unsupported image mode %q (want L, RGB or RGBA)", mode)
}

func layoutOf(mode string) (uint32, error) {
	if _, err := channels(mode); err != nil {
		return 0, err
	}
	switch mode {
	case "RGB":
		return 2, nil
	case "RGBA":
		return 4, nil
	}
	return 0, nil
}

func statusError(op string, status int32) error {
	switch status {
	case 0:
		return nil
	case -2:
		return fmt.Errorf("sillstone: %s rejected the arguments", op)
	}
	return fmt.Errorf("sillstone: %s failed (status %d)", op, status)
}

// validate checks the pixel buffer against the declared geometry.
func (img *Image) validate() (uint32, int, error) {
	layout, err := layoutOf(img.Mode)
	if err != nil {
		return 0, 0, err
	}
	ch, _ := channels(img.Mode)
	if img.Width <= 0 || img.Height <= 0 {
		return 0, 0, errors.New("sillstone: image dimensions must be positive")
	}
	if len(img.Data) != img.Width*img.Height*ch {
		return 0, 0, fmt.Errorf("sillstone: %d bytes do not fill a %dx%d %s image", len(img.Data), img.Width, img.Height, img.Mode)
	}
	return layout, ch, nil
}

func dataPtr(b []byte) unsafe.Pointer {
	if len(b) == 0 {
		return nil
	}
	return unsafe.Pointer(&b[0])
}

// apply runs one same-mode native operation that fills an output buffer of
// the given size. call receives the input pointer/length/layout and the
// output pointer/length.
func (img *Image) apply(op string, outW, outH int, call func(in unsafe.Pointer, inLen uint64, layout uint32, out unsafe.Pointer, outLen uint64) int32) (*Image, error) {
	layout, ch, err := img.validate()
	if err != nil {
		return nil, err
	}
	out := make([]byte, outW*outH*ch)
	status := call(dataPtr(img.Data), uint64(len(img.Data)), layout, dataPtr(out), uint64(len(out)))
	if err := statusError(op, status); err != nil {
		return nil, err
	}
	return &Image{Width: outW, Height: outH, Mode: img.Mode, Data: out}, nil
}

// Resize resamples to width x height with a Resample* filter (Pillow-exact).
func (img *Image) Resize(width, height int, resample uint32) (*Image, error) {
	if width <= 0 || height <= 0 {
		return nil, errors.New("sillstone: resize dimensions must be positive")
	}
	return img.apply("resize", width, height, func(in unsafe.Pointer, n uint64, layout uint32, out unsafe.Pointer, outLen uint64) int32 {
		return SillstoneImageResize(in, n, uint32(img.Width), uint32(img.Height), layout, uint32(width), uint32(height), resample, out, outLen)
	})
}

// Crop returns the [x0, x1) x [y0, y1) window, which must lie inside the image.
func (img *Image) Crop(x0, y0, x1, y1 int) (*Image, error) {
	if x0 < 0 || y0 < 0 || x1 <= x0 || y1 <= y0 || x1 > img.Width || y1 > img.Height {
		return nil, fmt.Errorf("sillstone: box (%d,%d,%d,%d) lies outside the %dx%d image", x0, y0, x1, y1, img.Width, img.Height)
	}
	return img.apply("crop", x1-x0, y1-y0, func(in unsafe.Pointer, n uint64, layout uint32, out unsafe.Pointer, outLen uint64) int32 {
		return SillstoneImageCrop(in, n, uint32(img.Width), uint32(img.Height), layout, uint32(x0), uint32(y0), uint32(x1), uint32(y1), out, outLen)
	})
}

// Transpose applies a Transpose* operation.
func (img *Image) Transpose(method uint32) (*Image, error) {
	w, h := img.Width, img.Height
	switch method {
	case TransposeRotate90, TransposeRotate270, TransposeMainDiagonal, TransposeAntiDiagonal:
		w, h = h, w
	}
	return img.apply("transpose", w, h, func(in unsafe.Pointer, n uint64, layout uint32, out unsafe.Pointer, outLen uint64) int32 {
		return SillstoneImageTranspose(in, n, uint32(img.Width), uint32(img.Height), layout, method, out, outLen)
	})
}

// Rotate turns the image counter-clockwise by angle degrees; expand grows the
// canvas to fit the rotated image.
func (img *Image) Rotate(angle float64, resample uint32, expand bool) (*Image, error) {
	flag := uint32(0)
	if expand {
		flag = 1
	}
	var w, h uint32
	if err := statusError("rotate", SillstoneImageRotateSize(uint32(img.Width), uint32(img.Height), angle, flag, &w, &h)); err != nil {
		return nil, err
	}
	return img.apply("rotate", int(w), int(h), func(in unsafe.Pointer, n uint64, layout uint32, out unsafe.Pointer, outLen uint64) int32 {
		return SillstoneImageRotate(in, n, uint32(img.Width), uint32(img.Height), layout, angle, flag, resample, out, outLen)
	})
}

// Filter applies a Filter* convolution.
func (img *Image) Filter(filter uint32) (*Image, error) {
	return img.apply("filter", img.Width, img.Height, func(in unsafe.Pointer, n uint64, layout uint32, out unsafe.Pointer, outLen uint64) int32 {
		return SillstoneImageFilter(in, n, uint32(img.Width), uint32(img.Height), layout, filter, out, outLen)
	})
}

// Brightness scales every sample by factor (1 = identity), clamped to 0..255.
func (img *Image) Brightness(factor float64) (*Image, error) {
	return img.apply("brightness", img.Width, img.Height, func(in unsafe.Pointer, n uint64, layout uint32, out unsafe.Pointer, outLen uint64) int32 {
		return SillstoneImageAdjust(in, n, uint32(img.Width), uint32(img.Height), layout, 0, factor, out, outLen)
	})
}

// Invert replaces each sample v with 255 - v.
func (img *Image) Invert() (*Image, error) {
	return img.apply("invert", img.Width, img.Height, func(in unsafe.Pointer, n uint64, layout uint32, out unsafe.Pointer, outLen uint64) int32 {
		return SillstoneImageAdjust(in, n, uint32(img.Width), uint32(img.Height), layout, 1, 0, out, outLen)
	})
}

// Convert changes the mode to "L", "RGB" or "RGBA".
func (img *Image) Convert(mode string) (*Image, error) {
	from, _, err := img.validate()
	if err != nil {
		return nil, err
	}
	to, err := layoutOf(mode)
	if err != nil {
		return nil, err
	}
	ch, _ := channels(mode)
	if mode == img.Mode {
		return &Image{Width: img.Width, Height: img.Height, Mode: mode, Data: append([]byte(nil), img.Data...)}, nil
	}
	out := make([]byte, img.Width*img.Height*ch)
	status := SillstoneImageColorConvert(dataPtr(img.Data), uint64(len(img.Data)), uint32(img.Width), uint32(img.Height), from, to, dataPtr(out), uint64(len(out)))
	if err := statusError("convert", status); err != nil {
		return nil, err
	}
	return &Image{Width: img.Width, Height: img.Height, Mode: mode, Data: out}, nil
}

// encode runs a native encoder that returns an engine-owned buffer and copies it out.
func (img *Image) encode(op string, call func(in unsafe.Pointer, n uint64, layout uint32, outPtr *unsafe.Pointer, outLen *uint64) int32) ([]byte, error) {
	layout, _, err := img.validate()
	if err != nil {
		return nil, err
	}
	var ptr unsafe.Pointer
	var n uint64
	if err := statusError(op, call(dataPtr(img.Data), uint64(len(img.Data)), layout, &ptr, &n)); err != nil {
		return nil, err
	}
	defer SillstoneImageFreeBytes(ptr, n)
	return append([]byte(nil), unsafe.Slice((*byte)(ptr), int(n))...), nil
}

// QoiEncode encodes the image as lossless QOI.
func (img *Image) QoiEncode() ([]byte, error) {
	return img.encode("qoi encode", func(in unsafe.Pointer, n uint64, layout uint32, outPtr *unsafe.Pointer, outLen *uint64) int32 {
		return SillstoneImageQoiEncode(in, n, uint32(img.Width), uint32(img.Height), layout, outPtr, outLen)
	})
}

// BmpEncode encodes the image as BMP.
func (img *Image) BmpEncode() ([]byte, error) {
	return img.encode("bmp encode", func(in unsafe.Pointer, n uint64, layout uint32, outPtr *unsafe.Pointer, outLen *uint64) int32 {
		return SillstoneImageBmpEncode(in, n, uint32(img.Width), uint32(img.Height), layout, outPtr, outLen)
	})
}

// QoiDecode decodes a QOI stream into an "L", "RGB" or "RGBA" image.
func QoiDecode(stream []byte) (*Image, error) {
	var w, h, layout uint32
	var ptr unsafe.Pointer
	var n uint64
	if err := statusError("qoi decode", SillstoneImageQoiDecode(dataPtr(stream), uint64(len(stream)), &w, &h, &layout, &ptr, &n)); err != nil {
		return nil, err
	}
	defer SillstoneImageFreeBytes(ptr, n)
	mode := ""
	switch layout {
	case 0:
		mode = "L"
	case 2:
		mode = "RGB"
	case 4:
		mode = "RGBA"
	default:
		return nil, fmt.Errorf("sillstone: decoded layout %d has no 8-bit mode", layout)
	}
	return &Image{Width: int(w), Height: int(h), Mode: mode, Data: append([]byte(nil), unsafe.Slice((*byte)(ptr), int(n))...)}, nil
}

// WHash computes the imagehash-compatible 64-bit wavelet hash.
func (img *Image) WHash() uint64 {
	return SillstoneImageWhash(img.Data, uint32(img.Width), uint32(img.Height), img.LayoutCode())
}

// ColorHash computes the imagehash-compatible colour hash.
func (img *Image) ColorHash() uint64 {
	return SillstoneImageColorhash(img.Data, uint32(img.Width), uint32(img.Height), img.LayoutCode())
}
