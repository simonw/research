use anyhow::{anyhow, Result};
use std::time::Duration;
use wasmtime::*;
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::WasiCtxBuilder;

pub struct StoreData {
    wasi: WasiP1Ctx,
    limits: StoreLimits,
}

pub struct WasmSandbox {
    engine: Engine,
    module: Module,
    store: Store<StoreData>,
    instance: Instance,
    memory_limit_bytes: u64,
}

pub struct SandboxStats {
    pub memory_used: u64,
    pub memory_limit: u64,
    pub ready: bool,
}

impl WasmSandbox {
    pub fn new(wasm_path: &str, memory_limit_mb: usize) -> Result<Self> {
        // Configure the engine with memory limits
        let mut config = Config::new();
        config.wasm_backtrace_details(WasmBacktraceDetails::Enable);

        let engine = Engine::new(&config)?;

        // Load the WASM module
        let module = Module::from_file(&engine, wasm_path)?;

        // Set memory limits
        let memory_limit_bytes = (memory_limit_mb * 1024 * 1024) as u64;

        // Create WASI context
        let wasi = WasiCtxBuilder::new().build_p1();

        // Create a store with StoreData
        let limits = StoreLimits::new(Some(memory_limit_bytes as usize), None);
        let store_data = StoreData { wasi, limits };
        let mut store = Store::new(&engine, store_data);
        store.limiter(|data| &mut data.limits);

        // Create a linker and add WASI Preview 1
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |data: &mut StoreData| &mut data.wasi)?;

        // Instantiate the module with WASI
        let instance = linker.instantiate(&mut store, &module)?;

        let mut sandbox = Self {
            engine,
            module,
            store,
            instance,
            memory_limit_bytes,
        };

        // Initialize the sandbox
        sandbox.initialize()?;

