# ============================================================
# Stage 1 — Build the Rust server binary
# ============================================================
FROM rust:1.78-slim AS builder

WORKDIR /build

# Cache dependency compilation separately from source changes
COPY server/Cargo.toml ./Cargo.toml
RUN mkdir -p src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Build the real binary
COPY server/src ./src
COPY server/templates ./templates
RUN touch src/main.rs \
    && cargo build --release

# ============================================================
# Stage 2 — Runtime image
# ============================================================
FROM debian:bookworm-slim

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 \
        openjdk-17-jdk-headless \
        curl \
        unzip \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# ---------------------------------------------------------------------------
# Install Android SDK command-line tools
# ---------------------------------------------------------------------------
ENV ANDROID_SDK_ROOT=/opt/android-sdk
ENV ANDROID_BUILD_TOOLS_VERSION=34.0.0
ENV ANDROID_PLATFORM_VERSION=android-28

RUN mkdir -p ${ANDROID_SDK_ROOT}/cmdline-tools && \
    curl -sSL "https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip" \
        -o /tmp/cmdline-tools.zip && \
    unzip -q /tmp/cmdline-tools.zip -d ${ANDROID_SDK_ROOT}/cmdline-tools && \
    mv ${ANDROID_SDK_ROOT}/cmdline-tools/cmdline-tools \
       ${ANDROID_SDK_ROOT}/cmdline-tools/latest && \
    rm /tmp/cmdline-tools.zip

ENV PATH="${ANDROID_SDK_ROOT}/cmdline-tools/latest/bin:${PATH}"

# Accept licences and install build-tools + platform
RUN yes | sdkmanager --licenses > /dev/null 2>&1 || true && \
    sdkmanager \
        "platform-tools" \
        "platforms;${ANDROID_PLATFORM_VERSION}" \
        "build-tools;${ANDROID_BUILD_TOOLS_VERSION}"

# ---------------------------------------------------------------------------
# App files
# ---------------------------------------------------------------------------
WORKDIR /app

COPY --from=builder /build/target/release/apk-builder ./apk-builder
COPY server/templates ./templates
COPY static ./static
COPY template ./template
COPY build_worker.py ./build_worker.py

# ---------------------------------------------------------------------------
# Runtime environment — override via docker run -e or .env
# ---------------------------------------------------------------------------
ENV AAPT_PATH="${ANDROID_SDK_ROOT}/build-tools/${ANDROID_BUILD_TOOLS_VERSION}/aapt"
ENV DX_PATH="${ANDROID_SDK_ROOT}/build-tools/${ANDROID_BUILD_TOOLS_VERSION}/dx"
ENV ZIPALIGN_PATH="${ANDROID_SDK_ROOT}/build-tools/${ANDROID_BUILD_TOOLS_VERSION}/zipalign"
ENV APKSIGNER_PATH="${ANDROID_SDK_ROOT}/build-tools/${ANDROID_BUILD_TOOLS_VERSION}/apksigner"
ENV PLATFORM_JAR="${ANDROID_SDK_ROOT}/platforms/${ANDROID_PLATFORM_VERSION}/android.jar"

ENV KEYSTORE_PATH="/app/debug.keystore"
ENV KEYSTORE_PASS="android"

ENV TEMPLATE_DIR="/app/template"
ENV JOBS_DIR="/tmp/apk_jobs"
ENV WORKER_SCRIPT="/app/build_worker.py"
ENV STATIC_DIR="/app/static"
ENV LISTEN_ADDR="0.0.0.0:3000"

# Copy debug keystore for signing (replace with release keystore in production)
COPY debug.keystore ./debug.keystore

EXPOSE 3000

CMD ["./apk-builder"]
