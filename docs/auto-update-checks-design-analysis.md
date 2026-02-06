# Auto-Update Checks: Design Alternatives Analysis

This document provides a detailed analysis of different design approaches for implementing auto-update checks in litra-autotoggle, along with trade-offs and recommendations.

## Executive Summary

**Recommended Approach**: Direct GitHub Releases API integration using existing `reqwest` dependency with opt-in configuration.

**Key Rationale**:
- Lightweight (no new heavy dependencies)
- Full control over implementation
- Privacy-respecting (opt-in by default)
- Aligns with Rust CLI best practices
- Uses existing infrastructure

---

## Alternative 1: `self_update` Crate

### Description
The `self_update` crate provides comprehensive auto-update capabilities including download, installation, and rollback functionality.

### Pros
- ✅ Complete solution (check, download, install)
- ✅ Built-in signature verification
- ✅ Rollback support
- ✅ Cross-platform binary detection
- ✅ Well-tested in production

### Cons
- ❌ Heavy dependency (adds ~15 additional dependencies)
- ❌ Includes features we don't need (auto-installation)
- ❌ Potential version conflicts with existing dependencies
- ❌ Less control over update flow
- ❌ May require special permissions for auto-installation
- ❌ Complexity overhead for simple notification needs

### Dependency Tree Impact
```toml
self_update = "0.41"
# Brings in:
# - reqwest (already have it)
# - semver (small addition)
# - tempfile (dev-only, we have it)
# - zip/tar for archive handling
# - several platform-specific dependencies
```

### Use Case Fit
- Best for: Applications that want one-click updates
- Poor fit for: Applications that just want to notify users
- **Decision**: ❌ Too heavy for our needs

---

## Alternative 2: `update-informer` Crate

### Description
Lightweight crate focused solely on checking for updates and notifying users (no auto-installation).

### Pros
- ✅ Focused on notification only
- ✅ Supports multiple registries (crates.io, GitHub, PyPI, npm)
- ✅ Built-in caching mechanism
- ✅ Simple API
- ✅ Smaller dependency footprint than `self_update`

### Cons
- ❌ Still an additional dependency
- ❌ Generic solution may not fit specific needs
- ❌ Less flexibility in notification format
- ❌ May include features we don't need (multi-registry support)
- ❌ Less transparent about what data is sent

### Example Usage
```rust
use update_informer::{registry, Check};

fn check_updates() {
    let informer = update_informer::new(registry::GitHub, "litra-autotoggle", "1.3.0")
        .interval(Duration::from_secs(60 * 60 * 24));
    
    if let Ok(Some(version)) = informer.check_version() {
        println!("New version available: {}", version);
    }
}
```

### Use Case Fit
- Best for: Quick integration with minimal code
- Poor fit for: Custom notification needs, minimal dependencies
- **Decision**: ❌ Functionality is simple enough to implement directly

---

## Alternative 3: Direct GitHub API with `reqwest` (RECOMMENDED)

### Description
Make direct HTTP calls to GitHub Releases API using the existing `reqwest` dependency.

### Pros
- ✅ No new dependencies (reqwest already present)
- ✅ Full control over implementation
- ✅ Transparent about what data is sent
- ✅ Customizable notification format
- ✅ Simple to understand and maintain
- ✅ Common pattern in Rust ecosystem (rustup, bat, etc.)
- ✅ Minimal code footprint (~200-300 lines)
- ✅ Easy to test with mocked responses

### Cons
- ❌ More code to write and maintain
- ❌ Need to implement version comparison logic
- ❌ Need to implement caching mechanism
- ❌ Manual error handling

### Implementation Effort
- Core functionality: ~150 lines
- Cache management: ~50 lines
- Version comparison: ~30 lines
- Tests: ~100 lines
- **Total**: ~330 lines of code

### Maintenance Burden
- Low - straightforward HTTP + JSON parsing
- Well-defined API contract (GitHub Releases API is stable)
- Easy to debug and modify

### Use Case Fit
- ✅ Perfect for our needs
- ✅ Aligns with project philosophy
- ✅ Gives users transparency
- **Decision**: ✅ **CHOSEN**

---

## Alternative 4: No Update Checks

### Description
Don't implement update checks at all. Rely on users to manually check for updates.

### Pros
- ✅ Zero code to maintain
- ✅ No privacy concerns
- ✅ No network dependencies
- ✅ Simplest approach

