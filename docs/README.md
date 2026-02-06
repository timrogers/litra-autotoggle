# Automatic Update Checks - Planning Summary

## Quick Links
- [Full Implementation Plan](./auto-update-plan.md) - Comprehensive technical specification
- [Implementation Checklist](./implementation-checklist.md) - Step-by-step task list
- [Code Examples](./code-examples.md) - Sample implementation code

## Executive Summary

This planning effort addresses the need for automatic update notifications in `litra-autotoggle`. Users currently have no automated way to know when new versions are available, leading to missed bug fixes, features, and security updates.

## Recommended Solution

**Use the `update-informer` crate** to check GitHub releases and notify users non-intrusively.

### Why This Approach?

1. **Respects Installation Methods**: Doesn't interfere with Homebrew, Cargo, or direct binary installations
2. **Simple & Lightweight**: Only checks and informs, doesn't modify binaries
3. **User-Friendly**: Non-intrusive with easy opt-out
4. **Cross-Platform**: Works on macOS, Linux, and Windows
5. **Privacy-Focused**: No telemetry, only uses public GitHub API

## Key Features

### What It Does
- ✅ Checks GitHub releases for new versions (once per 24 hours by default)
- ✅ Caches results to minimize API calls
- ✅ Displays friendly notification when updates are available
- ✅ Provides update instructions for all installation methods
- ✅ Easy to disable via environment variable or config

### What It Doesn't Do
- ❌ Auto-download or install updates
- ❌ Interfere with package managers
- ❌ Collect telemetry or analytics
- ❌ Block application startup

## User Experience

### When Update Available
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

### Configuration Options

**Environment Variables:**
```bash
# Disable checks
export LITRA_AUTOTOGGLE_NO_UPDATE_CHECK=1

# Custom interval (hours)
export LITRA_AUTOTOGGLE_UPDATE_CHECK_INTERVAL=48
```

**Config File (litra-autotoggle.yml):**
```yaml
disable_update_check: false
update_check_interval_hours: 24
```

**CLI Flags:**
```bash
litra-autotoggle --no-update-check    # Skip check this run
litra-autotoggle --check-update       # Force check now
```

## Implementation Overview

### New Dependencies
- `update-informer` - GitHub release checking
- `dirs` - Cross-platform cache directories
- `chrono` - Timestamp handling
- `serde_json` - Cache serialization

### New Code Structure
```
src/
├── main.rs (modified)
├── update_checker.rs (new)
└── cache.rs (optional)
```

### Key Functions
- `check_for_updates()` - Main entry point with caching
- `is_update_check_disabled()` - Check if disabled
- `format_update_message()` - Create notification
- `read_cache()` / `write_cache()` - Cache management

## Timeline

| Phase | Duration | Tasks |
|-------|----------|-------|
| Phase 1: Core | 1-2 days | Dependencies, update checker module, caching |
| Phase 2: Integration | 1 day | Config options, CLI flags, main() integration |
| Phase 3: Polish | 1 day | Error handling, UX, cross-platform testing |
| Phase 4: Testing | 1 day | Unit tests, integration tests, documentation |
| **Total** | **4-5 days** | |

## Technical Highlights

### Performance
- **Target**: < 50ms added to startup time
- **Most runs**: Instant (cache hit)
- **Network check**: Async, max 5 second timeout
- **No blocking**: Main functionality unaffected

### Security
- HTTPS only for API calls
- Safe deserialization
- No sensitive data in cache
- Proper input validation

### Cross-Platform
- Linux: `~/.cache/litra-autotoggle/`
- macOS: `~/Library/Caches/litra-autotoggle/`
- Windows: `%LOCALAPPDATA%\litra-autotoggle\cache\`

### Error Handling
- Network errors: Log and continue silently
- Cache errors: Recreate, don't fail
- API rate limits: Handle gracefully
- Offline: No error, just skip check

## Success Metrics

After implementation, verify:
- [ ] Works on all platforms (Linux, macOS, Windows)
- [ ] No significant performance impact
- [ ] Respects all disable options
- [ ] Clear, actionable notifications
- [ ] Graceful error handling
- [ ] Comprehensive documentation
- [ ] High test coverage

## Next Steps

To implement this plan:

1. **Review Documentation**
   - Read [Full Implementation Plan](./auto-update-plan.md)
   - Study [Code Examples](./code-examples.md)
   - Use [Implementation Checklist](./implementation-checklist.md)

2. **Set Up Development Environment**
   - Ensure Rust 1.89.0+ installed
   - Set up testing on all target platforms
   - Familiarize with codebase structure

3. **Begin Phase 1**
   - Add dependencies to Cargo.toml
   - Create src/update_checker.rs
   - Implement core checking logic
   - Add caching mechanism

4. **Follow Checklist**
   - Work through phases sequentially
   - Test frequently
   - Commit working code incrementally
   - Update documentation as you go

## Alternative Considered: `self_update`

The `self_update` crate was considered but rejected because:
- ❌ Would conflict with package manager installations
- ❌ More complex to implement
- ❌ Users prefer updating via their installation method
- ❌ Higher risk of permission issues
- ❌ Larger dependency footprint

## Questions & Concerns

### Will this annoy users?
- No: Checks only once per 24 hours (configurable)
- No: Easy to disable permanently
- No: Non-intrusive message format
- No: Only shown when update actually available

### What about offline usage?
- Handles gracefully with 5-second timeout
- Doesn't error or warn user
- Application continues normally
- Next check happens on next online run

### What about CI/CD?
- Disabled by default when `CI=true`
- Can be explicitly enabled if desired
- Won't interfere with automated builds

### Performance impact?
- Minimal: < 50ms on cache hit
- Async: Doesn't block main functionality
- Smart caching: Reduces API calls
- Timeout: Max 5 seconds on network check

## Resources

- [update-informer crate](https://crates.io/crates/update-informer)
- [GitHub Releases API](https://docs.github.com/en/rest/releases)
- [Rust CLI Best Practices](https://rust-cli-recommendations.sunshowers.io/)
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)

## Conclusion

This plan provides a comprehensive, well-thought-out approach to implementing automatic update checks that:
- Improves user experience
- Respects user preferences
- Works reliably across platforms
- Maintains application performance
- Protects user privacy

The implementation is straightforward, low-risk, and follows Rust CLI best practices.
