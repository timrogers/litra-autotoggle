# Code Implementation Examples

This document provides code examples for implementing automatic update checks using direct GitHub API calls.

> **Note:** These examples use direct GitHub API calls with `reqwest` rather than the `update-informer` crate. See [direct-api-analysis.md](./direct-api-analysis.md) for the rationale behind this decision.

## 1. Update Checker Module (src/update_checker.rs)

```rust
use chrono::{DateTime, Utc};
use log::{debug, warn};
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// GitHub API constants
const GITHUB_API_BASE: &str = "https://api.github.com";
const REPO_OWNER: &str = "timrogers";
const REPO_NAME: &str = "litra-autotoggle";
const CACHE_FILE_NAME: &str = "update-cache.json";
const DEFAULT_CHECK_INTERVAL_HOURS: u64 = 24;
const UPDATE_CHECK_TIMEOUT_SECS: u64 = 5;

/// GitHub Release API response
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    published_at: String,
}

/// Cache structure for storing update check results
#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    last_checked: DateTime<Utc>,
    latest_version: Option<String>,
    current_version: String,
}

/// Check if update checks are disabled via environment variable or CI detection
pub fn is_update_check_disabled() -> bool {
    // Explicit disable flag
    if std::env::var("LITRA_AUTOTOGGLE_NO_UPDATE_CHECK").is_ok() {
        return true;
    }

    // Disable in CI environments by default
    if std::env::var("CI").is_ok() {
        debug!("Running in CI environment, update checks disabled by default");
        return true;
    }

    false
}

/// Get the cache directory for storing update check results
fn get_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|mut path| {
        path.push("litra-autotoggle");
        path
    })
}

/// Get the cache file path
fn get_cache_file_path() -> Option<PathBuf> {
    get_cache_dir().map(|mut path| {
        path.push(CACHE_FILE_NAME);
        path
    })
}

/// Read the cache from disk
fn read_cache(cache_path: &Path) -> Option<UpdateCache> {
    match fs::read_to_string(cache_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(cache) => Some(cache),
            Err(e) => {
                debug!("Failed to parse cache file: {}", e);
                None
            }
        },
        Err(_) => None,
    }
}

/// Write the cache to disk
fn write_cache(cache_path: &Path, cache: &UpdateCache) -> Result<(), Box<dyn std::error::Error>> {
    // Create cache directory if it doesn't exist
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(cache)?;
    fs::write(cache_path, json)?;
    Ok(())
}

/// Check if the cache is expired based on the interval
fn is_cache_expired(cache: &UpdateCache, interval_hours: u64) -> bool {
    let now = Utc::now();
    let elapsed = now.signed_duration_since(cache.last_checked);
    elapsed.num_hours() >= interval_hours as i64
}

/// Get the update check interval from environment or config
fn get_check_interval_hours(config_interval: Option<u64>) -> u64 {
    // Check environment variable first
    if let Ok(val) = std::env::var("LITRA_AUTOTOGGLE_UPDATE_CHECK_INTERVAL") {
        if let Ok(hours) = val.parse::<u64>() {
            return hours;
        }
    }

    // Use config value or default
    config_interval.unwrap_or(DEFAULT_CHECK_INTERVAL_HOURS)
}

/// Check for updates using the GitHub Releases API
async fn check_github_releases(
    current_version: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let url = format!(
        "{}/repos/{}/{}/releases/latest",
        GITHUB_API_BASE, REPO_OWNER, REPO_NAME
    );

    // Build HTTP client with timeout and user agent
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(UPDATE_CHECK_TIMEOUT_SECS))
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?;

    debug!("Checking for updates at {}", url);

    // Make the API request
    let response = client.get(&url).send().await?;

    // Check if request was successful
    if !response.status().is_success() {
        debug!("GitHub API returned status: {}", response.status());
        return Ok(None);
    }

    // Parse JSON response
    let release: GitHubRelease = response.json().await?;
    debug!("Latest release: {}", release.tag_name);

    // Strip 'v' prefix if present
    let latest_version = release.tag_name.trim_start_matches('v').to_string();

    // Compare versions
    if latest_version != current_version {
        Ok(Some(latest_version))
    } else {
        Ok(None)
    }
}

/// Main function to check for updates with caching
pub async fn check_for_updates(
    current_version: &str,
    force_check: bool,
    config_interval: Option<u64>,
) -> Option<String> {
    // Check if updates are disabled
    if !force_check && is_update_check_disabled() {
        debug!("Update checks are disabled");
        return None;
    }

    // Get cache file path
    let cache_path = match get_cache_file_path() {
        Some(path) => path,
        None => {
            debug!("Could not determine cache directory");
            return None;
        }
    };

    // Get check interval
    let interval_hours = get_check_interval_hours(config_interval);

    // Check if we should skip based on cache
    if !force_check {
        if let Some(cache) = read_cache(&cache_path) {
            if cache.current_version == current_version && !is_cache_expired(&cache, interval_hours) {
                debug!("Using cached update check result");
                return cache.latest_version;
            }
        }
    }

    // Perform actual check using GitHub API
    debug!("Checking for updates...");
    let latest_version = match check_github_releases(current_version).await {
        Ok(version) => version,
        Err(e) => {
            warn!("Failed to check for updates: {}", e);
            None
        }
    };

    // Update cache
    let cache = UpdateCache {
        last_checked: Utc::now(),
        latest_version: latest_version.clone(),
        current_version: current_version.to_string(),
    };

    if let Err(e) = write_cache(&cache_path, &cache) {
        debug!("Failed to write cache: {}", e);
    }

    latest_version
}

/// Format the update notification message
pub fn format_update_message(current_version: &str, latest_version: &str) -> String {
    format!(
        r#"
╭──────────────────────────────────────────────────────╮
│ A new version of litra-autotoggle is available!     │
│ Current: v{:<7} → Latest: v{:<7}                 │
│                                                       │
│ Update instructions:                                 │
│ • Homebrew: brew upgrade litra-autotoggle            │
│ • Cargo:    cargo install litra-autotoggle           │
│ • Binary:   https://github.com/timrogers/           │
│             litra-autotoggle/releases/latest         │
│                                                       │
│ To disable these checks, set:                        │
│ export LITRA_AUTOTOGGLE_NO_UPDATE_CHECK=1           │
╰──────────────────────────────────────────────────────╯
"#,
        current_version, latest_version
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_update_check_disabled_when_env_var_set() {
        std::env::set_var("LITRA_AUTOTOGGLE_NO_UPDATE_CHECK", "1");
        assert!(is_update_check_disabled());
        std::env::remove_var("LITRA_AUTOTOGGLE_NO_UPDATE_CHECK");
    }

    #[test]
    fn test_is_update_check_disabled_in_ci() {
        std::env::set_var("CI", "true");
        assert!(is_update_check_disabled());
        std::env::remove_var("CI");
    }

    #[test]
    fn test_cache_expiration() {
        let old_cache = UpdateCache {
            last_checked: Utc::now() - chrono::Duration::hours(25),
            latest_version: Some("1.3.0".to_string()),
            current_version: "1.2.0".to_string(),
        };
        assert!(is_cache_expired(&old_cache, 24));

        let fresh_cache = UpdateCache {
            last_checked: Utc::now() - chrono::Duration::hours(12),
            latest_version: Some("1.3.0".to_string()),
            current_version: "1.2.0".to_string(),
        };
        assert!(!is_cache_expired(&fresh_cache, 24));
    }

    #[test]
    fn test_format_update_message() {
        let message = format_update_message("1.3.0", "1.4.0");
        assert!(message.contains("1.3.0"));
        assert!(message.contains("1.4.0"));
        assert!(message.contains("brew upgrade"));
        assert!(message.contains("cargo install"));
    }
}
```

