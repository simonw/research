package main

import (
	"context"
	"fmt"
	"log"

	"github.com/tetratelabs/wazero"
)

func main() {
	ctx := context.Background()

	// Create runtime
	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)

	// Simple calculator with 4 operations
	// Each function takes two i32 parameters and returns one i32 result
	wasmBinary := []byte{
		0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // magic + version
		// Type section: one signature (i32, i32) -> i32
		0x01, 0x07, 0x01,
		0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
		// Function section: 4 functions, all using type 0
		0x03, 0x05, 0x04, 0x00, 0x00, 0x00, 0x00,
		// Export section
		0x07, 0x27, 0x04,
		// "add" -> function 0
		0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
		// "sub" -> function 1
		0x03, 0x73, 0x75, 0x62, 0x00, 0x01,
		// "mul" -> function 2
		0x03, 0x6d, 0x75, 0x6c, 0x00, 0x02,
		// "div" -> function 3
		0x03, 0x64, 0x69, 0x76, 0x00, 0x03,
		// Code section
		0x0a, 0x21, 0x04,
		// Function 0: add (local.get 0; local.get 1; i32.add; end)
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
		// Function 1: sub (local.get 0; local.get 1; i32.sub; end)
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6b, 0x0b,
		// Function 2: mul (local.get 0; local.get 1; i32.mul; end)
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6c, 0x0b,
		// Function 3: div (local.get 0; local.get 1; i32.div_s; end)
		0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6d, 0x0b,
	}

	mod, err := r.Instantiate(ctx, wasmBinary)
	if err != nil {
		log.Fatalf("Failed to instantiate: %v", err)
	}
	defer mod.Close(ctx)

	fmt.Println("=== WebAssembly Calculator Demo ===\n")

	// Test addition
	add := mod.ExportedFunction("add")
	result, _ := add.Call(ctx, 15, 27)
	fmt.Printf("15 + 27 = %d\n", result[0])

	// Test subtraction
	sub := mod.ExportedFunction("sub")
	result, _ = sub.Call(ctx, 100, 42)
	fmt.Printf("100 - 42 = %d\n", result[0])

	// Test multiplication
	mul := mod.ExportedFunction("mul")
	result, _ = mul.Call(ctx, 12, 8)
	fmt.Printf("12 × 8 = %d\n", result[0])

	// Test division
	div := mod.ExportedFunction("div")
	result, _ = div.Call(ctx, 144, 12)
	fmt.Printf("144 ÷ 12 = %d\n", result[0])

	// Complex calculation: (10 + 5) * 4 / 2
	fmt.Println("\nComplex calculation: (10 + 5) × 4 ÷ 2")
	step1, _ := add.Call(ctx, 10, 5)
	fmt.Printf("  Step 1: 10 + 5 = %d\n", step1[0])
	step2, _ := mul.Call(ctx, step1[0], 4)
	fmt.Printf("  Step 2: %d × 4 = %d\n", step1[0], step2[0])
	step3, _ := div.Call(ctx, step2[0], 2)
	fmt.Printf("  Step 3: %d ÷ 2 = %d\n", step2[0], step3[0])

	fmt.Println("\n✓ All operations completed successfully!")
}
