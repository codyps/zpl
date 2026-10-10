//go:build backend_wazero && !backend_cgo && !backend_purego && !backend_wasm2go

package zpl

import (
	"context"
	_ "embed"
	"errors"
	"fmt"
	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

//go:embed guest.wasm
var guest []byte

type wasm struct {
	runtime wazero.Runtime
	module  api.Module
}

func newBackend(ctx context.Context) (backend, error) {
	// 256 MiB guest limit plus the renderer's own work budgets. Cancellation closes
	// the instance: https://pkg.go.dev/github.com/tetratelabs/wazero#RuntimeConfig
	rt := wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig().WithMemoryLimitPages(4096).WithCloseOnContextDone(true))
	mod, err := rt.Instantiate(ctx, guest)
	if err != nil {
		_ = rt.Close(ctx)
		return nil, err
	}
	b := &wasm{rt, mod}
	v, err := b.call(ctx, "zp_abi")
	if err != nil || v != 1 {
		_ = rt.Close(ctx)
		return nil, fmt.Errorf("ABI check: %d: %v", v, err)
	}
	return b, nil
}
func (w *wasm) call(ctx context.Context, name string, args ...uint64) (uint32, error) {
	v, e := w.module.ExportedFunction(name).Call(ctx, args...)
	if e != nil {
		return 0, e
	}
	if len(v) == 0 {
		return 0, nil
	}
	return uint32(v[0]), nil
}
func (w *wasm) render(ctx context.Context, r Request) ([]byte, error) {
	p, e := w.call(ctx, "zp_alloc", uint64(len(r.Input)))
	if e != nil {
		return nil, e
	}
	if p == 0 {
		return nil, errors.New("allocation rejected")
	}
	defer w.call(ctx, "zp_dealloc", uint64(p), uint64(len(r.Input)))
	if !w.module.Memory().Write(p, r.Input) {
		return nil, errors.New("input out of bounds")
	}
	result, e := w.call(ctx, "zp_render", uint64(p), uint64(len(r.Input)), uint64(r.Format), uint64(r.Profile), uint64(r.Width), uint64(r.Height))
	if e != nil {
		return nil, e
	}
	defer w.call(ctx, "zp_result_free", uint64(result))
	ptr, e := w.call(ctx, "zp_result_data", uint64(result))
	if e != nil {
		return nil, e
	}
	size, e := w.call(ctx, "zp_result_len", uint64(result))
	if e != nil {
		return nil, e
	}
	data, ok := w.module.Memory().Read(ptr, size)
	if !ok {
		return nil, errors.New("output out of bounds")
	}
	return append([]byte(nil), data...), nil
}
func (w *wasm) close(ctx context.Context) error { return w.runtime.Close(ctx) }
