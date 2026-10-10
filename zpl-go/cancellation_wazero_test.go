//go:build backend_wazero

package zpl

import (
	"context"
	"os"
	"testing"
	"time"
)

func TestWazeroInterruptsAndRetiresInstance(t *testing.T) {
	e, err := New(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close(context.Background())
	input, err := os.ReadFile("testdata/inputs/graphic.zpl")
	if err != nil {
		t.Fatal(err)
	}
	// This fixture takes tens of ms in the measured guest. If a faster runtime
	// finishes first, report that the cancellation condition was not exercised.
	ctx, cancel := context.WithTimeout(context.Background(), time.Millisecond)
	defer cancel()
	_, err = e.Render(ctx, Request{input, PNG, Specification, 512, 512})
	if err == nil {
		t.Fatal("render finished before cancellation; use a larger fixture")
	}
	if ctx.Err() == nil {
		t.Fatalf("unexpected non-cancellation failure: %v", err)
	}
	_, err = e.Render(context.Background(), Request{[]byte("^XA^XZ"), PNG, Specification, 20, 20})
	if err == nil {
		t.Fatal("canceled Wasm instance was reused")
	}
}
