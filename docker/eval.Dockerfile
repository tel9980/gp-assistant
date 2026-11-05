# 评测 / 开发镜像：Node 22 + Python 3.12 + Rust stable + Tauri 构建依赖
# 用途：在容器里跑 `node scripts/verify.mjs`、`npm run test:unit`、cargo test、evals/deepeval 批处理
# 注意：这不是可运行的应用镜像 —— 主程序是 Tauri 桌面 GUI，需在 Windows 上安装运行。
FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive \
    PYTHONUNBUFFERED=1 \
    CARGO_TERM_COLOR=always \
    NODE_VERSION=22

# 基础工具 + Tauri/WebKit 构建依赖（cargo test 桌面 crate 需要）
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl git xz-utils unzip zip jq \
      build-essential pkg-config libssl-dev \
      python3 python3-venv python3-pip \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
      librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev patchelf \
    && rm -rf /var/lib/apt/lists/*

# Node 22（与 CI 一致）
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && npm config set fund false \
    && rm -rf /var/lib/apt/lists/*

# Rust stable + rustfmt/clippy
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --component rustfmt,clippy \
    && . "$HOME/.cargo/env"
ENV PATH="/root/.cargo/bin:${PATH}"

# Python 评测依赖（deepeval 冻结版本见 evals/deepeval/pyproject.toml）
RUN python3 -m pip install --no-cache-dir --break-system-packages \
      "deepeval==4.2.6" "socksio>=1.0.0"

WORKDIR /workspace

# 预装依赖，缩短交互式使用的等待（源码在运行时挂载/复制，都会覆盖）
COPY desktop/package.json desktop/package-lock.json /workspace/desktop/
COPY desktop/frontend/package.json desktop/frontend/package-lock.json /workspace/desktop/frontend/
RUN cd /workspace/desktop && npm ci --no-audit --no-fund \
    && cd /workspace/desktop/frontend && npm ci --no-audit --no-fund

# 预热 Rust 依赖缓存
COPY native/gp-core /workspace/native/gp-core
RUN cd /workspace/native/gp-core && cargo fetch

COPY . /workspace

CMD ["bash"]
