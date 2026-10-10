//go:build (backend_cgo && backend_purego) || (backend_cgo && backend_wazero) || (backend_cgo && backend_wasm2go) || (backend_purego && backend_wazero) || (backend_purego && backend_wasm2go) || (backend_wazero && backend_wasm2go)

package zpl

import "context"

// Deliberately undefined identifier: fail at compile time with an actionable
// diagnostic. The default is wasm2go; conflicting explicit tags cannot silently
// select a backend. See https://pkg.go.dev/cmd/go#hdr-Build_constraints.
func newBackend(context.Context) (backend, error) {
	return nil, select_exactly_one_of_backend_cgo_backend_purego_backend_wazero_backend_wasm2go
}
