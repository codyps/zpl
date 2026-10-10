//go:build backend_purego && !backend_cgo && !backend_wazero && !backend_wasm2go && !linux && !darwin

package zpl

import "context"

// Deliberately undefined identifier until this prototype has more native loaders.
func newBackend(context.Context) (backend, error) {
	return nil, backend_purego_prototype_requires_linux_or_darwin
}