## 2. Integration in main.rs

```rust
// Add to the top of main.rs
mod update_checker;

// In the Cli struct, add new flags:
#[derive(Debug, Parser)]
#[clap(name = "litra-autotoggle", version)]
struct Cli {
    // ... existing fields ...

    #[clap(
        long,
        action,
        help = "Disable automatic update check for this run"
    )]
    no_update_check: bool,

    #[clap(
        long,
        action,
        help = "Force an immediate update check, ignoring cache"
    )]
    check_update: bool,
}

// In the Config struct, add:
#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct Config {
    // ... existing fields ...

    #[serde(skip_serializing_if = "Option::is_none")]
    disable_update_check: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    update_check_interval_hours: Option<u64>,
}

// In the main() function, add after config parsing:
#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();

    // Merge config file with CLI arguments
    let args = match merge_config_with_cli(args) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    // Perform update check if not disabled
    if !args.no_update_check && !args.config.disable_update_check.unwrap_or(false) {
        // Spawn update check as a background task to not block startup
        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let force_check = args.check_update;
        let check_interval = args.config.update_check_interval_hours;

        tokio::spawn(async move {
            if let Some(latest_version) = 
                update_checker::check_for_updates(&current_version, force_check, check_interval).await 
            {
                println!("{}", update_checker::format_update_message(&current_version, &latest_version));
            }
        });

        // Small delay to allow update check to complete if it's fast
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Continue with normal application logic...
    let result = handle_autotoggle_command(
        args.serial_number.as_deref(),
        args.device_path.as_deref(),
        args.device_type.as_deref(),
        args.require_device,
        #[cfg(target_os = "linux")]
        args.video_device.as_deref(),
        args.delay,
        args.back,
    )
    .await;

    if let Err(error) = result {
        error!("{error}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
```

