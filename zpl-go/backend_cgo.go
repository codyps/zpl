//go:build backend_cgo && !backend_purego && !backend_wazero && !backend_wasm2go && cgo

package zpl

/*
#cgo LDFLAGS: -lzpl_go_prototype
#include "bridge.h"
*/
import "C"
import (
	"context"
	"errors"
	"unsafe"
)

func newBackend(context.Context) (backend, error) {
	if C.zp_abi() != 1 {
		return nil, errors.New("ABI mismatch")
	}
	return &native{
		alloc:   func(n uintptr) unsafe.Pointer { return C.zp_alloc(C.size_t(n)) },
		dealloc: func(p unsafe.Pointer, n uintptr) { C.zp_dealloc(p, C.size_t(n)) },
		run: func(p unsafe.Pointer, n uintptr, f, s, w, h uint32) unsafe.Pointer {
			return C.zp_render(p, C.size_t(n), C.uint32_t(f), C.uint32_t(s), C.uint32_t(w), C.uint32_t(h))
		},
		data:   func(p unsafe.Pointer) unsafe.Pointer { return unsafe.Pointer(C.zp_result_data(p)) },
		length: func(p unsafe.Pointer) uintptr { return uintptr(C.zp_result_len(p)) },
		free:   func(p unsafe.Pointer) { C.zp_result_free(p) },
	}, nil
}
