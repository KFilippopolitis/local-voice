FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
WORKDIR /app

SHELL ["/bin/bash", "-lc"]

RUN apt-get update && apt-get install -y \
  ca-certificates \
  curl \
  git \
  sudo

COPY . /app

ARG SKIP_MODEL_DOWNLOAD=1

RUN chmod +x setup.sh doctor.sh run-dev.sh scripts/bootstrap-ubuntu.sh scripts/doctor.sh

RUN if [[ "$SKIP_MODEL_DOWNLOAD" == "1" ]]; then \
      ./setup.sh --skip-model-download; \
    else \
      ./setup.sh; \
    fi

RUN ./doctor.sh
RUN npm run build
RUN source "$HOME/.cargo/env" && CARGO_TARGET_DIR=/tmp/local-voice-smoke cargo check --manifest-path src-tauri/Cargo.toml
