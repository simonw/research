package main

import (
	"bufio"
	"context"
	"crypto/rand"
	_ "embed"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

//go:embed qjs.wasm
var qjsWasm []byte

func main() {
	var (
		evalCode string
		jsonlMode bool
	)

	flag.StringVar(&evalCode, "e", "", "JavaScript code to evaluate")
	flag.BoolVar(&jsonlMode, "jsonl", false, "Run in JSONL mode - read JSON requests from stdin")
	flag.Parse()

	if jsonlMode {
		runJSONLMode()
	} else if evalCode != "" {
		runEval(evalCode)
	} else {
		// Run REPL mode
		runREPL()
	}
}

// JSONLRequest represents an incoming JSONL request
type jsonlRequest struct {
	ID   string `json:"id"`
	Code string `json:"code"`
}

// JSONLResponse represents a JSONL response
type jsonlResponse struct {
	ID     string `json:"id"`
	Output string `json:"output,omitempty"`
	Error  string `json:"error,omitempty"`
}

func runJSONLMode() {
	ctx := context.Background()

	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	// Compile module once
	compiled, err := r.CompileModule(ctx, qjsWasm)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to compile QuickJS: %v\n", err)
		os.Exit(1)
	}
	defer compiled.Close(ctx)

	// Track all previous code for state persistence
	var previousCode strings.Builder
	scanner := bufio.NewScanner(os.Stdin)

	for scanner.Scan() {
		line := scanner.Text()
		if strings.TrimSpace(line) == "" {
			continue
		}

		var req jsonlRequest
		if err := json.Unmarshal([]byte(line), &req); err != nil {
			resp := jsonlResponse{
				ID:    req.ID,
				Error: fmt.Sprintf("Invalid JSON: %v", err),
			}
			outputJSON, _ := json.Marshal(resp)
			fmt.Println(string(outputJSON))
			continue
		}

		// Build the complete code: previous code (without output) + new code
		// Suppress output for previous code, then run new code
		var fullCode strings.Builder

		if previousCode.Len() > 0 {
			// Disable output functions, run previous code, then restore
			fullCode.WriteString("var __oldLog = console.log;\n")
			fullCode.WriteString("var __oldPrint = globalThis.print;\n")
			fullCode.WriteString("console.log = function() {};\n")
			fullCode.WriteString("globalThis.print = function() {};\n")
			fullCode.WriteString(previousCode.String())
			fullCode.WriteString("\nconsole.log = __oldLog;\n")
			fullCode.WriteString("globalThis.print = __oldPrint;\n")
		}

		// Add the new code
		fullCode.WriteString(req.Code)

		output, runErr := executeJS(ctx, r, compiled, fullCode.String())

		resp := jsonlResponse{ID: req.ID}
		if runErr != nil {
			resp.Error = runErr.Error()
		} else {
			resp.Output = output
			// Only add to previous code if execution succeeded
			previousCode.WriteString(req.Code)
			previousCode.WriteString("\n")
		}

		outputJSON, _ := json.Marshal(resp)
		fmt.Println(string(outputJSON))
	}
}

func runEval(code string) {
	ctx := context.Background()

	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	compiled, err := r.CompileModule(ctx, qjsWasm)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to compile QuickJS: %v\n", err)
		os.Exit(1)
	}
	defer compiled.Close(ctx)

	output, err := executeJS(ctx, r, compiled, code)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Print(output)
}

func runREPL() {
	ctx := context.Background()

	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	// Run interactive REPL by passing stdin through to qjs
	stdout := &strings.Builder{}
	stderr := &strings.Builder{}

	config := wazero.NewModuleConfig().
		WithStdout(io.MultiWriter(os.Stdout, stdout)).
		WithStderr(io.MultiWriter(os.Stderr, stderr)).
		WithStdin(os.Stdin).
		WithRandSource(rand.Reader).
		WithArgs("qjs")

	_, err := r.InstantiateWithConfig(ctx, qjsWasm, config)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

func executeJS(ctx context.Context, r wazero.Runtime, compiled wazero.CompiledModule, code string) (string, error) {
	stdout := &strings.Builder{}
	stderr := &strings.Builder{}

	config := wazero.NewModuleConfig().
		WithStdout(stdout).
		WithStderr(stderr).
		WithRandSource(rand.Reader).
		WithArgs("qjs", "-e", code).
		WithName("") // Empty name allows multiple instantiations

	mod, err := r.InstantiateModule(ctx, compiled, config)
	if err != nil {
		// Check if there's stderr output
		if stderr.Len() > 0 {
			return "", fmt.Errorf("%s", strings.TrimSpace(stderr.String()))
		}
		return "", err
	}
	defer mod.Close(ctx)

	return stdout.String(), nil
}
