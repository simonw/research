use std::collections::HashMap;
use std::sync::Mutex;

// Simple in-memory filesystem for the sandbox
static FILESYSTEM: Mutex<Option<HashMap<String, Vec<u8>>>> = Mutex::new(None);

fn get_fs() -> HashMap<String, Vec<u8>> {
    let mut fs = FILESYSTEM.lock().unwrap();
    if fs.is_none() {
        *fs = Some(HashMap::new());
    }
    fs.as_ref().unwrap().clone()
}

fn set_fs(new_fs: HashMap<String, Vec<u8>>) {
    let mut fs = FILESYSTEM.lock().unwrap();
    *fs = Some(new_fs);
}

/// Initialize the sandbox
#[no_mangle]
pub extern "C" fn init() {
    let mut fs = HashMap::new();
    // Create some initial files
    fs.insert("/etc/hostname".to_string(), b"sandbox".to_vec());
    fs.insert("/tmp/.keep".to_string(), b"".to_vec());
    set_fs(fs);
}

/// Execute a shell command (simplified simulation)
/// Returns JSON: {"stdout": "...", "stderr": "...", "exit_code": 0}
#[no_mangle]
pub extern "C" fn execute_command(cmd_ptr: *const u8, cmd_len: usize) -> *mut u8 {
    let cmd_bytes = unsafe { std::slice::from_raw_parts(cmd_ptr, cmd_len) };
    let cmd = String::from_utf8_lossy(cmd_bytes);

    let result = simulate_command(&cmd);
    let json = serde_json::to_string(&result).unwrap();
    let bytes = json.into_bytes();
    let ptr = bytes.as_ptr() as *mut u8;
    std::mem::forget(bytes);
    ptr
}

/// Get the length of the last result
#[no_mangle]
pub extern "C" fn get_result_len() -> usize {
    // This is a simplified approach - in production, you'd track this properly
    0
}

/// Write a file to the sandbox filesystem
#[no_mangle]
pub extern "C" fn write_file(
    path_ptr: *const u8,
    path_len: usize,
    content_ptr: *const u8,
    content_len: usize,
) -> i32 {
    let path_bytes = unsafe { std::slice::from_raw_parts(path_ptr, path_len) };
    let path = String::from_utf8_lossy(path_bytes).to_string();

    let content = unsafe { std::slice::from_raw_parts(content_ptr, content_len) }.to_vec();

    let mut fs = get_fs();
    fs.insert(path, content);
    set_fs(fs);

    0 // success
}

/// Read a file from the sandbox filesystem
#[no_mangle]
pub extern "C" fn read_file(path_ptr: *const u8, path_len: usize) -> *mut u8 {
    let path_bytes = unsafe { std::slice::from_raw_parts(path_ptr, path_len) };
    let path = String::from_utf8_lossy(path_bytes).to_string();

    let fs = get_fs();
    let content = fs.get(&path).cloned().unwrap_or_default();

    let ptr = content.as_ptr() as *mut u8;
    std::mem::forget(content);
    ptr
}

/// Reset the sandbox to initial state
#[no_mangle]
pub extern "C" fn reset() {
    init();
}

/// Allocate memory for passing data
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Free allocated memory
#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}

// Simulated command execution
fn simulate_command(cmd: &str) -> serde_json::Value {
    let parts: Vec<&str> = cmd.split_whitespace().collect();

    if parts.is_empty() {
        return serde_json::json!({
            "stdout": "",
            "stderr": "Error: empty command",
            "exit_code": 1
        });
    }

    let fs = get_fs();

    match parts[0] {
        "echo" => {
            let output = parts[1..].join(" ");
            serde_json::json!({
                "stdout": format!("{}\n", output),
                "stderr": "",
                "exit_code": 0
            })
        }
        "ls" => {
            let path = parts.get(1).unwrap_or(&"/");
            let mut files: Vec<String> = fs
                .keys()
                .filter(|k| k.starts_with(path))
                .map(|k| k.clone())
                .collect();
            files.sort();
            serde_json::json!({
                "stdout": files.join("\n") + "\n",
                "stderr": "",
                "exit_code": 0
            })
        }
        "cat" => {
            if parts.len() < 2 {
                serde_json::json!({
                    "stdout": "",
                    "stderr": "cat: missing file operand\n",
                    "exit_code": 1
                })
            } else {
                let path = parts[1];
                if let Some(content) = fs.get(path) {
                    serde_json::json!({
                        "stdout": String::from_utf8_lossy(content).to_string(),
                        "stderr": "",
                        "exit_code": 0
                    })
                } else {
                    serde_json::json!({
                        "stdout": "",
                        "stderr": format!("cat: {}: No such file or directory\n", path),
                        "exit_code": 1
                    })
                }
            }
        }
        "pwd" => {
            serde_json::json!({
                "stdout": "/\n",
                "stderr": "",
                "exit_code": 0
            })
        }
        "whoami" => {
            serde_json::json!({
                "stdout": "sandbox\n",
                "stderr": "",
                "exit_code": 0
            })
        }
        "uname" => {
            serde_json::json!({
                "stdout": "Linux\n",
                "stderr": "",
                "exit_code": 0
            })
        }
        _ => {
            serde_json::json!({
                "stdout": "",
                "stderr": format!("{}: command not found\n", parts[0]),
                "exit_code": 127
            })
        }
    }
}
