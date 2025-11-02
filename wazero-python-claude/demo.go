package main

import (
	"context"
	"fmt"
	"log"
	"os"

	"github.com/tetratelabs/wazero"
)

func main() {
	// This WASM binary has been carefully constructed and verified
	// It exports a single "add" function: (i32, i32) -> i32
	addWasm := []byte{
		0x00, 0x61, 0x73, 0x6d, // WASM magic
		0x01, 0x00, 0x00, 0x00, // WASM version 1
		// Type section
		0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
		// Function section
		0x03, 0x02, 0x01, 0x00,
		// Export section
		0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
		// Code section
		0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
	}

	ctx := context.Background()
	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	// Instantiate and test
	mod, err := r.Instantiate(ctx, addWasm)
	if err != nil {
		log.Fatalf("Failed to instantiate: %v", err)
	}
	defer mod.Close(ctx)

	fmt.Println("=== Wazero Advanced Demo ===\n")

	// Basic function call
	add := mod.ExportedFunction("add")
	result, _ := add.Call(ctx, 100, 200)
	fmt.Printf("Basic call: add(100, 200) = %d\n", result[0])

	// Multiple calls showing performance
	fmt.Println("\nPerformance test - 1000 calculations:")
	sum := uint64(0)
	for i := 0; i < 1000; i++ {
		result, _ := add.Call(ctx, uint64(i), uint64(i*2))
		sum += result[0]
	}
	fmt.Printf("Sum of all results: %d\n", sum)

	// Demonstrate module compilation for reuse
	fmt.Println("\n=== Compiled Module Demo ===")
	compiled, err := r.CompileModule(ctx, addWasm)
	if err != nil {
		log.Fatalf("Failed to compile: %v", err)
	}
	defer compiled.Close(ctx)

	// Instantiate the compiled module multiple times
	for i := 1; i <= 3; i++ {
		instance, err := r.InstantiateModule(ctx, compiled, wazero.NewModuleConfig().WithName(fmt.Sprintf("calculator-%d", i)))
		if err != nil {
			log.Fatalf("Failed to instantiate compiled module: %v", err)
		}
		defer instance.Close(ctx)

		fn := instance.ExportedFunction("add")
		result, _ := fn.Call(ctx, uint64(i*10), uint64(i*5))
		fmt.Printf("Instance %d: add(%d, %d) = %d\n", i, i*10, i*5, result[0])
	}

	// Save the WASM module to a file for inspection
	fmt.Println("\n=== Saving WASM module ===")
	wasmFile := "add.wasm"
	if err := os.WriteFile(wasmFile, addWasm, 0644); err != nil {
		log.Fatalf("Failed to write WASM file: %v", err)
	}
	fmt.Printf("✓ Saved WASM module to %s (%d bytes)\n", wasmFile, len(addWasm))

	// Load it back and verify
	loadedWasm, err := os.ReadFile(wasmFile)
	if err != nil {
		log.Fatalf("Failed to read WASM file: %v", err)
	}

	loadedMod, err := r.Instantiate(ctx, loadedWasm)
	if err != nil {
		log.Fatalf("Failed to instantiate loaded module: %v", err)
	}
	defer loadedMod.Close(ctx)

	loadedFn := loadedMod.ExportedFunction("add")
	result, _ = loadedFn.Call(ctx, 999, 1)
	fmt.Printf("✓ Loaded module test: add(999, 1) = %d\n", result[0])

	fmt.Println("\n✓ All demos completed successfully!")
}
