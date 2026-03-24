use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as StdCommand, Output, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::process::Command as TokioCommand;
use tokio::time::sleep;

#[cfg(desktop)]
use tauri::image::Image;
#[cfg(desktop)]
use tauri::menu::{MenuBuilder, MenuEvent, MenuItem};
#[cfg(desktop)]
use tauri::tray::TrayIconBuilder;

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
use arboard::Clipboard;
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    model_path: String,
    model_profile: String,
    language: String,
    temp_root: String,
    cleanup_interval_seconds: u64,
    transcript_ttl_seconds: u64,
    record_shortcut: String,
    paste_shortcut: String,
    auto_copy_on_completion: bool,
    auto_clear_after_copy: bool,
    prefer_gpu: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            model_path: default_model_path().to_string_lossy().into_owned(),
            model_profile: "small".into(),
            language: "en".into(),
            temp_root: default_temp_root().to_string_lossy().into_owned(),
            cleanup_interval_seconds: 300,
            transcript_ttl_seconds: 900,
            record_shortcut: "CommandOrControl+Shift+Space".into(),
            paste_shortcut: "CommandOrControl+Shift+Enter".into(),
            auto_copy_on_completion: false,
            auto_clear_after_copy: false,
            prefer_gpu: true,
        }
    }
}

impl AppSettings {
    fn sanitized(mut self) -> Self {
        if self.model_profile.trim().is_empty() {
            self.model_profile = "small".into();
        }
        self.language = normalize_language_setting(&self.language);
        if self.temp_root.trim().is_empty() {
            self.temp_root = default_temp_root().to_string_lossy().into_owned();
        }
        if self.record_shortcut.trim().is_empty() {
            self.record_shortcut = "CommandOrControl+Shift+Space".into();
        }
        if self.paste_shortcut.trim().is_empty() {
            self.paste_shortcut = "CommandOrControl+Shift+Enter".into();
        }
        self.cleanup_interval_seconds = self.cleanup_interval_seconds.max(60);
        self.transcript_ttl_seconds = self
            .transcript_ttl_seconds
            .max(self.cleanup_interval_seconds + 60)
            .max(120);
        self
    }

    fn temp_root_path(&self) -> PathBuf {
        PathBuf::from(&self.temp_root)
    }
}

struct RecordingSession {
    job_id: String,
    job_dir: PathBuf,
    raw_path: PathBuf,
    started_at: Instant,
    process: Child,
}

const MIN_CAPTURE_DURATION: Duration = Duration::from_millis(650);
const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "voice-tray";
const TRAY_SHOW_HIDE_ID: &str = "tray-show-hide";
const TRAY_RECORD_TOGGLE_ID: &str = "tray-record-toggle";
const TRAY_PASTE_ID: &str = "tray-paste";
const TRAY_QUIT_ID: &str = "tray-quit";
const TRAY_EVENT_TOGGLE_RECORDING: &str = "tray://toggle-recording";
const TRAY_EVENT_PASTE_TRANSCRIPT: &str = "tray://paste-transcript";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayStatus {
    Idle,
    Ready,
    Recording,
    Processing,
    Error,
}

impl TrayStatus {
    fn from_status_value(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "ready" => Self::Ready,
            "recording" => Self::Recording,
            "processing" => Self::Processing,
            "error" => Self::Error,
            _ => Self::Idle,
        }
    }

    fn record_menu_label(self) -> &'static str {
        match self {
            Self::Recording => "Stop recording",
            Self::Processing => "Transcribing…",
            Self::Error | Self::Idle | Self::Ready => "Start recording",
        }
    }

    fn tooltip(self, has_transcript: bool) -> &'static str {
        match self {
            Self::Recording => "voice: recording",
            Self::Processing => "voice: transcribing",
            Self::Ready => "voice: transcript ready",
            Self::Error => "voice: attention",
            Self::Idle if has_transcript => "voice: transcript ready",
            Self::Idle => "voice: idle",
        }
    }
}

#[cfg(desktop)]
struct TrayHandles {
    show_hide: MenuItem<tauri::Wry>,
    record_toggle: MenuItem<tauri::Wry>,
    paste_transcript: MenuItem<tauri::Wry>,
}