## 3. Updated Cargo.toml Dependencies

```toml
[dependencies]
clap = { version = "4.5.54", features = ["derive"] }
env_logger = "0.11.5"
litra = { version = "3.1.1", default-features = false }
log = "0.4.29"
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1.0"
tokio = { version = "1.49.0", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
dirs = "5.0"
chrono = { version = "0.4", features = ["serde"] }

[target.'cfg(target_os = "linux")'.dependencies]
inotify = { version = "0.11.0" }

[target.'cfg(target_os = "windows")'.dependencies]
winreg = "0.55"

[dev-dependencies]
tempfile = "3.24"
```

**Key changes:**
- Added `reqwest` with `rustls-tls` feature (avoids OpenSSL dependency)
- Added `dirs` for platform-specific cache directories
- Added `chrono` for timestamp handling
- `serde_json` for cache serialization (may already be present)

## 4. Updated Config File Example (litra-autotoggle.example.yml)

```yaml
# By default, the tool will control all connected Litra devices. You can specify ONE
# of the below filters to limit which device(s) it will control.
#
# device_type: glow
# serial_number: ABCD1
# device_path: DevSrvsID:4296789687
#
# By default, the tool will watch all connected video devices. On Linux, you can limit
# this to one specific device by specifying its path below.
#
# video_device: /dev/video0
#
# By default, the tool will wait 1.5 seconds after a video device event before toggling
# the light to reduce flickering. You can adjust this delay (in milliseconds) below.
#
# delay: 2000
#
# By default, if no Litra devices are found, the tool will keep running. You can change this
# behavior by setting the option below to true.
#
# require_device: true
#
# By default, the tool emits simple logs. You can enable debug logging by setting the option
# below to true.
#
# verbose: true
#
# By default, only the front light is toggled. On Litra Beam LX devices, you can also toggle
# the back light by setting the option below to true.
#
# back: true
#
# Update Check Settings
# ---------------------
# By default, litra-autotoggle checks for updates once every 24 hours using the GitHub
# releases API. This helps you stay informed about new features and bug fixes.
#
# To disable automatic update checks, set the option below to true.
#
# disable_update_check: false
#
# You can customize how often update checks occur (in hours). Default is 24 hours.
#
# update_check_interval_hours: 48
#
# Note: You can also disable update checks with the environment variable:
# export LITRA_AUTOTOGGLE_NO_UPDATE_CHECK=1
```

## 5. Usage Examples

### Check for updates on every run
```bash
litra-autotoggle --check-update
```

### Disable update check for a single run
```bash
litra-autotoggle --no-update-check
```

### Disable update checks via environment variable
```bash
export LITRA_AUTOTOGGLE_NO_UPDATE_CHECK=1
litra-autotoggle
```

### Configure update check interval
```bash
# Check every 48 hours instead of 24
export LITRA_AUTOTOGGLE_UPDATE_CHECK_INTERVAL=48
litra-autotoggle
```

### Using config file
```yaml
# config.yml
disable_update_check: false
update_check_interval_hours: 72
```

```bash
litra-autotoggle --config-file config.yml
```

## 6. Testing Examples

### Unit Test for Update Checker
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_cache_read_write() {
        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("cache.json");

        let cache = UpdateCache {
            last_checked: Utc::now(),
            latest_version: Some("1.4.0".to_string()),
            current_version: "1.3.0".to_string(),
        };

        // Write cache
        write_cache(&cache_path, &cache).unwrap();

        // Read cache
        let read_cache = read_cache(&cache_path).unwrap();
        assert_eq!(read_cache.current_version, "1.3.0");
        assert_eq!(read_cache.latest_version, Some("1.4.0".to_string()));
    }

    #[tokio::test]
    async fn test_force_check() {
        // Force check should ignore cache
        let result = check_for_updates("1.3.0", true, None).await;
        // Result depends on actual GitHub API, but should not panic
        assert!(result.is_some() || result.is_none());
    }
}
```

### Integration Test
```rust
#[tokio::test]
async fn test_update_check_with_disabled_env_var() {
    std::env::set_var("LITRA_AUTOTOGGLE_NO_UPDATE_CHECK", "1");
    
    let result = check_for_updates("1.3.0", false, None).await;
    
    assert!(result.is_none());
    
    std::env::remove_var("LITRA_AUTOTOGGLE_NO_UPDATE_CHECK");
}
```

## Notes

- All code examples are illustrative and may need adjustments based on the actual codebase structure
- Error handling should be comprehensive in production code
- Consider adding more detailed logging for debugging
- Platform-specific code should be properly gated with `#[cfg(...)]`
- Performance should be monitored to ensure minimal startup delay
