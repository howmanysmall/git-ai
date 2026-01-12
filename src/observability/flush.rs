use crate::config::{Config, id_file_path};
use crate::git::find_repository_in_path;
use futures::stream::{self, StreamExt};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Handle the flush-logs command
pub fn handle_flush_logs(_args: &[String]) {
    // FORK POLICY: All outbound telemetry is disabled in this fork.
    // This prevents any Sentry/PostHog events from being sent, even if DSNs or API keys are present.
    // See src/config.rs::outbound_network_reporting_disabled() for the policy gate.
    if crate::config::outbound_network_reporting_disabled() {
        eprintln!("Telemetry disabled by fork policy (no outbound network reporting).");
        std::process::exit(0);
    }

    let config = Config::get();

    // Check for Enterprise DSN: config takes precedence over env var, which takes precedence over build-time value
    let enterprise_dsn = config
        .telemetry_enterprise_dsn()
        .map(|s| s.to_string())
        .or_else(|| {
            std::env::var("SENTRY_ENTERPRISE")
                .ok()
                .or_else(|| option_env!("SENTRY_ENTERPRISE").map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
        });

    // Check for PostHog configuration: runtime env var takes precedence over build-time value
    let posthog_api_key = std::env::var("POSTHOG_API_KEY")
        .ok()
        .or_else(|| option_env!("POSTHOG_API_KEY").map(|s| s.to_string()))
        .filter(|s| !s.is_empty());

    let posthog_host = std::env::var("POSTHOG_HOST")
        .ok()
        .or_else(|| option_env!("POSTHOG_HOST").map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://us.i.posthog.com".to_string());

    // Find the .git/ai/logs directory
    let (repo_root, logs_dir) = match find_logs_directory() {
        Some(result) => result,
        None => {
            std::process::exit(1);
        }
    };

    // Check for OSS DSN: runtime env var takes precedence over build-time value
    // Can be explicitly disabled with empty string
    // Skip OSS DSN if OSS telemetry is disabled in config
    let oss_dsn = if config.is_telemetry_oss_disabled() {
        None
    } else {
        std::env::var("SENTRY_OSS")
            .ok()
            .or_else(|| option_env!("SENTRY_OSS").map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
    };

    // Need at least one DSN to proceed
    if oss_dsn.is_none() && enterprise_dsn.is_none() && posthog_api_key.is_none() {
        std::process::exit(1);
    }

    // Get current PID to exclude our own log file
    let current_pid = std::process::id();
    let current_log_file = format!("{}.log", current_pid);

    // Read all log files except current PID
    let log_files: Vec<PathBuf> = match fs::read_dir(&logs_dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n != current_log_file && n.ends_with(".log"))
                        .unwrap_or(false)
            })
            .collect(),
        Err(_) => {
            std::process::exit(1);
        }
    };

    if log_files.is_empty() {
        std::process::exit(1);
    }

    // Try to get repository info for metadata
    let repo = find_repository_in_path(&repo_root.to_string_lossy()).ok();
    let remotes_info: Vec<(String, String)> = repo
        .as_ref()
        .and_then(|r| r.remotes_with_urls().ok())
        .unwrap_or_default()
        .into_iter()
        .map(|(name, url)| (name, sanitize_git_url(&url)))
        .collect();

    // Get or create distinct_id from ~/.git-ai/internal/distinct_id
    let distinct_id = get_or_create_distinct_id();

    // Initialize Sentry clients
    let (oss_client, enterprise_client) = initialize_sentry_clients(oss_dsn, enterprise_dsn);

    // Initialize PostHog client
    let posthog_client = if config.is_telemetry_oss_disabled() {
        None
    } else {
        posthog_api_key
            .as_ref()
            .map(|api_key| PostHogClient::new(api_key.clone(), posthog_host.clone()))
    };

    // Check if clients are present (needed for cleanup logic later)
    let has_clients =
        oss_client.is_some() || enterprise_client.is_some() || posthog_client.is_some();

    eprintln!(
        "Processing {} log files (max 10 concurrent)...",
        log_files.len()
    );

    // Process log files in parallel (max 10 at a time)
    let results = smol::block_on(async {
        let oss_client = Arc::new(oss_client);
        let enterprise_client = Arc::new(enterprise_client);
        let posthog_client = Arc::new(posthog_client);
        let remotes_info = Arc::new(remotes_info);
        let distinct_id = Arc::new(distinct_id);

        stream::iter(log_files)
            .map(|log_file| {
                let oss_client = Arc::clone(&oss_client);
                let enterprise_client = Arc::clone(&enterprise_client);
                let posthog_client = Arc::clone(&posthog_client);
                let remotes_info = Arc::clone(&remotes_info);
                let distinct_id = Arc::clone(&distinct_id);

                smol::unblock(move || {
                    let file_name = log_file
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");

                    match process_log_file(
                        &log_file,
                        &oss_client,
                        &enterprise_client,
                        &posthog_client,
                        &remotes_info,
                        &distinct_id,
                    ) {
                        Ok(count) if count > 0 => {
                            eprintln!("  ✓ {} - sent {} events", file_name, count);
                            Some((log_file, count))
                        }
                        Ok(_) => {
                            eprintln!("  ○ {} - no events to send", file_name);
                            None
                        }
                        Err(e) => {
                            eprintln!("  ✗ {} - error: {}", file_name, e);
                            None
                        }
                    }
                })
            })
            .buffer_unordered(10) // Process max 10 files concurrently
            .collect::<Vec<_>>()
            .await
    });

    // Collect results
    let mut events_sent = 0;
    let mut files_to_delete = Vec::new();

    for result in results {
        if let Some((log_file, count)) = result {
            events_sent += count;
            files_to_delete.push(log_file);
        }
    }

    eprintln!(
        "\nSummary: {} events sent from {} files",
        events_sent,
        files_to_delete.len()
    );

    // Clean up old logs if no clients configured
    if !has_clients {
        eprintln!("Cleaning up old logs (no telemetry clients configured)...");
        cleanup_old_logs(&logs_dir);
    }

    if events_sent > 0 {
        eprintln!("Deleting {} processed log files", files_to_delete.len());
        for file_path in files_to_delete {
            let _ = fs::remove_file(&file_path);
        }

        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

/// Clean up old log files when count > 100
/// Deletes logs older than a week based on file modification time
fn cleanup_old_logs(logs_dir: &PathBuf) {
    let Ok(entries) = fs::read_dir(logs_dir) else {
        return;
    };

    // Collect all log files with their metadata
    let mut log_files: Vec<(PathBuf, fs::Metadata)> = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("log") {
                if let Ok(metadata) = entry.metadata() {
                    log_files.push((path, metadata));
                }
            }
        }
    }

    // Only clean up if count > 100
    if log_files.len() <= 100 {
        return;
    }

    // Calculate cutoff time (one week ago)
    let one_week_ago = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .saturating_sub(7 * 24 * 60 * 60); // 7 days in seconds

    // Delete logs older than a week
    for (path, metadata) in log_files {
        if let Ok(modified) = metadata.modified() {
            if let Ok(modified_secs) = modified.duration_since(UNIX_EPOCH) {
                if modified_secs.as_secs() < one_week_ago {
                    let _ = fs::remove_file(&path);
                }
            }
        } else if let Ok(created) = metadata.created() {
            // Fallback to creation time if modification time is not available
            if let Ok(created_secs) = created.duration_since(UNIX_EPOCH) {
                if created_secs.as_secs() < one_week_ago {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}

fn find_logs_directory() -> Option<(PathBuf, PathBuf)> {
    let mut current = std::env::current_dir().ok()?;

    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() && git_dir.is_dir() {
            let logs_dir = git_dir.join("ai").join("logs");
            if logs_dir.exists() && logs_dir.is_dir() {
                return Some((current.clone(), logs_dir));
            }
        }

        if !current.pop() {
            break;
        }
    }

    None
}

struct SentryClient {
    endpoint: String,
    public_key: String,
}

struct PostHogClient {
    api_key: String,
    endpoint: String,
}

impl SentryClient {
    fn from_dsn(dsn: &str) -> Option<Self> {
        // Parse DSN: https://PUBLIC_KEY@HOST/PROJECT_ID
        let url = url::Url::parse(dsn).ok()?;
        let public_key = url.username().to_string();
        let host = url.host_str()?;
        let project_id = url.path().trim_start_matches('/');

        let scheme = url.scheme();
        let endpoint = format!("{}://{}/api/{}/store/", scheme, host, project_id);

        Some(SentryClient {
            endpoint,
            public_key,
        })
    }

    fn send_event(&self, event: Value) -> Result<String, Box<dyn std::error::Error>> {
        let auth_header = format!(
            "Sentry sentry_version=7, sentry_key={}, sentry_client=git-ai/{}",
            self.public_key,
            env!("CARGO_PKG_VERSION")
        );

        let body = serde_json::to_string(&event)?;

        let response = minreq::post(&self.endpoint)
            .with_header("X-Sentry-Auth", auth_header)
            .with_header("Content-Type", "application/json")
            .with_body(body)
            .send()?;

        let status = response.status_code;
        let event_id = serde_json::from_str::<Value>(response.as_str()?)
            .ok()
            .and_then(|v| {
                v.get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());

        if status >= 200 && status < 300 {
            Ok(event_id)
        } else {
            Err(format!("Sentry returned status {}", status).into())
        }
    }
}

impl PostHogClient {
    fn new(api_key: String, host: String) -> Self {
        let endpoint = format!("{}/capture/", host.trim_end_matches('/'));
        PostHogClient { api_key, endpoint }
    }

    fn send_event(&self, event: Value) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::to_string(&event)?;

        let response = minreq::post(&self.endpoint)
            .with_header("Content-Type", "application/json")
            .with_body(body)
            .send()?;

        let status = response.status_code;

        if status >= 200 && status < 300 {
            Ok(())
        } else {
            Err(format!("PostHog returned status {}", status).into())
        }
    }
}

fn initialize_sentry_clients(
    oss_dsn: Option<String>,
    enterprise_dsn: Option<String>,
) -> (Option<SentryClient>, Option<SentryClient>) {
    let oss_client = oss_dsn.and_then(|dsn| SentryClient::from_dsn(&dsn));
    let enterprise_client = enterprise_dsn.and_then(|dsn| SentryClient::from_dsn(&dsn));

    (oss_client, enterprise_client)
}

fn process_log_file(
    path: &PathBuf,
    oss_client: &Option<SentryClient>,
    enterprise_client: &Option<SentryClient>,
    posthog_client: &Option<PostHogClient>,
    remotes_info: &[(String, String)],
    distinct_id: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut count = 0;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Value>(line) {
            Ok(envelope) => {
                let mut sent = false;

                // Send to OSS if configured
                if let Some(client) = oss_client {
                    if send_envelope_to_sentry(&envelope, client, remotes_info, distinct_id) {
                        sent = true;
                    }
                }

                // Send to Enterprise if configured
                if let Some(client) = enterprise_client {
                    if send_envelope_to_sentry(&envelope, client, remotes_info, distinct_id) {
                        sent = true;
                    }
                }

                // Send to PostHog if configured
                if let Some(client) = posthog_client {
                    if send_envelope_to_posthog(&envelope, client, remotes_info, distinct_id) {
                        sent = true;
                    }
                }

                if sent {
                    count += 1;
                }
            }
            Err(_) => {}
        }
    }

    Ok(count)
}

fn send_envelope_to_sentry(
    envelope: &Value,
    client: &SentryClient,
    remotes_info: &[(String, String)],
    distinct_id: &str,
) -> bool {
    let event_type = envelope.get("type").and_then(|t| t.as_str());
    let timestamp = envelope
        .get("timestamp")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    // Build tags
    let mut tags = BTreeMap::new();
    tags.insert("os".to_string(), json!(std::env::consts::OS));
    tags.insert("arch".to_string(), json!(std::env::consts::ARCH));
    tags.insert("distinct_id".to_string(), json!(distinct_id));
    for (remote_name, remote_url) in remotes_info {
        tags.insert(format!("remote.{}", remote_name), json!(remote_url));
    }

    let event = match event_type {
        Some("error") => {
            let message = envelope
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            let context = envelope.get("context");

            let mut extra = BTreeMap::new();
            if let Some(ctx) = context {
                if let Some(obj) = ctx.as_object() {
                    for (key, value) in obj {
                        extra.insert(key.clone(), value.clone());
                    }
                }
            }

            json!({
                "message": message,
                "level": "error",
                "timestamp": timestamp,
                "platform": "other",
                "tags": tags,
                "extra": extra,
                "release": format!("git-ai@{}", env!("CARGO_PKG_VERSION")),
            })
        }
        Some("performance") => {
            let operation = envelope
                .get("operation")
                .and_then(|o| o.as_str())
                .unwrap_or("unknown");
            let duration_ms = envelope
                .get("duration_ms")
                .and_then(|d| d.as_u64())
                .unwrap_or(0);
            let context = envelope.get("context");

            let mut extra = BTreeMap::new();
            extra.insert("operation".to_string(), json!(operation));
            extra.insert("duration_ms".to_string(), json!(duration_ms));
            if let Some(ctx) = context {
                if let Some(obj) = ctx.as_object() {
                    for (key, value) in obj {
                        extra.insert(key.clone(), value.clone());
                    }
                }
            }

            json!({
                "message": format!("Performance: {} ({}ms)", operation, duration_ms),
                "level": "info",
                "timestamp": timestamp,
                "platform": "other",
                "tags": tags,
                "extra": extra,
                "release": format!("git-ai@{}", env!("CARGO_PKG_VERSION")),
            })
        }
        Some("message") => {
            let message = envelope
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown message");
            let level = envelope
                .get("level")
                .and_then(|l| l.as_str())
                .unwrap_or("info");
            let context = envelope.get("context");

            let mut extra = BTreeMap::new();
            if let Some(ctx) = context {
                if let Some(obj) = ctx.as_object() {
                    for (key, value) in obj {
                        extra.insert(key.clone(), value.clone());
                    }
                }
            }

            json!({
                "message": message,
                "level": level,
                "timestamp": timestamp,
                "platform": "other",
                "tags": tags,
                "extra": extra,
                "release": format!("git-ai@{}", env!("CARGO_PKG_VERSION")),
            })
        }
        _ => {
            return false;
        }
    };

    match client.send_event(event) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn send_envelope_to_posthog(
    envelope: &Value,
    client: &PostHogClient,
    remotes_info: &[(String, String)],
    distinct_id: &str,
) -> bool {
    let event_type = envelope.get("type").and_then(|t| t.as_str());

    // Only send log messages to PostHog, not errors or performance
    if event_type != Some("message") {
        return false;
    }

    let timestamp = envelope.get("timestamp").and_then(|t| t.as_str());

    // Build properties
    let mut properties = BTreeMap::new();
    properties.insert("os".to_string(), json!(std::env::consts::OS));
    properties.insert("arch".to_string(), json!(std::env::consts::ARCH));
    properties.insert("version".to_string(), json!(env!("CARGO_PKG_VERSION")));

    for (remote_name, remote_url) in remotes_info {
        properties.insert(format!("remote_{}", remote_name), json!(remote_url));
    }

    let message = envelope
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("Unknown message");
    let level = envelope
        .get("level")
        .and_then(|l| l.as_str())
        .unwrap_or("info");
    let context = envelope.get("context");

    properties.insert("message".to_string(), json!(message));
    properties.insert("level".to_string(), json!(level));

    if let Some(ctx) = context {
        if let Some(obj) = ctx.as_object() {
            for (key, value) in obj {
                properties.insert(key.clone(), value.clone());
            }
        }
    }

    let mut event = json!({
        "api_key": client.api_key,
        "event": message,
        "properties": properties,
        "distinct_id": distinct_id,
    });

    if let Some(ts) = timestamp {
        event["timestamp"] = json!(ts);
    }

    match client.send_event(event) {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Sanitize git URLs by replacing passwords with asterisks
/// Handles URLs like: https://username:password@github.com/repo.git
fn sanitize_git_url(url: &str) -> String {
    // Look for the pattern: ://username:password@
    if let Some(protocol_end) = url.find("://") {
        let after_protocol = &url[protocol_end + 3..];

        // Check if there's an @ symbol (indicating credentials)
        if let Some(at_pos) = after_protocol.find('@') {
            let credentials_part = &after_protocol[..at_pos];

            // Check if there's a colon in the credentials (indicating password)
            if let Some(colon_pos) = credentials_part.find(':') {
                let username = &credentials_part[..colon_pos];
                let host_part = &after_protocol[at_pos..];

                // Reconstruct URL with password replaced by asterisks
                return format!("{}://{}:*****{}", &url[..protocol_end], username, host_part);
            }
        }
    }

    // If no password pattern found, return original URL
    url.to_string()
}

/// Get or create the distinct_id (UUID) from ~/.git-ai/internal/distinct_id
/// If the file doesn't exist, generates a new UUID and writes it to the file
fn get_or_create_distinct_id() -> String {
    let id_path = match id_file_path() {
        Some(path) => path,
        None => return "unknown".to_string(),
    };

    // Try to read existing ID
    if let Ok(existing_id) = fs::read_to_string(&id_path) {
        let trimmed = existing_id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    // Generate new UUID
    let new_id = Uuid::new_v4().to_string();

    // Ensure directory exists
    if let Some(parent) = id_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Write the new ID to file
    if let Err(e) = fs::write(&id_path, &new_id) {
        eprintln!("Warning: Failed to write distinct_id file: {}", e);
    }

    new_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_flush_logs_exits_when_telemetry_disabled() {
        // FORK POLICY TEST: Verify that flush-logs exits early when telemetry is disabled
        // This test would normally call handle_flush_logs, but since it calls std::process::exit,
        // we instead verify the policy gate is enabled
        assert!(
            crate::config::outbound_network_reporting_disabled(),
            "Fork policy requires outbound network reporting to be disabled"
        );
        // When the policy gate is enabled, handle_flush_logs should exit with code 0
        // without performing any network operations
    }

    #[test]
    fn test_sanitize_git_url_removes_password() {
        // Test that passwords are sanitized from git URLs
        let url = "https://user:password@github.com/repo.git";
        let sanitized = sanitize_git_url(url);
        assert_eq!(sanitized, "https://user:*****@github.com/repo.git");
    }

    #[test]
    fn test_sanitize_git_url_no_password() {
        // Test that URLs without passwords are unchanged
        let url = "https://github.com/repo.git";
        let sanitized = sanitize_git_url(url);
        assert_eq!(sanitized, "https://github.com/repo.git");
    }

    #[test]
    fn test_sanitize_git_url_ssh() {
        // Test that SSH URLs are unchanged
        let url = "git@github.com:user/repo.git";
        let sanitized = sanitize_git_url(url);
        assert_eq!(sanitized, "git@github.com:user/repo.git");
    }
}