struct SharedState {
    settings: Mutex<AppSettings>,
    recording: Mutex<Option<RecordingSession>>,
    active_transcript: Mutex<String>,
    tray_status: Mutex<TrayStatus>,
    #[cfg(desktop)]
    tray_handles: Mutex<Option<TrayHandles>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticItem {
    level: String,
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    settings: AppSettings,
    diagnostics: Vec<DiagnosticItem>,
    dev_seed_transcript: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendError {
    code: String,
    message: String,
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendResponse {
    ok: bool,
    transcript: Option<String>,
    detected_language: Option<String>,
    device: Option<String>,
    warnings: Option<Vec<String>>,
    error: Option<BackendError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessRecordingResponse {
    job_id: String,
    transcript: String,
    detected_language: Option<String>,
    device: Option<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartRecordingResponse {
    job_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PasteActionResult {
    did_paste: bool,
    strategy: String,
    clipboard_restore: String,
    warning: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendUiStatePayload {
    status: String,
    transcript: String,
}

async fn transcribe_job(
    settings: &AppSettings,
    job_id: String,
    job_dir: PathBuf,
    raw_path: PathBuf,
    mime_type: String,
) -> Result<ProcessRecordingResponse, String> {
    let workspace_root = workspace_root();
    let backend_root = workspace_root.join("python-backend");
    if !backend_root.exists() {
        return Err("Python backend directory is missing.".into());
    }

    let python_command = resolve_python_command(&workspace_root)
        .ok_or_else(|| "Python was not found. Install Python 3 and the backend environment first.".to_string())?;

    let mut command = TokioCommand::new(python_command);
    command
        .current_dir(&backend_root)
        .arg("-m")
        .arg("offline_voice_worker.cli")
        .arg("transcribe")
        .arg("--job-dir")
        .arg(&job_dir)
        .arg("--raw-path")
        .arg(&raw_path)
        .arg("--mime-type")
        .arg(&mime_type)
        .arg("--model-profile")
        .arg(&settings.model_profile)
        .arg("--language")
        .arg(normalize_language_setting(&settings.language))
        .arg("--prefer-gpu")
        .arg(settings.prefer_gpu.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if !settings.model_path.trim().is_empty() {
        command.arg("--model-path").arg(&settings.model_path);
    }

    let output = command
        .output()
        .await
        .map_err(|error| format!("Unable to launch Python backend: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    let parsed: BackendResponse = serde_json::from_str(&stdout).map_err(|error| {
        if stderr.is_empty() {
            format!("Backend returned invalid JSON: {error}")
        } else {
            format!("Backend returned invalid JSON: {error}. Stderr: {stderr}")
        }
    })?;

    if !output.status.success() || !parsed.ok {
        let backend_error = parsed
            .error
            .map(|error| match error.detail {
                Some(detail) if !detail.is_empty() => format!("{}: {} ({detail})", error.code, error.message),
                _ => format!("{}: {}", error.code, error.message),
            })
            .unwrap_or_else(|| {
                if stderr.is_empty() {
                    "Local transcription failed.".into()
                } else {
                    format!("Local transcription failed: {stderr}")
                }
            });
        return Err(backend_error);
    }

    let transcript = parsed
        .transcript
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "The local backend produced an empty transcript.".to_string())?;

    Ok(ProcessRecordingResponse {
        job_id,
        transcript,
        detected_language: parsed.detected_language,
        device: parsed.device,
        warnings: parsed.warnings.unwrap_or_default(),
    })
}

fn start_ffmpeg_recording(raw_path: &Path) -> Result<Child, String> {
    let mut command = StdCommand::new("ffmpeg");
    command.arg("-hide_banner").arg("-loglevel").arg("error").arg("-y");

    #[cfg(target_os = "linux")]
    {
        command.arg("-f").arg("pulse").arg("-i").arg("default");
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = raw_path;
        return Err("Native microphone capture is not implemented for this platform yet.".into());
    }

    command
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg(raw_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command
        .spawn()
        .map_err(|error| format!("Unable to start FFmpeg microphone capture: {error}"))
}

fn stop_ffmpeg_recording(mut process: Child) -> Result<Output, String> {
    if let Some(mut stdin) = process.stdin.take() {
        stdin
            .write_all(b"q\n")
            .map_err(|error| format!("Unable to finalize FFmpeg capture: {error}"))?;
        let _ = stdin.flush();
    }

    process
        .wait_with_output()
        .map_err(|error| format!("Unable to wait for FFmpeg capture to finish: {error}"))
}

fn abort_ffmpeg_recording(mut process: Child) {
    let _ = process.kill();
    let _ = process.wait();
}

fn is_empty_capture_error(stderr: &str) -> bool {
    let normalized = stderr.to_lowercase();
    normalized.contains("nothing was written into output file")
        || normalized.contains("received no packets")
        || normalized.contains("output file is empty")
}

fn looks_like_empty_wav(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.len() <= 44)
        .unwrap_or(true)
}

#[tauri::command]
fn bootstrap(_app: AppHandle, state: State<'_, SharedState>) -> Result<BootstrapPayload, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?
        .clone();
    Ok(BootstrapPayload {
        diagnostics: collect_diagnostics(&settings),
        dev_seed_transcript: dev_seed_transcript(),
        settings,
    })
}

#[tauri::command]
fn sync_backend_state(
    app: AppHandle,
    state: State<'_, SharedState>,
    payload: BackendUiStatePayload,
) -> Result<(), String> {
    *state
        .tray_status
        .lock()
        .map_err(|_| "State lock poisoned".to_string())? = TrayStatus::from_status_value(&payload.status);
    *state
        .active_transcript
        .lock()
        .map_err(|_| "State lock poisoned".to_string())? = payload.transcript;
    update_tray_ui(&app, state.inner())
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, SharedState>,
    settings: AppSettings,
) -> Result<BootstrapPayload, String> {
    let settings = settings.sanitized();
    ensure_temp_root(&settings.temp_root_path())?;
    write_settings_to_disk(&app, &settings)?;
    *state
        .settings
        .lock()
        .map_err(|_| "State lock poisoned".to_string())? = settings.clone();

    Ok(BootstrapPayload {
        diagnostics: collect_diagnostics(&settings),
        dev_seed_transcript: dev_seed_transcript(),
        settings,
    })
}

#[tauri::command]
fn start_recording_session(state: State<'_, SharedState>) -> Result<StartRecordingResponse, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?
        .clone();
    let temp_root = settings.temp_root_path();
    ensure_temp_root(&temp_root)?;

    let mut recording = state
        .recording
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?;
    if recording.is_some() {
        return Err("A recording session is already active.".into());
    }

    let job_id = make_job_id();
    let job_dir = temp_root.join(&job_id);
    fs::create_dir_all(&job_dir).map_err(|error| format!("Unable to create job directory: {error}"))?;
    let raw_path = job_dir.join("raw.wav");

    let process = match start_ffmpeg_recording(&raw_path) {
        Ok(process) => process,
        Err(error) => {
            let _ = fs::remove_dir_all(&job_dir);
            return Err(error);
        }
    };

    *recording = Some(RecordingSession {
        job_id: job_id.clone(),
        job_dir,
        raw_path,
        started_at: Instant::now(),
        process,
    });

    Ok(StartRecordingResponse { job_id })
}

#[tauri::command]
async fn stop_recording_session(state: State<'_, SharedState>) -> Result<ProcessRecordingResponse, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?
        .clone();
    let session = {
        let mut recording = state
            .recording
            .lock()
            .map_err(|_| "State lock poisoned".to_string())?;
        recording
            .take()
            .ok_or_else(|| "No recording session is active.".to_string())?
    };

    let elapsed = session.started_at.elapsed();
    if elapsed < MIN_CAPTURE_DURATION {
        sleep(MIN_CAPTURE_DURATION - elapsed).await;
    }

    let output = stop_ffmpeg_recording(session.process)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let _ = fs::remove_dir_all(&session.job_dir);
        return Err(if is_empty_capture_error(&stderr) {
            "No microphone audio was captured. Record for a bit longer and try again.".into()
        } else if stderr.is_empty() {
            "Unable to finalize the microphone recording.".into()
        } else {
            format!("Unable to finalize the microphone recording: {stderr}")
        });
    }

    if looks_like_empty_wav(&session.raw_path) {
        let _ = fs::remove_dir_all(&session.job_dir);
        return Err("No microphone audio was captured. Record for a bit longer and try again.".into());
    }

    transcribe_job(
        &settings,
        session.job_id,
        session.job_dir,
        session.raw_path,
        "audio/wav".into(),
    )
    .await
}

#[tauri::command]
fn cancel_recording_session(state: State<'_, SharedState>) -> Result<(), String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?
        .clone();
    let session = {
        let mut recording = state
            .recording
            .lock()
            .map_err(|_| "State lock poisoned".to_string())?;
        recording.take()
    };

    if let Some(session) = session {
        abort_ffmpeg_recording(session.process);
        let _ = delete_job_folder(&settings.temp_root_path(), &session.job_id);
    }

    Ok(())
}

#[tauri::command]
async fn process_recording(
    _app: AppHandle,
    state: State<'_, SharedState>,
    audio_bytes: Vec<u8>,
    mime_type: String,
) -> Result<ProcessRecordingResponse, String> {
    if audio_bytes.is_empty() {
        return Err("No audio payload was provided.".into());
    }

    let settings = state
        .settings
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?
        .clone();
    let temp_root = settings.temp_root_path();
    ensure_temp_root(&temp_root)?;

    let job_id = make_job_id();
    let job_dir = temp_root.join(&job_id);
    fs::create_dir_all(&job_dir).map_err(|error| format!("Unable to create job directory: {error}"))?;

    let raw_path = job_dir.join(format!("raw.{}", extension_for_mime(&mime_type)));
    fs::write(&raw_path, audio_bytes).map_err(|error| format!("Unable to write captured audio: {error}"))?;

    transcribe_job(&settings, job_id, job_dir, raw_path, mime_type).await
}

#[tauri::command]
fn delete_job_artifacts(state: State<'_, SharedState>, job_id: String) -> Result<(), String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?
        .clone();
    delete_job_folder(&settings.temp_root_path(), &job_id)
}

#[tauri::command]
fn copy_text_to_clipboard(text: String) -> Result<(), String> {
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        let mut clipboard = Clipboard::new().map_err(|error| format!("Clipboard unavailable: {error}"))?;
        clipboard
            .set_text(text)
            .map_err(|error| format!("Unable to write to clipboard: {error}"))
    }

    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = text;
        Err("Clipboard integration is not implemented for this platform.".into())
    }
}

#[tauri::command]
async fn paste_text_into_focused_app(text: String) -> Result<PasteActionResult, String> {
    if text.trim().is_empty() {
        return Ok(PasteActionResult {
            did_paste: false,
            strategy: "none".into(),
            clipboard_restore: "none".into(),
            warning: Some("No transcript is currently active.".into()),
        });
    }

    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        let previous_clipboard = Clipboard::new()
            .ok()
            .and_then(|mut clipboard| clipboard.get_text().ok());

        let mut clipboard = Clipboard::new().map_err(|error| format!("Clipboard unavailable: {error}"))?;
        clipboard
            .set_text(text)
            .map_err(|error| format!("Unable to update the system clipboard: {error}"))?;
        drop(clipboard);

        sleep(Duration::from_millis(120)).await;
        let (strategy, warning) = simulate_platform_paste()?;

        let clipboard_restore = if let Some(previous_text) = previous_clipboard {
            tauri::async_runtime::spawn(async move {
                sleep(Duration::from_millis(750)).await;
                if let Ok(mut clipboard) = Clipboard::new() {
                    let _ = clipboard.set_text(previous_text);
                }
            });
            "scheduled".into()
        } else {
            "none".into()
        };

        return Ok(PasteActionResult {
            did_paste: true,
            strategy,
            clipboard_restore,
            warning,
        });
    }

    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = text;
        Err("Cross-app paste automation is not implemented for this platform.".into())
    }
}

fn collect_diagnostics(settings: &AppSettings) -> Vec<DiagnosticItem> {
    let mut diagnostics = Vec::new();
    let workspace_root = workspace_root();
    let backend_root = workspace_root.join("python-backend");

    if !backend_root.exists() {
        diagnostics.push(DiagnosticItem {
            level: "error".into(),
            code: "backend-missing".into(),
            message: "The python-backend directory is missing from the project.".into(),
        });
    }

    if find_in_path("ffmpeg").is_none() {
        diagnostics.push(DiagnosticItem {
            level: "error".into(),
            code: "ffmpeg-missing".into(),
            message: "FFmpeg is not installed. Recording can finish, but transcription cannot start until ffmpeg is available.".into(),
        });
    }

    let python_command = resolve_python_command(&workspace_root);
    if python_command.is_none() {
        diagnostics.push(DiagnosticItem {
            level: "error".into(),
            code: "python-missing".into(),
            message: "Python 3 was not found. Install Python and create the backend environment before launching the widget.".into(),
        });
    } else if backend_root.exists() {
        let output = StdCommand::new(python_command.unwrap())
            .current_dir(&backend_root)
            .arg("-c")
            .arg("import faster_whisper, ctranslate2")
            .output();

        match output {
            Ok(output) if output.status.success() => {}
            Ok(output) => diagnostics.push(DiagnosticItem {
                level: "error".into(),
                code: "backend-deps-missing".into(),
                message: format!(
                    "The backend Python dependencies are incomplete: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            }),
            Err(error) => diagnostics.push(DiagnosticItem {
                level: "error".into(),
                code: "backend-check-failed".into(),
                message: format!("Unable to inspect the backend environment: {error}"),
            }),
        }
    }

    if settings.model_path.trim().is_empty() {
        diagnostics.push(DiagnosticItem {
            level: "warning".into(),
            code: "model-path-empty".into(),
            message: "No local model path is configured. The named model profile only works offline if it is already cached on this machine.".into(),
        });
    } else if !Path::new(&settings.model_path).exists() {
        diagnostics.push(DiagnosticItem {
            level: "error".into(),
            code: "model-path-missing".into(),
            message: format!("Configured model path does not exist: {}", settings.model_path),
        });
    }

    if let Err(error) = assert_temp_root_writable(&settings.temp_root_path()) {
        diagnostics.push(DiagnosticItem {
            level: "error".into(),
            code: "temp-root-unwritable".into(),
            message: error,
        });
    }

    if diagnostics.is_empty() {
        diagnostics.push(DiagnosticItem {
            level: "info".into(),
            code: "runtime-ok".into(),
            message: "The runtime checks currently look healthy.".into(),
        });
    }

    diagnostics
}

fn write_settings_to_disk(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Unable to create settings directory: {error}"))?;
    }

    let payload = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Unable to serialize settings: {error}"))?;
    fs::write(path, payload).map_err(|error| format!("Unable to persist settings: {error}"))
}

fn load_settings_from_disk(app: &AppHandle) -> AppSettings {
    let path = match settings_path(app) {
        Ok(path) => path,
        Err(_) => return AppSettings::default(),
    };

    if !path.exists() {
        return AppSettings::default();
    }

    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<AppSettings>(&contents).ok())
        .map(AppSettings::sanitized)
        .unwrap_or_default()
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Unable to resolve app config directory: {error}"))?;
    dir.push("settings.json");
    Ok(dir)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn default_temp_root() -> PathBuf {
    std::env::temp_dir().join("local-voice")
}

fn default_model_path() -> PathBuf {
    let candidate = workspace_root().join("python-backend/models/faster-whisper-small");
    if candidate.exists() {
        candidate
    } else {
        PathBuf::new()
    }
}

fn normalize_language_setting(value: &str) -> String {
    match value.trim().to_lowercase().as_str() {
        "" => "en".into(),
        "auto" => "en".into(),
        "english" => "en".into(),
        "en-us" | "en-gb" => "en".into(),
        other => other.into(),
    }
}

fn dev_seed_transcript() -> Option<String> {
    std::env::var("OFFLINE_VOICE_WIDGET_DEV_TRANSCRIPT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn ensure_temp_root(root: &Path) -> Result<(), String> {
    if root.exists() {
        let metadata = fs::symlink_metadata(root)
            .map_err(|error| format!("Unable to inspect temp root: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("Temp root cannot be a symlink.".into());
        }
    }
    fs::create_dir_all(root).map_err(|error| format!("Unable to create temp root: {error}"))
}

fn assert_temp_root_writable(root: &Path) -> Result<(), String> {
    ensure_temp_root(root)?;
    let probe = root.join(".write-test");
    fs::write(&probe, b"ok").map_err(|error| format!("Temp root is not writable: {error}"))?;
    fs::remove_file(probe).map_err(|error| format!("Temp root cleanup failed: {error}"))
}

fn canonicalize_root(root: &Path) -> Result<PathBuf, String> {
    ensure_temp_root(root)?;
    fs::canonicalize(root).map_err(|error| format!("Unable to canonicalize temp root: {error}"))
}

fn delete_job_folder(root: &Path, job_id: &str) -> Result<(), String> {
    if job_id.trim().is_empty() || job_id.contains('/') || job_id.contains('\\') {
        return Err("Refusing to delete an invalid job identifier.".into());
    }

    let canonical_root = canonicalize_root(root)?;
    let candidate = root.join(job_id);
    if !candidate.exists() {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| format!("Unable to inspect job directory: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("Refusing to delete symlinked job artifacts.".into());
    }

    let canonical_candidate = fs::canonicalize(&candidate)
        .map_err(|error| format!("Unable to canonicalize job directory: {error}"))?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err("Refusing to delete outside the app temp root.".into());
    }

    fs::remove_dir_all(canonical_candidate)
        .map_err(|error| format!("Unable to delete transcript artifacts: {error}"))
}

fn cleanup_stale_jobs(settings: &AppSettings) -> Result<(), String> {
    let root = settings.temp_root_path();
    if !root.exists() {
        return Ok(());
    }

    let ttl = Duration::from_secs(settings.transcript_ttl_seconds);
    let canonical_root = canonicalize_root(&root)?;
    let now = SystemTime::now();

    for entry in fs::read_dir(&root).map_err(|error| format!("Unable to enumerate temp root: {error}"))? {
        let entry = entry.map_err(|error| format!("Unable to read temp root entry: {error}"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Unable to inspect temp entry: {error}"))?;

        if metadata.file_type().is_symlink() {
            continue;
        }

        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        let age = now.duration_since(modified).unwrap_or_default();
        if age <= ttl {
            continue;
        }

        let canonical = fs::canonicalize(&path)
            .map_err(|error| format!("Unable to canonicalize temp entry: {error}"))?;
        if !canonical.starts_with(&canonical_root) {
            continue;
        }

        if metadata.is_dir() {
            fs::remove_dir_all(canonical)
                .map_err(|error| format!("Unable to delete stale temp directory: {error}"))?;
        } else {
            fs::remove_file(canonical)
                .map_err(|error| format!("Unable to delete stale temp file: {error}"))?;
        }
    }

    Ok(())
}

fn cleanup_all_jobs(settings: &AppSettings) -> Result<(), String> {
    let root = settings.temp_root_path();
    if !root.exists() {
        return Ok(());
    }

    let canonical_root = canonicalize_root(&root)?;
    for entry in fs::read_dir(&root).map_err(|error| format!("Unable to enumerate temp root: {error}"))? {
        let entry = entry.map_err(|error| format!("Unable to read temp root entry: {error}"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Unable to inspect temp entry: {error}"))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let canonical = fs::canonicalize(&path)
            .map_err(|error| format!("Unable to canonicalize temp entry: {error}"))?;
        if !canonical.starts_with(&canonical_root) {
            continue;
        }
        if metadata.is_dir() {
            let _ = fs::remove_dir_all(canonical);
        } else {
            let _ = fs::remove_file(canonical);
        }
    }
    Ok(())
}

fn make_job_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("job-{now}-{}", std::process::id())
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    if mime_type.contains("ogg") {
        "ogg"
    } else if mime_type.contains("wav") {
        "wav"
    } else if mime_type.contains("mp4") || mime_type.contains("m4a") {
        "m4a"
    } else {
        "webm"
    }
}

fn find_in_path(executable: &str) -> Option<PathBuf> {
    let candidates: Vec<String> = if cfg!(windows) {
        vec![format!("{executable}.exe"), executable.to_string()]
    } else {
        vec![executable.to_string()]
    };

    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|path| {
            candidates
                .iter()
                .map(|candidate| path.join(candidate))
                .find(|candidate| candidate.is_file())
        })
    })
}

fn resolve_python_command(workspace_root: &Path) -> Option<PathBuf> {
    let candidates = [
        workspace_root.join("python-backend/.venv/bin/python3"),
        workspace_root.join("python-backend/.venv/bin/python"),
        workspace_root.join("python-backend/.venv/Scripts/python.exe"),
    ];

    for candidate in candidates {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    find_in_path("python3").or_else(|| find_in_path("python"))
}

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
fn simulate_platform_paste() -> Result<(String, Option<String>), String> {
    let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|error| {
        format!(
            "Paste automation is unavailable. On Linux/macOS this may require accessibility or input automation permissions. {error}"
        )
    })?;

    #[cfg(target_os = "macos")]
    {
        enigo
            .key(Key::Command, Direction::Press)
            .and_then(|_| enigo.key(Key::Unicode('v'), Direction::Click))
            .and_then(|_| enigo.key(Key::Command, Direction::Release))
            .map_err(|error| format!("Unable to trigger Command+V: {error}"))?;
        return Ok(("command-v".into(), None));
    }

    #[cfg(windows)]
    {
        enigo
            .key(Key::Control, Direction::Press)
            .and_then(|_| enigo.key(Key::Unicode('v'), Direction::Click))
            .and_then(|_| enigo.key(Key::Control, Direction::Release))
            .map_err(|error| format!("Unable to trigger Ctrl+V: {error}"))?;
        return Ok(("ctrl-v".into(), None));
    }

    #[cfg(target_os = "linux")]
    {
        let active_window_class = active_window_class_linux();
        let prefer_terminal_paste = active_window_class
            .as_deref()
            .map(is_terminal_window_class)
            .unwrap_or(false);

        #[cfg(debug_assertions)]
        eprintln!(
            "voice: linux paste target class={active_window_class:?} prefer_terminal_paste={prefer_terminal_paste}"
        );

        if prefer_terminal_paste {
            enigo
                .key(Key::Control, Direction::Press)
                .and_then(|_| enigo.key(Key::Shift, Direction::Press))
                .and_then(|_| enigo.key(Key::Unicode('v'), Direction::Click))
                .and_then(|_| enigo.key(Key::Shift, Direction::Release))
                .and_then(|_| enigo.key(Key::Control, Direction::Release))
                .map_err(|error| format!("Unable to trigger Ctrl+Shift+V: {error}"))?;
            return Ok(("ctrl-shift-v".into(), None));
        }

        enigo
            .key(Key::Control, Direction::Press)
            .and_then(|_| enigo.key(Key::Unicode('v'), Direction::Click))
            .and_then(|_| enigo.key(Key::Control, Direction::Release))
            .map_err(|error| format!("Unable to trigger Ctrl+V: {error}"))?;
        return Ok((
            "ctrl-v".into(),
            active_window_class.is_none().then_some(
                "Linux paste defaulted to Ctrl+V because the active window class could not be detected.".into(),
            ),
        ));
    }

    #[allow(unreachable_code)]
    Err("Paste automation is not implemented for this platform.".into())
}

#[cfg(target_os = "linux")]
fn active_window_class_linux() -> Option<String> {
    let active_window = StdCommand::new("xprop")
        .arg("-root")
        .arg("_NET_ACTIVE_WINDOW")
        .output()
        .ok()?;
    if !active_window.status.success() {
        return None;
    }

    let active_window_stdout = String::from_utf8_lossy(&active_window.stdout);
    let window_id = active_window_stdout
        .split_whitespace()
        .rev()
        .find(|part| part.starts_with("0x"))?;
    if window_id == "0x0" {
        return None;
    }

    let wm_class = StdCommand::new("xprop")
        .arg("-id")
        .arg(window_id)
        .arg("WM_CLASS")
        .output()
        .ok()?;
    if !wm_class.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&wm_class.stdout);
    let value = stdout.split('=').nth(1)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_lowercase())
    }
}

#[cfg(target_os = "linux")]
fn is_terminal_window_class(window_class: &str) -> bool {
    const TERMINAL_MARKERS: &[&str] = &[
        "alacritty",
        "blackbox",
        "com.raggesilver.blackbox",
        "ghostty",
        "gnome-terminal",
        "guake",
        "hyper",
        "io.elementary.terminal",
        "kgx",
        "kitty",
        "konsole",
        "lxterminal",
        "org.gnome.console",
        "org.wezfurlong.wezterm",
        "ptyxis",
        "qterminal",
        "rio",
        "tabby",
        "terminal",
        "terminator",
        "tilix",
        "urxvt",
        "wezterm",
        "xfce4-terminal",
        "xterm",
    ];

    TERMINAL_MARKERS
        .iter()
        .any(|marker| window_class.contains(marker))
}

fn spawn_cleanup_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let settings = {
                let state = app.state::<SharedState>();
                let next_settings = match state.settings.lock() {
                    Ok(guard) => guard.clone(),
                    Err(_) => AppSettings::default(),
                };
                next_settings
            };

            let _ = cleanup_stale_jobs(&settings);
            sleep(Duration::from_secs(settings.cleanup_interval_seconds)).await;
        }
    });
}