### Cons
- ❌ Users may miss important updates
- ❌ Security patches may not reach users quickly
- ❌ Reduced user experience
- ❌ More support burden (users on old versions)

### Current Reality
- GitHub releases page shows download stats
- Most downloads are from latest release
- Suggests users check manually, but not systematically

### Use Case Fit
- Acceptable for: Small tools with infrequent updates
- Poor fit for: Actively maintained tools with regular releases
- **Decision**: ❌ Not chosen - users would benefit from notifications

---

## Design Decision: Update Check Timing

### Option A: On Startup with Caching (RECOMMENDED)

**How it works**:
1. Check cache on startup
2. If cache is expired (>24 hours), check API
3. Run check asynchronously (non-blocking)
4. Cache results

**Pros**:
- ✅ Simple to implement
- ✅ Natural timing (users see notifications when they start)
- ✅ Minimal complexity
- ✅ No background threads after startup

**Cons**:
- ❌ No updates for long-running instances (mitigated by caching)
- ❌ Slight startup delay (mitigated by async execution)

**Decision**: ✅ **CHOSEN**

### Option B: Periodic Background Checks

**How it works**:
1. Spawn background task after startup
2. Check API every N hours while running
3. Show notification when update found

**Pros**:
- ✅ Works for long-running instances
- ✅ More timely notifications

**Cons**:
- ❌ More complex (tokio intervals, task management)
- ❌ Background task runs entire time
- ❌ More things that can go wrong
- ❌ Resource usage during entire runtime

**Decision**: ❌ Not needed for this use case

### Option C: One-Time Check (No Caching)

**How it works**:
1. Check API every time on startup
2. No caching, always fresh

**Pros**:
- ✅ Always current information
- ✅ Simpler (no cache management)

**Cons**:
- ❌ API calls on every startup
- ❌ Hits rate limits quickly
- ❌ Network delay on every startup
- ❌ Unnecessary load on GitHub API

**Decision**: ❌ Unacceptable for rate limiting reasons

---

## Design Decision: Version Comparison

### Option A: Simple String Comparison

```rust
fn is_newer_version(current: &str, latest: &str) -> bool {
    latest > current
}
```

**Pros**:
- ✅ Zero dependencies
- ✅ Simple implementation

**Cons**:
- ❌ Incorrect for versions like "1.10.0" vs "1.9.0"
- ❌ Doesn't handle pre-release versions

**Decision**: ❌ Too simplistic

### Option B: Manual Semver Parsing

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

**Pros**:
- ✅ No dependencies
- ✅ Handles most cases correctly
- ✅ Simple to understand

**Cons**:
- ❌ Doesn't handle pre-release versions
- ❌ No support for build metadata
- ❌ Manual parsing can have edge cases

**Decision**: ✅ **CHOSEN** as primary approach

### Option C: `semver` Crate

```rust
use semver::Version;

fn is_newer_version(current: &str, latest: &str) -> bool {
    let current = Version::parse(current.trim_start_matches('v')).ok();
    let latest = Version::parse(latest.trim_start_matches('v')).ok();
    
    match (current, latest) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}
```

**Pros**:
- ✅ Correct semver semantics
- ✅ Handles pre-release and build metadata
- ✅ Well-tested
- ✅ Small dependency (~15KB)

**Cons**:
- ❌ Additional dependency
- ❌ Slight complexity overhead

**Decision**: ✅ Recommended as **optional enhancement**

---

## Design Decision: Configuration Defaults

### Option A: Opt-Out (Enabled by Default)

**Behavior**: Update checks run unless explicitly disabled

**Pros**:
- ✅ Maximum user awareness
- ✅ Better security (users learn about patches)
- ✅ Higher adoption

**Cons**:
- ❌ Privacy concern (unexpected network calls)
- ❌ May surprise users
- ❌ Requires opt-out for privacy-conscious users

**Decision**: ❌ Not privacy-respecting enough

### Option B: Opt-In (Disabled by Default) - RECOMMENDED

**Behavior**: Update checks only run if explicitly enabled

**Pros**:
- ✅ Privacy-respecting (no surprise network calls)
- ✅ User control
- ✅ Transparent behavior
- ✅ Aligns with GDPR principles

**Cons**:
- ❌ Lower adoption
- ❌ Users may not know feature exists

**Mitigation**:
- Document feature clearly in README
- Show first-run message suggesting to enable
- Include in example config

**Decision**: ✅ **CHOSEN**

### Option C: First-Run Prompt

**Behavior**: Ask user on first run whether to enable

