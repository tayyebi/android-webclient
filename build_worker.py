#!/usr/bin/env python3
"""
Build worker for the URL-to-APK SaaS platform.

Usage:
    python3 build_worker.py <job_dir> <package_name> <app_name> <load_url>
                            <domain_filter> <version_name> <version_code>

Required environment variables:
    AAPT_PATH       — path to the aapt binary
    DX_PATH         — path to the dx (or d8) binary
    ZIPALIGN_PATH   — path to zipalign
    APKSIGNER_PATH  — path to apksigner
    PLATFORM_JAR    — path to android.jar for the target SDK platform
    KEYSTORE_PATH   — path to the signing keystore (.keystore / .jks)
    KEYSTORE_PASS   — keystore + key password
"""

import os
import sys
import shutil
import subprocess
import pathlib
import logging
import glob as glob_module


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def substitute(path: pathlib.Path, replacements: dict) -> None:
    """Replace all {{KEY}} placeholders in *path* with their values."""
    text = path.read_text(encoding="utf-8")
    for key, value in replacements.items():
        text = text.replace(f"{{{{{key}}}}}", value)
    path.write_text(text, encoding="utf-8")


def run(log: logging.Logger, *args, cwd: pathlib.Path) -> None:
    """Run a subprocess, streaming output to the log file. Raises on failure."""
    cmd = [str(a) for a in args]
    log.info("$ " + " ".join(cmd))
    result = subprocess.run(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        cwd=str(cwd),
    )
    if result.stdout:
        log.info(result.stdout.decode("utf-8", errors="replace").rstrip())
    if result.returncode != 0:
        raise RuntimeError(f"Command failed (exit {result.returncode}): {' '.join(cmd)}")


def require_env(name: str) -> str:
    value = os.environ.get(name, "").strip()
    if not value:
        raise EnvironmentError(f"Required environment variable {name!r} is not set")
    return value


# ---------------------------------------------------------------------------
# Main build pipeline
# ---------------------------------------------------------------------------

