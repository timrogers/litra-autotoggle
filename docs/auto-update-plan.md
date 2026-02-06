# Automatic Update Checks Implementation Plan

## Executive Summary

This document outlines a comprehensive plan to implement automatic update checks for `litra-autotoggle` using GitHub releases. The feature will check periodically for new versions and notify users non-intrusively, improving user awareness of updates without interrupting workflow.

## Background

### Current State
- **Version**: 1.3.0 (as per Cargo.toml)
- **Distribution**: Multiple channels (Cargo, Homebrew, direct binary downloads)
- **Platforms**: macOS, Linux, Windows (x86_64 and ARM64)
- **Release Process**: Automated via GitHub Actions, creates binaries for multiple platforms
- **Repository**: `timrogers/litra-autotoggle`

### Problem Statement
Users may not be aware when new versions are available, missing out on:
- Bug fixes
- New features (e.g., recent --back option for Beam LX back light)
- Security patches
- Performance improvements

## Technical Approach

### Library Selection

> **UPDATE:** After further analysis, we recommend calling the GitHub API directly rather than using a library. See [direct-api-analysis.md](./direct-api-analysis.md) for detailed reasoning.

After research, three main approaches are available:

#### Option 1: Direct GitHub API Calls (RECOMMENDED) ⭐
**Pros:**
- Minimal dependencies (just reqwest + serde_json)
- Full control over caching, timing, error handling
- Simpler - only implement what we need
- Transparent and easy to debug
- More flexible for customization
- Smaller binary size
- reqwest is general-purpose (useful for future features)

**Cons:**
- More code to write (~100-150 lines)
- Need to implement caching ourselves
- Need to handle API quirks ourselves

**Use Case:** Best for projects that want minimal dependencies, full control, and don't need multi-registry support

#### Option 2: `update-informer`
**Pros:**
- Less code to write
- Built-in caching logic
- Well-tested by community
- Supports multiple registries (GitHub, crates.io, npm, PyPI)

**Cons:**
- Additional dependency (~6 transitive dependencies)
- Less control over caching strategy and error handling
- Opinionated about how checks work
- Dependency maintenance burden

**Use Case:** Good for projects needing multi-registry support or wanting minimal code

#### Option 3: `self_update`
**Pros:**
- Full featured - checks AND downloads/installs updates
- Can replace binary in-place
- Supports cryptographic signature verification

**Cons:**
- More complex implementation
- May conflict with package manager installations (Homebrew, Cargo)
- Largest dependency footprint
- Users may prefer updating via their installation method

**Use Case:** Best for standalone binaries not distributed via package managers

### Recommended Approach: Direct GitHub API Calls

**Rationale:**
1. **Minimal Dependencies**: Only adds reqwest (general-purpose HTTP client) instead of niche update-checking crate
2. **Full Control**: Complete control over caching, error handling, and behavior
3. **Transparency**: Code is in our repository, easy to understand and audit
4. **Maintainability**: No need to track update-informer updates or breaking changes
5. **Common Pattern**: Many successful Rust CLI tools (rustup, bat, ripgrep) use this approach
6. **Flexible**: Easy to customize for specific needs
7. **Respects Installation Method**: Like update-informer, just notifies users without auto-updating

## Implementation Design

### 1. Update Check Mechanism

#### When to Check
- **On Application Start**: Check during CLI initialization
- **Frequency**: Maximum once per 24 hours (configurable)
- **Cache Location**: Platform-specific cache directory using `dirs` crate
  - Linux: `$XDG_CACHE_HOME/litra-autotoggle/` or `~/.cache/litra-autotoggle/`
  - macOS: `~/Library/Caches/litra-autotoggle/`
  - Windows: `%LOCALAPPDATA%\litra-autotoggle\cache\`

#### What to Check
- Query GitHub API: `/repos/timrogers/litra-autotoggle/releases/latest`
- Compare latest release tag (e.g., `v1.3.0`) against current version
- Use semantic versioning comparison

#### Cache File Structure
```json
{
  "last_checked": "2026-02-06T14:00:00Z",
  "latest_version": "1.3.0",
  "current_version": "1.3.0"
}
```

### 2. User Experience

#### Notification Display
When a new version is detected, display a non-intrusive message:

```
╭──────────────────────────────────────────────────────╮
│ A new version of litra-autotoggle is available!     │
│ Current: v1.3.0 → Latest: v1.4.0                    │
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
```

#### Message Timing
- Display AFTER initialization but BEFORE starting main functionality
- Should not delay application startup significantly
- Run check asynchronously (non-blocking)

### 3. Configuration Options

#### Environment Variables
```bash
# Disable update checks entirely
LITRA_AUTOTOGGLE_NO_UPDATE_CHECK=1