**Pros**:
- ✅ User explicitly chooses
- ✅ High awareness
- ✅ Privacy-friendly

**Cons**:
- ❌ Interactive prompt breaks automation
- ❌ Complex for background services
- ❌ Requires storing "first run" state

**Decision**: ❌ Too complex for this use case

---

## Design Decision: Notification Format

### Option A: Simple Log Message

```
INFO: A new version (1.4.0) is available. Visit: https://github.com/...
```

**Pros**:
- ✅ Simple
- ✅ Consistent with existing logs

**Cons**:
- ❌ Easy to miss
- ❌ Not visually distinctive

**Decision**: ❌ Too easy to overlook

### Option B: Boxed Notification (RECOMMENDED)

```
╭──────────────────────────────────────────╮
│ A new version is available!             │
│ Current: 1.3.0 → Latest: 1.4.0          │
│ https://github.com/.../releases/latest  │
╰──────────────────────────────────────────╯
```

**Pros**:
- ✅ Visually distinctive
- ✅ Hard to miss
- ✅ Professional appearance
- ✅ Contains all relevant info

**Cons**:
- ❌ Slightly more code
- ❌ May not work in all terminals (rare)

**Decision**: ✅ **CHOSEN**

### Option C: Interactive Prompt

```
A new version is available. Update now? [y/N]
```

**Pros**:
- ✅ Immediate action possible

**Cons**:
- ❌ Blocks startup
- ❌ Breaks automation
- ❌ Requires auto-update capability

**Decision**: ❌ Too intrusive

---

## Design Decision: Cache Storage Format

### Option A: Plain Text

```
1706180000
v1.3.0
```

**Pros**:
- ✅ Human-readable
- ✅ Simple parsing

**Cons**:
- ❌ Fragile (order-dependent)
- ❌ Hard to extend

**Decision**: ❌ Too rigid

### Option B: JSON (RECOMMENDED)

```json
{
  "last_check_timestamp": 1706180000,
  "last_checked_version": "v1.3.0",
  "last_notified_version": "v1.3.0"
}
```

**Pros**:
- ✅ Structured format
- ✅ Easy to extend
- ✅ Robust parsing with serde
- ✅ Self-documenting

**Cons**:
- ❌ Slightly larger file size (negligible)

**Decision**: ✅ **CHOSEN**

### Option C: TOML

```toml
last_check_timestamp = 1706180000
last_checked_version = "v1.3.0"
last_notified_version = "v1.3.0"
```

**Pros**:
- ✅ Human-readable
- ✅ Consistent with config format (YAML)

**Cons**:
- ❌ Need TOML dependency (not present)
- ❌ Overkill for simple data

**Decision**: ❌ Unnecessary dependency

---

## Design Decision: Error Handling Philosophy

### Option A: Fail Fast

**Behavior**: Exit with error if update check fails

**Pros**:
- ✅ Clear error reporting

**Cons**:
- ❌ Blocks main functionality
- ❌ Network issues prevent using the tool
- ❌ Poor user experience

**Decision**: ❌ Unacceptable

### Option B: Silent Failure

**Behavior**: Swallow all errors, no logging

**Pros**:
- ✅ Never blocks functionality

**Cons**:
- ❌ Hard to debug
- ❌ Users don't know if feature is working

**Decision**: ❌ Too opaque

### Option C: Graceful Degradation with Logging (RECOMMENDED)

**Behavior**: Log errors at appropriate level, continue execution

```rust
match check_for_updates().await {
    Ok(Some(release)) => { /* process */ },
    Ok(None) => debug!("No update available"),
    Err(e) => warn!("Update check failed: {}. Continuing normally.", e),
}
```

**Pros**:
- ✅ Never blocks main functionality
- ✅ Errors are logged for debugging
- ✅ User experience not degraded

**Cons**:
- ❌ Errors may go unnoticed (acceptable trade-off)

**Decision**: ✅ **CHOSEN**

---

## Design Decision: Async vs Blocking

### Option A: Blocking in Background Thread

```rust
std::thread::spawn(|| {
    let client = reqwest::blocking::Client::new();
    // ... check for updates ...
});
```

**Pros**:
- ✅ Simple
- ✅ Doesn't require async context

**Cons**:
- ❌ Additional thread overhead
- ❌ Less idiomatic in Tokio application

**Decision**: ❌ Not idiomatic

### Option B: Async with Tokio Spawn (RECOMMENDED)