def build(job_dir: pathlib.Path, package_name: str, app_name: str,
          load_url: str, domain_filter: str, version_name: str,
          version_code: str) -> None:

    log_path = job_dir / "build.log"
    status_path = job_dir / "status.txt"

    # Configure logging to write to both the job log file and stdout
    logger = logging.getLogger("build")
    logger.setLevel(logging.DEBUG)
    fmt = logging.Formatter("%(asctime)s  %(message)s", datefmt="%H:%M:%S")
    fh = logging.FileHandler(log_path, encoding="utf-8")
    fh.setFormatter(fmt)
    sh = logging.StreamHandler(sys.stdout)
    sh.setFormatter(fmt)
    logger.addHandler(fh)
    logger.addHandler(sh)

    def fail(msg: str) -> None:
        logger.error("FATAL: %s", msg)
        status_path.write_text("error", encoding="utf-8")
        sys.exit(1)

    status_path.write_text("building", encoding="utf-8")

    try:
        # Read required SDK tool paths from environment
        aapt       = require_env("AAPT_PATH")
        dx         = require_env("DX_PATH")
        zipalign   = require_env("ZIPALIGN_PATH")
        apksigner  = require_env("APKSIGNER_PATH")
        platform   = require_env("PLATFORM_JAR")
        keystore   = require_env("KEYSTORE_PATH")
        ks_pass    = require_env("KEYSTORE_PASS")
    except EnvironmentError as exc:
        fail(str(exc))
        return

    # ------------------------------------------------------------------
    # Step 1 — Remove stale R.java from template (will be regenerated)
    # ------------------------------------------------------------------
    stale_r = job_dir / "src" / "com" / "gordarg" / "app" / "R.java"
    if stale_r.exists():
        stale_r.unlink()
        logger.info("Removed stale R.java")

    # ------------------------------------------------------------------
    # Step 2 — Substitute placeholders in template files
    # ------------------------------------------------------------------
    logger.info("==> Substituting template values ...")

    shared = {
        "PACKAGE_NAME":  package_name,
        "VERSION_NAME":  version_name,
        "VERSION_CODE":  version_code,
    }

    substitute(job_dir / "AndroidManifest.xml", shared)
    substitute(job_dir / "res" / "values" / "strings.xml", {"APP_NAME": app_name})

    # ------------------------------------------------------------------
    # Step 3 — Rename Java source directory to match new package
    # ------------------------------------------------------------------
    pkg_rel_path = pathlib.Path(*package_name.split("."))   # e.g. com/example/myapp
    src_new = job_dir / "src" / pkg_rel_path
    src_old = job_dir / "src" / "com" / "gordarg" / "app"

    if src_new != src_old:
        logger.info("==> Moving Java sources to %s ...", src_new)
        src_new.mkdir(parents=True, exist_ok=True)
        for java_file in src_old.glob("*.java"):
            shutil.copy2(java_file, src_new / java_file.name)
        # Remove only the old template package directory, then prune any
        # now-empty ancestor directories.  We must NOT blindly remove
        # job_dir/src/com because the new package may also live under com/.
        shutil.rmtree(src_old)
        for ancestor in (src_old.parent, src_old.parent.parent):
            try:
                ancestor.rmdir()   # succeeds only if the directory is empty
            except OSError:
                break

    # Substitute remaining placeholders in Java source files
    java_subs = {
        "PACKAGE_NAME":   package_name,
        "LOAD_URL":       load_url,
        "DOMAIN_FILTER":  domain_filter,
    }
    for java_file in src_new.glob("*.java"):
        # Replace template package declaration
        text = java_file.read_text(encoding="utf-8")
        text = text.replace("package com.gordarg.app;", f"package {package_name};")
        java_file.write_text(text, encoding="utf-8")
        substitute(java_file, java_subs)

    # ------------------------------------------------------------------
    # Step 4 — Create output directories
    # ------------------------------------------------------------------
    (job_dir / "obj").mkdir(exist_ok=True)
    (job_dir / "bin").mkdir(exist_ok=True)

    # ------------------------------------------------------------------
    # Step 5 — Generate R.java with aapt
    # ------------------------------------------------------------------
    logger.info("==> Generating R.java ...")
    try:
        run(logger, aapt, "package", "-f", "-m",
            "-J", str(job_dir / "src"),
            "-M", str(job_dir / "AndroidManifest.xml"),
            "-S", str(job_dir / "res"),
            "-I", platform,
            cwd=job_dir)
    except RuntimeError as exc:
        fail(str(exc))
        return

    # ------------------------------------------------------------------
    # Step 6 — Compile Java sources with javac
    # ------------------------------------------------------------------
    logger.info("==> Compiling Java sources ...")
    java_files = list((job_dir / "src").rglob("*.java"))
    if not java_files:
        fail("No Java source files found after template substitution")
        return

    sources_file = job_dir / "sources.txt"
    sources_file.write_text("\n".join(str(f) for f in java_files), encoding="utf-8")

    try:
        run(logger, "javac",
            "-source", "1.8", "-target", "1.8",
            "-classpath", platform,
            "-d", str(job_dir / "obj"),
            f"@{sources_file}",
            cwd=job_dir)
    except RuntimeError as exc:
        fail(str(exc))
        return

    # ------------------------------------------------------------------
    # Step 7 — Package resources into unsigned APK
    # ------------------------------------------------------------------
    logger.info("==> Packaging resources ...")
    unsigned_apk = job_dir / "bin" / "app.unaligned.apk"
    try:
        run(logger, aapt, "package", "-f", "-m",
            "-F", str(unsigned_apk),
            "-M", str(job_dir / "AndroidManifest.xml"),
            "-S", str(job_dir / "res"),
            "-I", platform,
            cwd=job_dir)
    except RuntimeError as exc:
        fail(str(exc))
        return

    # ------------------------------------------------------------------
    # Step 8 — Convert class files to Dalvik bytecode (dex)
    # ------------------------------------------------------------------
    logger.info("==> Creating classes.dex ...")
    classes_dex = job_dir / "classes.dex"
    try:
        run(logger, dx, "--dex",
            f"--output={classes_dex}",
            str(job_dir / "obj"),
            cwd=job_dir)
    except RuntimeError as exc:
        fail(str(exc))
        return

    # Add dex to APK (aapt add requires classes.dex to be relative to cwd)
    try:
        run(logger, aapt, "add",
            str(unsigned_apk),
            "classes.dex",
            cwd=job_dir)
    except RuntimeError as exc:
        fail(str(exc))
        return

    # ------------------------------------------------------------------
    # Step 9 — Sign the APK
    # ------------------------------------------------------------------
    logger.info("==> Signing APK ...")
    try:
        run(logger, apksigner, "sign",
            "--ks", keystore,
            "--v1-signing-enabled", "true",
            "--v2-signing-enabled", "false",
            "--ks-pass", f"pass:{ks_pass}",
            str(unsigned_apk),
            cwd=job_dir)
    except RuntimeError as exc:
        fail(str(exc))
        return

    # ------------------------------------------------------------------
    # Step 10 — Align the APK
    # ------------------------------------------------------------------
    logger.info("==> Aligning APK ...")
    final_apk = job_dir / "bin" / "app.apk"
    try:
        run(logger, zipalign, "-f", "4",
            str(unsigned_apk),
            str(final_apk),
            cwd=job_dir)
    except RuntimeError as exc:
        fail(str(exc))
        return

    # ------------------------------------------------------------------
    # Done!
    # ------------------------------------------------------------------
    logger.info("==> Build complete: %s", final_apk)
    status_path.write_text("done", encoding="utf-8")


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    if len(sys.argv) != 8:
        print(
            "Usage: build_worker.py <job_dir> <package_name> <app_name> "
            "<load_url> <domain_filter> <version_name> <version_code>",
            file=sys.stderr,
        )
        sys.exit(2)

    _job_dir      = pathlib.Path(sys.argv[1]).resolve()
    _package_name = sys.argv[2]
    _app_name     = sys.argv[3]
    _load_url     = sys.argv[4]
    _domain       = sys.argv[5]
    _version_name = sys.argv[6]
    _version_code = sys.argv[7]

    build(
        _job_dir, _package_name, _app_name,
        _load_url, _domain, _version_name, _version_code,
    )