# Customize check interval (in hours, default: 24)
LITRA_AUTOTOGGLE_UPDATE_CHECK_INTERVAL=48

# Force check on every run (for testing)
LITRA_AUTOTOGGLE_FORCE_UPDATE_CHECK=1
```

#### Config File Options (litra-autotoggle.yml)
```yaml
# Disable automatic update checks
disable_update_check: false

# Update check interval in hours (default: 24)
update_check_interval_hours: 24
```

#### CLI Flag
```bash
# Disable update check for this run
litra-autotoggle --no-update-check

# Force update check (ignore cache)
litra-autotoggle --check-update
```

### 4. Implementation Steps

#### Phase 1: Core Infrastructure
1. Add `update-informer` dependency to Cargo.toml
2. Add `dirs` crate for platform-specific cache directories
3. Create update check module (`src/update_checker.rs`)
4. Implement cache management (read/write/validate)
5. Implement version comparison logic

#### Phase 2: Integration
1. Add configuration options to `Config` struct
2. Add CLI flags to `Cli` struct
3. Integrate check into `main()` function
4. Add environment variable handling
5. Implement async check (non-blocking)

#### Phase 3: User Interface
1. Design and implement notification message
2. Add colored output (using `colored` crate)
3. Test message formatting on different terminal sizes
4. Implement quiet mode (respect `--verbose` flag)

#### Phase 4: Testing & Documentation
1. Write unit tests for version comparison
2. Write integration tests for cache management
3. Test on all platforms (Linux, macOS, Windows)
4. Update README.md with new options
5. Add examples to documentation
6. Update example config file

### 5. Code Structure

#### New Files
```
src/
├── main.rs (modified)
├── update_checker.rs (new)
└── cache.rs (new, optional - for cache management)
```

#### Key Functions

**update_checker.rs:**
```rust
// GitHub API constants
const GITHUB_API_BASE: &str = "https://api.github.com";
const REPO_OWNER: &str = "timrogers";
const REPO_NAME: &str = "litra-autotoggle";

// GitHub Release response structure
#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    published_at: String,
}

// Call GitHub Releases API
async fn check_github_releases() -> Result<Option<String>, Box<dyn std::error::Error>>

// Check for updates with caching
pub async fn check_for_updates(
    current_version: &str,
    cache_dir: &Path,
    interval_hours: u64,
) -> Result<Option<String>, UpdateError>

// Force check (ignore cache)
pub async fn force_check_for_updates(
    current_version: &str,
) -> Result<Option<String>, UpdateError>

// Format update notification message
pub fn format_update_message(
    current_version: &str,
    latest_version: &str,
) -> String

// Check if updates are disabled
pub fn is_update_check_disabled() -> bool
```

**cache.rs:**
```rust
// Read cache
pub fn read_cache(cache_dir: &Path) -> Result<UpdateCache, CacheError>

// Write cache
pub fn write_cache(cache_dir: &Path, cache: &UpdateCache) -> Result<(), CacheError>

