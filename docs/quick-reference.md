# Auto-Update Checks: Quick Reference Card

A one-page reference for implementing auto-update checks in litra-autotoggle.

## The Essentials

**Goal**: Notify users when a new version is available via GitHub Releases API  
**Approach**: Direct API calls with existing `reqwest`, opt-in by default, non-blocking  
**Effort**: ~6 hours implementation + testing

---

## Quick Implementation Guide

### 1. Add Configuration (2 min)

```rust
// In Config struct
#[serde(skip_serializing_if = "Option::is_none")]
check_updates: Option<bool>,

// In Cli struct
#[clap(long, action)]
check_updates: bool,

#[clap(long, action)]
no_update_check: bool,
```

### 2. Add API Response Struct (1 min)

```rust
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}
```

### 3. Add Cache Struct (2 min)

```rust
#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    last_check_timestamp: u64,
    last_checked_version: String,
    last_notified_version: Option<String>,
}
```

### 4. Implement Version Comparison (5 min)

```rust
fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.trim_start_matches('v').split('.').collect();
        Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
    };
    match (parse(current), parse(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}
```

### 5. Implement Cache Functions (15 min)

```rust
fn get_cache_file_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    { /* XDG_CONFIG_HOME or ~/.config/litra-autotoggle/last_update_check */ }
    #[cfg(target_os = "macos")]
    { /* ~/Library/Application Support/litra-autotoggle/last_update_check */ }
    #[cfg(target_os = "windows")]
    { /* %APPDATA%\litra-autotoggle\last_update_check */ }
}

fn read_update_cache() -> Result<UpdateCache, std::io::Error> { /* ... */ }
fn write_update_cache(cache: &UpdateCache) -> Result<(), std::io::Error> { /* ... */ }
```

### 6. Implement API Client (20 min)

```rust
async fn check_for_updates() -> Result<Option<GitHubRelease>, Box<dyn std::error::Error>> {
    let response = reqwest::Client::new()
        .get("https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest")
        .header("User-Agent", format!("litra-autotoggle/{}", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;
    
    if !response.status().is_success() { return Ok(None); }
    Ok(Some(response.json().await?))
}
```

### 7. Implement Notification (5 min)

```rust
fn display_update_notification(current: &str, latest: &str, url: &str) {
    info!("╭──────────────────────────────────────────────────────────╮");
    info!("│ A new version of litra-autotoggle is available!         │");
    info!("│ Current: {:<47} │", current);
    info!("│ Latest:  {:<47} │", latest);
    info!("│ Download: {:<47} │", url);
    info!("╰──────────────────────────────────────────────────────────╯");
}
```

### 8. Implement Orchestration (20 min)

```rust
async fn perform_update_check(interval_hours: u64) -> Result<(), Box<dyn std::error::Error>> {
    let mut cache = read_update_cache().unwrap_or_default();
    
    // Check if we need to run
    if !cache.is_check_needed(interval_hours) { return Ok(()); }
    
    // Check API
    if let Some(release) = check_for_updates().await? {
        let current = env!("CARGO_PKG_VERSION");
        cache.last_check_timestamp = current_timestamp();
        
        if is_newer_version(current, &release.tag_name) && cache.should_notify(&release.tag_name) {
            display_update_notification(current, &release.tag_name, &release.html_url);
            cache.last_notified_version = Some(release.tag_name);
        }
        
        write_update_cache(&cache)?;
    }
    Ok(())
}

fn spawn_update_check(interval_hours: u64) {
    tokio::spawn(async move {
        if let Err(e) = perform_update_check(interval_hours).await {
            warn!("Update check failed: {}", e);
        }
    });
}
```

### 9. Integrate into Main (5 min)

```rust
#[tokio::main]
async fn main() -> ExitCode {
    // ... existing initialization ...
    
    let should_check = if cli.no_update_check {
        false
    } else {
        cli.check_updates || config.check_updates.unwrap_or(false)
    };
    
    if should_check {
        spawn_update_check(24); // 24 hours default
    }
    
    // ... rest of main ...
}
```

---

## API Quick Reference

**URL**: `https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest`

**Headers**:
- `User-Agent: litra-autotoggle/{version}` (required)
- `Accept: application/vnd.github+json`

**Response** (we need):
```json
{
  "tag_name": "v1.4.0",
  "html_url": "https://github.com/.../releases/tag/v1.4.0"
}
```

**Rate Limit**: 60 req/hour (unauthenticated) → Mitigate with 24h cache

---

## Testing Checklist

### Unit Tests
```rust
#[test]
fn test_version_comparison() {
    assert!(is_newer_version("1.0.0", "1.0.1"));
    assert!(!is_newer_version("1.0.1", "1.0.0"));
}

#[test]
fn test_cache_serialization() {
    let cache = UpdateCache { /* ... */ };
    let json = serde_json::to_string(&cache).unwrap();
    let parsed: UpdateCache = serde_json::from_str(&json).unwrap();
    assert_eq!(cache.last_check_timestamp, parsed.last_check_timestamp);
}
```

