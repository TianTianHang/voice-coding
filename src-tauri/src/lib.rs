mod acp;
mod asr;
mod audio;
mod business;
mod model_paths;
mod tts;
mod vad;
mod vad_commands;

use parking_lot::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseBehavior {
    HideToTray,
    Exit,
}

struct AppLifecycleState {
    close_behavior: Mutex<CloseBehavior>,
}

impl AppLifecycleState {
    fn new() -> Self {
        Self {
            close_behavior: Mutex::new(CloseBehavior::HideToTray),
        }
    }
}

#[tauri::command]
fn set_close_behavior(
    state: tauri::State<'_, AppLifecycleState>,
    behavior: String,
) -> Result<(), String> {
    let next = match behavior.as_str() {
        "hide" => CloseBehavior::HideToTray,
        "exit" => CloseBehavior::Exit,
        other => return Err(format!("Unknown close behavior: {other}")),
    };
    *state.close_behavior.lock() = next;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("backend".into()),
                    }),
                ])
                .rotation_strategy(RotationStrategy::KeepSome(5))
                .max_file_size(2_000_000)
                .level(log::LevelFilter::Info)
                .level_for("voice_coding_lib", log::LevelFilter::Debug)
                .level_for("stt_qwen3", log::LevelFilter::Info)
                .level_for("tts_moss", log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .manage(vad_commands::VadRecorderState::new())
        .manage(vad_commands::VadRuntimeConfigState::new())
        .manage(acp::AcpRuntime::default())
        .manage(business::BusinessRuntime::default())
        .manage(AppLifecycleState::new())
        .setup(|app| {
            log::info!("voice-coding backend setup started");
            app.manage(tts::TtsRuntime::with_app(app.handle()));
            #[cfg(feature = "stt-qwen3")]
            asr::prewarm_asr(app.handle().clone());
            setup_tray(app)?;
            log::info!("voice-coding backend setup finished");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let behavior = *app.state::<AppLifecycleState>().close_behavior.lock();
                match behavior {
                    CloseBehavior::HideToTray => {
                        log::info!("main window close requested; hiding to tray");
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    CloseBehavior::Exit => {
                        log::info!("main window close requested; exiting application");
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = vad_commands::stop_listening_runtime(
                                app.clone(),
                                app.state::<vad_commands::VadRecorderState>(),
                            );
                            let runtime = app.state::<acp::AcpRuntime>();
                            let _ = runtime.disconnect(app.clone()).await;
                            app.exit(0);
                        });
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            asr::debug_prepare_asr,
            asr::debug_get_asr_status,
            asr::debug_transcribe,
            asr::debug_transcribe_audio_data,
            asr::debug_streaming_asr,
            tts::debug_prepare_tts,
            tts::debug_get_tts_status,
            tts::debug_synthesize_tts,
            tts::debug_play_tts,
            tts::debug_stream_tts,
            tts::debug_cancel_tts_playback,
            tts::debug_get_auto_tts_status,
            tts::debug_set_auto_tts_enabled,
            tts::debug_stop_auto_tts,
            tts::debug_speak_latest_result,
            business::get_app_status,
            business::prepare_app,
            business::get_app_preferences,
            business::set_app_preferences,
            business::start_voice_session,
            business::stop_voice_session,
            business::pause_voice_session,
            business::resume_voice_session,
            business::get_voice_session_status,
            business::update_voice_session_config,
            business::submit_transcript_to_agent,
            business::edit_and_submit_transcript,
            business::discard_transcript,
            business::send_agent_message,
            business::cancel_agent_turn,
            business::speak_text,
            business::speak_agent_result,
            business::stop_speech,
            business::get_speech_status,
            business::set_speech_preferences,
            vad_commands::debug_start_listening,
            vad_commands::debug_stop_listening,
            vad_commands::debug_get_vad_state,
            vad_commands::debug_get_vad_config,
            vad_commands::debug_set_vad_config,
            acp::session::connect_agent,
            acp::session::disconnect_agent,
            acp::session::get_agent_status,
            acp::session::send_agent_prompt,
            acp::session::respond_agent_confirmation,
            acp::timeline::get_agent_timeline,
            acp::timeline::respond_agent_stream_confirmation,
            set_close_behavior,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "hide" => {
                log::debug!("tray hide requested");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            "quit" => {
                log::info!("tray quit requested");
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = vad_commands::stop_listening_runtime(
                        app.clone(),
                        app.state::<vad_commands::VadRecorderState>(),
                    );
                    let runtime = app.state::<acp::AcpRuntime>();
                    let _ = runtime.disconnect(app.clone()).await;
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        log::debug!("showing main window");
        let _ = window.show();
        let _ = window.set_focus();
    }
}
