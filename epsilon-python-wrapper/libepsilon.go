package main

/*
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
*/
import "C"
import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"sync"
	"time"
	"unsafe"

	"github.com/ziggy42/epsilon/epsilon"
)

// Global storage for runtimes and modules
var (
	runtimes     = make(map[int64]*runtimeWrapper)
	modules      = make(map[int64]*moduleWrapper)
	nextID       int64 = 1
	mu           sync.Mutex
	lastError    string
	lastErrorMu  sync.Mutex
)

// runtimeWrapper holds the epsilon runtime and configuration
type runtimeWrapper struct {
	runtime        *epsilon.Runtime
	maxMemoryPages uint32
}

// moduleWrapper holds module instance and metadata
type moduleWrapper struct {
	instance  *epsilon.ModuleInstance
	runtimeID int64
}

func setLastError(err error) {
	lastErrorMu.Lock()
	defer lastErrorMu.Unlock()
	if err != nil {
		lastError = err.Error()
	} else {
		lastError = ""
	}
}

//export epsilon_new_runtime
func epsilon_new_runtime(maxMemoryPages C.uint32_t) C.longlong {
	mu.Lock()
	defer mu.Unlock()

	r := epsilon.NewRuntime()

	id := nextID
	nextID++

	runtimes[id] = &runtimeWrapper{
		runtime:        r,
		maxMemoryPages: uint32(maxMemoryPages),
	}

	setLastError(nil)
	return C.longlong(id)
}

//export epsilon_runtime_close
func epsilon_runtime_close(runtimeID C.longlong) {
	mu.Lock()
	defer mu.Unlock()

	id := int64(runtimeID)
	if _, ok := runtimes[id]; ok {
		// Close all modules belonging to this runtime
		for modID, mod := range modules {
			if mod.runtimeID == id {
				delete(modules, modID)
			}
		}
		delete(runtimes, id)
	}
}

//export epsilon_instantiate
func epsilon_instantiate(runtimeID C.longlong, wasmBytes *C.uint8_t, length C.int) C.longlong {
	mu.Lock()
	defer mu.Unlock()

	id := int64(runtimeID)
	rw, ok := runtimes[id]
	if !ok {
		setLastError(errors.New("runtime not found"))
		return -1
	}

	// Convert C bytes to Go slice
	data := C.GoBytes(unsafe.Pointer(wasmBytes), length)

	// Instantiate the module
	instance, err := rw.runtime.InstantiateModuleFromBytes(data)
	if err != nil {
		setLastError(err)
		return -1
	}

	// Store the module
	modID := nextID
	nextID++
	modules[modID] = &moduleWrapper{
		instance:  instance,
		runtimeID: id,
	}

	setLastError(nil)
	return C.longlong(modID)
}

//export epsilon_instantiate_with_memory_limit
func epsilon_instantiate_with_memory_limit(runtimeID C.longlong, wasmBytes *C.uint8_t, length C.int, maxMemoryPages C.uint32_t) C.longlong {
	mu.Lock()
	defer mu.Unlock()

	id := int64(runtimeID)
	rw, ok := runtimes[id]
	if !ok {
		setLastError(errors.New("runtime not found"))
		return -1
	}

	// Convert C bytes to Go slice
	data := C.GoBytes(unsafe.Pointer(wasmBytes), length)

	// Create imports with memory limit if specified
	var imports map[string]map[string]any
	if maxMemoryPages > 0 {
		maxPages := uint32(maxMemoryPages)
		memType := epsilon.MemoryType{
			Limits: epsilon.Limits{
				Min: 1,
				Max: &maxPages,
			},
		}
		imports = epsilon.NewImportBuilder().
			AddMemory("env", "memory", epsilon.NewMemory(memType)).
			Build()
	}

	// Instantiate the module with or without imports
	var instance *epsilon.ModuleInstance
	var err error

	if imports != nil {
		instance, err = rw.runtime.InstantiateModuleWithImports(bytes.NewReader(data), imports)
	} else {
		instance, err = rw.runtime.InstantiateModuleFromBytes(data)
	}

	if err != nil {
		setLastError(err)
		return -1
	}

	// Store the module
	modID := nextID
	nextID++
	modules[modID] = &moduleWrapper{
		instance:  instance,
		runtimeID: id,
	}

	setLastError(nil)
	return C.longlong(modID)
}

//export epsilon_module_close
func epsilon_module_close(moduleID C.longlong) {
	mu.Lock()
	defer mu.Unlock()

	id := int64(moduleID)
	if _, ok := modules[id]; ok {
		delete(modules, id)
	}
}

