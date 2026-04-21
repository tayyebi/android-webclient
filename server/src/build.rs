use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::process::Command;

use crate::AppState;

/// Copy a directory tree from `src` to `dst` recursively.
async fn copy_dir_recursive(src: PathBuf, dst: PathBuf) -> anyhow::Result<()> {
    fs::create_dir_all(&dst).await?;
    let mut entries = fs::read_dir(&src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().await?.is_dir() {
            Box::pin(copy_dir_recursive(src_path, dst_path)).await?;
        } else {
            fs::copy(&src_path, &dst_path).await?;
        }
    }
    Ok(())
}

/// Start a build job asynchronously.
///
/// Copies the Android template to a new job directory, writes the initial
/// status, then spawns `build_worker.py` in the background.
pub async fn start_job(
    state: &Arc<AppState>,
    job_id: &str,
    url: String,
    app_name: String,
    package_name: String,
    domain: String,
    version_name: String,
) -> anyhow::Result<()> {
    let job_dir = state.jobs_dir.join(job_id);
    fs::create_dir_all(&job_dir).await?;

    // Write initial status before copying template (fast feedback to UI)
    fs::write(job_dir.join("status.txt"), "pending").await?;
    fs::write(job_dir.join("build.log"), "").await?;

    // Copy the Android template into the job directory
    copy_dir_recursive(state.template_dir.clone(), job_dir.clone()).await?;

    // Ensure the bin/ directory exists
    fs::create_dir_all(job_dir.join("bin")).await?;

    let worker_script = state.worker_script.clone();

    // Collect SDK environment variables once; missing ones will cause the
    // worker to write "error" to status.txt with a descriptive message.
    let sdk_env: Vec<(&str, String)> = vec![
        ("AAPT_PATH",      std::env::var("AAPT_PATH").unwrap_or_default()),
        ("DX_PATH",        std::env::var("DX_PATH").unwrap_or_default()),
        ("ZIPALIGN_PATH",  std::env::var("ZIPALIGN_PATH").unwrap_or_default()),
        ("APKSIGNER_PATH", std::env::var("APKSIGNER_PATH").unwrap_or_default()),
        ("PLATFORM_JAR",   std::env::var("PLATFORM_JAR").unwrap_or_default()),
        ("KEYSTORE_PATH",  std::env::var("KEYSTORE_PATH").unwrap_or_default()),
        ("KEYSTORE_PASS",  std::env::var("KEYSTORE_PASS").unwrap_or_default()),
    ];

    tokio::spawn(async move {
        let result = Command::new("python3")
            .arg(&worker_script)
            .arg(&job_dir)
            .arg(&package_name)
            .arg(&app_name)
            .arg(&url)
            .arg(&domain)
            .arg(&version_name)
            .arg("1")   // version_code — always 1 for new apps
            .envs(sdk_env)
            .output()
            .await;

        // If the OS-level spawn failed, write an error status ourselves.
        // Normally the worker script manages status.txt on its own.
        if let Err(err) = result {
            let _ = fs::write(job_dir.join("status.txt"), "error").await;
            let _ = fs::write(
                job_dir.join("build.log"),
                format!("Failed to launch build worker: {err}\n"),
            )
            .await;
        }
    });

    Ok(())
}
