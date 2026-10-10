// probe records end-to-end render timings, including copies into Go-owned output.
package main

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/json"
	"flag"
	"fmt"
	zpl "github.com/codyps/zpl/zpl-go"
	"image/png"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"time"
)

type Case struct {
	Name, Input, Expected string
	Format                zpl.Format
	Profile               zpl.Profile
	Width, Height         uint32
	Bench                 bool
	Status                uint32
}
type Result struct {
	Name       string
	Pass       bool
	Detail     string  `json:",omitempty"`
	NS         []int64 `json:",omitempty"`
	Bytes      int
	SHA256     string
	AllocBytes uint64 `json:",omitempty"`
}

func main() {
	manifest := flag.String("cases", "artifacts/cases.json", "oracle case manifest")
	iterations := flag.Int("n", 20, "iterations per warm batch")
	rounds := flag.Int("rounds", 5, "warm batches")
	cold := flag.Bool("cold", false, "initialize and render the first case only")
	bench := flag.Bool("bench", false, "benchmark instead of checking")
	flag.Parse()
	if *iterations < 1 || *rounds < 1 {
		panic("positive counts required")
	}
	data, err := os.ReadFile(*manifest)
	must(err)
	var cases []Case
	must(json.Unmarshal(data, &cases))
	base := filepath.Dir(*manifest)
	ctx := context.Background()
	start := time.Now()
	engine, err := zpl.New(ctx)
	must(err)
	initNS := time.Since(start).Nanoseconds()
	defer engine.Close(ctx)
	results := []Result{}
	failed := 0
	var firstNS int64
	for _, c := range cases {
		if *bench && !c.Bench {
			continue
		}
		input, err := os.ReadFile(filepath.Join(base, c.Input))
		must(err)
		r := zpl.Request{Input: input, Format: c.Format, Profile: c.Profile, Width: c.Width, Height: c.Height}
		start = time.Now()
		got, renderErr := engine.Render(ctx, r)
		elapsed := time.Since(start).Nanoseconds()
		if firstNS == 0 {
			firstNS = elapsed
		}
		expected, err := os.ReadFile(filepath.Join(base, c.Expected))
		must(err)
		want, wantErr := zpl.Decode(expected)
		result := Result{Name: c.Name, Pass: reflect.DeepEqual(got, want) && (renderErr == nil) == (wantErr == nil) && got.Status == c.Status, Bytes: len(got.Body), SHA256: fmt.Sprintf("%x", sha256.Sum256(got.Body))}
		if !result.Pass {
			result.Detail = fmt.Sprintf("got status=%d offset=%d bytes=%d err=%v; want status=%d offset=%d bytes=%d err=%v", got.Status, got.Offset, len(got.Body), renderErr, want.Status, want.Offset, len(want.Body), wantErr)
			failed++
		}
		if got.Status == 0 && c.Format == zpl.PNG {
			img, e := png.Decode(bytes.NewReader(got.Body))
			if e != nil {
				panic(e)
			}
			if uint32(img.Bounds().Dx()) != got.Width || uint32(img.Bounds().Dy()) != got.Height {
				panic("PNG dimension mismatch")
			}
		}
		if *bench && !*cold && result.Pass && renderErr == nil {
			var before, after runtime.MemStats
			runtime.ReadMemStats(&before)
			for batch := 0; batch < *rounds; batch++ {
				start = time.Now()
				for i := 0; i < *iterations; i++ {
					v, e := engine.Render(ctx, r)
					must(e)
					if len(v.Body) != result.Bytes {
						panic("unstable output size")
					}
				}
				result.NS = append(result.NS, time.Since(start).Nanoseconds()/int64(*iterations))
			}
			runtime.ReadMemStats(&after)
			result.AllocBytes = (after.TotalAlloc - before.TotalAlloc) / uint64(*iterations**rounds)
		}
		results = append(results, result)
		if *cold {
			break
		}
	}
	must(json.NewEncoder(os.Stdout).Encode(map[string]any{"init_ns": initNS, "first_render_ns": firstNS, "go": runtime.Version(), "results": results, "failed": failed}))
	if failed > 0 {
		os.Exit(1)
	}
}
func must(err error) {
	if err != nil {
		panic(err)
	}
}
