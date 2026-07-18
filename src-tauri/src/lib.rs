mod chat;
mod hardware;
mod inference;
mod models;
mod settings;

use chat::{
    delete_conversation, list_conversations, load_conversation, new_conversation,
    save_conversation,
};
use hardware::HardwareInfo;
use inference::{
    check_inference_health, get_server_status, resolve_llama_binary, start_inference_server,
    stop_inference_server, InferenceServer,
};
use models::{
    cancel_model_download, delete_local_model, get_download_progress, get_models_dir,
    list_model_library, pause_model_download, resume_model_download, start_model_download,
    DownloadManager,
};
use settings::{
    get_inference_settings, reset_inference_settings, save_inference_settings,
};

#[tauri::command]
fn get_hardware_info() -> HardwareInfo {
    hardware::detect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(InferenceServer::new())
        .manage(DownloadManager::new())
        .invoke_handler(tauri::generate_handler![
            get_hardware_info,
            get_server_status,
            start_inference_server,
            stop_inference_server,
            check_inference_health,
            resolve_llama_binary,
            list_model_library,
            get_models_dir,
            start_model_download,
            pause_model_download,
            resume_model_download,
            cancel_model_download,
            get_download_progress,
            delete_local_model,
            list_conversations,
            load_conversation,
            save_conversation,
            delete_conversation,
            new_conversation,
            get_inference_settings,
            save_inference_settings,
            reset_inference_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
