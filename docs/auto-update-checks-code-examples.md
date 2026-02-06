# Auto-Update Checks: Code Examples and API Details

This document provides detailed code examples and API interaction patterns for implementing auto-update checks in litra-autotoggle.

## GitHub Releases API Details

### API Endpoint
```
GET https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest
```

### Required Headers
```rust
let client = reqwest::Client::new();
let response = client
    .get("https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest")
    .header("User-Agent", format!("litra-autotoggle/{}", env!("CARGO_PKG_VERSION")))
    .header("Accept", "application/vnd.github+json")
    .timeout(std::time::Duration::from_secs(5))
    .send()
    .await?;
```

### Example API Response
```json
{
  "url": "https://api.github.com/repos/timrogers/litra-autotoggle/releases/123456",
  "html_url": "https://github.com/timrogers/litra-autotoggle/releases/tag/v1.4.0",
  "assets_url": "https://api.github.com/repos/timrogers/litra-autotoggle/releases/123456/assets",
  "upload_url": "https://uploads.github.com/repos/timrogers/litra-autotoggle/releases/123456/assets{?name,label}",
  "tarball_url": "https://api.github.com/repos/timrogers/litra-autotoggle/tarball/v1.4.0",
  "zipball_url": "https://api.github.com/repos/timrogers/litra-autotoggle/zipball/v1.4.0",
  "id": 123456,
  "node_id": "RE_kwDOABC123",
  "tag_name": "v1.4.0",
  "target_commitish": "main",
  "name": "v1.4.0",
  "body": "## What's Changed\n\n* Feature: New functionality\n* Fix: Bug fixes\n\n**Full Changelog**: https://github.com/timrogers/litra-autotoggle/compare/v1.3.0...v1.4.0",
  "draft": false,
  "prerelease": false,
  "created_at": "2025-02-01T10:30:00Z",
  "published_at": "2025-02-01T10:35:00Z",
  "author": {
    "login": "timrogers",
    "id": 1234567,
    "avatar_url": "https://avatars.githubusercontent.com/u/1234567?v=4",
    "type": "User"
  },
  "assets": [
    {
      "url": "https://api.github.com/repos/timrogers/litra-autotoggle/releases/assets/456789",
      "browser_download_url": "https://github.com/timrogers/litra-autotoggle/releases/download/v1.4.0/litra-autotoggle_v1.4.0_linux-amd64",
      "id": 456789,
      "name": "litra-autotoggle_v1.4.0_linux-amd64",
      "label": "",
      "state": "uploaded",
      "content_type": "application/octet-stream",
      "size": 4567890,
      "download_count": 42,
      "created_at": "2025-02-01T10:32:00Z",
      "updated_at": "2025-02-01T10:33:00Z"
    }
  ]
}
```

### Minimal Struct for Deserialization
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}
```

## Complete Implementation Example

### 1. Data Structures

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// GitHub API release response
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

/// Cache file structure
#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    last_check_timestamp: u64,
    last_checked_version: String,
    last_notified_version: Option<String>,
}

impl UpdateCache {
    fn new() -> Self {
        Self {
            last_check_timestamp: 0,
            last_checked_version: String::new(),
            last_notified_version: None,
        }
    }
    
    fn is_check_needed(&self, interval_hours: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let interval_secs = interval_hours * 3600;
        now - self.last_check_timestamp >= interval_secs
    }
    
    fn should_notify(&self, latest_version: &str) -> bool {
        match &self.last_notified_version {
            Some(notified) => notified != latest_version,
            None => true,
        }
    }
}
```

### 2. Cache Management

```rust
use std::fs;
use std::io::ErrorKind;

fn get_cache_file_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let xdg_config = std::env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
                format!("{}/.config", home)
            });
        PathBuf::from(xdg_config)
            .join("litra-autotoggle")
            .join("last_update_check")
    }
    
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("litra-autotoggle")
            .join("last_update_check")
    }
    
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA")
            .unwrap_or_else(|_| String::from("."));
        PathBuf::from(appdata)
            .join("litra-autotoggle")
            .join("last_update_check")
    }
}

fn read_update_cache() -> Result<UpdateCache, std::io::Error> {
    let path = get_cache_file_path();
    match fs::read_to_string(&path) {
        Ok(contents) => {
            serde_json::from_str(&contents)
                .map_err(|e| std::io::Error::new(ErrorKind::InvalidData, e))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            Ok(UpdateCache::new())
        }
        Err(e) => Err(e),
    }
}

fn write_update_cache(cache: &UpdateCache) -> Result<(), std::io::Error> {
    let path = get_cache_file_path();
    
    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let json = serde_json::to_string(cache)
        .map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
    
    fs::write(path, json)
}
```

### 3. Version Comparison

