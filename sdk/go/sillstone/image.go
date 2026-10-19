// Package sillstone provides zero-dependency multimedia, hashing, and array computing.
package sillstone

import (
	"fmt"
	"image"
	"image/color"
)

// Resampling filter constants matching Pillow.
const (
	ResampleNearest  uint32 = 0
	ResampleBox      uint32 = 1
	ResampleBilinear uint32 = 2
	ResampleBicubic  uint32 = 3
	ResampleLanczos  uint32 = 4
)

// Transpose operation constants.
const (
	TransposeFlipLeftRight uint32 = 0
	TransposeFlipTopBottom uint32 = 1
	TransposeRotate90      uint32 = 2
	TransposeRotate180     uint32 = 3
	TransposeRotate270     uint32 = 4
	TransposeMainDiagonal  uint32 = 5
	TransposeAntiDiagonal  uint32 = 6
)

// Filter operation constants.
const (
	FilterBlur        uint32 = 0
	FilterSharpen     uint32 = 1
	FilterContour     uint32 = 2
	FilterDetail      uint32 = 3
	FilterEdgeEnhance uint32 = 4
	FilterFindEdges   uint32 = 5
	FilterEmboss      uint32 = 6
)

// Image represents a Pillow-compatible multi-channel raster image.
type Image struct {
	Width  int
	Height int
	Mode   string // "RGB", "RGBA", "L"
	Data   []byte
}

// NewImage creates a new empty Image with the specified mode and dimensions.
func NewImage(mode string, width, height int) *Image {
	ch := 3
	if mode == "RGBA" {
		ch = 4
	} else if mode == "L" {
		ch = 1
	}
	return &Image{
		Width:  width,
		Height: height,
		Mode:   mode,
		Data:   make([]byte, width*height*ch),
	}
}

// FromImage converts a standard Go image.Image to a Sillstone Image.
func FromImage(src image.Image) *Image {
	bounds := src.Bounds()
	w, h := bounds.Dx(), bounds.Dy()
	img := NewImage("RGBA", w, h)
	idx := 0
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			r, g, b, a := src.At(x, y).RGBA()
			img.Data[idx] = byte(r >> 8)
			img.Data[idx+1] = byte(g >> 8)
			img.Data[idx+2] = byte(b >> 8)
			img.Data[idx+3] = byte(a >> 8)
			idx += 4
		}
	}
	return img
}

// ToRGBA converts the Sillstone Image to standard Go *image.RGBA.
func (img *Image) ToRGBA() *image.RGBA {
	rgba := image.NewRGBA(image.Rect(0, 0, img.Width, img.Height))
	if img.Mode == "RGBA" {
		copy(rgba.Pix, img.Data)
	} else if img.Mode == "RGB" {
		srcIdx := 0
		dstIdx := 0
		for range img.Width * img.Height {
			rgba.Pix[dstIdx] = img.Data[srcIdx]
			rgba.Pix[dstIdx+1] = img.Data[srcIdx+1]
			rgba.Pix[dstIdx+2] = img.Data[srcIdx+2]
			rgba.Pix[dstIdx+3] = 255
			srcIdx += 3
			dstIdx += 4
		}
	} else if img.Mode == "L" {
		srcIdx := 0
		dstIdx := 0
		for range img.Width * img.Height {
			l := img.Data[srcIdx]
			rgba.Pix[dstIdx] = l
			rgba.Pix[dstIdx+1] = l
			rgba.Pix[dstIdx+2] = l
			rgba.Pix[dstIdx+3] = 255
			srcIdx++
			dstIdx += 4
		}
	}
	return rgba
}

// ColorAt returns the color of pixel at (x, y).
func (img *Image) ColorAt(x, y int) color.Color {
	if x < 0 || x >= img.Width || y < 0 || y >= img.Height {
		return color.RGBA{}
	}
	idx := (y*img.Width + x)
	if img.Mode == "RGBA" {
		return color.RGBA{R: img.Data[idx*4], G: img.Data[idx*4+1], B: img.Data[idx*4+2], A: img.Data[idx*4+3]}
	} else if img.Mode == "RGB" {
		return color.RGBA{R: img.Data[idx*3], G: img.Data[idx*3+1], B: img.Data[idx*3+2], A: 255}
	}
	return color.Gray{Y: img.Data[idx]}
}

// LayoutCode returns the C-ABI layout code.
func (img *Image) LayoutCode() uint32 {
	if img.Mode == "RGBA" {
		return 4
	} else if img.Mode == "RGB" {
		return 2
	}
	return 0
}

// PHash computes the 64-bit perceptual hash using the suite's native implementation.
func (img *Image) PHash() uint64 {
	return SillstoneImagePhash(img.Data, uint32(img.Width), uint32(img.Height), img.LayoutCode())
}

// AHash computes the 64-bit average hash.
func (img *Image) AHash() uint64 {
	return SillstoneImageAhash(img.Data, uint32(img.Width), uint32(img.Height), img.LayoutCode())
}

// DHash computes the 64-bit difference hash.
func (img *Image) DHash() uint64 {
	return SillstoneImageDhash(img.Data, uint32(img.Width), uint32(img.Height), img.LayoutCode())
}

// String returns image description.
func (img *Image) String() string {
	return fmt.Sprintf("Sillstone.Image(mode=%s, width=%d, height=%d)", img.Mode, img.Width, img.Height)
}