//export epsilon_call_function
func epsilon_call_function(moduleID C.longlong, funcName *C.char, args *C.uint64_t, nargs C.int, results *C.uint64_t, maxResults C.int) C.int {
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		setLastError(errors.New("module not found"))
		return -1
	}

	name := C.GoString(funcName)

	// Convert args to Go slice
	var goArgs []any
	if nargs > 0 {
		argsSlice := unsafe.Slice(args, nargs)
		goArgs = make([]any, nargs)
		for i, arg := range argsSlice {
			// Store as int32 for compatibility with common WASM functions
			// For i64 functions, the caller should handle appropriately
			goArgs[i] = int32(arg)
		}
	}

	// Call the function
	res, err := mod.instance.Invoke(name, goArgs...)
	if err != nil {
		setLastError(err)
		return -3
	}

	// Copy results back
	if len(res) > 0 && maxResults > 0 {
		resultsSlice := unsafe.Slice(results, maxResults)
		for i := 0; i < len(res) && i < int(maxResults); i++ {
			switch v := res[i].(type) {
			case int32:
				resultsSlice[i] = C.uint64_t(uint64(uint32(v)))
			case int64:
				resultsSlice[i] = C.uint64_t(uint64(v))
			case float32:
				resultsSlice[i] = C.uint64_t(*(*uint32)(unsafe.Pointer(&v)))
			case float64:
				resultsSlice[i] = C.uint64_t(*(*uint64)(unsafe.Pointer(&v)))
			default:
				resultsSlice[i] = 0
			}
		}
	}

	setLastError(nil)
	return C.int(len(res))
}

//export epsilon_call_function_i64
func epsilon_call_function_i64(moduleID C.longlong, funcName *C.char, args *C.uint64_t, nargs C.int, argTypes *C.uint8_t, results *C.uint64_t, maxResults C.int) C.int {
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		setLastError(errors.New("module not found"))
		return -1
	}

	name := C.GoString(funcName)

	// Convert args to Go slice with proper types
	// argTypes: 0=i32, 1=i64, 2=f32, 3=f64
	var goArgs []any
	if nargs > 0 {
		argsSlice := unsafe.Slice(args, nargs)
		typesSlice := unsafe.Slice(argTypes, nargs)
		goArgs = make([]any, nargs)
		for i, arg := range argsSlice {
			switch typesSlice[i] {
			case 0: // i32
				goArgs[i] = int32(arg)
			case 1: // i64
				goArgs[i] = int64(arg)
			case 2: // f32
				bits := uint32(arg)
				goArgs[i] = *(*float32)(unsafe.Pointer(&bits))
			case 3: // f64
				goArgs[i] = *(*float64)(unsafe.Pointer(&arg))
			default:
				goArgs[i] = int32(arg)
			}
		}
	}

	// Call the function
	res, err := mod.instance.Invoke(name, goArgs...)
	if err != nil {
		setLastError(err)
		return -3
	}

	// Copy results back
	if len(res) > 0 && maxResults > 0 {
		resultsSlice := unsafe.Slice(results, maxResults)
		for i := 0; i < len(res) && i < int(maxResults); i++ {
			switch v := res[i].(type) {
			case int32:
				resultsSlice[i] = C.uint64_t(uint64(uint32(v)))
			case int64:
				resultsSlice[i] = C.uint64_t(uint64(v))
			case float32:
				resultsSlice[i] = C.uint64_t(*(*uint32)(unsafe.Pointer(&v)))
			case float64:
				resultsSlice[i] = C.uint64_t(*(*uint64)(unsafe.Pointer(&v)))
			default:
				resultsSlice[i] = 0
			}
		}
	}

	setLastError(nil)
	return C.int(len(res))
}

//export epsilon_call_function_with_timeout
func epsilon_call_function_with_timeout(moduleID C.longlong, funcName *C.char, args *C.uint64_t, nargs C.int, results *C.uint64_t, maxResults C.int, timeoutMs C.int64_t) C.int {
	// Note: Epsilon doesn't support context cancellation natively
	// This implementation uses a goroutine with timeout, but the WASM execution
	// cannot be interrupted mid-execution. The timeout only applies to the
	// overall operation.

	if timeoutMs <= 0 {
		return epsilon_call_function(moduleID, funcName, args, nargs, results, maxResults)
	}

	type callResult struct {
		count int
		err   error
	}

	resultChan := make(chan callResult, 1)

	// Make copies of the data we need
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		setLastError(errors.New("module not found"))
		return -1
	}

	name := C.GoString(funcName)

	// Convert args
	var goArgs []any
	if nargs > 0 {
		argsSlice := unsafe.Slice(args, nargs)
		goArgs = make([]any, nargs)
		for i, arg := range argsSlice {
			goArgs[i] = int32(arg)
		}
	}

	// Run in goroutine with timeout
	ctx, cancel := context.WithTimeout(context.Background(), time.Duration(timeoutMs)*time.Millisecond)
	defer cancel()

	go func() {
		res, err := mod.instance.Invoke(name, goArgs...)
		if err != nil {
			resultChan <- callResult{count: -3, err: err}
			return
		}

		// Copy results back (in goroutine)
		count := len(res)
		if count > 0 && maxResults > 0 {
			resultsSlice := unsafe.Slice(results, maxResults)
			for i := 0; i < count && i < int(maxResults); i++ {
				switch v := res[i].(type) {
				case int32:
					resultsSlice[i] = C.uint64_t(uint64(uint32(v)))
				case int64:
					resultsSlice[i] = C.uint64_t(uint64(v))
				case float32:
					resultsSlice[i] = C.uint64_t(*(*uint32)(unsafe.Pointer(&v)))
				case float64:
					resultsSlice[i] = C.uint64_t(*(*uint64)(unsafe.Pointer(&v)))
				default:
					resultsSlice[i] = 0
				}
			}
		}
		resultChan <- callResult{count: count, err: nil}
	}()

	select {
	case result := <-resultChan:
		if result.err != nil {
			setLastError(result.err)
		} else {
			setLastError(nil)
		}
		return C.int(result.count)
	case <-ctx.Done():
		setLastError(errors.New("execution timeout"))
		return -4
	}
}