#[cfg(desktop)]
fn tray_icon_image(status: TrayStatus) -> Image<'static> {
    let size = 18usize;
    let center = (size as f32 - 1.0) / 2.0;
    let mut rgba = vec![0u8; size * size * 4];

    let (r, g, b, alpha, radius, core_radius) = match status {
        TrayStatus::Idle => (176u8, 186u8, 198u8, 236u8, 5.2f32, 2.5f32),
        TrayStatus::Ready => (97u8, 214u8, 143u8, 255u8, 5.5f32, 2.5f32),
        TrayStatus::Recording => (255u8, 93u8, 93u8, 255u8, 5.6f32, 2.9f32),
        TrayStatus::Processing => (99u8, 187u8, 255u8, 255u8, 5.6f32, 2.2f32),
        TrayStatus::Error => (255u8, 122u8, 154u8, 255u8, 5.6f32, 2.6f32),
    };

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let index = (y * size + x) * 4;

            if distance <= radius {
                let falloff = if distance <= core_radius {
                    1.0
                } else {
                    ((radius - distance) / (radius - core_radius)).clamp(0.0, 1.0)
                };
                let glow_alpha = (alpha as f32 * (0.28 + falloff * 0.72)).round() as u8;
                rgba[index] = r;
                rgba[index + 1] = g;
                rgba[index + 2] = b;
                rgba[index + 3] = glow_alpha;
            }
        }
    }

    if matches!(status, TrayStatus::Processing) {
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - center + 0.65;
                let dy = y as f32 - center - 0.8;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance > 2.1 || distance < 0.9 {
                    continue;
                }
                let index = (y * size + x) * 4;
                rgba[index] = 242;
                rgba[index + 1] = 249;
                rgba[index + 2] = 255;
                rgba[index + 3] = 210;
            }
        }
    }

    Image::new_owned(rgba, size as u32, size as u32)
}

