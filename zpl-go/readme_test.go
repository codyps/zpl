//go:build !backend_cgo && !backend_purego && !backend_wazero

package zpl_test

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

// Fenced code blocks: https://spec.commonmark.org/0.31.2/#fenced-code-blocks
var goFence = regexp.MustCompile("(?ms)^```go[ \\t]*\\n(.*?)^```[ \\t]*$")

// TestReadmeExamples builds and runs each README Go block as a main package of
// this module, using the default wasm2go backend. The temporary package
// directory starts with "_", so ./... patterns ignore it if a run is interrupted:
// https://pkg.go.dev/cmd/go#hdr-Package_lists_and_patterns
// A `<!-- readme-test: skip -->` line directly before a fence excludes a block.
func TestReadmeExamples(t *testing.T) {
	gobin, err := exec.LookPath("go")
	if err != nil {
		t.Skip("go command not on PATH")
	}
	readme, err := os.ReadFile("README.md")
	if err != nil {
		t.Fatal(err)
	}
	matches := goFence.FindAllSubmatchIndex(readme, -1)
	if len(matches) == 0 {
		t.Fatal("README has no Go examples")
	}
	for _, match := range matches {
		before := strings.TrimRight(string(readme[:match[0]]), "\n")
		previous := strings.TrimSpace(before[strings.LastIndex(before, "\n")+1:])
		line := bytes.Count(readme[:match[0]], []byte("\n")) + 1
		code := readme[match[2]:match[3]]
		t.Run(fmt.Sprintf("README.md:%d", line), func(t *testing.T) {
			if previous == "<!-- readme-test: skip -->" {
				t.Skip("marked readme-test: skip")
			}
			if !bytes.HasPrefix(code, []byte("package main\n")) {
				t.Fatalf("README.md:%d: Go examples must be complete main packages", line)
			}
			pkg, err := os.MkdirTemp(".", "_readme-")
			if err != nil {
				t.Fatal(err)
			}
			defer os.RemoveAll(pkg)
			if err := os.WriteFile(filepath.Join(pkg, "main.go"), code, 0o644); err != nil {
				t.Fatal(err)
			}
			work := t.TempDir()
			binary := filepath.Join(work, "readme.exe")
			build := exec.Command(gobin, "build", "-o", binary, "./"+filepath.ToSlash(pkg))
			if output, err := build.CombinedOutput(); err != nil {
				t.Fatalf("README.md:%d: go build: %v\n%s", line, err, output)
			}
			run := exec.Command(binary)
			run.Dir = work
			if output, err := run.CombinedOutput(); err != nil {
				t.Fatalf("README.md:%d: %v\n%s", line, err, output)
			}
		})
	}
}
