// Public default wasm2go API; constructor and Close are outside render timing.
// https://pkg.go.dev/time#Since and https://pkg.go.dev/runtime#KeepAlive
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"runtime"
	"strconv"
	"time"

	zpl "github.com/codyps/zpl/zpl-go"
)

func positive(value string) int {
	n, err := strconv.Atoi(value)
	if err != nil || n <= 0 {
		panic("expected positive integer")
	}
	return n
}

func dimension(value string) uint32 {
	n, err := strconv.ParseUint(value, 10, 32)
	if err != nil || n == 0 || n > 0xffffffff {
		panic("expected dimension from 1 to 4294967295")
	}
	return uint32(n)
}

func main() {
	ctx := context.Background()
	start := time.Now()
	engine, err := zpl.New(ctx)
	elapsed := time.Since(start).Nanoseconds()
	if err != nil {
		panic(err)
	}
	defer engine.Close(ctx)
	if os.Args[1] == "initialize" {
		if err := json.NewEncoder(os.Stdout).Encode(map[string]any{"iterations": 1, "ns": elapsed, "checksum": 1}); err != nil {
			panic(err)
		}
		return
	}
	input, err := os.ReadFile(os.Args[2])
	if err != nil {
		panic(err)
	}
	width, height := dimension(os.Args[3]), dimension(os.Args[4])
	duration := time.Duration(positive(os.Args[5])) * time.Millisecond
	request := zpl.Request{Input: input, Format: zpl.PNG, Profile: zpl.ZD621, Width: width, Height: height}
	render := func() []byte {
		result, err := engine.Render(ctx, request)
		if err != nil {
			panic(err)
		}
		if result.Labels != 1 || result.Width != width || result.Height != height || len(result.Body) == 0 {
			panic(fmt.Sprint("invalid package output: ", result.Labels, " ", result.Width, "x", result.Height))
		}
		return result.Body
	}
	if err := os.WriteFile(os.Args[6], render(), 0644); err != nil {
		panic(err)
	}
	start = time.Now()
	for time.Since(start) < 250*time.Millisecond {
		runtime.KeepAlive(render())
	}
	var iterations, checksum int64
	start = time.Now()
	for {
		for i := 0; i < 8; i++ {
			body := render()
			checksum += int64(len(body))
			runtime.KeepAlive(body)
		}
		iterations += 8
		if time.Since(start) >= duration {
			break
		}
	}
	elapsed = time.Since(start).Nanoseconds()
	if err := json.NewEncoder(os.Stdout).Encode(map[string]any{"iterations": iterations, "ns": elapsed, "checksum": checksum}); err != nil {
		panic(err)
	}
}
