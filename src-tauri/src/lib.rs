pub mod ai;
pub mod ble;
pub mod crypto;
pub mod protocol;
pub mod wifi;

use std::path::PathBuf;

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadRecordingRequest {
    endpoint: String,
    host: String,
    file_id: u32,
    #[serde(default)]
    output_directory: String,
}

#[tauri::command]
async fn download_recording(
    app: AppHandle,
    state: State<'_, ble::BleState>,
    request: DownloadRecordingRequest,
) -> Result<wifi::DownloadResult, String> {
    let output_directory = if request.output_directory.trim().is_empty() {
        app.path()
            .document_dir()
            .map_err(|error| format!("cannot resolve the Documents directory: {error}"))?
            .join("UnboundBean")
    } else {
        PathBuf::from(request.output_directory.trim())
    };
    let output_path = output_directory.join(format!("{}.opus.bin", request.file_id));
    let session_key = ble::session_key(&state).await?;

    wifi::download_and_decrypt(
        &request.endpoint,
        &request.host,
        request.file_id,
        &output_path,
        &session_key,
    )
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ble::BleState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            ble::scan_devices,
            ble::connect_device,
            ble::list_recordings,
            ble::prepare_wifi_export,
            ai::summarize_transcript,
            download_recording,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run UnboundBean");
}