```rust
/// Compare two semantic version strings
/// Returns true if `latest` is newer than `current`
fn is_newer_version(current: &str, latest: &str) -> bool {
    // Strip 'v' prefix if present
    let current = current.trim_start_matches('v');
    let latest = latest.trim_start_matches('v');
    
    // Parse versions as (major, minor, patch)
    let parse_version = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            parts[2].parse().ok()?,
        ))
    };
    
    match (parse_version(current), parse_version(latest)) {
        (Some(curr), Some(lat)) => lat > curr,
        _ => {
            // Fall back to string comparison if parsing fails
            latest > current
        }
    }
}

#[cfg(test)]
mod version_tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.0.0", "1.0.1"));
        assert!(is_newer_version("1.0.0", "1.1.0"));
        assert!(is_newer_version("1.0.0", "2.0.0"));
        assert!(is_newer_version("v1.0.0", "v1.0.1"));
        assert!(!is_newer_version("1.0.1", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
    }
}
```

### 4. GitHub API Client

```rust
use log::{info, warn, debug};

/// Check for updates from GitHub Releases API
async fn check_for_updates() -> Result<Option<GitHubRelease>, Box<dyn std::error::Error>> {
    const REPO_OWNER: &str = "timrogers";
    const REPO_NAME: &str = "litra-autotoggle";
    const API_URL: &str = "https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest";
    
    debug!("Checking for updates from GitHub Releases API");
    
    let client = reqwest::Client::new();
    let user_agent = format!("litra-autotoggle/{}", env!("CARGO_PKG_VERSION"));
    
    let response = client
        .get(API_URL)
        .header("User-Agent", &user_agent)
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;
    
    // Check for rate limiting
    if response.status() == reqwest::StatusCode::FORBIDDEN {
        if let Some(remaining) = response.headers().get("x-ratelimit-remaining") {
            if remaining == "0" {
                info!("GitHub API rate limit reached. Skipping update check.");
                return Ok(None);
            }
        }
    }
    
    if !response.status().is_success() {
        warn!("Failed to check for updates: HTTP {}", response.status());
        return Ok(None);
    }
    
    let release: GitHubRelease = response.json().await?;
    
    // Skip pre-releases and drafts
    if release.prerelease || release.draft {
        debug!("Latest release is a pre-release or draft, skipping");
        return Ok(None);
    }
    
    Ok(Some(release))
}
```

### 5. Update Check Orchestration

```rust
/// Main update check function with caching and notification logic
async fn perform_update_check(interval_hours: u64) -> Result<(), Box<dyn std::error::Error>> {
    // Read cache
    let mut cache = read_update_cache().unwrap_or_else(|e| {
        warn!("Failed to read update cache: {}. Starting fresh.", e);
        UpdateCache::new()
    });
    
    // Check if we need to perform a check
    if !cache.is_check_needed(interval_hours) {
        debug!("Update check not needed yet (last check was recent)");
        return Ok(());
    }
    
    // Perform API request
    match check_for_updates().await {
        Ok(Some(release)) => {
            let current_version = env!("CARGO_PKG_VERSION");
            let latest_version = release.tag_name.trim_start_matches('v');
            
            // Update cache
            cache.last_check_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs();
            cache.last_checked_version = release.tag_name.clone();
            
            // Check if newer version is available
            if is_newer_version(current_version, latest_version) {
                // Only notify if we haven't already notified about this version
                if cache.should_notify(&release.tag_name) {
                    display_update_notification(current_version, latest_version, &release.html_url);
                    cache.last_notified_version = Some(release.tag_name.clone());
                }
            } else {
                debug!("Current version {} is up to date", current_version);
            }
            
            // Write updated cache
            if let Err(e) = write_update_cache(&cache) {
                warn!("Failed to write update cache: {}", e);
            }
        }
        Ok(None) => {
            debug!("Update check completed but no release information available");
        }
        Err(e) => {
            warn!("Failed to check for updates: {}. Continuing normally.", e);
        }
    }
    
    Ok(())
}

/// Spawn update check in background (non-blocking)
fn spawn_update_check(interval_hours: u64) {
    tokio::spawn(async move {
        if let Err(e) = perform_update_check(interval_hours).await {
            warn!("Update check failed: {}", e);
        }
    });
}
```

### 6. User Notification

```rust
fn display_update_notification(current_version: &str, latest_version: &str, url: &str) {
    info!("╭──────────────────────────────────────────────────────────╮");
    info!("│ A new version of litra-autotoggle is available!         │");
    info!("│                                                          │");
    info!("│ Current version: {:<39} │", current_version);
    info!("│ Latest version:  {:<39} │", latest_version);
    info!("│                                                          │");
    info!("│ Download: {:<47} │", url);
    info!("│                                                          │");
    info!("│ To disable update checks, add 'check_updates: false'   │");
    info!("│ to your config file or use --no-update-check           │");
    info!("╰──────────────────────────────────────────────────────────╯");
}
```

### 7. Integration with Main

