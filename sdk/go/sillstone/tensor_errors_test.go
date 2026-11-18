package sillstone

import (
	"reflect"
	"testing"
)

// The shape checks of the tensor API, and the zero-size cases that never
// reach the native core.

func TestTensorRefusesMismatchedShapes(t *testing.T) {
	if _, err := NewTensor([]int{2, 2}, []float64{1, 2, 3}); err == nil {
		t.Fatal("NewTensor accepted 3 values for a 2x2 shape")
	} else {
		wantErrContaining(t, err, "data length mismatch with shape")
	}

	a, _ := NewTensor([]int{2, 3}, []float64{1, 2, 3, 4, 5, 6})
	if _, err := a.Reshape([]int{4}); err == nil {
		t.Fatal("Reshape accepted a shape of a different size")
	} else {
		wantErrContaining(t, err, "cannot reshape with different size")
	}

	flat, _ := NewTensor([]int{6}, []float64{1, 2, 3, 4, 5, 6})
	turned, _ := NewTensor([]int{3, 2}, []float64{1, 2, 3, 4, 5, 6})
	for name, other := range map[string]*Tensor{"different rank": flat, "same rank, different dims": turned} {
		if _, err := a.Add(other); err == nil {
			t.Fatalf("Add accepted %s", name)
		} else {
			wantErrContaining(t, err, "add requires identical shapes")
		}
	}

	if _, err := a.Matmul(flat); err == nil {
		t.Fatal("Matmul accepted a 1D operand")
	} else {
		wantErrContaining(t, err, "matmul requires 2D tensors")
	}
	if _, err := flat.Matmul(a); err == nil {
		t.Fatal("Matmul accepted a 1D receiver")
	} else {
		wantErrContaining(t, err, "matmul requires 2D tensors")
	}
}

func TestTensorZeroSizeResults(t *testing.T) {
	empty, err := NewTensor([]int{0}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if empty.Size() != 0 || empty.Ndim() != 1 || empty.Sum() != 0 {
		t.Fatalf("zero-size tensor: size %d, ndim %d, sum %v", empty.Size(), empty.Ndim(), empty.Sum())
	}
	sum, err := empty.Add(empty)
	if err != nil || sum.Size() != 0 {
		t.Fatalf("zero-size Add = %v, %v", sum, err)
	}

	// An inner dimension of 0 gives an m x n product of empty sums, which
	// are 0 (numpy.matmul gives the same zeros for (2, 0) @ (0, 3)).
	left, _ := NewTensor([]int{2, 0}, nil)
	right, _ := NewTensor([]int{0, 3}, nil)
	prod, err := left.Matmul(right)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(prod.Shape, []int{2, 3}) || !reflect.DeepEqual(prod.Data, make([]float64, 6)) {
		t.Fatalf("(2, 0) @ (0, 3) = shape %v data %v, want a 2x3 tensor of zeros", prod.Shape, prod.Data)
	}
}

func TestNewTensorCopiesItsArguments(t *testing.T) {
	shape := []int{2}
	data := []float64{1, 2}
	ten, _ := NewTensor(shape, data)
	shape[0], data[0] = 9, 9
	if ten.Shape[0] != 2 || ten.Data[0] != 1 {
		t.Fatalf("tensor follows later writes to its arguments: shape %v data %v", ten.Shape, ten.Data)
	}
}
