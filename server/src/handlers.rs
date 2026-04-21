use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use askama::Template;
use serde::Deserialize;
use std::sync::Arc;
use tokio::fs;
use tokio_util::io::ReaderStream;
use axum::body::Body;

use crate::{build, AppState};

// ---------------------------------------------------------------------------
// Template definitions
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "status.html")]
struct StatusTemplate {
    job_id: String,
    status: String,
    log: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn index() -> impl IntoResponse {
    render(IndexTemplate { error: None })
}

#[derive(Deserialize)]
pub struct BuildForm {
    pub url: String,
    pub app_name: String,
    pub package_name: String,
    pub version_name: String,
    pub domain: String,
}

pub async fn submit_build(
    State(state): State<Arc<AppState>>,
    Form(form): Form<BuildForm>,
) -> Response {
    // Validate URL scheme
    let url = form.url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return bad_request("URL must start with http:// or https://");
    }

    // Validate app name
    let app_name = form.app_name.trim().to_string();
    if app_name.is_empty() {
        return bad_request("App name must not be empty");
    }

    // Validate package name: at least two dot-separated identifiers,
    // each starting with a lowercase letter followed by [a-z0-9_]
    let package_name = form.package_name.trim().to_string();
    if !is_valid_package_name(&package_name) {
        return bad_request(
            "Package name must be like com.example.myapp \
             (lowercase letters, digits, underscores, separated by dots)",
        );
    }

    let version_name = {
        let v = form.version_name.trim();
        if v.is_empty() { "1.0".to_string() } else { v.to_string() }
    };

    let domain = {
        let d = form.domain.trim();
        if d.is_empty() {
            extract_domain(&url)
        } else {
            d.to_string()
        }
    };

    // Generate a unique job ID
    let job_id = uuid::Uuid::new_v4().to_string();

    match build::start_job(&state, &job_id, url, app_name, package_name, domain, version_name).await {
        Ok(()) => Redirect::to(&format!("/status/{job_id}")).into_response(),
        Err(err) => bad_request(&format!("Failed to start build: {err}")),
    }
}

pub async fn build_status(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Response {
    // Sanitise job_id: must be a valid UUID (hyphenated hex)
    if !is_safe_id(&job_id) {
        return (StatusCode::BAD_REQUEST, "Invalid job ID").into_response();
    }

    let job_dir = state.jobs_dir.join(&job_id);
    if !job_dir.exists() {
        return (StatusCode::NOT_FOUND, "Job not found").into_response();
    }

    let status = fs::read_to_string(job_dir.join("status.txt"))
        .await
        .unwrap_or_else(|_| "pending".to_string());
    let status = status.trim().to_string();

    let raw_log = fs::read_to_string(job_dir.join("build.log"))
        .await
        .unwrap_or_default();

    // Show the last 100 lines of the log
    let log: String = {
        let lines: Vec<&str> = raw_log.lines().collect();
        let start = lines.len().saturating_sub(100);
        lines[start..].join("\n")
    };

    render(StatusTemplate { job_id, status, log })
}

pub async fn download_apk(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Response {
    if !is_safe_id(&job_id) {
        return (StatusCode::BAD_REQUEST, "Invalid job ID").into_response();
    }

    let apk_path = state.jobs_dir.join(&job_id).join("bin/app.apk");
    if !apk_path.exists() {
        return (StatusCode::NOT_FOUND, "APK not ready yet").into_response();
    }

    match tokio::fs::File::open(&apk_path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);
            Response::builder()
                .header(header::CONTENT_TYPE, "application/vnd.android.package-archive")
                .header(header::CONTENT_DISPOSITION, "attachment; filename=\"app.apk\"")
                .body(body)
                .unwrap()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read APK").into_response(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn render<T: Template>(tmpl: T) -> Response {
    match tmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Template error").into_response(),
    }
}

fn bad_request(msg: &str) -> Response {
    render(IndexTemplate { error: Some(msg.to_string()) })
}

/// Very basic Android package name validation.
fn is_valid_package_name(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    parts.iter().all(|p| {
        !p.is_empty()
            && p.chars()
                .next()
                .map(|c| c.is_ascii_lowercase())
                .unwrap_or(false)
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// Allow only UUID-shaped strings (hex + hyphens) to prevent path traversal.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 36
        && id
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Extract the hostname from a URL string.
fn extract_domain(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')   // strip port if present
        .next()
        .unwrap_or("")
        .to_string()
}