// Check if cache is expired
pub fn is_cache_expired(cache: &UpdateCache, interval_hours: u64) -> bool
```

### 6. Dependencies to Add

```toml
[dependencies]
# Existing dependencies...
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
dirs = "5.0"
serde_json = "1.0"  # For cache serialization (may already be present)
chrono = { version = "0.4", features = ["serde"] }  # For timestamps
```

**Why reqwest with rustls-tls?**
- Pure Rust TLS implementation (no OpenSSL dependency)
- Better cross-platform compatibility
- Smaller binary size
- More secure and maintained

**Why not OpenSSL?**
- Requires system OpenSSL library
- Harder to cross-compile
- Potential security vulnerabilities in system libraries

**Total new dependencies:** 4 direct + ~8 transitive (compared to ~15 with update-informer)

### 7. Testing Strategy

#### Unit Tests
- Version comparison (semantic versioning)
- Cache expiration logic
- Configuration parsing
- Environment variable handling

#### Integration Tests
- Cache read/write operations
- Update check with mock GitHub API
- Message formatting
- Cross-platform path handling

#### Manual Testing Checklist
- [ ] Test on Linux (x86_64 and ARM64)
- [ ] Test on macOS (Intel and Apple Silicon)
- [ ] Test on Windows (x86_64 and ARM64)
- [ ] Test with Homebrew installation
- [ ] Test with Cargo installation
- [ ] Test with direct binary
- [ ] Test in CI/CD environment
- [ ] Test with slow network
- [ ] Test with no network
- [ ] Test with GitHub API rate limiting

### 8. Cross-Platform Considerations

#### Network Access
- Handle offline gracefully (don't block or error)
- Implement timeout (5 seconds max)
- Handle GitHub API rate limits

#### Cache Directory Permissions
- Handle permission errors gracefully
- Fall back to temp directory if cache dir unavailable
- Don't fail application if cache operations fail

#### CI/CD Environments
- Respect CI environment variables
- Disable by default if `CI=true` detected
- Allow override with explicit flag

### 9. Security Considerations

#### API Safety
- Use HTTPS only for GitHub API
- Validate response structure
- Handle malformed responses
- Timeout on slow requests

#### Cache Integrity
- Validate cache file structure
- Handle corrupted cache files
- Use safe deserialization

#### Privacy
- No telemetry or tracking
- No data sent to any service
- Only check GitHub public API
- Respect user's privacy settings

### 10. Error Handling

#### Network Errors
- Log warning but continue execution
- Cache failure for shorter period to retry sooner
- Don't show error to user unless `--verbose`

#### Cache Errors
- Fall back to no caching if cache operations fail
- Log errors in verbose mode
- Application continues normally

#### Version Parsing Errors
- Handle non-semver versions gracefully
- Log errors in verbose mode
- Assume no update available on error

### 11. Performance Considerations

#### Async Implementation
- Run check in background task
- Don't block main application logic
- Timeout after 5 seconds

#### Minimal Overhead
- Check only once per interval
- Efficient cache operations
- Small dependency footprint

#### Startup Time Impact
- Target: < 50ms added to startup time
- Most checks will be cache hits (instant)
- Network checks are async and don't block

### 12. Documentation Updates

#### README.md
- Add section on update checks
- Document environment variables
- Document CLI flags
- Add FAQ about disabling checks

#### Example Config File
- Add update check options
- Include comments explaining behavior

#### Man Page / Help Text
- Document new flags
- Update examples

## Implementation Timeline

### Phase 1: Core (1-2 days)
- Add dependencies
- Implement basic check logic
- Implement caching

### Phase 2: Integration (1 day)
- Add configuration options
- Integrate into main.rs
- Environment variable support

### Phase 3: Polish (1 day)
- Notification formatting
- Error handling
- Cross-platform testing

### Phase 4: Testing & Documentation (1 day)
- Write tests
- Update documentation
- Final testing

**Total Estimated Time: 4-5 days**

## Future Enhancements

### Potential Future Features
1. **Self-Update Command**: Add `litra-autotoggle self-update` for direct binary installations
2. **Release Notes**: Display changelog in notification
3. **Update Channels**: Support stable/beta/nightly channels
4. **Version Pinning**: Allow users to pin to specific versions
5. **Telemetry**: Optional anonymous usage statistics (opt-in only)

### Not Planned (Out of Scope)
- Automatic updates without user action
- Updating Homebrew or Cargo installations
- Downloading and installing binaries
- Version rollback

## Success Criteria

### Must Have
- ✅ Check GitHub releases for updates
- ✅ Cache results to avoid excessive API calls
- ✅ Non-intrusive notification
- ✅ Respect user preferences (disable option)
- ✅ Work on all supported platforms
- ✅ No significant performance impact
- ✅ Comprehensive documentation

### Nice to Have
- Colored/formatted output
- Detailed release notes in notification
- Configurable check interval
- Multiple notification methods

## Risks & Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| GitHub API rate limiting | Medium | Low | Cache results, handle gracefully |
| Network unavailable | Low | Medium | Timeout, fail silently |
| Cache corruption | Low | Low | Validate, recreate on error |
| Performance impact | Medium | Low | Async check, efficient caching |
| User annoyance | High | Medium | Easy disable, respectful frequency |
| Package manager conflicts | Low | Low | Inform only, don't auto-update |

## Alternatives Considered

### Alternative 1: No Update Checks
- Users manually check for updates
- Simple but poor user experience
- Users miss important updates

### Alternative 2: self_update Crate
- Full auto-update capability
- Conflicts with package managers
- More complex, higher risk

### Alternative 3: External Service
- Centralized update tracking
- Privacy concerns
- Additional infrastructure
- Rejected due to complexity and privacy

## Conclusion

Implementing automatic update checks using `update-informer` provides a balanced approach that:
- Improves user awareness of new versions
- Respects user preferences and installation methods
- Maintains cross-platform compatibility
- Minimizes implementation complexity
- Preserves application performance
- Protects user privacy

The recommended implementation is lightweight, non-intrusive, and aligns with Rust CLI best practices.
