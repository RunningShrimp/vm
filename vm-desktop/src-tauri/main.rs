// Tauri Main Entry Point
// This is the Rust backend for the Tauri-based VM desktop application

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tauri::State;
use vm_desktop::{
    ipc::{VmConfig, VmInstance, VmMetrics},
    AppState, MonitoringService, VmController,
};
use std::path::PathBuf;

#[tauri::command]
async fn list_vms(state: State<'_, Arc<AppState>>) -> Result<Vec<VmInstance>, String> {
    state.vm_controller.list_vms()
}

#[tauri::command]
async fn get_vm(state: State<'_, Arc<AppState>>, id: String) -> Result<Option<VmInstance>, String> {
    state.vm_controller.get_vm(&id)
}

#[tauri::command]
async fn create_vm(
    state: State<'_, Arc<AppState>>,
    config: VmConfig,
) -> Result<VmInstance, String> {
    state.vm_controller.create_vm(config)
}

#[tauri::command]
async fn start_vm(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.vm_controller.start_vm(&id).await
}

#[tauri::command]
async fn stop_vm(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.vm_controller.stop_vm(&id).await
}

#[tauri::command]
async fn pause_vm(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.vm_controller.pause_vm(&id).await
}

#[tauri::command]
async fn resume_vm(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.vm_controller.resume_vm(&id).await
}

#[tauri::command]
async fn delete_vm(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.vm_controller.delete_vm(&id)
}

#[tauri::command]
async fn update_vm_config(
    state: State<'_, Arc<AppState>>,
    config: VmConfig,
) -> Result<VmInstance, String> {
    state.vm_controller.update_vm_config(config)
}

#[tauri::command]
async fn get_vm_metrics(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Option<VmMetrics>, String> {
    state.monitoring.get_metrics(&id)
}

#[tauri::command]
async fn get_all_metrics(state: State<'_, Arc<AppState>>) -> Result<Vec<VmMetrics>, String> {
    state.monitoring.get_all_metrics()
}

#[tauri::command]
async fn set_kernel_path(
    state: State<'_, Arc<AppState>>,
    id: String,
    path: String,
) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    state.vm_controller.set_kernel_path(&id, path_buf)
}

#[tauri::command]
async fn set_start_pc(
    state: State<'_, Arc<AppState>>,
    id: String,
    start_pc: String,
) -> Result<(), String> {
    let pc = u64::from_str_radix(&start_pc.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Invalid start PC: {}", e))?;
    state.vm_controller.set_start_pc(&id, pc)
}

#[tauri::command]
async fn create_snapshot(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    description: String,
) -> Result<String, String> {
    state.vm_controller.create_snapshot(&id, name, description).await
}

#[tauri::command]
async fn restore_snapshot(
    state: State<'_, Arc<AppState>>,
    id: String,
    snapshot_id: String,
) -> Result<(), String> {
    state.vm_controller.restore_snapshot(&id, &snapshot_id).await
}

#[tauri::command]
async fn list_snapshots(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<Vec<vm_core::snapshot::Snapshot>, String> {
    state.vm_controller.list_snapshots(&id).await
}

fn main() {
    let app_state = Arc::new(AppState {
        vm_controller: VmController::new(),
        monitoring: MonitoringService::new(),
    });

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            list_vms,
            get_vm,
            create_vm,
            start_vm,
            stop_vm,
            pause_vm,
            resume_vm,
            delete_vm,
            update_vm_config,
            get_vm_metrics,
            get_all_metrics,
            set_kernel_path,
            set_start_pc,
            create_snapshot,
            restore_snapshot,
            list_snapshots,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