### Manual Tests
- [ ] Enable via config file → see notification (if version differs)
- [ ] Enable via `--check-updates` flag → works
- [ ] Disable via `--no-update-check` flag → no check
- [ ] Second run within 24h → no redundant check
- [ ] Test on Linux, macOS, Windows
- [ ] Test with network disconnected → graceful failure
- [ ] Check cache file created in correct location

---

## Common Pitfalls & Solutions

### ❌ Pitfall: Blocking Startup
**Solution**: Use `tokio::spawn()`, don't `.await` in main thread

### ❌ Pitfall: Rate Limiting
**Solution**: 24-hour cache, check `x-ratelimit-remaining` header

### ❌ Pitfall: Version "1.10.0" < "1.9.0" (string comparison)
**Solution**: Parse as (major, minor, patch) tuples

### ❌ Pitfall: Network failures crash program
**Solution**: Wrap in Result, log errors, never propagate

### ❌ Pitfall: Repeated notifications
**Solution**: Track `last_notified_version` in cache

### ❌ Pitfall: Cache in wrong location on different platforms
**Solution**: Use platform-specific paths with cfg attributes

---

## Configuration Examples

### In YAML Config
```yaml
check_updates: true
```

### In Code
```rust
let should_check = cli.check_updates 
    || config.check_updates.unwrap_or(false);
```

### Command Line
```bash
litra-autotoggle --check-updates       # Enable
litra-autotoggle --no-update-check     # Disable
```

---

## Cache File Locations

| Platform | Path |
|----------|------|
| Linux | `~/.config/litra-autotoggle/last_update_check` |
| macOS | `~/Library/Application Support/litra-autotoggle/last_update_check` |
| Windows | `%APPDATA%\litra-autotoggle\last_update_check` |

---

## Error Handling Pattern

```rust
match check_for_updates().await {
    Ok(Some(release)) => { /* process update */ },
    Ok(None) => debug!("No update available"),
    Err(e) => {
        warn!("Update check failed: {}. Continuing normally.", e);
        return Ok(()); // Don't propagate!
    }
}
```

---

## Documentation Updates Needed

### README.md
Add to "Using a configuration file" section:
```yaml
# Enable automatic update checks (default: false)
check_updates: true
```

Add to "From the command line" section:
- `--check-updates` to enable update checks on this run
- `--no-update-check` to disable update checks even if configured

### litra-autotoggle.example.yml
Add:
```yaml
# Enable automatic update checks (default: false for privacy)
# check_updates: true
```

---

## Memory Aid: Implementation Order

1. **Config** → Add fields to Config and Cli structs
2. **Types** → Define GitHubRelease and UpdateCache
3. **Version** → Implement is_newer_version()
4. **Cache** → Implement get_cache_file_path(), read/write functions
5. **API** → Implement check_for_updates()
6. **Notify** → Implement display_update_notification()
7. **Orchestrate** → Implement perform_update_check() and spawn_update_check()
8. **Integrate** → Add to main() function
9. **Test** → Unit tests + manual testing
10. **Document** → Update README and example config

---

## Time Estimates

| Task | Time | Cumulative |
|------|------|------------|
| Add configuration | 10 min | 10 min |
| Add data structures | 10 min | 20 min |
| Version comparison | 15 min | 35 min |
| Cache management | 30 min | 65 min |
| API client | 30 min | 95 min |
| Orchestration | 30 min | 125 min |
| Integration | 20 min | 145 min |
| Unit tests | 45 min | 190 min |
| Manual testing | 60 min | 250 min |
| Documentation | 30 min | 280 min |
| **Total** | **~4.5 hours** | |

---

## When You're Stuck

1. **Can't parse version?** → See code example #4 (version comparison)
2. **Cache not working?** → Check platform-specific path in code example #5
3. **API failing?** → Check headers (User-Agent required), verify URL
4. **Rate limited?** → Check cache is working, verify 24h interval
5. **Main blocked?** → Ensure using `tokio::spawn()`, not `.await`

---

## Success Checklist

Before submitting PR:

- [ ] Runs on Linux, macOS, Windows
- [ ] No startup delay (async spawn)
- [ ] Graceful with network errors
- [ ] Respects `--no-update-check` flag
- [ ] Cache prevents redundant checks
- [ ] Notification shows when version differs
- [ ] No notification on repeated runs (cache tracking)
- [ ] Unit tests pass
- [ ] `cargo clippy` passes
- [ ] Documentation updated

---

**Pro Tip**: Copy code examples from `auto-update-checks-code-examples.md` - they're production-ready!

**Remember**: Update checks should NEVER block or crash the application. When in doubt, fail gracefully.

---

**Document Version**: 1.0  
**Estimated Implementation Time**: 4-6 hours  
**Last Updated**: 2026-02-06