```rust
#[tokio::main]
async fn main() -> ExitCode {
    // ... existing initialization code ...
    
    // Determine if update checks should be performed
    let should_check_updates = if cli.no_update_check {
        false
    } else if cli.check_updates {
        true
    } else {
        // Check config file
        config.check_updates.unwrap_or(false)
    };
    
    // Spawn update check in background if enabled
    if should_check_updates {
        let interval_hours = config.update_check_interval_hours.unwrap_or(24);
        spawn_update_check(interval_hours);
    }
    
    // ... rest of main function ...
}
```

## Error Handling Patterns

### Graceful Degradation
```rust
// Update checks should never crash the application
match check_for_updates().await {
    Ok(Some(release)) => {
        // Process release
    }
    Ok(None) => {
        // No update or rate limited - continue silently
    }
    Err(e) => {
        // Log error but don't propagate
        warn!("Update check failed: {}. Continuing normally.", e);
    }
}
```

### Network Timeout
```rust
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(5))
    .connect_timeout(std::time::Duration::from_secs(3))
    .build()?;
```

### Rate Limit Detection
```rust
if response.status() == reqwest::StatusCode::FORBIDDEN {
    if let Some(reset) = response.headers().get("x-ratelimit-reset") {
        if let Ok(reset_time) = reset.to_str() {
            info!("GitHub API rate limit reached. Resets at: {}", reset_time);
        }
    }
    return Ok(None);
}
```

## Platform-Specific Cache Paths

### Linux
```rust
// Respects XDG Base Directory specification
// Default: ~/.config/litra-autotoggle/last_update_check
let xdg_config = std::env::var("XDG_CONFIG_HOME")
    .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap()));
PathBuf::from(xdg_config).join("litra-autotoggle").join("last_update_check")
```

### macOS
```rust
// Follows macOS Application Support conventions
// ~/Library/Application Support/litra-autotoggle/last_update_check
let home = std::env::var("HOME").unwrap();
PathBuf::from(home)
    .join("Library")
    .join("Application Support")
    .join("litra-autotoggle")
    .join("last_update_check")
```

### Windows
```rust
// Uses APPDATA environment variable
// %APPDATA%\litra-autotoggle\last_update_check
let appdata = std::env::var("APPDATA").unwrap();
PathBuf::from(appdata)
    .join("litra-autotoggle")
    .join("last_update_check")
```

## Testing Examples

### Mock API Response
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_github_release() {
        let json = r#"{
            "tag_name": "v1.4.0",
            "html_url": "https://github.com/timrogers/litra-autotoggle/releases/tag/v1.4.0",
            "prerelease": false,
            "draft": false
        }"#;
        
        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v1.4.0");
        assert!(!release.prerelease);
        assert!(!release.draft);
    }
    
    #[test]
    fn test_cache_serialization() {
        let cache = UpdateCache {
            last_check_timestamp: 1706180000,
            last_checked_version: "v1.3.0".to_string(),
            last_notified_version: Some("v1.3.0".to_string()),
        };
        
        let json = serde_json::to_string(&cache).unwrap();
        let deserialized: UpdateCache = serde_json::from_str(&json).unwrap();
        
        assert_eq!(cache.last_check_timestamp, deserialized.last_check_timestamp);
        assert_eq!(cache.last_checked_version, deserialized.last_checked_version);
    }
}
```

## Configuration Examples

### Minimal Config (opt-in)
```yaml
check_updates: true
```

### Full Config
```yaml
check_updates: true
update_check_interval_hours: 24
```

### CLI Usage
```bash
# Enable for this run only
litra-autotoggle --check-updates

# Disable even if configured
litra-autotoggle --no-update-check

# Use with config file
litra-autotoggle --config-file config.yml
```

## Performance Considerations

### Non-Blocking Execution
```rust
// Spawn in background, don't await
tokio::spawn(async move {
    perform_update_check(24).await.ok();
});

// Main function continues immediately
info!("Starting litra-autotoggle...");
```

### Memory Usage
- Cache file: ~100 bytes (JSON with 3 fields)
- In-memory structures: < 1 KB
- HTTP client: Reused, minimal overhead

### Network Usage
- Single HTTP GET request
- Response size: ~2-5 KB (compressed)
- Frequency: Once per 24 hours (default)
- Total: ~150 KB per month (worst case)

## Security Best Practices

1. **Always use HTTPS** - GitHub API enforces this
2. **No authentication tokens** - Public API, no secrets needed
3. **Validate version strings** - Parse before comparison
4. **Timeout requests** - Prevent hanging
5. **No auto-execution** - Only notify, never auto-update
6. **Fail safely** - Never crash on update check failure

## References

- [GitHub REST API - Releases](https://docs.github.com/en/rest/releases/releases?apiVersion=latest)
- [reqwest Documentation](https://docs.rs/reqwest/latest/reqwest/)
- [serde_json Documentation](https://docs.rs/serde_json/latest/serde_json/)
- [tokio spawn Documentation](https://docs.rs/tokio/latest/tokio/fn.spawn.html)
