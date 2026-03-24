import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { PhysicalPosition, currentMonitor, getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
import { register, unregisterAll } from '@tauri-apps/plugin-global-shortcut';
import './styles.css';

type Status = 'idle' | 'recording' | 'processing' | 'error';
type VisualStatus = Status | 'ready';
type NoticeTone = 'info' | 'warning' | 'error';
type ToggleSource = 'button' | 'shortcut';
type PasteSource = 'button' | 'shortcut';

interface AppSettings {
  modelPath: string;
  modelProfile: string;
  language: string;
  tempRoot: string;
  cleanupIntervalSeconds: number;
  transcriptTtlSeconds: number;
  recordShortcut: string;
  pasteShortcut: string;
  autoCopyOnCompletion: boolean;
  autoClearAfterCopy: boolean;
  preferGpu: boolean;
}

interface DiagnosticItem {
  level: 'info' | 'warning' | 'error';
  code: string;
  message: string;
}

interface BootstrapPayload {
  settings: AppSettings;
  diagnostics: DiagnosticItem[];
  devSeedTranscript?: string | null;
}

interface ProcessRecordingResponse {
  jobId: string;
  transcript: string;
  detectedLanguage: string | null;
  device: string | null;
  warnings: string[];
}

interface StartRecordingResponse {
  jobId: string;
}

interface PasteActionResult {
  didPaste: boolean;
  strategy: string;
  clipboardRestore: string;
  warning: string | null;
}

interface AppState {
  status: Status;
  settings: AppSettings | null;
  diagnostics: DiagnosticItem[];
  transcript: string;
  currentJobId: string | null;
  isExpanded: boolean;
  settingsOpen: boolean;
  isPointerInside: boolean;
  hasFocus: boolean;
  isDimmed: boolean;
  notice: { tone: NoticeTone; message: string };
  elapsedSeconds: number;
  recordingStartedAtMs: number | null;
  elapsedTimer: number | null;
  maxRecordTimer: number | null;
  idleDimTimer: number | null;
}

const collapsedWidth = 156;
const collapsedHeight = 58;
const expandedWidth = 360;
const snapThreshold = 42;
const snapInset = 12;
const idleDimDelayMs = 1600;
const toggleDebounceMs = 350;
const minimumShortcutStopMs = 1400;
const pasteShortcutDebounceMs = 700;
const trayToggleRecordingEvent = 'tray://toggle-recording';
const trayPasteTranscriptEvent = 'tray://paste-transcript';
const appWindow = getCurrentWindow();

let dragIntent = false;
let isSnappingWindow = false;
let snapTimer: number | null = null;
let lastToggleAtMs = 0;
let lastPasteAtMs = 0;
let pasteInFlight = false;

const state: AppState = {
  status: 'idle',
  settings: null,
  diagnostics: [],
  transcript: '',
  currentJobId: null,
  isExpanded: false,
  settingsOpen: false,
  isPointerInside: false,
  hasFocus: false,
  isDimmed: false,
  notice: { tone: 'info', message: 'Checking local runtime...' },
  elapsedSeconds: 0,
  recordingStartedAtMs: null,
  elapsedTimer: null,
  maxRecordTimer: null,
  idleDimTimer: null,
};

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
  <main class="shell" data-status="idle" data-expanded="false">
    <section class="widget-card">
      <header class="widget-header" id="drag-strip">
        <div class="brand-lockup">
          <h1 class="brand-title">voice</h1>
          <span class="status-dot" id="status-dot"></span>
        </div>
        <div class="header-actions">
          <button class="chip-button hidden" id="settings-toggle" type="button">Prefs</button>
        </div>
      </header>

      <section class="compact-body" id="compact-body">
        <div class="compact-copy">
          <p class="status-label" id="status-label">Shortcut Ready</p>
          <p class="status-detail" id="status-detail">Use the shortcut or tap record.</p>
        </div>
        <div class="compact-controls">
          <p class="compact-chip hidden" id="compact-chip"></p>
          <button class="record-button" id="record-button" type="button" aria-label="Toggle recording">
            <span class="record-button__core"></span>
            <span class="record-button__ring"></span>
          </button>
        </div>
      </section>

      <section class="expanded-panel hidden" id="expanded-panel">
        <section class="diagnostics hidden" id="diagnostics-panel">
          <div class="diagnostics__header">Runtime checks</div>
          <div class="diagnostics__list" id="diagnostics-list"></div>
        </section>

        <section class="transcript-panel hidden" id="transcript-panel">
          <textarea
            id="transcript-input"
            class="transcript-input"
            placeholder="Transcript appears here and stays editable."
            spellcheck="false"
          ></textarea>
          <div class="actions-row">
            <button class="action-button" id="copy-button" type="button">Copy</button>
            <button class="action-button" id="paste-button" type="button">Paste</button>
            <button class="action-button" id="copy-clear-button" type="button">Copy + Clear</button>
            <button class="action-button action-button--danger" id="delete-button" type="button">Delete</button>
          </div>
        </section>

        <section class="empty-state hidden" id="empty-state">
          No transcript yet. Record first, or open settings to adjust the local model and shortcuts.
        </section>

        <section class="settings-panel hidden" id="settings-panel">
          <div class="settings-panel__title">Settings</div>
          <form id="settings-form" class="settings-form">
            <label>
              <span>Model path</span>
              <input id="model-path" name="modelPath" type="text" placeholder="Optional local faster-whisper model path" />
            </label>
            <label>
              <span>Model profile</span>
              <select id="model-profile" name="modelProfile">
                <option value="tiny">tiny</option>
                <option value="base">base</option>
                <option value="small">small</option>
                <option value="medium">medium</option>
                <option value="large-v3">large-v3</option>
              </select>
            </label>
            <label>
              <span>Language</span>
              <input id="language" name="language" type="text" placeholder="en" />
            </label>
            <label>
              <span>Temp root</span>
              <input id="temp-root" name="tempRoot" type="text" />
            </label>
            <label>
              <span>Cleanup interval (seconds)</span>
              <input id="cleanup-interval" name="cleanupIntervalSeconds" type="number" min="60" step="30" />
            </label>
            <label>
              <span>Transcript TTL (seconds)</span>
              <input id="transcript-ttl" name="transcriptTtlSeconds" type="number" min="120" step="60" />
            </label>
            <label>
              <span>Record shortcut</span>
              <input id="record-shortcut" name="recordShortcut" type="text" />
            </label>
            <label>
              <span>Paste shortcut</span>
              <input id="paste-shortcut" name="pasteShortcut" type="text" />
            </label>
            <label class="checkbox-row">
              <input id="prefer-gpu" name="preferGpu" type="checkbox" />
              <span>Prefer GPU when the backend can use it</span>
            </label>
            <label class="checkbox-row">
              <input id="auto-copy" name="autoCopyOnCompletion" type="checkbox" />
              <span>Auto copy after transcription finishes</span>
            </label>
            <label class="checkbox-row">
              <input id="auto-clear" name="autoClearAfterCopy" type="checkbox" />
              <span>Auto clear after a copy action</span>
            </label>
            <div class="settings-actions">
              <button class="action-button" id="download-model-button" type="button">Download default model</button>
              <button class="action-button" type="submit">Save settings</button>
            </div>
            <p class="settings-help">
              Packaged builds still need a local model. This downloads the default small model into the app data directory.
            </p>
          </form>
        </section>
      </section>

      <p class="notice hidden" id="notice"></p>

      <button class="footer-toggle" id="expand-button" type="button">
        <span class="footer-toggle__line"></span>
        <span class="footer-toggle__label">Expand</span>
      </button>
    </section>
  </main>
`;

const shell = document.querySelector<HTMLElement>('.shell')!;
const compactBody = document.querySelector<HTMLElement>('#compact-body')!;
const dragStrip = document.querySelector<HTMLElement>('#drag-strip')!;
const brandTitle = document.querySelector<HTMLHeadingElement>('.brand-title')!;
const settingsToggle = document.querySelector<HTMLButtonElement>('#settings-toggle')!;
const settingsPanel = document.querySelector<HTMLElement>('#settings-panel')!;
const settingsForm = document.querySelector<HTMLFormElement>('#settings-form')!;
const downloadModelButton = document.querySelector<HTMLButtonElement>('#download-model-button')!;
const recordButton = document.querySelector<HTMLButtonElement>('#record-button')!;
const statusDot = document.querySelector<HTMLSpanElement>('#status-dot')!;
const statusLabel = document.querySelector<HTMLParagraphElement>('#status-label')!;
const statusDetail = document.querySelector<HTMLParagraphElement>('#status-detail')!;
const diagnosticsPanel = document.querySelector<HTMLElement>('#diagnostics-panel')!;
const diagnosticsList = document.querySelector<HTMLDivElement>('#diagnostics-list')!;
const expandedPanel = document.querySelector<HTMLElement>('#expanded-panel')!;
const transcriptPanel = document.querySelector<HTMLElement>('#transcript-panel')!;
const transcriptInput = document.querySelector<HTMLTextAreaElement>('#transcript-input')!;
const emptyState = document.querySelector<HTMLElement>('#empty-state')!;
const compactChip = document.querySelector<HTMLParagraphElement>('#compact-chip')!;
const copyButton = document.querySelector<HTMLButtonElement>('#copy-button')!;
const pasteButton = document.querySelector<HTMLButtonElement>('#paste-button')!;
const copyClearButton = document.querySelector<HTMLButtonElement>('#copy-clear-button')!;
const deleteButton = document.querySelector<HTMLButtonElement>('#delete-button')!;
const expandButton = document.querySelector<HTMLButtonElement>('#expand-button')!;
const expandButtonLabel = document.querySelector<HTMLSpanElement>('.footer-toggle__label')!;
const notice = document.querySelector<HTMLParagraphElement>('#notice')!;
const modelPathInput = document.querySelector<HTMLInputElement>('#model-path')!;
const modelProfileInput = document.querySelector<HTMLSelectElement>('#model-profile')!;
const languageInput = document.querySelector<HTMLInputElement>('#language')!;
const tempRootInput = document.querySelector<HTMLInputElement>('#temp-root')!;
const cleanupIntervalInput = document.querySelector<HTMLInputElement>('#cleanup-interval')!;
const transcriptTtlInput = document.querySelector<HTMLInputElement>('#transcript-ttl')!;
const recordShortcutInput = document.querySelector<HTMLInputElement>('#record-shortcut')!;
const pasteShortcutInput = document.querySelector<HTMLInputElement>('#paste-shortcut')!;
const preferGpuInput = document.querySelector<HTMLInputElement>('#prefer-gpu')!;
const autoCopyInput = document.querySelector<HTMLInputElement>('#auto-copy')!;
const autoClearInput = document.querySelector<HTMLInputElement>('#auto-clear')!;

function formatElapsed(seconds: number): string {
  const mins = Math.floor(seconds / 60)
    .toString()
    .padStart(2, '0');
  const secs = Math.floor(seconds % 60)
    .toString()
    .padStart(2, '0');
  return `${mins}:${secs}`;
}

function clearIdleDimTimer(): void {
  if (state.idleDimTimer !== null) {
    window.clearTimeout(state.idleDimTimer);
    state.idleDimTimer = null;
  }
}

function clearSnapTimer(): void {
  if (snapTimer !== null) {
    window.clearTimeout(snapTimer);
    snapTimer = null;
  }
}

function isInteracting(): boolean {
  return state.isExpanded || state.status !== 'idle' || state.isPointerInside || state.hasFocus;
}

function syncAttentionState(): void {
  if (isInteracting()) {
    const wasDimmed = state.isDimmed;
    clearIdleDimTimer();
    state.isDimmed = false;
    if (wasDimmed) {
      render();
    }
    return;
  }

  if (state.isDimmed || state.idleDimTimer !== null) {
    return;
  }

  state.idleDimTimer = window.setTimeout(() => {
    state.idleDimTimer = null;
    if (isInteracting()) {
      return;
    }
    state.isDimmed = true;
    render();
  }, idleDimDelayMs);
}

function setNotice(message: string, tone: NoticeTone = 'info'): void {
  state.notice = { message, tone };
  render();
}

async function syncBackendState(): Promise<void> {
  const backendStatus =
    state.status === 'idle' && state.transcript.trim()
      ? 'ready'
      : state.status;
  try {
    await invoke('sync_backend_state', {
      payload: {
        status: backendStatus,
        transcript: state.transcript,
      },
    });
  } catch {
    // Ignore tray sync failures. The widget should still function without the tray bridge.
  }
}

function queueBackendStateSync(): void {
  void syncBackendState();
}

function applySettingsToForm(settings: AppSettings): void {
  modelPathInput.value = settings.modelPath;
  modelProfileInput.value = settings.modelProfile;
  languageInput.value = settings.language;
  tempRootInput.value = settings.tempRoot;
  cleanupIntervalInput.value = String(settings.cleanupIntervalSeconds);
  transcriptTtlInput.value = String(settings.transcriptTtlSeconds);
  recordShortcutInput.value = settings.recordShortcut;
  pasteShortcutInput.value = settings.pasteShortcut;
  preferGpuInput.checked = settings.preferGpu;
  autoCopyInput.checked = settings.autoCopyOnCompletion;
  autoClearInput.checked = settings.autoClearAfterCopy;
}

function collectSettingsFromForm(): AppSettings {
  return {
    modelPath: modelPathInput.value.trim(),
    modelProfile: modelProfileInput.value,
    language: languageInput.value.trim() || 'en',
    tempRoot: tempRootInput.value.trim(),
    cleanupIntervalSeconds: Number(cleanupIntervalInput.value || 300),
    transcriptTtlSeconds: Number(transcriptTtlInput.value || 900),
    recordShortcut: recordShortcutInput.value.trim(),
    pasteShortcut: pasteShortcutInput.value.trim(),
    preferGpu: preferGpuInput.checked,
    autoCopyOnCompletion: autoCopyInput.checked,
    autoClearAfterCopy: autoClearInput.checked,
  };
}

async function downloadDefaultModel(): Promise<void> {
  downloadModelButton.disabled = true;
  setNotice('Downloading the default small model...', 'info');

  try {
    const saved = await invoke<BootstrapPayload>('download_default_model');
    state.settings = saved.settings;
    state.diagnostics = saved.diagnostics;
    applySettingsToForm(saved.settings);
    await registerShortcuts();
    setNotice('Default model downloaded and selected.', 'info');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setNotice(`Unable to download the default model: ${message}`, 'error');
  } finally {
    downloadModelButton.disabled = false;
    render();
  }
}

function getExpandedHeight(): number {
  const hasTranscript = Boolean(state.transcript.trim());
  let nextHeight = 168;

  if (hasTranscript) {
    nextHeight = 366;
  }
  if (state.settingsOpen) {
    nextHeight = hasTranscript ? 576 : 474;
  }

  return nextHeight;
}

async function syncWindowFrame(): Promise<void> {
  const width = state.isExpanded ? expandedWidth : collapsedWidth;
  const height = state.isExpanded ? getExpandedHeight() : collapsedHeight;
  await appWindow.setSize(new LogicalSize(width, height));
}

function resetTimers(): void {
  if (state.elapsedTimer !== null) {
    window.clearInterval(state.elapsedTimer);
    state.elapsedTimer = null;
  }
  if (state.maxRecordTimer !== null) {
    window.clearTimeout(state.maxRecordTimer);
    state.maxRecordTimer = null;
  }
  clearIdleDimTimer();
  clearSnapTimer();
}

function setStatus(status: Status): void {
  state.status = status;
  queueBackendStateSync();
  render();
}

function invalidateTranscriptForNewRecording(nextJobId: string): void {
  const previousJobId = state.currentJobId;
  state.currentJobId = nextJobId;
  state.transcript = '';
  transcriptInput.value = '';

  if (previousJobId && previousJobId !== nextJobId) {
    void invoke('delete_job_artifacts', { jobId: previousJobId }).catch(() => undefined);
  }
}

async function startRecording(): Promise<void> {
  if (state.status === 'recording' || state.status === 'processing') {
    return;
  }

  try {
    const response = await invoke<StartRecordingResponse>('start_recording_session');
    invalidateTranscriptForNewRecording(response.jobId);
    state.elapsedSeconds = 0;
    state.recordingStartedAtMs = Date.now();
    setStatus('recording');
    await syncWindowFrame();
    setNotice('Recording.', 'info');

    state.elapsedTimer = window.setInterval(() => {
      state.elapsedSeconds += 1;
      render();
    }, 1000);

    state.maxRecordTimer = window.setTimeout(() => {
      void stopRecording();
    }, 300_000);
  } catch (error) {
    state.recordingStartedAtMs = null;
    setStatus('error');
    const message = error instanceof Error ? error.message : 'Unknown microphone error';
    setNotice(`Unable to start microphone capture: ${message}`, 'error');
  }
}

async function stopRecording(): Promise<void> {
  if (state.status !== 'recording') {
    return;
  }
  resetTimers();
  state.recordingStartedAtMs = null;
  setStatus('processing');
  setNotice('Normalizing audio and transcribing locally...', 'info');

  try {
    const response = await invoke<ProcessRecordingResponse>('stop_recording_session');
    state.currentJobId = response.jobId;
    state.transcript = response.transcript;
    transcriptInput.value = state.transcript;
    setStatus('idle');

    const warnings = response.warnings.join(' ');
    const detectedLanguage = response.detectedLanguage ? ` Language: ${response.detectedLanguage}.` : '';
    const device = response.device ? ` Device: ${response.device}.` : '';
    const warningText = warnings ? ` ${warnings}` : '';
    setNotice(`Transcript ready.${detectedLanguage}${device}${warningText}`.trim(), 'info');
    await syncWindowFrame();

    if (state.settings?.autoCopyOnCompletion) {
      await copyCurrentTranscript(state.settings.autoClearAfterCopy);
    }
  } catch (error) {
    setStatus('error');
    const message = error instanceof Error ? error.message : String(error);
    const isCaptureError =
      message.startsWith('No microphone audio was captured') ||
      message.startsWith('Unable to finalize the microphone recording');
    setNotice(isCaptureError ? message : `Transcription failed: ${message}`, 'error');
    render();
  }
}

async function toggleRecording(source: ToggleSource): Promise<void> {
  const now = Date.now();
  if (now - lastToggleAtMs < toggleDebounceMs) {
    return;
  }

  if (state.status === 'recording') {
    if (
      source === 'shortcut' &&
      state.recordingStartedAtMs !== null &&
      now - state.recordingStartedAtMs < minimumShortcutStopMs
    ) {
      return;
    }

    lastToggleAtMs = now;
    await stopRecording();
    return;
  }

  lastToggleAtMs = now;
  await startRecording();
}

async function copyCurrentTranscript(clearAfterCopy = false): Promise<void> {
  const text = state.transcript.trim();
  if (!text) {
    setNotice('No transcript to copy.', 'warning');
    return;
  }

  try {
    await invoke('copy_text_to_clipboard', { text });
    setNotice('Transcript copied to the system clipboard.', 'info');
    if (clearAfterCopy) {
      await clearTranscript();
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setNotice(`Copy failed: ${message}`, 'error');
  }
}

async function pasteCurrentTranscript(source: PasteSource): Promise<void> {
  if (state.status === 'recording' || state.status === 'processing') {
    setNotice('Wait for the new transcript to finish before pasting.', 'warning');
    return;
  }

  const text = state.transcript.trim();
  if (!text) {
    setNotice('No transcript to paste.', 'warning');
    return;
  }

  const now = Date.now();
  if (pasteInFlight) {
    return;
  }
  if (source === 'shortcut' && now - lastPasteAtMs < pasteShortcutDebounceMs) {
    return;
  }

  try {
    pasteInFlight = true;
    lastPasteAtMs = now;
    const result = await invoke<PasteActionResult>('paste_text_into_focused_app', { text });
    const restoreSuffix = result.clipboardRestore === 'scheduled' ? ' Clipboard restore was scheduled.' : '';
    const warningSuffix = result.warning ? ` ${result.warning}` : '';
    setNotice(`Paste triggered via ${result.strategy}.${restoreSuffix}${warningSuffix}`.trim(), 'info');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setNotice(`Paste automation failed: ${message}`, 'error');
  } finally {
    pasteInFlight = false;
  }
}

async function clearTranscript(): Promise<void> {
  if (state.currentJobId) {
    try {
      await invoke('delete_job_artifacts', { jobId: state.currentJobId });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setNotice(`Transcript cleared, but cleanup failed: ${message}`, 'warning');
    }
  }

  state.currentJobId = null;
  state.transcript = '';
  transcriptInput.value = '';
  queueBackendStateSync();
  await syncWindowFrame();
  setNotice('Transcript cleared.', 'info');
  render();
}

async function registerShortcuts(): Promise<void> {
  if (!state.settings) {
    return;
  }

  try {
    await unregisterAll();
  } catch {
    // Ignore missing registrations.
  }

  try {
    await register(state.settings.recordShortcut, () => {
      void toggleRecording('shortcut');
    });
    await register(state.settings.pasteShortcut, () => {
      void pasteCurrentTranscript('shortcut');
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setNotice(`Shortcut registration failed: ${message}`, 'warning');
  }
}

function renderDiagnostics(): void {
  const items = state.settingsOpen
    ? state.diagnostics
    : state.diagnostics.filter((item) => item.code !== 'runtime-ok');

  diagnosticsList.innerHTML = '';
  if (!state.isExpanded || items.length === 0) {
    diagnosticsPanel.classList.add('hidden');
    return;
  }

  diagnosticsPanel.classList.remove('hidden');
  for (const item of items) {
    const row = document.createElement('div');
    row.className = `diagnostic-row ${item.level}`;
    row.innerHTML = `
      <span class="diagnostic-code">${item.code}</span>
      <p>${item.message}</p>
    `;
    diagnosticsList.appendChild(row);
  }
}

function render(): void {
  const hasTranscript = Boolean(state.transcript.trim());
  const transcriptActionsEnabled = hasTranscript && state.status === 'idle';
  const visualStatus: VisualStatus = state.status === 'idle' && hasTranscript ? 'ready' : state.status;
  const showNotice = state.isExpanded || state.notice.tone !== 'info' || state.status !== 'idle';
  const compactMode = !state.isExpanded;
  let noticeMessage = state.notice.message;
  let noticeTone = state.notice.tone;

  shell.dataset.status = state.status;
  shell.dataset.expanded = state.isExpanded ? 'true' : 'false';
  shell.dataset.attention = state.isDimmed ? 'idle' : 'active';
  expandedPanel.classList.toggle('hidden', !state.isExpanded);
  settingsPanel.classList.toggle('hidden', !state.isExpanded || !state.settingsOpen || state.settings === null);
  transcriptPanel.classList.toggle('hidden', !state.isExpanded || !hasTranscript);
  emptyState.classList.toggle('hidden', !state.isExpanded || hasTranscript || state.settingsOpen);
  settingsToggle.classList.toggle('hidden', !state.isExpanded);

  compactChip.classList.toggle('hidden', !hasTranscript || state.isExpanded);
  compactChip.textContent = hasTranscript ? 'Transcript ready' : '';

  transcriptInput.value = state.transcript;
  statusDot.dataset.status = visualStatus;
  recordButton.dataset.status = visualStatus;
  recordButton.disabled = state.status === 'processing';
  copyButton.disabled = !transcriptActionsEnabled;
  pasteButton.disabled = !transcriptActionsEnabled;
  copyClearButton.disabled = !transcriptActionsEnabled;
  deleteButton.disabled = !transcriptActionsEnabled;
  downloadModelButton.disabled = state.status === 'recording' || state.status === 'processing';

  expandButtonLabel.textContent = state.isExpanded ? 'Collapse panel' : 'Expand panel';
  expandButton.setAttribute('aria-label', state.isExpanded ? 'Collapse panel' : 'Expand panel');
  settingsToggle.textContent = state.settingsOpen && state.isExpanded ? 'Hide' : 'Prefs';
  brandTitle.textContent = 'voice';

  if (state.status === 'recording') {
    noticeMessage = `${formatElapsed(state.elapsedSeconds)} / ${formatElapsed(300)}`;
    noticeTone = 'info';
  }

  notice.textContent = noticeMessage;
  notice.className = `notice ${noticeTone}${showNotice ? '' : ' hidden'}`;

  if (state.status === 'recording') {
    statusLabel.textContent = compactMode ? `${formatElapsed(state.elapsedSeconds)} / ${formatElapsed(300)}` : 'Recording';
    statusDetail.textContent = `${formatElapsed(state.elapsedSeconds)} captured locally.`;
  } else if (state.status === 'processing') {
    statusLabel.textContent = compactMode ? 'Transcribing' : 'Processing';
    statusDetail.textContent = 'Local transcription in progress.';
  } else if (state.status === 'error') {
    statusLabel.textContent = compactMode ? 'Attention' : 'Attention';
    statusDetail.textContent = 'Open the widget if you need the full error text.';
  } else if (hasTranscript) {
    statusLabel.textContent = compactMode ? 'Transcript Ready' : 'Transcript ready';
    statusDetail.textContent = state.isExpanded
      ? 'Edit, copy, or paste the current transcript.'
      : 'Expand when you want to edit or paste it.';
  } else {
    statusLabel.textContent = compactMode ? '' : 'Idle';
    statusDetail.textContent = 'Use the shortcut or tap record.';
  }

  renderDiagnostics();
  syncAttentionState();
}

async function snapWindowToEdge(): Promise<void> {
  const monitor = await currentMonitor();
  if (!monitor) {
    return;
  }

  const position = await appWindow.outerPosition();
  const size = await appWindow.outerSize();
  const workArea = monitor.workArea;
  const left = workArea.position.x;
  const top = workArea.position.y;
  const right = left + workArea.size.width;
  const bottom = top + workArea.size.height;

  let nextX = position.x;
  let nextY = position.y;
  let shouldSnap = false;

  if (Math.abs(position.x - left) <= snapThreshold) {
    nextX = left + snapInset;
    shouldSnap = true;
  } else if (Math.abs(position.x + size.width - right) <= snapThreshold) {
    nextX = right - size.width - snapInset;
    shouldSnap = true;
  }

  if (Math.abs(position.y - top) <= snapThreshold) {
    nextY = top + snapInset;
    shouldSnap = true;
  } else if (Math.abs(position.y + size.height - bottom) <= snapThreshold) {
    nextY = bottom - size.height - snapInset;
    shouldSnap = true;
  }

  if (!shouldSnap || (nextX === position.x && nextY === position.y)) {
    return;
  }

  isSnappingWindow = true;
  await appWindow.setPosition(new PhysicalPosition(nextX, nextY));
}

async function bootstrap(): Promise<void> {
  try {
    await listen(trayToggleRecordingEvent, () => {
      void toggleRecording('button');
    });
    await listen(trayPasteTranscriptEvent, () => {
      void pasteCurrentTranscript('button');
    });

    const boot = await invoke<BootstrapPayload>('bootstrap');
    state.settings = boot.settings;
    state.diagnostics = boot.diagnostics;
    if (boot.devSeedTranscript?.trim()) {
      state.transcript = boot.devSeedTranscript;
      transcriptInput.value = boot.devSeedTranscript;
    }
    applySettingsToForm(boot.settings);
    await registerShortcuts();
    await appWindow.onMoved(() => {
      if (isSnappingWindow) {
        isSnappingWindow = false;
        return;
      }
      if (!dragIntent) {
        return;
      }
      clearSnapTimer();
      snapTimer = window.setTimeout(() => {
        dragIntent = false;
        clearSnapTimer();
        void snapWindowToEdge();
      }, 220);
    });
    await syncWindowFrame();
    await syncBackendState();
    setNotice('Ready for offline capture.', 'info');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setStatus('error');
    setNotice(`Startup failed: ${message}`, 'error');
  }
}

function beginWindowDrag(event: PointerEvent): void {
  const target = event.target instanceof HTMLElement ? event.target : null;
  if (event.button !== 0 || target?.closest('button, input, select, textarea')) {
    return;
  }

  event.preventDefault();
  window.getSelection()?.removeAllRanges();
  dragIntent = true;
  state.isDimmed = false;
  void appWindow.startDragging();
}

dragStrip.addEventListener('pointerdown', beginWindowDrag);
compactBody.addEventListener('pointerdown', beginWindowDrag);

shell.addEventListener('pointerenter', () => {
  state.isPointerInside = true;
  render();
});

shell.addEventListener('pointerleave', () => {
  state.isPointerInside = false;
  render();
});

window.addEventListener('focus', () => {
  state.hasFocus = true;
  render();
});

window.addEventListener('blur', () => {
  state.hasFocus = false;
  render();
});

settingsToggle.addEventListener('click', () => {
  state.isExpanded = true;
  state.settingsOpen = !state.settingsOpen;
  void syncWindowFrame();
  render();
});

expandButton.addEventListener('click', () => {
  state.isExpanded = !state.isExpanded;
  if (!state.isExpanded) {
    state.settingsOpen = false;
  }
  void syncWindowFrame();
  render();
});

recordButton.addEventListener('click', () => {
  void toggleRecording('button');
});

copyButton.addEventListener('click', () => {
  void copyCurrentTranscript(state.settings?.autoClearAfterCopy ?? false);
});

pasteButton.addEventListener('click', () => {
  void pasteCurrentTranscript('button');
});

copyClearButton.addEventListener('click', () => {
  void copyCurrentTranscript(true);
});

deleteButton.addEventListener('click', () => {
  void clearTranscript();
});

transcriptInput.addEventListener('input', () => {
  state.transcript = transcriptInput.value;
  queueBackendStateSync();
  render();
});

settingsForm.addEventListener('submit', async (event) => {
  event.preventDefault();
  try {
    const saved = await invoke<BootstrapPayload>('save_settings', { settings: collectSettingsFromForm() });
    state.settings = saved.settings;
    state.diagnostics = saved.diagnostics;
    applySettingsToForm(saved.settings);
    await registerShortcuts();
    setNotice('Settings saved.', 'info');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setNotice(`Unable to save settings: ${message}`, 'error');
  }
});

downloadModelButton.addEventListener('click', () => {
  void downloadDefaultModel();
});

window.addEventListener('beforeunload', () => {
  resetTimers();
  if (state.status === 'recording') {
    void invoke('cancel_recording_session').catch(() => undefined);
  }
});

render();
void bootstrap();
