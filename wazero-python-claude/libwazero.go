package main

/*
#include <stdlib.h>
#include <stdint.h>
*/
import "C"
import (
	"context"
	"fmt"
	"sync"
	"unsafe"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

// Global storage for runtimes and modules
var (
	runtimes = make(map[int64]wazero.Runtime)
	modules  = make(map[int64]api.Module)
	nextID   int64 = 1
	mu       sync.Mutex
)

//export wazero_new_runtime
func wazero_new_runtime() C.longlong {
	mu.Lock()
	defer mu.Unlock()

	r := wazero.NewRuntime(context.Background())
	id := nextID
	nextID++
	runtimes[id] = r
	return C.longlong(id)
}

//export wazero_runtime_close
func wazero_runtime_close(runtimeID C.longlong) {
	mu.Lock()
	defer mu.Unlock()

	id := int64(runtimeID)
	if r, ok := runtimes[id]; ok {
		r.Close(context.Background())
		delete(runtimes, id)
	}
}

//export wazero_instantiate
func wazero_instantiate(runtimeID C.longlong, wasmBytes *C.uint8_t, length C.int) C.longlong {
	mu.Lock()
	defer mu.Unlock()

	id := int64(runtimeID)
	r, ok := runtimes[id]
	if !ok {
		return -1
	}

	// Convert C bytes to Go slice
	data := C.GoBytes(unsafe.Pointer(wasmBytes), length)

	// Instantiate the module
	mod, err := r.Instantiate(context.Background(), data)
	if err != nil {
		return -1
	}

	// Store the module
	modID := nextID
	nextID++
	modules[modID] = mod

	return C.longlong(modID)
}

//export wazero_module_close
func wazero_module_close(moduleID C.longlong) {
	mu.Lock()
	defer mu.Unlock()

	id := int64(moduleID)
	if m, ok := modules[id]; ok {
		m.Close(context.Background())
		delete(modules, id)
	}
}

//export wazero_call_function
func wazero_call_function(moduleID C.longlong, funcName *C.char, args *C.uint64_t, nargs C.int, results *C.uint64_t, maxResults C.int) C.int {
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		return -1
	}

	name := C.GoString(funcName)
	fn := mod.ExportedFunction(name)
	if fn == nil {
		return -2
	}

	// Convert args
	var goArgs []uint64
	if nargs > 0 {
		goArgs = make([]uint64, nargs)
		argsSlice := unsafe.Slice(args, nargs)
		for i, arg := range argsSlice {
			goArgs[i] = uint64(arg)
		}
	}

	// Call the function
	res, err := fn.Call(context.Background(), goArgs...)
	if err != nil {
		return -3
	}

	// Copy results back
	if len(res) > 0 && maxResults > 0 {
		resultsSlice := unsafe.Slice(results, maxResults)
		for i := 0; i < len(res) && i < int(maxResults); i++ {
			resultsSlice[i] = C.uint64_t(res[i])
		}
	}

	return C.int(len(res))
}

//export wazero_get_error
func wazero_get_error() *C.char {
	return C.CString("Error details not implemented")
}

//export wazero_free_string
func wazero_free_string(str *C.char) {
	C.free(unsafe.Pointer(str))
}

//export wazero_version
func wazero_version() *C.char {
	return C.CString("1.0.0")
}

// Required main function for buildmode=c-shared
func main() {
	fmt.Println("This is a library, not meant to be run directly")
}