```rust
tokio::spawn(async move {
    let client = reqwest::Client::new();
    // ... check for updates ...
});
```

**Pros**:
- ✅ Idiomatic for Tokio application
- ✅ Efficient (no thread overhead)
- ✅ Aligns with existing code style

**Cons**:
- None significant

**Decision**: ✅ **CHOSEN**

---

## Comparison Matrix

| Aspect | Self-Update | Update-Informer | Direct API | No Updates |
|--------|-------------|-----------------|------------|------------|
| **Dependencies** | Heavy (15+) | Light (5+) | None new | N/A |
| **Code to Write** | Minimal | Minimal | Moderate | None |
| **Flexibility** | Low | Medium | High | N/A |
| **Control** | Low | Medium | High | N/A |
| **Maintenance** | Dependency | Dependency | Self | None |
| **Privacy** | Medium | Medium | High | High |
| **Transparency** | Low | Medium | High | N/A |
| **Binary Size** | +1-2 MB | +200-500 KB | Negligible | N/A |
| **Ecosystem Pattern** | Less common | Uncommon | **Common** | N/A |
| **Recommendation** | ❌ | ❌ | ✅ | ❌ |

---

## Implementation Roadmap

### Phase 1: MVP (Recommended for Initial PR)
- ✅ Direct GitHub API integration
- ✅ Manual semver parsing (no new deps)
- ✅ Startup check with caching
- ✅ Opt-in configuration
- ✅ Boxed notification
- ✅ JSON cache format
- ✅ Graceful error handling

**Effort**: ~4-6 hours
**Risk**: Low
**Value**: High

### Phase 2: Enhancements (Future PR)
- Consider adding `semver` crate if edge cases arise
- ETag support for bandwidth savings
- Configurable check intervals
- Release notes in notification

**Effort**: ~2-3 hours
**Risk**: Very Low
**Value**: Medium

### Phase 3: Advanced (Optional Future)
- Multiple update channels (stable/beta)
- Signature verification
- Platform-specific download links
- Auto-download capability (with explicit permission)

**Effort**: ~8-12 hours
**Risk**: Medium
**Value**: Low (most users don't need this)

---

## Success Criteria

### Must Have
- ✅ Works on all platforms (Linux, macOS, Windows)
- ✅ No impact on startup performance
- ✅ Graceful handling of network errors
- ✅ Privacy-respecting (opt-in)
- ✅ Clear documentation

### Should Have
- ✅ Proper version comparison
- ✅ Rate limit handling
- ✅ Caching to avoid redundant checks
- ✅ Comprehensive tests

### Nice to Have
- ⭕ Platform-specific update instructions
- ⭕ Release notes display
- ⭕ Configurable check frequency

---

## Risk Analysis

### Low Risk
- ✅ Direct API approach (proven pattern)
- ✅ Using existing dependencies
- ✅ Non-critical feature (graceful failure)

### Medium Risk
- ⚠️ GitHub API rate limits (mitigated by caching)
- ⚠️ Version comparison edge cases (mitigated by testing)

### High Risk
- ❌ None identified

### Mitigation Strategies
1. **Rate Limiting**: 24-hour cache, graceful failure
2. **Version Parsing**: Comprehensive test suite, fallback logic
3. **Network Failures**: Timeouts, error logging, non-blocking
4. **Privacy**: Opt-in default, clear documentation

---

## Conclusion

After analyzing all alternatives, the recommended approach is:

**Direct GitHub Releases API integration using existing `reqwest` dependency**

This approach offers the best balance of:
- ✅ Minimal complexity and dependencies
- ✅ Full control and transparency
- ✅ Privacy-respecting design
- ✅ Alignment with Rust CLI best practices
- ✅ Maintainability and extensibility

The implementation is straightforward, well-tested in the ecosystem, and provides exactly the features needed without unnecessary overhead.

---

## References

- [Rust CLI Best Practices](https://rust-cli-recommendations.sunshowers.io/)
- [GitHub API Documentation](https://docs.github.com/en/rest/releases/releases)
- [reqwest Documentation](https://docs.rs/reqwest/)
- [Tokio Spawn Pattern](https://tokio.rs/tokio/tutorial/spawning)
- [Rustup Update Checker](https://github.com/rust-lang/rustup/blob/master/src/cli/self_update.rs) (reference implementation)
- [Bat Update Checker](https://github.com/sharkdp/bat/blob/master/src/bin/bat/main.rs) (reference implementation)
