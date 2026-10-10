//go:build backend_cgo || backend_purego

package zpl

import (
	"context"
	"errors"
	"unsafe"
)

// All transfers use Rust allocations; no retained Go pointers or thread-local errors.
// ABI/lifetime rules: bridge/src/lib.rs and https://pkg.go.dev/cmd/cgo#hdr-Passing_pointers
type native struct {
	alloc   func(uintptr) unsafe.Pointer
	dealloc func(unsafe.Pointer, uintptr)
	run     func(unsafe.Pointer, uintptr, uint32, uint32, uint32, uint32) unsafe.Pointer
	data    func(unsafe.Pointer) unsafe.Pointer
	length  func(unsafe.Pointer) uintptr
	free    func(unsafe.Pointer)
	unload  func() error
}

func (n *native) render(_ context.Context, r Request) ([]byte, error) {
	p := n.alloc(uintptr(len(r.Input)))
	if p == nil {
		return nil, errors.New("allocation rejected")
	}
	defer n.dealloc(p, uintptr(len(r.Input)))
	copy(unsafe.Slice((*byte)(p), len(r.Input)), r.Input)
	result := n.run(p, uintptr(len(r.Input)), uint32(r.Format), uint32(r.Profile), r.Width, r.Height)
	if result == nil {
		return nil, errors.New("null result")
	}
	defer n.free(result)
	size := n.length(result)
	if size > 512*1024*1024 {
		return nil, errors.New("oversized response")
	}
	return append([]byte(nil), unsafe.Slice((*byte)(n.data(result)), int(size))...), nil
}
func (n *native) close(context.Context) error {
	if n.unload != nil {
		return n.unload()
	}
	return nil
}
