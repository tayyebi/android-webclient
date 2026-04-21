use axum::{routing::{get, post}, Router};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::fs;
use tower_http::services::ServeDir;

mod build;
mod handlers;

#[derive(Clone)]
pub struct AppState {
    pub template_dir: PathBuf,
    pub jobs_dir: PathBuf,
    pub worker_script: PathBuf,
}

#[tokio::main]
async fn main() {
    let template_dir = PathBuf::from(
        std::env::var("TEMPLATE_DIR").unwrap_or_else(|_| "template".to_string()),
    );
    let jobs_dir = PathBuf::from(
        std::env::var("JOBS_DIR").unwrap_or_else(|_| "/tmp/apk_jobs".to_string()),
    );
    let worker_script = PathBuf::from(
        std::env::var("WORKER_SCRIPT").unwrap_or_else(|_| "build_worker.py".to_string()),
    );

    fs::create_dir_all(&jobs_dir)
        .await
        .expect("failed to create jobs directory");

    let state = Arc::new(AppState {
        template_dir,
        jobs_dir: jobs_dir.clone(),
        worker_script,
    });

    // Background task: remove job directories older than 24 hours
    let cleanup_dir = jobs_dir.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            cleanup_old_jobs(&cleanup_dir).await;
        }
    });

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());

    let app = Router::new()
        .route("/", get(handlers::index))
        .route("/build", post(handlers::submit_build))
        .route("/status/:job_id", get(handlers::build_status))
        .route("/download/:job_id", get(handlers::download_apk))
        .nest_service("/static", ServeDir::new(static_dir))
        .with_state(state);

    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
    println!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app).await.expect("server error");
}

async fn cleanup_old_jobs(jobs_dir: &PathBuf) {
    let cutoff = std::time::SystemTime::now()
        .checked_sub(Duration::from_secs(86400))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let mut entries = match fs::read_dir(jobs_dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(meta) = entry.metadata().await else { continue };
        let Ok(modified) = meta.modified() else { continue };
        if modified < cutoff {
            let _ = fs::remove_dir_all(entry.path()).await;
        }
    }
}
