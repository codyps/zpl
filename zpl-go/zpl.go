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
	GX430t
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
	// retired reports that the instance can no longer render (a wasm2go trap
	// or a wazero cancellation closed it), so callers must replace the engine.
	retired() bool
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

// usable reports whether e is open and its backend instance has not retired.
func (e *Engine) usable() bool {
	e.mu.Lock()
	defer e.mu.Unlock()
	return e.impl != nil && !e.impl.retired()
}

// shared is the lazily created engine behind the package-level Render.
var shared struct {
	mu sync.Mutex
	e  *Engine
}

// Render renders with a package-wide engine that is created on first use, so
// importing the package does no backend setup. A setup failure is returned and
// retried on the next call. If a render retires the instance (a translated Wasm
// trap, or a wazero cancellation), the engine is closed and the next call
// creates a new one. Calls are serialized; use New for parallel rendering or to
// control engine setup and lifetime. Results are the same as Engine.Render.
func Render(ctx context.Context, r Request) (Response, error) {
	shared.mu.Lock()
	defer shared.mu.Unlock()
	if shared.e != nil && !shared.e.usable() {
		_ = shared.e.Close(ctx)
		shared.e = nil
	}
	if shared.e == nil {
		if err := ctx.Err(); err != nil {
			return Response{}, err
		}
		e, err := New(ctx)
		if err != nil {
			return Response{}, err
		}
		shared.e = e
	}
	return shared.e.Render(ctx, r)
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
