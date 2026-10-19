package sillstone

import (
	"errors"
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

// Sum returns total sum of all elements.
func (t *Tensor) Sum() float64 {
	acc := 0.0
	for _, v := range t.Data {
		acc += v
	}
	return acc
}

// Matmul performs 2D matrix multiplication.
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
	for i := 0; i < m; i++ {
		for j := 0; j < n; j++ {
			acc := 0.0
			for p := 0; p < k1; p++ {
				acc += t.Data[i*k1+p] * other.Data[p*n+j]
			}
			out[i*n+j] = acc
		}
	}
	return NewTensor([]int{m, n}, out)
}