        Ok(sandbox)
    }

    fn initialize(&mut self) -> Result<()> {
        let init = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "init")?;
        init.call(&mut self.store, ())?;
        Ok(())
    }

    pub fn execute_command(
        &mut self,
        command: &str,
        _timeout: Duration,
    ) -> Result<(String, String, i32)> {
        // Get the execute_command function
        let execute_fn = self
            .instance
            .get_typed_func::<(i32, i32), i32>(&mut self.store, "execute_command")?;

        // Allocate memory for the command
        let alloc_fn = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "alloc")?;

        let cmd_bytes = command.as_bytes();
        let cmd_len = cmd_bytes.len() as i32;
        let cmd_ptr = alloc_fn.call(&mut self.store, cmd_len)?;

        // Write command to memory
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow!("failed to find memory export"))?;

        memory.write(&mut self.store, cmd_ptr as usize, cmd_bytes)?;

        // Execute the command
        let result_ptr = execute_fn.call(&mut self.store, (cmd_ptr, cmd_len))?;

        // Read the result JSON
        let result_json = self.read_string_from_memory(result_ptr)?;

        // Parse the JSON result
        let result: serde_json::Value = serde_json::from_str(&result_json)?;

        let stdout = result["stdout"].as_str().unwrap_or("").to_string();
        let stderr = result["stderr"].as_str().unwrap_or("").to_string();
        let exit_code = result["exit_code"].as_i64().unwrap_or(-1) as i32;

        // Clean up allocated memory
        let dealloc_fn = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "dealloc")
            .ok();
        if let Some(dealloc) = dealloc_fn {
            let _ = dealloc.call(&mut self.store, (cmd_ptr, cmd_len));
        }

        Ok((stdout, stderr, exit_code))
    }

    pub fn write_file(&mut self, path: &str, content: &[u8]) -> Result<()> {
        let write_fn = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), i32>(&mut self.store, "write_file")?;

        let alloc_fn = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "alloc")?;

        // Allocate and write path
        let path_bytes = path.as_bytes();
        let path_len = path_bytes.len() as i32;
        let path_ptr = alloc_fn.call(&mut self.store, path_len)?;

        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow!("failed to find memory export"))?;

        memory.write(&mut self.store, path_ptr as usize, path_bytes)?;

        // Allocate and write content
        let content_len = content.len() as i32;
        let content_ptr = alloc_fn.call(&mut self.store, content_len)?;
        memory.write(&mut self.store, content_ptr as usize, content)?;

        // Call write_file
        let result =
            write_fn.call(&mut self.store, (path_ptr, path_len, content_ptr, content_len))?;

        // Clean up
        let dealloc_fn = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "dealloc")
            .ok();
        if let Some(dealloc) = dealloc_fn {
            let _ = dealloc.call(&mut self.store, (path_ptr, path_len));
            let _ = dealloc.call(&mut self.store, (content_ptr, content_len));
        }

        if result != 0 {
            return Err(anyhow!("write_file failed with code: {}", result));
        }

        Ok(())
    }

    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let read_fn = self
            .instance
            .get_typed_func::<(i32, i32), i32>(&mut self.store, "read_file")?;

        let alloc_fn = self
            .instance
            .get_typed_func::<i32, i32>(&mut self.store, "alloc")?;

        // Allocate and write path
        let path_bytes = path.as_bytes();
        let path_len = path_bytes.len() as i32;
        let path_ptr = alloc_fn.call(&mut self.store, path_len)?;

        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow!("failed to find memory export"))?;

        memory.write(&mut self.store, path_ptr as usize, path_bytes)?;

        // Call read_file
        let content_ptr = read_fn.call(&mut self.store, (path_ptr, path_len))?;

        // Read content from memory
        // In a real implementation, we'd need to know the length
        // For now, read until we hit a null or use a fixed size
        let mut content = Vec::new();
        let mut offset = content_ptr as usize;
        let data = memory.data(&self.store);

        // Read up to 1MB or until we find end marker
        for _ in 0..1024 * 1024 {
            if offset >= data.len() {
                break;
            }
            let byte = data[offset];
            if byte == 0 && content.is_empty() {
                break; // Empty file
            }
            content.push(byte);
            offset += 1;

            // Check if this looks like the end of content
            if content.len() > 0 && offset < data.len() && data[offset] == 0 {
                break;
            }
        }

        // Clean up
        let dealloc_fn = self
            .instance
            .get_typed_func::<(i32, i32), ()>(&mut self.store, "dealloc")
            .ok();
        if let Some(dealloc) = dealloc_fn {
            let _ = dealloc.call(&mut self.store, (path_ptr, path_len));
        }

        Ok(content)
    }

    pub fn reset(&mut self) -> Result<()> {
        let reset_fn = self
            .instance
            .get_typed_func::<(), ()>(&mut self.store, "reset")?;
        reset_fn.call(&mut self.store, ())?;
        Ok(())
    }

    pub fn get_stats(&mut self) -> SandboxStats {
        let memory = self.instance.get_memory(&mut self.store, "memory");

        let memory_used = if let Some(mem) = memory {
            (mem.size(&self.store) * 65536) as u64
        } else {
            0
        };

        SandboxStats {
            memory_used,
            memory_limit: self.memory_limit_bytes,
            ready: true,
        }
    }

    fn read_string_from_memory(&mut self, ptr: i32) -> Result<String> {
        let memory = self
            .instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| anyhow!("failed to find memory export"))?;

        let data = memory.data(&self.store);
        let mut bytes = Vec::new();
        let mut offset = ptr as usize;

        // Read until null terminator or reasonable limit
        for _ in 0..10 * 1024 * 1024 {
            if offset >= data.len() {
                break;
            }
            let byte = data[offset];
            if byte == 0 {
                break;
            }
            bytes.push(byte);
            offset += 1;
        }

        Ok(String::from_utf8_lossy(&bytes).to_string())
    }
}

// Implement ResourceLimiter for memory limits
struct StoreLimits {
    memory_size: usize,
}

impl StoreLimits {
    fn new(memory_size: Option<usize>, _table_elements: Option<usize>) -> Self {
        Self {
            memory_size: memory_size.unwrap_or(64 * 1024 * 1024),
        }
    }
}

impl ResourceLimiter for StoreLimits {
    fn memory_growing(&mut self, current: usize, desired: usize, _maximum: Option<usize>) -> Result<bool> {
        Ok(desired <= self.memory_size && desired >= current)
    }

    fn table_growing(&mut self, _current: usize, _desired: usize, _maximum: Option<usize>) -> Result<bool> {
        Ok(true)
    }
}
