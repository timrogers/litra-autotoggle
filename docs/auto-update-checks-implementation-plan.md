# Auto-Update Checks Implementation Plan

## Overview

This document outlines a comprehensive plan for implementing automatic update checks in `litra-autotoggle` using the GitHub Releases API. The implementation will follow Rust CLI best practices and provide a user-friendly, privacy-respecting approach to notifying users about new versions.

## Current State

### Existing Infrastructure
- **Current Version**: 1.3.0 (from Cargo.toml)
- **HTTP Client**: Already has `reqwest` with `rustls-tls` features for HTTP requests
- **Async Runtime**: Uses `tokio` with full features
- **Release Process**: Automated releases via GitHub Actions when tags starting with 'v' are pushed
- **Release Assets**: Binaries for Linux (x86_64, ARM64), macOS (Intel, ARM, Universal), Windows (x86_64, ARM64)
- **Configuration**: YAML-based configuration file support already exists

### Dependencies Already Available
- `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }` ✅
- `tokio = { version = "1.49.0", features = ["full"] }` ✅
- `serde = { version = "1.0", features = ["derive"] }` ✅
- `log = "0.4.29"` ✅

## Implementation Strategy

### 1. Core Components

#### 1.1 GitHub Releases API Integration

**Endpoint**: `GET https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest`

**Response Structure** (key fields):
```json
{
  "tag_name": "v1.3.0",
  "name": "v1.3.0",
  "html_url": "https://github.com/timrogers/litra-autotoggle/releases/tag/v1.3.0",
  "published_at": "2025-01-15T10:30:00Z",
  "body": "Release notes...",
  "prerelease": false,
  "draft": false
}
```

**Required Struct**:
```rust
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    published_at: String,
}
```

#### 1.2 Version Comparison

**Approach**: Use semver-style comparison
- Current version from: `env!("CARGO_PKG_VERSION")` 
- Latest version from: API response's `tag_name` (strip leading 'v')
- Simple string comparison after normalization (major.minor.patch)

**Optional Dependency to Add**:
```toml
semver = "1.0"  # For robust version comparison (optional but recommended)
```

#### 1.3 Update Check Function

**Location**: `src/main.rs` (add new function)

**Signature**:
```rust
async fn check_for_updates(repo_owner: &str, repo_name: &str) -> Result<Option<String>, Box<dyn std::error::Error>>
```

**Logic**:
1. Make HTTP GET request to GitHub Releases API
2. Set User-Agent header (required by GitHub): `litra-autotoggle/{version}`
3. Parse JSON response
4. Compare versions
5. Return `Some(new_version)` if update available, `None` otherwise
6. Handle errors gracefully (network, rate limits, parsing)

**Error Handling**:
- Network failures: Log warning, don't fail the program
- Rate limiting: Log info message about check skipped
- Parse errors: Log warning, continue
- Never block the main application functionality

### 2. Configuration Options

Add to `Config` struct and `Cli` struct:

```rust
// In Config struct
#[serde(skip_serializing_if = "Option::is_none")]
check_updates: Option<bool>,

#[serde(skip_serializing_if = "Option::is_none")]
update_check_interval_hours: Option<u64>,

// In Cli struct  
#[clap(long, action, help = "Check for updates on startup")]
check_updates: bool,

#[clap(long, action, help = "Skip update checks even if configured")]
no_update_check: bool,
```

**Config File Example**:
```yaml
# Enable automatic update checks (default: false for privacy)
check_updates: true

# How often to check for updates in hours (default: 24)
# update_check_interval_hours: 24
```

**Default Behavior**:
- Updates checks: **OFF by default** (opt-in, privacy-respecting)
- Check interval: 24 hours (when enabled)
- Non-blocking: Runs in background, doesn't delay startup

### 3. Update Check Timing Strategy

**Option A: Startup Check with Caching (Recommended)**
- Check on startup if enabled
- Cache last check time in a state file (e.g., `~/.config/litra-autotoggle/last_update_check`)
- Only check if interval has elapsed
- Run check asynchronously in background (spawn task)

**Option B: Periodic Background Checks**
- After startup, schedule periodic checks
- Use tokio interval timer
- More complex, may not be needed for a tool that runs continuously

**Recommendation**: Implement Option A for simplicity and effectiveness

**Cache File Location** (platform-specific):
- Linux: `~/.config/litra-autotoggle/last_update_check` or `$XDG_CONFIG_HOME/litra-autotoggle/last_update_check`
- macOS: `~/Library/Application Support/litra-autotoggle/last_update_check`
- Windows: `%APPDATA%\litra-autotoggle\last_update_check`

**Cache Format** (simple):
```
1706180000
v1.3.0
```
(Unix timestamp of last check + version checked)

### 4. User Notification

**Notification Format**:
```
╭──────────────────────────────────────────────────────────╮
│ A new version of litra-autotoggle is available!         │
│                                                          │
│ Current version: 1.3.0                                  │
│ Latest version:  1.4.0                                  │
│                                                          │
│ Download: https://github.com/timrogers/litra-autotoggle/releases/latest │
│                                                          │
│ To disable update checks, add 'check_updates: false'   │
│ to your config file or use --no-update-check           │
╰──────────────────────────────────────────────────────────╯
```