#[cfg(desktop)]
fn emit_tray_request(app: &AppHandle, event_name: &str) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "The main window is unavailable.".to_string())?;
    window
        .emit(event_name, ())
        .map_err(|error| format!("Unable to emit tray event: {error}"))
}

#[cfg(desktop)]
fn toggle_main_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "The main window is unavailable.".to_string())?;
    let is_visible = window
        .is_visible()
        .map_err(|error| format!("Unable to inspect widget visibility: {error}"))?;

    if is_visible {
        window.hide().map_err(|error| format!("Unable to hide the widget: {error}"))?;
    } else {
        window.show().map_err(|error| format!("Unable to show the widget: {error}"))?;
        let _ = window.set_focus();
    }

    let state = app.state::<SharedState>();
    update_tray_ui(app, state.inner())
}

#[cfg(desktop)]
fn update_tray_ui(app: &AppHandle, state: &SharedState) -> Result<(), String> {
    let status = *state
        .tray_status
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?;
    let has_transcript = !state
        .active_transcript
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?
        .trim()
        .is_empty();

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_icon(Some(tray_icon_image(status)))
            .map_err(|error| format!("Unable to update tray icon: {error}"))?;
        let _ = tray.set_tooltip(Some(status.tooltip(has_transcript)));
    }

    let show_hide_label = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .and_then(|window| window.is_visible().ok())
        .map(|visible| if visible { "Hide widget" } else { "Show widget" })
        .unwrap_or("Show widget");

    let tray_handles = state
        .tray_handles
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?;
    if let Some(handles) = tray_handles.as_ref() {
        handles
            .show_hide
            .set_text(show_hide_label)
            .map_err(|error| format!("Unable to update tray menu label: {error}"))?;
        handles
            .record_toggle
            .set_text(status.record_menu_label())
            .map_err(|error| format!("Unable to update tray record label: {error}"))?;
        handles
            .record_toggle
            .set_enabled(status != TrayStatus::Processing)
            .map_err(|error| format!("Unable to update tray record availability: {error}"))?;
        handles
            .paste_transcript
            .set_enabled(has_transcript)
            .map_err(|error| format!("Unable to update tray paste availability: {error}"))?;
    }

    Ok(())
}

