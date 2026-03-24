# Local Voice Backend

This worker runs the local FFmpeg plus `faster-whisper` transcription pipeline for the desktop widget.

Use the root repository setup flow when possible:

- [README.md](../README.md)

Direct backend setup is still:

```bash
uv venv
uv sync --frozen
```

To download the default local model from this directory:

```bash
uv run python -m offline_voice_worker.cli download-model --model-profile small --output-dir models
```