**Implementation**:
- Use `info!` log level (visible by default)
- Show once per new version (track in cache)
- Don't repeat on every startup
- Include direct link to latest release

### 5. Rate Limiting & API Usage

**GitHub API Rate Limits**:
- Unauthenticated: 60 requests per hour per IP
- Authenticated: 5,000 requests per hour (not needed for this use case)

**Mitigation Strategy**:
1. Default check interval: 24 hours (very conservative)
2. Cache results locally
3. Fail gracefully on rate limit (log info, continue)
4. Use conditional requests (If-None-Match header) to save quota when possible

**Headers to Include**:
```rust
.header("User-Agent", format!("litra-autotoggle/{}", env!("CARGO_PKG_VERSION")))
.header("Accept", "application/vnd.github+json")
```

### 6. Privacy & Transparency

**Privacy Considerations**:
- ✅ Opt-in by default (no automatic checks without user consent)
- ✅ Clear documentation about what data is sent (just HTTP GET request)
- ✅ Easy opt-out mechanism
- ✅ No telemetry or tracking
- ✅ Direct GitHub API calls (no intermediary services)
- ✅ Local caching to minimize requests

**What Gets Sent**:
- User's IP address (inherent to HTTP)
- User-Agent header with tool name and version
- No user-specific identifiers
- No usage data or telemetry

### 7. Testing Strategy

#### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        // Test version parsing and comparison logic
    }

    #[test]
    fn test_cache_read_write() {
        // Test cache file operations
    }
}
```

#### Integration Tests
- Mock GitHub API responses
- Test rate limiting handling
- Test network error handling
- Test cache expiration logic

#### Manual Testing
1. Enable update checks with current version
2. Verify no update notification (current is latest)
3. Manually set cache to old version
4. Verify update notification appears
5. Verify notification doesn't repeat
6. Test with `--no-update-check` flag
7. Test with network disconnected

### 8. Code Structure

**New Functions**:
```rust
// Main update checking logic
async fn check_for_updates() -> Result<Option<UpdateInfo>, UpdateCheckError>

// Version comparison
fn is_newer_version(current: &str, latest: &str) -> bool

// Cache management
fn read_update_cache() -> Result<UpdateCache, std::io::Error>
fn write_update_cache(cache: &UpdateCache) -> Result<(), std::io::Error>
fn get_cache_file_path() -> PathBuf

// Notification
fn display_update_notification(current: &str, latest: &str, url: &str)

