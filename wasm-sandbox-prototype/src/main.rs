use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

mod sandbox;
use sandbox::WasmSandbox;

/// WASM Sandbox - A self-contained Linux sandbox using wasmtime
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Maximum memory limit in megabytes
    #[arg(long, default_value_t = 64)]
    memory_limit: usize,

    /// Path to the WASM module (if not using embedded)
    #[arg(long)]
    wasm_module: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum Request {
    #[serde(rename = "shell")]
    Shell {
        id: String,
        command: String,
        #[serde(default = "default_timeout")]
        time_limit_ms: u64,
    },
    #[serde(rename = "write_file")]
    WriteFile {
        id: String,
        path: String,
        content: String,
        #[serde(default)]
        encoding: FileEncoding,
    },
    #[serde(rename = "read_file")]
    ReadFile {
        id: String,
        path: String,
        #[serde(default)]
        encoding: FileEncoding,
    },
    #[serde(rename = "reset")]
    Reset { id: String },
    #[serde(rename = "status")]
    Status { id: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum Response {
    #[serde(rename = "shell")]
    Shell {
        id: String,
        stdout: String,
        stderr: String,
        exit_code: i32,
        timed_out: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "write_file")]
    WriteFile {
        id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "read_file")]
    ReadFile {
        id: String,
        success: bool,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    #[serde(rename = "reset")]
    Reset { id: String, success: bool },
    #[serde(rename = "status")]
    Status {
        id: String,
        uptime_ms: u128,
        memory_used_bytes: u64,
        memory_limit_bytes: u64,
        ready: bool,
    },
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
enum FileEncoding {
    #[default]
    Text,
    Base64,
}

fn default_timeout() -> u64 {
    5000
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Determine WASM module path
    let wasm_path = args.wasm_module.unwrap_or_else(|| {
        "guest/target/wasm32-wasip1/release/sandbox_guest.wasm".to_string()
    });

    // Initialize sandbox
    let mut sandbox = WasmSandbox::new(&wasm_path, args.memory_limit)?;
    let start_time = Instant::now();

    eprintln!("WASM Sandbox initialized");
    eprintln!("Memory limit: {} MB", args.memory_limit);
    eprintln!("WASM module: {}", wasm_path);
    eprintln!("Ready to accept commands via stdin (line-delimited JSON)");
    eprintln!();

    // Process stdin line by line
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin);

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => handle_request(&mut sandbox, request, &start_time).await,
            Err(e) => {
                eprintln!("Error parsing request: {}", e);
                continue;
            }
        };

        // Send response as JSON line
        let json = serde_json::to_string(&response)?;
        println!("{}", json);
        std::io::stdout().flush()?;
    }

    Ok(())
}

async fn handle_request(
    sandbox: &mut WasmSandbox,
    request: Request,
    start_time: &Instant,
) -> Response {
    match request {
        Request::Shell {
            id,
            command,
            time_limit_ms,
        } => {
            let timeout = Duration::from_millis(time_limit_ms);
            match sandbox.execute_command(&command, timeout) {
                Ok((stdout, stderr, exit_code)) => Response::Shell {
                    id,
                    stdout,
                    stderr,
                    exit_code,
                    timed_out: false,
                    error: None,
                },
                Err(e) => {
                    let error_msg = e.to_string();
                    let timed_out = error_msg.contains("timeout") || error_msg.contains("time limit");
                    Response::Shell {
                        id,
                        stdout: String::new(),
                        stderr: String::new(),
                        exit_code: -1,
                        timed_out,
                        error: Some(error_msg),
                    }
                }
            }
        }
        Request::WriteFile {
            id,
            path,
            content,
            encoding,
        } => {
            let content_bytes = match encoding {
                FileEncoding::Text => content.into_bytes(),
                FileEncoding::Base64 => match base64::decode(&content) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return Response::WriteFile {
                            id,
                            success: false,
                            error: Some(format!("Base64 decode error: {}", e)),
                        }
                    }
                },
            };

            match sandbox.write_file(&path, &content_bytes) {
                Ok(_) => Response::WriteFile {
                    id,
                    success: true,
                    error: None,
                },
                Err(e) => Response::WriteFile {
                    id,
                    success: false,
                    error: Some(e.to_string()),
                },
            }
        }
        Request::ReadFile { id, path, encoding } => match sandbox.read_file(&path) {
            Ok(content_bytes) => {
                let content = match encoding {
                    FileEncoding::Text => String::from_utf8_lossy(&content_bytes).to_string(),
                    FileEncoding::Base64 => base64::encode(&content_bytes),
                };
                Response::ReadFile {
                    id,
                    success: true,
                    content,
                    error: None,
                }
            }
            Err(e) => Response::ReadFile {
                id,
                success: false,
                content: String::new(),
                error: Some(e.to_string()),
            },
        },
        Request::Reset { id } => {
            match sandbox.reset() {
                Ok(_) => Response::Reset { id, success: true },
                Err(e) => {
                    eprintln!("Reset error: {}", e);
                    Response::Reset { id, success: false }
                }
            }
        }
        Request::Status { id } => {
            let stats = sandbox.get_stats();
            Response::Status {
                id,
                uptime_ms: start_time.elapsed().as_millis(),
                memory_used_bytes: stats.memory_used,
                memory_limit_bytes: stats.memory_limit,
                ready: stats.ready,
            }
        }
    }
}

// Base64 encoding/decoding helper
mod base64 {
    pub fn encode(data: &[u8]) -> String {
        use std::fmt::Write;
        let mut result = String::new();
        for chunk in data.chunks(3) {
            let b1 = chunk[0];
            let b2 = chunk.get(1).copied().unwrap_or(0);
            let b3 = chunk.get(2).copied().unwrap_or(0);

            let _ = write!(
                result,
                "{}{}{}{}",
                encode_byte((b1 >> 2) & 0x3F),
                encode_byte(((b1 & 0x03) << 4) | ((b2 >> 4) & 0x0F)),
                if chunk.len() > 1 {
                    encode_byte(((b2 & 0x0F) << 2) | ((b3 >> 6) & 0x03))
                } else {
                    '='
                },
                if chunk.len() > 2 {
                    encode_byte(b3 & 0x3F)
                } else {
                    '='
                }
            );
        }
        result
    }

    fn encode_byte(b: u8) -> char {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        TABLE[b as usize] as char
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        let s = s.trim_end_matches('=');
        let mut result = Vec::new();
        let chars: Vec<char> = s.chars().collect();

        for chunk in chars.chunks(4) {
            let b1 = decode_char(chunk[0])?;
            let b2 = chunk.get(1).map(|&c| decode_char(c)).transpose()?;
            let b3 = chunk.get(2).map(|&c| decode_char(c)).transpose()?;
            let b4 = chunk.get(3).map(|&c| decode_char(c)).transpose()?;

            result.push((b1 << 2) | (b2.unwrap_or(0) >> 4));
            if let Some(b2) = b2 {
                if let Some(b3) = b3 {
                    result.push((b2 << 4) | (b3 >> 2));
                    if let Some(b4) = b4 {
                        result.push((b3 << 6) | b4);
                    }
                }
            }
        }

        Ok(result)
    }

    fn decode_char(c: char) -> Result<u8, String> {
        match c {
            'A'..='Z' => Ok((c as u8) - b'A'),
            'a'..='z' => Ok((c as u8) - b'a' + 26),
            '0'..='9' => Ok((c as u8) - b'0' + 52),
            '+' => Ok(62),
            '/' => Ok(63),
            _ => Err(format!("Invalid base64 character: {}", c)),
        }
    }
}
