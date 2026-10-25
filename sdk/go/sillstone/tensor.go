package sillstone

import (
	"errors"
	"fmt"
	"unsafe"
)

// Tensor represents an N-dimensional strided array of float64 elements.
type Tensor struct {
	Shape []int
	Data  []float64
}

// NewTensor creates a new Tensor with given shape and data buffer.
func NewTensor(shape []int, data []float64) (*Tensor, error) {
	size := 1
	for _, dim := range shape {
		size *= dim
	}
	if len(data) != size {
		return nil, errors.New("data length mismatch with shape")
	}
	d := make([]float64, size)
	copy(d, data)
	s := make([]int, len(shape))
	copy(s, shape)
	return &Tensor{Shape: s, Data: d}, nil
}

// Size returns total number of elements.
func (t *Tensor) Size() int {
	return len(t.Data)
}

// Ndim returns number of dimensions.
func (t *Tensor) Ndim() int {
	return len(t.Shape)
}

// Reshape changes tensor shape while preserving data.
func (t *Tensor) Reshape(newShape []int) (*Tensor, error) {
	newSize := 1
	for _, dim := range newShape {
		newSize *= dim
	}
	if newSize != len(t.Data) {
		return nil, errors.New("cannot reshape with different size")
	}
	return NewTensor(newShape, t.Data)
}

// Sum returns the total of all elements, computed by the native core.
func (t *Tensor) Sum() float64 {
	var out float64
	if len(t.Data) == 0 {
		return 0
	}
	SillstoneTensorSumF64(unsafe.Pointer(&t.Data[0]), uint64(len(t.Data)), unsafe.Pointer(&out))
	return out
}

// Add returns the element-wise sum of two tensors of the same shape.
func (t *Tensor) Add(other *Tensor) (*Tensor, error) {
	if len(t.Shape) != len(other.Shape) {
		return nil, errors.New("add requires identical shapes")
	}
	for i, d := range t.Shape {
		if d != other.Shape[i] {
			return nil, errors.New("add requires identical shapes")
		}
	}
	out := make([]float64, len(t.Data))
	if len(out) > 0 {
		if status := SillstoneTensorAddF64(unsafe.Pointer(&t.Data[0]), unsafe.Pointer(&other.Data[0]), uint64(len(out)), unsafe.Pointer(&out[0])); status != 0 {
			return nil, fmt.Errorf("sillstone: tensor add failed (status %d)", status)
		}
	}
	return NewTensor(t.Shape, out)
}

// Matmul performs 2D matrix multiplication in the native core.
func (t *Tensor) Matmul(other *Tensor) (*Tensor, error) {
	if t.Ndim() != 2 || other.Ndim() != 2 {
		return nil, errors.New("matmul requires 2D tensors")
	}
	m, k1 := t.Shape[0], t.Shape[1]
	k2, n := other.Shape[0], other.Shape[1]
	if k1 != k2 {
		return nil, errors.New("inner dimensions must match for matmul")
	}
	out := make([]float64, m*n)
	if len(out) > 0 && k1 > 0 {
		status := SillstoneTensorMatmulF64(unsafe.Pointer(&t.Data[0]), uint64(m), uint64(k1), unsafe.Pointer(&other.Data[0]), uint64(k2), uint64(n), unsafe.Pointer(&out[0]))
		if status != 0 {
			return nil, fmt.Errorf("sillstone: tensor matmul failed (status %d)", status)
		}
	}
	return NewTensor([]int{m, n}, out)
}