// Async wrapper for non-blocking check
fn spawn_update_check(config: UpdateCheckConfig)
```

**Error Type**:
```rust
#[derive(Debug)]
enum UpdateCheckError {
    NetworkError(reqwest::Error),
    RateLimited,
    ParseError(serde_json::Error),
    CacheError(std::io::Error),
}
```

### 9. Implementation Steps

1. **Add semver dependency** (optional but recommended)
   ```toml
   semver = "1.0"
   ```

2. **Add update check structs and types**
   - `GitHubRelease` struct
   - `UpdateCache` struct
   - `UpdateCheckError` enum
   - Configuration fields

3. **Implement cache management**
   - `get_cache_file_path()` with platform-specific logic
   - `read_update_cache()` and `write_update_cache()`

4. **Implement version comparison**
   - `is_newer_version()` using semver or string comparison

5. **Implement GitHub API client**
   - `check_for_updates()` function
   - HTTP request with proper headers
   - JSON deserialization
   - Error handling

6. **Implement notification display**
   - `display_update_notification()` function
   - Formatted output

7. **Integrate into main()**
   - Add configuration options to CLI and Config
   - Spawn async task for update check
   - Handle all three platform-specific main functions

8. **Add tests**
   - Unit tests for version comparison
   - Integration tests with mocked API

9. **Update documentation**
   - README.md (add configuration option)
   - Example config file
   - Code comments

10. **Test thoroughly**
    - All platforms
    - Various network conditions
    - Rate limiting scenarios

### 10. Documentation Updates

**README.md additions**:

In the "Using a configuration file" section, add:
```yaml
# Enable automatic update checks to be notified about new versions
# (default: false for privacy). When enabled, litra-autotoggle will
# check GitHub releases once per day on startup.
#
# check_updates: true
```

In the command-line arguments section:
- `--check-updates` to enable update checks on this run
- `--no-update-check` to disable update checks even if configured

**litra-autotoggle.example.yml updates**:
Add the `check_updates` option with explanation

### 11. Rollout Considerations

**Phase 1: Initial Implementation**
- Implement with opt-in default
- Basic caching and rate limiting
- Simple version comparison

**Phase 2: Enhancements (Future)**
- ETag/If-None-Match support for bandwidth savings
- Release notes display in notification
- Configurable notification format
- Platform-specific update instructions

**Phase 3: Advanced Features (Future)**
- Auto-download capability (with user permission)
- Signature verification for security
- Rollback protection
- Pre-release channel support

## Dependencies to Add

### Required
None! All necessary dependencies are already present:
- `reqwest` (already in Cargo.toml)
- `serde` (already in Cargo.toml)
- `tokio` (already in Cargo.toml)

### Optional but Recommended
```toml
semver = "1.0"  # For robust semantic version comparison
```

Alternative: Implement simple version comparison without additional dependency.

## Security Considerations

1. **HTTPS Only**: Always use HTTPS for GitHub API (enforced by URL)
2. **No Secrets**: No authentication tokens needed (public API)
3. **Input Validation**: Validate version strings before comparison
4. **Error Handling**: Never crash on update check failure
5. **Rate Limiting**: Respect GitHub's rate limits
6. **No Telemetry**: No tracking or analytics
7. **Safe Defaults**: Opt-in by default for privacy

## Performance Considerations

1. **Non-Blocking**: Update check runs asynchronously
2. **Cached Results**: Avoid redundant API calls
3. **Fast Timeout**: Set reasonable HTTP timeout (e.g., 5 seconds)
4. **Minimal Impact**: Should not delay startup or affect core functionality
5. **Memory Efficient**: Small cache file, minimal memory overhead

## Success Metrics

- ✅ Update checks work on all supported platforms (Linux, macOS, Windows)
- ✅ No impact on startup time or performance
- ✅ Graceful handling of network errors and rate limits
- ✅ Clear and non-intrusive user notifications
- ✅ Respects user privacy and preferences
- ✅ Comprehensive test coverage
- ✅ Well-documented configuration options

## Timeline Estimate

- **Design & Planning**: ✅ Complete
- **Implementation**: 4-6 hours
  - Core functionality: 2-3 hours
  - Testing: 1-2 hours
  - Documentation: 1 hour
- **Testing & Refinement**: 1-2 hours
- **Total**: 5-8 hours of development time

## References

- [GitHub REST API - Releases](https://docs.github.com/en/rest/releases/releases)
- [Rust Cookbook - Web APIs](https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html)
- [Rain's Rust CLI Recommendations](https://rust-cli-recommendations.sunshowers.io/)
- [reqwest Documentation](https://docs.rs/reqwest/)
- [semver Crate](https://docs.rs/semver/)

## Appendix: Alternative Approaches Considered

### A. Using `self_update` crate
**Pros**: Pre-built solution, handles downloads and installation
**Cons**: Heavy dependency, potential version conflicts, less control
**Decision**: Not chosen - direct API approach provides better control and lighter dependencies

### B. Using `update-informer` crate
**Pros**: Simple notification-only solution
**Cons**: Additional dependency, basic functionality we can implement ourselves
**Decision**: Not chosen - functionality is simple enough to implement directly

### C. Direct API with `reqwest` (Chosen)
**Pros**: 
- Uses existing dependencies
- Full control over implementation
- Lightweight
- Transparent to users
- Common pattern in Rust ecosystem (used by rustup, bat, etc.)

**Cons**: 
- Requires more code
- Need to implement caching and version comparison

**Decision**: ✅ Chosen - Best fit for this project's needs and philosophy

## Implementation Checklist

### Configuration & Types
- [ ] Add `check_updates` field to `Config` struct
- [ ] Add `update_check_interval_hours` field to `Config` struct (optional, future enhancement)
- [ ] Add `--check-updates` flag to `Cli` struct
- [ ] Add `--no-update-check` flag to `Cli` struct
- [ ] Define `GitHubRelease` struct for API response deserialization
- [ ] Define `UpdateCache` struct for cache file format
- [ ] Consider adding `semver` dependency for version comparison

### Core Functionality
- [ ] Implement `get_cache_file_path()` with platform-specific logic
- [ ] Implement `read_update_cache()` for reading cache file
- [ ] Implement `write_update_cache()` for writing cache file
- [ ] Implement `is_newer_version()` for version comparison
- [ ] Implement `check_for_updates()` async function for GitHub API call
- [ ] Implement `display_update_notification()` for user notification
- [ ] Implement `spawn_update_check()` wrapper for non-blocking execution

### Integration
- [ ] Integrate update check into macOS main function
- [ ] Integrate update check into Linux main function
- [ ] Integrate update check into Windows main function
- [ ] Ensure configuration merging (CLI flags override config file)
- [ ] Add proper error handling and logging

### Testing
- [ ] Add unit tests for version comparison logic
- [ ] Add unit tests for cache read/write operations
- [ ] Add integration tests for API mocking scenarios
- [ ] Manual testing on Linux
- [ ] Manual testing on macOS
- [ ] Manual testing on Windows
- [ ] Test with network errors
- [ ] Test with rate limiting

### Documentation
- [ ] Update README.md with `check_updates` configuration option
- [ ] Update README.md with command-line flags
- [ ] Update `litra-autotoggle.example.yml` with examples
- [ ] Add code comments for new functions
- [ ] Document privacy considerations

### Final Steps
- [ ] Run cargo clippy to check for issues
- [ ] Run cargo test to verify all tests pass
- [ ] Build on all platforms
- [ ] Review code for security issues
- [ ] Update CHANGELOG if one exists
