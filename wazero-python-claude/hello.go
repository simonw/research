package main

import (
	"context"
	"fmt"
	"log"

	"github.com/tetratelabs/wazero"
)

func main() {
	// Create a WASM module in binary format that exports an "add" function
	// This is the binary representation of:
	// (module
	//   (func (export "add") (param i32 i32) (result i32)
	//     local.get 0
	//     local.get 1
	//     i32.add))
	wasmBinary := []byte{
		0x00, 0x61, 0x73, 0x6d, // WASM magic number
		0x01, 0x00, 0x00, 0x00, // WASM version
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

	// Create a new WebAssembly Runtime
	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	// Instantiate the module
	mod, err := r.Instantiate(ctx, wasmBinary)
	if err != nil {
		log.Fatalf("Failed to instantiate module: %v", err)
	}
	defer mod.Close(ctx)

	// Call the "add" function
	addFunc := mod.ExportedFunction("add")
	if addFunc == nil {
		log.Fatal("add function not exported")
	}

	results, err := addFunc.Call(ctx, 5, 7)
	if err != nil {
		log.Fatalf("Failed to call add: %v", err)
	}

	fmt.Printf("Hello from Wazero! 5 + 7 = %d\n", results[0])
}
