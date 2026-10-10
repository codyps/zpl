//go:build backend_cgo && !cgo && !backend_purego && !backend_wazero && !backend_wasm2go

package zpl

import "context"

// Deliberately undefined identifier: selecting cgo must not fall back silently.
func newBackend(context.Context) (backend, error) {
	return nil, backend_cgo_requires_CGO_ENABLED_1_and_a_C_toolchain
}
