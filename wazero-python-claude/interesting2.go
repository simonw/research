package main

import (
	"context"
	"fmt"
	"log"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

func main() {
	ctx := context.Background()

	// Create a runtime
	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	// Create a more interesting WASM module with multiple functions
	// This module exports: add, subtract, multiply, divide
	wasmBinary := []byte{
		0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // WASM header
		// Type section
		0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
		// Function section - 4 functions
		0x03, 0x05, 0x04, 0x00, 0x00, 0x00, 0x00,
		// Memory section - 1 page
		0x05, 0x03, 0x01, 0x00, 0x01,
		// Export section
		0x07, 0x2b, 0x05,
		// Export "add"
		0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
		// Export "subtract"
		0x08, 0x73, 0x75, 0x62, 0x74, 0x72, 0x61, 0x63, 0x74, 0x00, 0x01,
		// Export "multiply"
		0x08, 0x6d, 0x75, 0x6c, 0x74, 0x69, 0x70, 0x6c, 0x79, 0x00, 0x02,
		// Export "divide"
		0x06, 0x64, 0x69, 0x76, 0x69, 0x64, 0x65, 0x00, 0x03,
		// Export "memory"
		0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00,
		// Code section
		0x0a, 0x21, 0x04,
		// add: local.get 0, local.get 1, i32.add
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
		// subtract: local.get 0, local.get 1, i32.sub
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6b, 0x0b,
		// multiply: local.get 0, local.get 1, i32.mul
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6c, 0x0b,
		// divide: local.get 0, local.get 1, i32.div_s
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6d, 0x0b,
	}

	// Instantiate the module
	mod, err := r.Instantiate(ctx, wasmBinary)
	if err != nil {
		log.Fatalf("Failed to instantiate module: %v", err)
	}
	defer mod.Close(ctx)

	// Test all arithmetic operations
	operations := []struct {
		name string
		a, b uint64
	}{
		{"add", 42, 8},
		{"subtract", 100, 37},
		{"multiply", 12, 7},
		{"divide", 144, 12},
	}

	fmt.Println("=== Testing Arithmetic Operations ===")
	for _, op := range operations {
		fn := mod.ExportedFunction(op.name)
		if fn == nil {
			log.Fatalf("%s function not exported", op.name)
		}

		results, err := fn.Call(ctx, op.a, op.b)
		if err != nil {
			log.Fatalf("Failed to call %s: %v", op.name, err)
		}
		fmt.Printf("%s(%d, %d) = %d\n", op.name, op.a, op.b, int32(results[0]))
	}

	// Demonstrate memory operations
	fmt.Println("\n=== Testing Memory Operations ===")
	memory := mod.Memory()
	if memory == nil {
		log.Fatal("memory not exported")
	}

	// Write different data types to memory
	testMessage := []byte("Wazero is awesome! 🚀")
	if !memory.Write(0, testMessage) {
		log.Fatal("failed to write message to memory")
	}

	// Write some integers to memory at different offsets
	numbers := []byte{0x42, 0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00} // 66 and 100 as little-endian i32
	if !memory.Write(100, numbers) {
		log.Fatal("failed to write numbers to memory")
	}

	// Read back the message
	readMsg, ok := memory.Read(0, uint32(len(testMessage)))
	if !ok {
		log.Fatal("failed to read message from memory")
	}
	fmt.Printf("Message from memory: %s\n", string(readMsg))

	// Read back the numbers
	readNums, ok := memory.Read(100, 8)
	if !ok {
		log.Fatal("failed to read numbers from memory")
	}
	fmt.Printf("Numbers from memory (raw bytes): %v\n", readNums)

	// Show memory statistics
	fmt.Printf("\nMemory size: %d bytes (%d KB, %d pages)\n",
		memory.Size(),
		memory.Size()/1024,
		memory.Size()/65536)

	// Test function composition
	fmt.Println("\n=== Testing Function Composition ===")
	addFn := mod.ExportedFunction("add")
	mulFn := mod.ExportedFunction("multiply")

	// Calculate (5 + 3) * 4
	sum, _ := addFn.Call(ctx, 5, 3)
	product, _ := mulFn.Call(ctx, sum[0], 4)
	fmt.Printf("(5 + 3) × 4 = %d\n", product[0])

	// List available functions
	fmt.Println("\n=== Available Functions ===")
	funcNames := []string{"add", "subtract", "multiply", "divide"}
	for _, name := range funcNames {
		fn := mod.ExportedFunction(name)
		if fn != nil {
			def := fn.Definition()
			params := def.ParamTypes()
			results := def.ResultTypes()
			fmt.Printf("- %s: (%v) -> (%v)\n", name, formatTypes(params), formatTypes(results))
		}
	}
}

func formatTypes(types []api.ValueType) string {
	if len(types) == 0 {
		return ""
	}
	result := ""
	for i, t := range types {
		if i > 0 {
			result += ", "
		}
		switch t {
		case api.ValueTypeI32:
			result += "i32"
		case api.ValueTypeI64:
			result += "i64"
		case api.ValueTypeF32:
			result += "f32"
		case api.ValueTypeF64:
			result += "f64"
		default:
			result += "unknown"
		}
	}
	return result
}
