package main

import (
	"context"
	"crypto/rand"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/experimental/sysfs"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

var (
	runtimeDir string
)

func main() {
	var (
		evalCode  string
		jsonlMode bool
	)

	// Determine default runtime directory (next to executable or in current dir)
	execPath, _ := os.Executable()
	defaultRuntimeDir := filepath.Join(filepath.Dir(execPath), "runtime")
	if _, err := os.Stat(defaultRuntimeDir); os.IsNotExist(err) {
		// Try current directory
		defaultRuntimeDir = "runtime"
	}

	flag.StringVar(&runtimeDir, "runtime", defaultRuntimeDir, "Path to runtime directory containing python.wasm and lib/")
	flag.StringVar(&evalCode, "c", "", "Python code to evaluate")
	flag.BoolVar(&jsonlMode, "jsonl", false, "Run in JSONL mode - read JSON requests from stdin")
	flag.Parse()

	// Validate runtime directory
	pythonWasm := filepath.Join(runtimeDir, "python.wasm")
	libDir := filepath.Join(runtimeDir, "lib")

	if _, err := os.Stat(pythonWasm); os.IsNotExist(err) {
		fmt.Fprintf(os.Stderr, "Error: python.wasm not found at %s\n", pythonWasm)
		fmt.Fprintf(os.Stderr, "Use -runtime flag to specify the runtime directory\n")
		os.Exit(1)
	}

	if _, err := os.Stat(libDir); os.IsNotExist(err) {
		fmt.Fprintf(os.Stderr, "Error: lib directory not found at %s\n", libDir)
		os.Exit(1)
	}

	if jsonlMode {
		runJSONLMode(pythonWasm, libDir)
	} else if evalCode != "" {
		runEval(pythonWasm, libDir, evalCode)
	} else if flag.NArg() > 0 {
		// Run a Python file
		runFile(pythonWasm, libDir, flag.Arg(0), flag.Args()[1:])
	} else {
		// Run REPL mode
		runREPL(pythonWasm, libDir)
	}
}

func loadWasm(path string) ([]byte, error) {
	return os.ReadFile(path)
}

func runJSONLMode(pythonWasm, libDir string) {
	ctx := context.Background()

	wasm, err := loadWasm(pythonWasm)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to load python.wasm: %v\n", err)
		os.Exit(1)
	}

	// Run Python with jsonl_runner.py - it handles JSON parsing and maintains state
	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	absLibDir, _ := filepath.Abs(libDir)
	absRuntimeDir, _ := filepath.Abs(runtimeDir)

	fsConfig := wazero.NewFSConfig()
	fsConfig = fsConfig.(sysfs.FSConfig).WithSysFSMount(sysfs.DirFS(absLibDir), "/lib")
	fsConfig = fsConfig.(sysfs.FSConfig).WithSysFSMount(sysfs.DirFS(absRuntimeDir), "/runtime")

	config := wazero.NewModuleConfig().
		WithStdout(os.Stdout).
		WithStderr(os.Stderr).
		WithStdin(os.Stdin).
		WithRandSource(rand.Reader).
		WithEnv("PYTHONHOME", "/lib").
		WithEnv("PYTHONPATH", "/lib").
		WithFSConfig(fsConfig).
		WithArgs("python", "/runtime/jsonl_runner.py")

	_, err = r.InstantiateWithConfig(ctx, wasm, config)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

func runEval(pythonWasm, libDir, code string) {
	ctx := context.Background()

	wasm, err := loadWasm(pythonWasm)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to load python.wasm: %v\n", err)
		os.Exit(1)
	}

	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	compiled, err := r.CompileModule(ctx, wasm)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to compile Python: %v\n", err)
		os.Exit(1)
	}
	defer compiled.Close(ctx)

	output, err := executePython(ctx, r, compiled, libDir, code)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Print(output)
}

func runFile(pythonWasm, libDir, pyFile string, args []string) {
	ctx := context.Background()

	wasm, err := loadWasm(pythonWasm)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to load python.wasm: %v\n", err)
		os.Exit(1)
	}

	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	// Build args
	wasmArgs := []string{"python", pyFile}
	wasmArgs = append(wasmArgs, args...)

	// Mount the lib directory and the directory containing the Python file
	pyFileDir := filepath.Dir(pyFile)
	absLibDir, _ := filepath.Abs(libDir)
	absPyFileDir, _ := filepath.Abs(pyFileDir)

	fsConfig := wazero.NewFSConfig()
	fsConfig = fsConfig.(sysfs.FSConfig).WithSysFSMount(sysfs.DirFS(absLibDir), "/lib")
	fsConfig = fsConfig.(sysfs.FSConfig).WithSysFSMount(sysfs.DirFS(absPyFileDir), "/work")

	config := wazero.NewModuleConfig().
		WithStdout(os.Stdout).
		WithStderr(os.Stderr).
		WithStdin(os.Stdin).
		WithRandSource(rand.Reader).
		WithEnv("PYTHONHOME", "/lib").
		WithEnv("PYTHONPATH", "/lib").
		WithFSConfig(fsConfig).
		WithArgs(wasmArgs...)

	_, err = r.InstantiateWithConfig(ctx, wasm, config)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

func runREPL(pythonWasm, libDir string) {
	ctx := context.Background()

	wasm, err := loadWasm(pythonWasm)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to load python.wasm: %v\n", err)
		os.Exit(1)
	}

	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	absLibDir, _ := filepath.Abs(libDir)
	fsConfig := wazero.NewFSConfig()
	fsConfig = fsConfig.(sysfs.FSConfig).WithSysFSMount(sysfs.DirFS(absLibDir), "/lib")

	config := wazero.NewModuleConfig().
		WithStdout(os.Stdout).
		WithStderr(os.Stderr).
		WithStdin(os.Stdin).
		WithRandSource(rand.Reader).
		WithEnv("PYTHONHOME", "/lib").
		WithEnv("PYTHONPATH", "/lib").
		WithFSConfig(fsConfig).
		WithArgs("python")

	_, err = r.InstantiateWithConfig(ctx, wasm, config)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}

func executePython(ctx context.Context, r wazero.Runtime, compiled wazero.CompiledModule, libDir, code string) (string, error) {
	stdout := &strings.Builder{}
	stderr := &strings.Builder{}

	absLibDir, _ := filepath.Abs(libDir)
	fsConfig := wazero.NewFSConfig()
	fsConfig = fsConfig.(sysfs.FSConfig).WithSysFSMount(sysfs.DirFS(absLibDir), "/lib")

	config := wazero.NewModuleConfig().
		WithStdout(stdout).
		WithStderr(stderr).
		WithRandSource(rand.Reader).
		WithEnv("PYTHONHOME", "/lib").
		WithEnv("PYTHONPATH", "/lib").
		WithFSConfig(fsConfig).
		WithArgs("python", "-c", code).
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
