//go:build backend_purego && !backend_cgo && !backend_wazero && !backend_wasm2go && (linux || darwin)

package zpl

import (
	"context"
	"errors"
	"github.com/ebitengine/purego"
	"os"
)

func newBackend(context.Context) (backend, error) {
	path := os.Getenv("ZPL_GO_LIBRARY")
	if path == "" {
		return nil, errors.New("set ZPL_GO_LIBRARY to the absolute shared-library path")
	}
	h, err := purego.Dlopen(path, purego.RTLD_NOW|purego.RTLD_LOCAL)
	if err != nil {
		return nil, err
	}
	n := &native{unload: func() error { return purego.Dlclose(h) }}
	var abi func() uint32
	for name, fn := range map[string]any{"zp_abi": &abi, "zp_alloc": &n.alloc, "zp_dealloc": &n.dealloc, "zp_render": &n.run, "zp_result_data": &n.data, "zp_result_len": &n.length, "zp_result_free": &n.free} {
		sym, e := purego.Dlsym(h, name)
		if e != nil {
			_ = n.unload()
			return nil, e
		}
		purego.RegisterFunc(fn, sym)
	}
	if abi() != 1 {
		_ = n.unload()
		return nil, errors.New("ABI mismatch")
	}
	return n, nil
}
