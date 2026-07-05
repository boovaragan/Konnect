#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

//! Konnect — Live Schematic Viewer
//!
//! Watches a .kicad_sch file, renders to SVG via kicad-cli, and displays
//! in a native window with pan/zoom and auto-refresh.
//!
//! Usage: schematic-viewer [path/to/file.kicad_sch]

use notify::{EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

// ─── State ──────────────────────────────────────────────────────────────────

struct ViewerState {
    schematic_path: Mutex<Option<PathBuf>>,
    kicad_cli: Mutex<String>,
}

impl Default for ViewerState {
    fn default() -> Self {
        ViewerState {
            schematic_path: Mutex::new(None),
            kicad_cli: Mutex::new(detect_kicad_cli()),
        }
    }
}

fn detect_kicad_cli() -> String {
    let candidates = [
        r"C:\KiCad\10.0\bin\kicad-cli.exe",
        r"C:\Program Files\KiCad\10.0\bin\kicad-cli.exe",
    ];
    for c in &candidates {
        if Path::new(c).exists() { return c.to_string(); }
    }
    "kicad-cli".to_string()
}

// ─── SVG Rendering ──────────────────────────────────────────────────────────

fn render_to_svg(cli: &str, schematic: &Path) -> Result<String, String> {
    let temp_dir = std::env::temp_dir().join("konnect-viewer");
    std::fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let output = Command::new(cli)
        .args([
            "sch", "export", "svg",
            "--output", temp_dir.to_str().unwrap(),
            schematic.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run kicad-cli: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("kicad-cli failed: {}", stderr));
    }

    let stem = schematic.file_stem().unwrap_or_default().to_string_lossy();
    let svg_path = temp_dir.join(format!("{}.svg", stem));
    std::fs::read_to_string(&svg_path)
        .map_err(|e| format!("Failed to read SVG: {}", e))
}

// ─── Tauri Commands ─────────────────────────────────────────────────────────

#[tauri::command]
fn open_schematic(
    app: AppHandle,
    state: tauri::State<'_, ViewerState>,
    path: String,
) -> Result<String, String> {
    let sch_path = PathBuf::from(&path);
    if !sch_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    *state.schematic_path.lock().unwrap() = Some(sch_path.clone());

    let cli = state.kicad_cli.lock().unwrap().clone();
    let svg = render_to_svg(&cli, &sch_path)?;

    if let Some(window) = app.get_webview_window("main") {
        let name = sch_path.file_name().unwrap_or_default().to_string_lossy();
        let _ = window.set_title(&format!("{} — Schematic Viewer", name));
    }

    // Start file watcher
    let app_handle = app.clone();
    let cli_clone = cli.clone();
    let path_clone = sch_path.clone();
    std::thread::spawn(move || {
        start_file_watcher(app_handle, cli_clone, path_clone);
    });

    Ok(svg)
}

#[tauri::command]
fn refresh(state: tauri::State<'_, ViewerState>) -> Result<String, String> {
    let path = state.schematic_path.lock().unwrap().clone();
    let cli = state.kicad_cli.lock().unwrap().clone();
    match path {
        Some(p) => render_to_svg(&cli, &p),
        None => Err("No schematic loaded".to_string()),
    }
}

#[tauri::command]
fn open_in_kicad(state: tauri::State<'_, ViewerState>) -> Result<(), String> {
    let path = state.schematic_path.lock().unwrap().clone();
    match path {
        Some(p) => {
            let candidates = [
                r"C:\KiCad\10.0\bin\kicad.exe",
                r"C:\Program Files\KiCad\10.0\bin\kicad.exe",
            ];
            let kicad = candidates.iter().find(|c| Path::new(c).exists()).unwrap_or(&"kicad");
            Command::new(kicad).arg(p.to_str().unwrap())
                .spawn().map_err(|e| format!("Failed to launch KiCAD: {}", e))?;
            Ok(())
        }
        None => Err("No schematic loaded".to_string()),
    }
}

// ─── File Watcher ───────────────────────────────────────────────────────────

fn start_file_watcher(app: AppHandle, cli: String, schematic: PathBuf) {
    let app_handle = Arc::new(app);
    let last_event = Arc::new(Mutex::new(Instant::now()));
    let target_name = schematic.file_name().unwrap_or_default().to_os_string();

    let app_ref = app_handle.clone();
    let cli_ref = cli.clone();
    let sch_ref = schematic.clone();
    let last_ref = last_event.clone();

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            // Only trigger on file modifications
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {}
                _ => return,
            }

            // Only trigger for our target file
            let is_our_file = event.paths.iter().any(|p| {
                p.file_name().map(|n| n == target_name).unwrap_or(false)
            });
            if !is_our_file { return; }

            // Debounce
            let mut last = last_ref.lock().unwrap();
            if last.elapsed() < Duration::from_millis(500) { return; }
            *last = Instant::now();
            drop(last);

            // Re-render and emit event
            match render_to_svg(&cli_ref, &sch_ref) {
                Ok(svg) => { let _ = app_ref.emit("schematic-updated", svg); }
                Err(e) => eprintln!("[Viewer] Render error: {}", e),
            }
        }
    }).expect("Failed to create file watcher");

    let watch_dir = schematic.parent().unwrap_or(Path::new("."));
    watcher.watch(watch_dir, RecursiveMode::NonRecursive)
        .expect("Failed to start file watching");

    // Block this thread to keep watcher alive
    loop { std::thread::sleep(Duration::from_secs(3600)); }
}

// ─── Main ───────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_arg = args.get(1).cloned();

    tauri::Builder::default()
        .manage(ViewerState::default())
        .invoke_handler(tauri::generate_handler![open_schematic, refresh, open_in_kicad])
        .setup(move |app| {
            if let Some(ref path) = file_arg {
                let path = path.clone();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(800));
                    if let Some(window) = handle.get_webview_window("main") {
                        let js = format!(
                            "if(window.loadSchematic) window.loadSchematic({});",
                            serde_json::to_string(&path).unwrap_or_default()
                        );
                        let _ = window.eval(&js);
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running schematic viewer");
}
