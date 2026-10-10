// Package zpl provides an experimental rendering API with build-time backend
// selection. Without a backend tag it uses wasm2go. Override with backend_cgo,
// backend_purego, or backend_wazero; backend_wasm2go explicitly selects the default.
// The API is not yet stable.
//
//go:generate python3 generate.py
package zpl

import (
	"context"
	"encoding/binary"
	"errors"
	"fmt"
	"sync"
)

type Format uint32

const (
	PNG Format = iota
	SVG
	PDF
	Gray8
)

type Profile uint32

const (
	Specification Profile = iota
	ZD621
	ZQ610Plus
)

type Request struct {
	Input         []byte
	Format        Format
	Profile       Profile
	Width, Height uint32
}
type Response struct {
	Status, Offset, Width, Height, Labels uint32
	Warnings                              string
	Body                                  []byte
}
type RenderError struct {
	Status, Offset uint32
	Message        string
}

func (e *RenderError) Error() string { return e.Message }

type backend interface {
	render(context.Context, Request) ([]byte, error)
	close(context.Context) error
}
type Engine struct {
	mu   sync.Mutex
	impl backend
}

// New creates an engine using the backend selected at build time.
// The application uses the same Request, Response, and RenderError types for all
// backends. See README.md for native-library and generated-artifact prerequisites.
func New(ctx context.Context) (*Engine, error) {
	b, e := newBackend(ctx)
	if e != nil {
		return nil, e
	}
	return &Engine{impl: b}, nil
}

// Render returns Go-owned data that survives subsequent calls and Close.
// One engine serializes calls; use multiple engines for parallel rendering.
// Every backend checks ctx before rendering; only wazero currently supports
// interrupting an in-progress render (which closes its Wasm instance).
func (e *Engine) Render(ctx context.Context, r Request) (Response, error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.impl == nil {
		return Response{}, errors.New("engine closed")
	}
	if err := ctx.Err(); err != nil {
		return Response{}, err
	}
	if len(r.Input) > 1024*1024 {
		return Response{}, errors.New("prototype input exceeds 1 MiB")
	}
	packet, err := e.impl.render(ctx, r)
	if err != nil {
		return Response{}, err
	}
	return Decode(packet)
}

// Close releases engine resources and is safe to call more than once.
func (e *Engine) Close(ctx context.Context) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if e.impl == nil {
		return nil
	}
	b := e.impl
	e.impl = nil
	return b.close(ctx)
}

// Decode validates the private, versioned response format before slicing.
func Decode(p []byte) (Response, error) {
	if len(p) < 32 {
		return Response{}, errors.New("short response")
	}
	u := func(i int) uint32 { return binary.LittleEndian.Uint32(p[i*4:]) }
	if u(0) != 1 || uint64(u(6))+uint64(u(7))+32 != uint64(len(p)) {
		return Response{}, fmt.Errorf("invalid response header")
	}
	r := Response{Status: u(1), Offset: u(2), Width: u(3), Height: u(4), Labels: u(5), Warnings: string(p[32 : 32+int(u(6))]), Body: p[32+int(u(6)):]}
	if r.Status != 0 {
		return r, &RenderError{r.Status, r.Offset, string(r.Body)}
	}
	return r, nil
}
