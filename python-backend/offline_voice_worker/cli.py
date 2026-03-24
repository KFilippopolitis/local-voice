from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


def emit(payload: dict[str, Any], exit_code: int = 0) -> int:
    print(json.dumps(payload), flush=True)
    return exit_code


def parse_bool(value: str) -> bool:
    return value.strip().lower() in {"1", "true", "yes", "on"}


def normalize_language(value: str) -> str:
    normalized = value.strip().lower()
    if normalized in {"", "auto", "english", "en-us", "en-gb"}:
        return "en"
    return normalized


def error_payload(code: str, message: str, detail: str | None = None) -> dict[str, Any]:
    return {
        "ok": False,
        "error": {
            "code": code,
            "message": message,
            "detail": detail,
        },
    }


def ensure_ffmpeg() -> None:
    if shutil.which("ffmpeg") is None:
        raise RuntimeError("ffmpeg is missing from PATH")


def normalize_audio(raw_path: Path, normalized_path: Path) -> None:
    command = [
        "ffmpeg",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        str(raw_path),
        "-ac",
        "1",
        "-ar",
        "16000",
        "-c:a",
        "pcm_s16le",
        str(normalized_path),
    ]
    completed = subprocess.run(command, capture_output=True, text=True)
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "ffmpeg returned a non-zero exit status"
        raise RuntimeError(detail)


def load_model(model_path: str, model_profile: str, prefer_gpu: bool):
    from faster_whisper import WhisperModel

    model_spec = model_path or model_profile
    attempts: list[tuple[str, str]] = []
    if prefer_gpu:
        attempts.extend([
            ("cuda", "float16"),
            ("cpu", "int8"),
        ])
    else:
        attempts.append(("cpu", "int8"))

    last_error: Exception | None = None
    fallback_warning: str | None = None

    for device, compute_type in attempts:
        try:
            model = WhisperModel(model_spec, device=device, compute_type=compute_type)
            if device == "cpu" and prefer_gpu:
                fallback_warning = "GPU backend was unavailable, so transcription fell back to CPU."
            return model, device, fallback_warning
        except Exception as exc:  # noqa: BLE001
            last_error = exc

    raise RuntimeError(str(last_error) if last_error is not None else "Unable to load the selected faster-whisper model")


def transcribe_audio(
    normalized_path: Path,
    model_path: str,
    model_profile: str,
    language: str,
    prefer_gpu: bool,
) -> tuple[str, str | None, str, list[str]]:
    model, device, fallback_warning = load_model(model_path, model_profile, prefer_gpu)
    language = normalize_language(language)
    segment_iter, info = model.transcribe(
        str(normalized_path),
        language=language,
        vad_filter=True,
        beam_size=1,
        condition_on_previous_text=False,
    )
    transcript = " ".join(segment.text.strip() for segment in segment_iter if segment.text.strip()).strip()
    warnings = [fallback_warning] if fallback_warning else []
    return transcript, getattr(info, "language", None), device, warnings


def download_model(model_profile: str, output_dir: Path) -> Path:
    from huggingface_hub import snapshot_download

    repo_id = f"Systran/faster-whisper-{model_profile}"
    output_dir.mkdir(parents=True, exist_ok=True)
    target_dir = output_dir / f"faster-whisper-{model_profile}"

    snapshot_download(
        repo_id=repo_id,
        local_dir=target_dir,
    )
    return target_dir


def handle_transcribe(args: argparse.Namespace) -> int:
    job_dir = Path(args.job_dir).resolve()
    raw_path = Path(args.raw_path).resolve()
    mime_type = args.mime_type
    normalized_path = job_dir / "normalized.wav"
    transcript_json_path = job_dir / "transcript.json"

    if not job_dir.exists():
        return emit(error_payload("job-dir-missing", "Job directory does not exist."), 1)
    if not raw_path.exists():
        return emit(error_payload("raw-audio-missing", "Raw audio input is missing."), 1)
    if job_dir not in raw_path.parents:
        return emit(error_payload("unsafe-raw-path", "Raw audio path must live inside the job directory."), 1)

    try:
        ensure_ffmpeg()
    except RuntimeError as exc:
        return emit(error_payload("ffmpeg-missing", "FFmpeg is unavailable.", str(exc)), 1)

    try:
        normalize_audio(raw_path, normalized_path)
    except RuntimeError as exc:
        return emit(error_payload("ffmpeg-normalize-failed", "FFmpeg failed to normalize the recording.", str(exc)), 1)

    try:
        transcript, detected_language, device, warnings = transcribe_audio(
            normalized_path=normalized_path,
            model_path=args.model_path,
            model_profile=args.model_profile,
            language=args.language,
            prefer_gpu=parse_bool(args.prefer_gpu),
        )
    except ModuleNotFoundError as exc:
        return emit(
            error_payload(
                "backend-deps-missing",
                "The faster-whisper backend dependencies are not installed.",
                str(exc),
            ),
            1,
        )
    except RuntimeError as exc:
        detail = str(exc)
        code = "transcription-failed"
        message = "Local transcription failed."
        lowered = detail.lower()
        if "ctranslate2" in lowered or "cudnn" in lowered or "cublas" in lowered:
            code = "gpu-runtime-missing"
            message = "GPU transcription could not start with the current runtime libraries."
        elif "no such file" in lowered or "not found" in lowered:
            code = "model-missing"
            message = "The selected faster-whisper model could not be found locally."
        return emit(error_payload(code, message, detail), 1)
    except Exception as exc:  # noqa: BLE001
        return emit(error_payload("transcription-failed", "Local transcription failed.", str(exc)), 1)

    if not transcript:
        return emit(error_payload("empty-transcript", "The audio produced an empty transcript."), 1)

    transcript_json_path.write_text(
        json.dumps(
            {
                "transcript": transcript,
                "detectedLanguage": detected_language,
                "device": device,
                "warnings": warnings,
                "mimeType": mime_type,
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    return emit(
        {
            "ok": True,
            "transcript": transcript,
            "detectedLanguage": detected_language,
            "device": device,
            "warnings": warnings,
        }
    )


def handle_download_model(args: argparse.Namespace) -> int:
    output_dir = Path(args.output_dir).resolve()

    try:
        target_dir = download_model(args.model_profile, output_dir)
    except Exception as exc:  # noqa: BLE001
        return emit(
            error_payload(
                "model-download-failed",
                "Unable to download the faster-whisper model.",
                str(exc),
            ),
            1,
        )

    return emit(
        {
            "ok": True,
            "modelProfile": args.model_profile,
            "modelPath": str(target_dir),
        }
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="offline-voice-worker")
    subparsers = parser.add_subparsers(dest="command", required=True)

    transcribe = subparsers.add_parser("transcribe", help="Normalize and transcribe one recording job")
    transcribe.add_argument("--job-dir", required=True)
    transcribe.add_argument("--raw-path", required=True)
    transcribe.add_argument("--mime-type", required=True)
    transcribe.add_argument("--model-path", default="")
    transcribe.add_argument("--model-profile", default="small")
    transcribe.add_argument("--language", default="en")
    transcribe.add_argument("--prefer-gpu", default="true")

    download = subparsers.add_parser("download-model", help="Download a local faster-whisper model")
    download.add_argument("--model-profile", default="small")
    download.add_argument("--output-dir", required=True)

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    if args.command == "transcribe":
        return handle_transcribe(args)
    if args.command == "download-model":
        return handle_download_model(args)

    return emit(error_payload("unknown-command", "Unknown worker command."), 1)


if __name__ == "__main__":
    sys.exit(main())
