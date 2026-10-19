package sillstone

import (
	"reflect"
	"testing"
)

func TestGoTensor(t *testing.T) {
	tensor, err := NewTensor([]int{2, 3}, []float64{1, 2, 3, 4, 5, 6})
	if err != nil {
		t.Fatalf("NewTensor failed: %v", err)
	}
	if tensor.Size() != 6 {
		t.Fatalf("expected size 6, got %d", tensor.Size())
	}
	if tensor.Sum() != 21.0 {
		t.Fatalf("expected sum 21.0, got %f", tensor.Sum())
	}

	reshaped, err := tensor.Reshape([]int{3, 2})
	if err != nil {
		t.Fatalf("Reshape failed: %v", err)
	}
	if !reflect.DeepEqual(reshaped.Shape, []int{3, 2}) {
		t.Fatalf("expected shape [3, 2], got %v", reshaped.Shape)
	}

	b, _ := NewTensor([]int{3, 2}, []float64{7, 8, 9, 1, 2, 3})
	c, err := tensor.Matmul(b)
	if err != nil {
		t.Fatalf("Matmul failed: %v", err)
	}
	expected := []float64{31, 19, 85, 55}
	if !reflect.DeepEqual(c.Data, expected) {
		t.Fatalf("expected %v, got %v", expected, c.Data)
	}
}
