package zpl

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"testing"
)

func TestOracleAndLifetime(t *testing.T) {
	data, err := os.ReadFile("testdata/cases.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Name, Input, OracleSHA256 string
		Format                    Format
		Profile                   Profile
		Width, Height             uint32
		Status                    uint32
	}
	if err = json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	ctx := context.Background()
	e, err := New(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close(ctx)
	for _, c := range cases {
		t.Run(c.Name, func(t *testing.T) {
			input, err := os.ReadFile(filepath.Join("testdata", c.Input))
			if err != nil {
				t.Fatal(err)
			}
			got, gotErr := e.Render(ctx, Request{input, c.Format, c.Profile, c.Width, c.Height})
			// Reconstruct the canonical wire packet and compare its SHA-256 with
			// the fixed native-oracle digest. This covers every output byte and
			// metadata field without storing large encoded golden images.
			var packet []byte
			for _, n := range []uint32{1, got.Status, got.Offset, got.Width, got.Height, got.Labels, uint32(len(got.Warnings)), uint32(len(got.Body))} {
				packet = binary.LittleEndian.AppendUint32(packet, n)
			}
			packet = append(packet, got.Warnings...)
			packet = append(packet, got.Body...)
			if fmt.Sprintf("%x", sha256.Sum256(packet)) != c.OracleSHA256 || got.Status != c.Status || (gotErr == nil) != (c.Status == 0) {
				t.Fatalf("native oracle mismatch: status=%d want=%d err=%v", got.Status, c.Status, gotErr)
			}
			if gotErr != nil {
				var diagnostic *RenderError
				if !errors.As(gotErr, &diagnostic) || diagnostic.Status != got.Status || diagnostic.Offset != got.Offset || diagnostic.Message != string(got.Body) {
					t.Fatalf("inconsistent public error: %v", gotErr)
				}
			}
		})
	}
	r := Request{Input: []byte("^XA^FO10,10^GB30,30,2^FS^XZ"), Format: PNG, Profile: Specification, Width: 100, Height: 100}
	first, err := e.Render(ctx, r)
	if err != nil {
		t.Fatal(err)
	}
	saved := append([]byte(nil), first.Body...)
	var wg sync.WaitGroup
	for i := 0; i < 4; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 5; j++ {
				v, err := e.Render(ctx, r)
				if err != nil || !bytes.Equal(v.Body, saved) {
					t.Errorf("concurrent render: %v", err)
				}
			}
		}()
	}
	wg.Wait()
	canceled, cancel := context.WithCancel(ctx)
	cancel()
	if _, err = e.Render(canceled, r); !errors.Is(err, context.Canceled) {
		t.Fatal("canceled context accepted", err)
	}
	if err = e.Close(ctx); err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first.Body, saved) {
		t.Fatal("result did not survive engine close")
	}
	if _, err = e.Render(ctx, r); err == nil {
		t.Fatal("closed engine accepted render")
	}
	if err = e.Close(ctx); err != nil {
		t.Fatal("second close", err)
	}
}
func TestDecodeRejectsInvalidWire(t *testing.T) {
	for _, p := range [][]byte{nil, make([]byte, 31), make([]byte, 32), append([]byte{1}, make([]byte, 32)...)} {
		if _, err := Decode(p); err == nil {
			t.Fatal("accepted malformed wire")
		}
	}
}

// Native GX430t V56.17.17Z 300-DPI pixels, with original source/origin/canvas.
// Provenance: zpl/tests/fixtures/gx430t-300-v1/{main,dpi-controls}.
func TestGX430tNativeDensityAndStrikes(t *testing.T) {
	ctx := context.Background()
	e, err := New(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close(ctx)
	for _, c := range []struct{ name, hash string }{
		{"ocr", "c46677d9d948b5a1276796021472147d7bb0b6d129c0a60a7d83005606aa47b7"},
		{"font0", "c2a80a21ed2b86bb8a2f2ddcc7e109144eaebc7d234824b78316d513fb19a3c5"},
		{"maxicode", "1296483435c4f030a7e796b54effcbc77a2989dce17890de8507c523c4e72b4e"},
	} {
		t.Run(c.name, func(t *testing.T) {
			input, err := os.ReadFile("testdata/inputs/gx430t-" + c.name + ".zpl")
			if err != nil {
				t.Fatal(err)
			}
			got, err := e.Render(ctx, Request{Input: input, Format: Gray8, Profile: GX430t, Width: 832, Height: 2558})
			if err != nil {
				t.Fatal(err)
			}
			if got.Width != 832 || got.Height != 2558 || len(got.Body) != 832*2558 || fmt.Sprintf("%x", sha256.Sum256(got.Body)) != c.hash {
				t.Fatalf("GX430t native raster mismatch: %dx%d", got.Width, got.Height)
			}
		})
	}
}