#[cfg(desktop)]
fn handle_tray_menu_event(app: &AppHandle, item_id: &str) -> Result<(), String> {
    match item_id {
        TRAY_SHOW_HIDE_ID => toggle_main_window(app),
        TRAY_RECORD_TOGGLE_ID => emit_tray_request(app, TRAY_EVENT_TOGGLE_RECORDING),
        TRAY_PASTE_ID => emit_tray_request(app, TRAY_EVENT_PASTE_TRANSCRIPT),
        TRAY_QUIT_ID => {
            app.exit(0);
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(desktop)]
fn build_tray(app: &AppHandle, state: &SharedState) -> Result<(), String> {
    let show_hide = MenuItem::with_id(app, TRAY_SHOW_HIDE_ID, "Hide widget", true, None::<&str>)
        .map_err(|error| format!("Unable to create tray menu item: {error}"))?;
    let record_toggle =
        MenuItem::with_id(app, TRAY_RECORD_TOGGLE_ID, "Start recording", true, None::<&str>)
            .map_err(|error| format!("Unable to create tray record item: {error}"))?;
    let paste_transcript =
        MenuItem::with_id(app, TRAY_PASTE_ID, "Paste transcript", false, None::<&str>)
            .map_err(|error| format!("Unable to create tray paste item: {error}"))?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "Quit", true, None::<&str>)
        .map_err(|error| format!("Unable to create tray quit item: {error}"))?;

    let menu = MenuBuilder::new(app)
        .item(&show_hide)
        .item(&record_toggle)
        .item(&paste_transcript)
        .separator()
        .item(&quit)
        .build()
        .map_err(|error| format!("Unable to build tray menu: {error}"))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(tray_icon_image(TrayStatus::Idle))
        .tooltip("voice: idle")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event: MenuEvent| {
            let _ = handle_tray_menu_event(app, event.id().as_ref());
        })
        .build(app)
        .map_err(|error| format!("Unable to build the tray icon: {error}"))?;

    *state
        .tray_handles
        .lock()
        .map_err(|_| "State lock poisoned".to_string())? = Some(TrayHandles {
        show_hide,
        record_toggle,
        paste_transcript,
    });

    update_tray_ui(app, state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(SharedState {
            settings: Mutex::new(AppSettings::default()),
            recording: Mutex::new(None),
            active_transcript: Mutex::new(String::new()),
            tray_status: Mutex::new(TrayStatus::Idle),
            #[cfg(desktop)]
            tray_handles: Mutex::new(None),
        })
        .setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;

            let loaded_settings = load_settings_from_disk(app.handle()).sanitized();
            {
                let state = app.state::<SharedState>();
                *state.settings.lock().expect("state lock poisoned") = loaded_settings.clone();
            }

            ensure_temp_root(&loaded_settings.temp_root_path())
                .map_err(std::io::Error::other)?;
            spawn_cleanup_loop(app.handle().clone());

            #[cfg(desktop)]
            {
                let state = app.state::<SharedState>();
                if let Err(error) = build_tray(app.handle(), state.inner()) {
                    eprintln!("Unable to initialize tray icon: {error}");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            sync_backend_state,
            save_settings,
            start_recording_session,
            stop_recording_session,
            cancel_recording_session,
            process_recording,
            delete_job_artifacts,
            copy_text_to_clipboard,
            paste_text_into_focused_app,
        ]);

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
            let state = app_handle.state::<SharedState>();
            let recording = {
                let mut recording = state.recording.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                recording.take()
            };
            if let Some(session) = recording {
                abort_ffmpeg_recording(session.process);
            }

            let settings = match state.settings.lock() {
                Ok(settings) => settings.clone(),
                Err(_) => AppSettings::default(),
            };
            let _ = cleanup_all_jobs(&settings);
        }
        _ => {}
    });
}
