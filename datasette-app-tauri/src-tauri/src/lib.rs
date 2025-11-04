use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State, Window};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetteConfig {
    port: u16,
    url: String,
}

#[derive(Debug)]
pub struct DatasetteState {
    process: Arc<Mutex<Option<Child>>>,
    config: Arc<Mutex<Option<DatasetteConfig>>>,
}

impl DatasetteState {
    fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            config: Arc::new(Mutex::new(None)),
        }
    }
}

// Start the Datasette server
#[tauri::command]
async fn start_datasette(state: State<'_, DatasetteState>) -> Result<DatasetteConfig, String> {
    // Check if already running
    {
        let config_lock = state.config.lock().unwrap();
        if config_lock.is_some() {
            return Ok(config_lock.clone().unwrap());
        }
    }

    // Find a free port (simplified - using a fixed port for POC)
    let port = 8765;

    // For this POC, we'll use the system Python and datasette if available
    // In a real implementation, we'd bundle Python like the Electron version does

    // Try to start datasette
    let child = Command::new("datasette")
        .arg("--port")
        .arg(port.to_string())
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--setting")
        .arg("sql_time_limit_ms")
        .arg("10000")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start datasette: {}. Make sure datasette is installed (pip install datasette)", e))?;

    let config = DatasetteConfig {
        port,
        url: format!("http://127.0.0.1:{}", port),
    };

    // Store the process and config
    {
        let mut process_lock = state.process.lock().unwrap();
        *process_lock = Some(child);
    }
    {
        let mut config_lock = state.config.lock().unwrap();
        *config_lock = Some(config.clone());
    }

    // Wait a moment for the server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

    Ok(config)
}

// Get the Datasette server URL
#[tauri::command]
async fn get_server_url(state: State<'_, DatasetteState>) -> Result<String, String> {
    let config_lock = state.config.lock().unwrap();
    match &*config_lock {
        Some(config) => Ok(config.url.clone()),
        None => Err("Datasette server not started".to_string()),
    }
}

// Stop the Datasette server
#[tauri::command]
async fn stop_datasette(state: State<'_, DatasetteState>) -> Result<(), String> {
    let mut process_lock = state.process.lock().unwrap();
    if let Some(mut child) = process_lock.take() {
        child.kill().map_err(|e| format!("Failed to stop datasette: {}", e))?;
    }

    let mut config_lock = state.config.lock().unwrap();
    *config_lock = None;

    Ok(())
}

// Open a database file
#[tauri::command]
async fn open_database_file(
    path: String,
    state: State<'_, DatasetteState>,
    window: Window,
) -> Result<(), String> {
    // First, stop any running instance
    stop_datasette(state.clone()).await?;

    // Start datasette with the specified database file
    let port = 8765;

    let child = Command::new("datasette")
        .arg(&path)
        .arg("--port")
        .arg(port.to_string())
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--setting")
        .arg("sql_time_limit_ms")
        .arg("10000")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start datasette with database: {}", e))?;

    let config = DatasetteConfig {
        port,
        url: format!("http://127.0.0.1:{}", port),
    };

    // Store the process and config
    {
        let mut process_lock = state.process.lock().unwrap();
        *process_lock = Some(child);
    }
    {
        let mut config_lock = state.config.lock().unwrap();
        *config_lock = Some(config.clone());
    }

    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

    // The frontend will navigate to the URL after receiving the success response
    Ok(())
}

// Check if datasette is available in the system
#[tauri::command]
async fn check_datasette_available() -> Result<bool, String> {
    match Command::new("datasette").arg("--version").output() {
        Ok(output) => Ok(output.status.success()),
        Err(_) => Ok(false),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(DatasetteState::new())
        .invoke_handler(tauri::generate_handler![
            start_datasette,
            get_server_url,
            stop_datasette,
            open_database_file,
            check_datasette_available,
        ])
        .setup(|_app| {
            // Setup code here if needed
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Cleanup when window closes
                let state = window.state::<DatasetteState>();
                let mut process_lock = state.process.lock().unwrap();
                if let Some(mut child) = process_lock.take() {
                    let _ = child.kill();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
