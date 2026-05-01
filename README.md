# Android WebClient

A Gradle-free Android WebView app and a self-hostable SaaS that turns any URL into a signed APK — no IDE required.

---

## Table of Contents

- [Manual APK Build](#manual-apk-build)
  - [Prerequisites](#prerequisites)
  - [Generate a Keystore](#generate-a-keystore)
  - [Build and Install](#build-and-install)
- [Running the SaaS (APK Builder Service)](#running-the-saas-apk-builder-service)
  - [Option A — Docker (recommended)](#option-a--docker-recommended)
  - [Option B — Run Locally Without Docker](#option-b--run-locally-without-docker)
  - [Environment Variables](#environment-variables)
- [Project Structure](#project-structure)
- [Learn More](#learn-more)

---

## Manual APK Build

Build and sideload the APK directly to a device or emulator from your Linux machine.

### Prerequisites

Install the required tools:

```bash
# Apache Ant (build system)
sudo apt install ant
ant -v
# Apache Ant(TM) version 1.10.12 (or later)

# Android Debug Bridge (for sideloading)
sudo apt install adb

# Android SDK (command-line tools, build-tools, platform)
sudo snap install androidsdk
androidsdk "platform-tools"
androidsdk "platforms;android-28"
androidsdk "build-tools;34.0.0"
```

> **Note:** After installing the SDK, update the tool paths near the top of `build.sh` to match your SDK installation directory (e.g. `~/AndroidSDK/build-tools/34.0.0/aapt`).

### Generate a Keystore

You only need to do this once. Skip this step if you already have a `release.keystore`.

```bash
keytool -genkey -v \
  -keystore release.keystore \
  -alias alias_name \
  -keyalg RSA \
  -keysize 2048 \
  -validity 10000
```

### Build and Install

```bash
# Compile and package the APK
./build.sh build

# Install on a connected device / emulator and launch the app
./build.sh run

# Or do both in one step
./build.sh build-run
```

The final signed APK is written to `bin/app.apk`.

---

## Running the SaaS (APK Builder Service)

The SaaS is a web service (Rust + Axum) that accepts a URL via a web form and returns a signed WebView APK. It uses `build_worker.py` to drive the Android SDK build pipeline on the server.

### Option A — Docker (recommended)

```bash
# 1. Build the Docker image
docker build -t apk-builder .

# 2. Copy the example env file and fill in your values
cp .env.example .env
# Edit .env — the defaults work for the bundled debug.keystore

# 3. Run the container
docker run --rm -p 3000:3000 --env-file .env apk-builder
```

Open <http://localhost:3000> in your browser, enter a URL, and download your APK.

> **Production tip:** Mount a real release keystore instead of the bundled debug one:
> ```bash
> docker run --rm -p 3000:3000 --env-file .env \
>   -v /path/to/release.keystore:/app/release.keystore \
>   -e KEYSTORE_PATH=/app/release.keystore \
>   -e KEYSTORE_PASS=your_password \
>   apk-builder
> ```

### Option B — Run Locally Without Docker

**Prerequisites:** Rust toolchain, Python 3, JDK 17, and the Android SDK (same packages as the manual build section above).

```bash
# 1. Build the Rust server
cd server
cargo build --release
cd ..

# 2. Copy and configure the environment
cp .env.example .env
# Edit .env — set AAPT_PATH, DX_PATH, ZIPALIGN_PATH, APKSIGNER_PATH, PLATFORM_JAR,
#              KEYSTORE_PATH, and KEYSTORE_PASS to match your local SDK install

# 3. Export the variables and start the server
export $(grep -v '^#' .env | xargs)
./server/target/release/apk-builder
```

The server listens on `http://0.0.0.0:3000` by default (override with `LISTEN_ADDR`).

### Environment Variables

| Variable | Default (in Docker) | Description |
|---|---|---|
| `AAPT_PATH` | SDK `build-tools/34.0.0/aapt` | Path to the `aapt` binary |
| `DX_PATH` | SDK `build-tools/34.0.0/dx` | Path to the `dx` (dex compiler) binary |
| `ZIPALIGN_PATH` | SDK `build-tools/34.0.0/zipalign` | Path to `zipalign` |
| `APKSIGNER_PATH` | SDK `build-tools/34.0.0/apksigner` | Path to `apksigner` |
| `PLATFORM_JAR` | SDK `platforms/android-28/android.jar` | Android platform JAR |
| `KEYSTORE_PATH` | `/app/debug.keystore` | Signing keystore (`.keystore` / `.jks`) |
| `KEYSTORE_PASS` | `android` | Keystore and key password |
| `TEMPLATE_DIR` | `/app/template` | Android project template directory |
| `JOBS_DIR` | `/tmp/apk_jobs` | Directory for build job workspaces |
| `WORKER_SCRIPT` | `/app/build_worker.py` | Path to the Python build worker |
| `STATIC_DIR` | `/app/static` | Directory for static web assets |
| `LISTEN_ADDR` | `0.0.0.0:3000` | Server bind address |

---

## Project Structure

```
.
├── build.sh            # Manual local build script (compile → sign → sideload)
├── build_worker.py     # Python build pipeline invoked per SaaS job
├── Dockerfile          # Multi-stage Docker image (Rust builder + Debian runtime)
├── .env.example        # Environment variable template for the SaaS server
├── AndroidManifest.xml # Android app manifest (root copy used by build.sh)
├── build.xml           # Ant build file for local compilation
├── src/                # Java source files
├── res/                # Android resources (layouts, strings, drawables)
├── template/           # Parameterised Android project copied per SaaS job
├── static/             # Static web assets served by the SaaS (CSS, etc.)
├── server/             # Rust/Axum web server source
│   ├── src/
│   │   ├── main.rs     # Server entry point, routing, background cleanup
│   │   ├── handlers.rs # HTTP handlers (form submit, status poll, APK download)
│   │   └── build.rs    # Job spawning logic
│   └── templates/      # Askama HTML templates (index, status pages)
├── debug.keystore      # Bundled debug keystore (password: android)
└── release.keystore    # Your release keystore (not committed — create your own)
```

---

## Learn More

- [Android Build Tools reference](https://androidsdkmanager.azurewebsites.net/Buildtools)
- [Gradle-free Android development](https://medium.com/@authmane512/how-to-do-android-development-faster-without-gradle-9046b8c1cf68)