//export epsilon_get_export_names
func epsilon_get_export_names(moduleID C.longlong, buffer *C.char, bufferSize C.int) C.int {
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		setLastError(errors.New("module not found"))
		return -1
	}

	names := mod.instance.ExportNames()

	// Join names with null separator
	var result string
	for i, name := range names {
		if i > 0 {
			result += "\x00"
		}
		result += name
	}

	if bufferSize == 0 {
		// Just return required size
		return C.int(len(result) + 1)
	}

	// Copy to buffer
	resultBytes := []byte(result)
	copyLen := len(resultBytes)
	if copyLen >= int(bufferSize) {
		copyLen = int(bufferSize) - 1
	}

	C.memcpy(unsafe.Pointer(buffer), unsafe.Pointer(&resultBytes[0]), C.size_t(copyLen))
	(*[1 << 30]byte)(unsafe.Pointer(buffer))[copyLen] = 0

	setLastError(nil)
	return C.int(len(names))
}

//export epsilon_get_memory_size
func epsilon_get_memory_size(moduleID C.longlong, memoryName *C.char) C.int32_t {
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		setLastError(errors.New("module not found"))
		return -1
	}

	name := C.GoString(memoryName)
	mem, err := mod.instance.GetMemory(name)
	if err != nil {
		setLastError(err)
		return -1
	}

	setLastError(nil)
	return C.int32_t(mem.Size())
}

//export epsilon_read_memory
func epsilon_read_memory(moduleID C.longlong, memoryName *C.char, offset C.uint32_t, length C.uint32_t, buffer *C.uint8_t) C.int {
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		setLastError(errors.New("module not found"))
		return -1
	}

	name := C.GoString(memoryName)
	mem, err := mod.instance.GetMemory(name)
	if err != nil {
		setLastError(err)
		return -1
	}

	data, err := mem.Get(uint32(offset), 0, uint32(length))
	if err != nil {
		setLastError(err)
		return -2
	}

	// Copy to buffer
	bufSlice := unsafe.Slice((*byte)(unsafe.Pointer(buffer)), length)
	copy(bufSlice, data)

	setLastError(nil)
	return C.int(length)
}

//export epsilon_write_memory
func epsilon_write_memory(moduleID C.longlong, memoryName *C.char, offset C.uint32_t, data *C.uint8_t, length C.uint32_t) C.int {
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		setLastError(errors.New("module not found"))
		return -1
	}

	name := C.GoString(memoryName)
	mem, err := mod.instance.GetMemory(name)
	if err != nil {
		setLastError(err)
		return -1
	}

	// Convert C data to Go slice
	goData := C.GoBytes(unsafe.Pointer(data), C.int(length))

	err = mem.Set(uint32(offset), 0, goData)
	if err != nil {
		setLastError(err)
		return -2
	}

	setLastError(nil)
	return C.int(length)
}

//export epsilon_get_global
func epsilon_get_global(moduleID C.longlong, globalName *C.char, result *C.uint64_t) C.int {
	mu.Lock()
	id := int64(moduleID)
	mod, ok := modules[id]
	mu.Unlock()

	if !ok {
		setLastError(errors.New("module not found"))
		return -1
	}

	name := C.GoString(globalName)
	val, err := mod.instance.GetGlobal(name)
	if err != nil {
		setLastError(err)
		return -1
	}

	switch v := val.(type) {
	case int32:
		*result = C.uint64_t(uint64(uint32(v)))
	case int64:
		*result = C.uint64_t(uint64(v))
	case float32:
		*result = C.uint64_t(*(*uint32)(unsafe.Pointer(&v)))
	case float64:
		*result = C.uint64_t(*(*uint64)(unsafe.Pointer(&v)))
	default:
		setLastError(fmt.Errorf("unsupported global type: %T", val))
		return -2
	}

	setLastError(nil)
	return 0
}

//export epsilon_get_error
func epsilon_get_error() *C.char {
	lastErrorMu.Lock()
	defer lastErrorMu.Unlock()
	if lastError == "" {
		return nil
	}
	return C.CString(lastError)
}

//export epsilon_free_string
func epsilon_free_string(str *C.char) {
	C.free(unsafe.Pointer(str))
}

//export epsilon_version
func epsilon_version() *C.char {
	return C.CString(epsilon.Version)
}

//export epsilon_wrapper_version
func epsilon_wrapper_version() *C.char {
	return C.CString("0.1.0")
}

// Required main function for buildmode=c-shared
func main() {
	fmt.Println("This is a library, not meant to be run directly")
}
