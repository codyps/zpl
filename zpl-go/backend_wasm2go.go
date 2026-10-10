//go:build !backend_cgo && !backend_purego && !backend_wazero

package zpl

import (
	"context"
	"errors"
	"fmt"
	"github.com/codyps/zpl/zpl-go/internal/translated"
)

type converted struct{ module *translated.Module }

func newBackend(context.Context) (backend, error) {
	m := translated.New()
	if m.Xzp_abi() != 1 {
		return nil, errors.New("ABI mismatch")
	}
	return &converted{m}, nil
}
func (w *converted) render(_ context.Context, r Request) (out []byte, err error) {
	if w.module == nil {
		return nil, errors.New("translated instance closed")
	}
	// Generated traps become Go panics. Retire a trapped instance rather than reuse
	// a potentially inconsistent Rust allocator/stack. Native aborts are not caught.
	defer func() {
		if p := recover(); p != nil {
			w.module = nil
			out = nil
			err = fmt.Errorf("translated Wasm trap: %v", p)
		}
	}()
	m := w.module
	p := m.Xzp_alloc(int32(len(r.Input)))
	if p == 0 {
		return nil, errors.New("allocation rejected")
	}
	defer m.Xzp_dealloc(p, int32(len(r.Input)))
	copy((*m.Xmemory().Slice())[uint32(p):uint64(uint32(p))+uint64(len(r.Input))], r.Input)
	result := m.Xzp_render(p, int32(len(r.Input)), int32(r.Format), int32(r.Profile), int32(r.Width), int32(r.Height))
	defer m.Xzp_result_free(result)
	ptr, size := uint32(m.Xzp_result_data(result)), uint32(m.Xzp_result_len(result))
	return append([]byte(nil), (*m.Xmemory().Slice())[ptr:uint64(ptr)+uint64(size)]...), nil
}
func (w *converted) close(context.Context) error { w.module = nil; return nil }
