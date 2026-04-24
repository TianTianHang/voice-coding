mod asr;
mod audio;
mod vad;
mod vad_commands;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(vad_commands::VadRecorderState::new())
        .invoke_handler(tauri::generate_handler![
            greet,
            asr::transcribe,
            asr::transcribe_audio_data,
            vad_commands::start_listening,
            vad_commands::stop_listening,
            vad_commands::get_vad_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
