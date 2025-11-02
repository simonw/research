package main

import (
	"context"
	"fmt"
	"log"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

func main() {
	// More interesting example: Fibonacci calculator with memory
	// This WASM module exports:
	// - fibonacci(n) -> returns nth fibonacci number
	// - multiply(a, b) -> returns a * b
	wasmBinary := []byte{
		0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // WASM header
		// Type section: Define function signatures
		0x01, 0x0b, // section size
		0x02,       // 2 types
		// Type 0: (i32, i32) -> i32
		0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
		// Type 1: (i32) -> i32
		0x60, 0x01, 0x7f, 0x01, 0x7f,
		// Function section: Declare functions
		0x03, 0x03, // section size
		0x02,       // 2 functions
		0x00, 0x01, // function 0: type 0, function 1: type 1
		// Memory section: 1 page (64KB)
		0x05, 0x03, 0x01, 0x00, 0x01,
		// Export section
		0x07, 0x17, // section size
		0x03,       // 3 exports
		// Export "multiply"
		0x08, 0x6d, 0x75, 0x6c, 0x74, 0x69, 0x70, 0x6c, 0x79, 0x00, 0x00,
		// Export "fibonacci"
		0x09, 0x66, 0x69, 0x62, 0x6f, 0x6e, 0x61, 0x63, 0x63, 0x69, 0x00, 0x01,
		// Export "memory"
		0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00,
		// Code section
		0x0a, 0x2e, // section size
		0x02,       // 2 functions
		// Function 0: multiply(a, b) -> a * b
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6c, 0x0b,
		// Function 1: fibonacci(n) - iterative implementation
		0x23, 0x01, // 1 local
		0x02, 0x7f, // 2 i32 locals (a, b)
		// if n <= 1 return n
		0x20, 0x00, 0x41, 0x01, 0x4d, 0x04, 0x7f, 0x20, 0x00, 0x05,
		// else: a=0, b=1
		0x41, 0x00, 0x21, 0x01, 0x41, 0x01, 0x21, 0x02,
		// loop n-1 times
		0x03, 0x40,
		0x20, 0x01, 0x20, 0x02, 0x6a, 0x21, 0x03, // temp = a + b
		0x20, 0x02, 0x21, 0x01,                   // a = b
		0x20, 0x03, 0x21, 0x02,                   // b = temp
		0x20, 0x00, 0x41, 0x01, 0x6b, 0x21, 0x00, // n--
		0x20, 0x00, 0x41, 0x01, 0x4f, 0x0d, 0x00, // while n > 1
		0x0b, 0x20, 0x02, 0x0b, 0x0b,
	}

	ctx := context.Background()

	// Create a new WebAssembly Runtime with WASI support
	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	// Instantiate WASI to support system calls (though we're not using them in this example)
	wasi_snapshot_preview1.MustInstantiate(ctx, r)

	// Instantiate our custom module
	mod, err := r.Instantiate(ctx, wasmBinary)
	if err != nil {
		log.Fatalf("Failed to instantiate module: %v", err)
	}
	defer mod.Close(ctx)

	// Test multiply function
	multiplyFunc := mod.ExportedFunction("multiply")
	if multiplyFunc == nil {
		log.Fatal("multiply function not exported")
	}

	results, err := multiplyFunc.Call(ctx, 6, 7)
	if err != nil {
		log.Fatalf("Failed to call multiply: %v", err)
	}
	fmt.Printf("Multiply: 6 × 7 = %d\n", results[0])

	// Test fibonacci function
	fibFunc := mod.ExportedFunction("fibonacci")
	if fibFunc == nil {
		log.Fatal("fibonacci function not exported")
	}

	for n := uint64(0); n <= 10; n++ {
		results, err := fibFunc.Call(ctx, n)
		if err != nil {
			log.Fatalf("Failed to call fibonacci(%d): %v", n, err)
		}
		fmt.Printf("Fibonacci(%d) = %d\n", n, results[0])
	}

	// Demonstrate memory access
	memory := mod.Memory()
	if memory == nil {
		log.Fatal("memory not exported")
	}

	// Write data to memory
	testData := []byte("Hello from WASM memory!")
	ok := memory.Write(0, testData)
	if !ok {
		log.Fatal("failed to write to memory")
	}

	// Read data back from memory
	readBuf, ok := memory.Read(0, uint32(len(testData)))
	if !ok {
		log.Fatal("failed to read from memory")
	}

	fmt.Printf("\nMemory test: Wrote and read back: %s\n", string(readBuf))
	fmt.Printf("Memory size: %d bytes (%d pages)\n", memory.Size(), memory.Size()/65536)
}
