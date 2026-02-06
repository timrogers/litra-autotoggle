# Auto-Update Checks: Project Summary

## Overview

This directory contains comprehensive documentation for implementing auto-update checks in `litra-autotoggle` powered by the GitHub Releases API.

## Documents

### 1. Implementation Plan (`auto-update-checks-implementation-plan.md`)
**Purpose**: Comprehensive technical plan for implementation

**Contents**:
- Current state analysis
- Implementation strategy
- Configuration options
- Testing strategy
- Timeline and effort estimates
- Complete implementation checklist

**Audience**: Developers implementing the feature

---

### 2. Code Examples (`auto-update-checks-code-examples.md`)
**Purpose**: Detailed code examples and API interaction patterns

**Contents**:
- Complete working code examples
- GitHub API details and response formats
- Platform-specific implementations
- Error handling patterns
- Testing examples
- Configuration examples

**Audience**: Developers writing the code

---

### 3. Design Analysis (`auto-update-checks-design-analysis.md`)
**Purpose**: Analysis of design alternatives and trade-offs

**Contents**:
- Comparison of different approaches (self_update, update-informer, direct API)
- Design decision rationale
- Risk analysis
- Success criteria
- Comparison matrices

**Audience**: Reviewers and decision makers

---

## Quick Start

If you're implementing this feature:

1. **Read First**: `auto-update-checks-implementation-plan.md` (section 1-2 for overview)
2. **Code Reference**: `auto-update-checks-code-examples.md` (copy/adapt examples)
3. **Decision Context**: `auto-update-checks-design-analysis.md` (understand why we chose this approach)

## Key Decisions Summary

### ✅ Chosen Approach
**Direct GitHub Releases API integration using existing `reqwest`**

### ✅ Key Features
- Opt-in by default (privacy-respecting)
- Check on startup with 24-hour caching
- Async execution (non-blocking)
- Graceful error handling
- Platform-specific cache locations
- Boxed notification format

### ✅ No New Dependencies Required
All needed dependencies already present:
- `reqwest` (HTTP client)
- `serde`/`serde_json` (JSON parsing)
- `tokio` (async runtime)

### ⭕ Optional Enhancement
Consider adding `semver` crate for robust version comparison (small dependency, ~15KB)

## Implementation Checklist

See `auto-update-checks-implementation-plan.md` section 9 for the complete step-by-step implementation checklist.

**High-Level Steps**:
1. Add configuration structures
2. Implement cache management
3. Implement version comparison
4. Implement GitHub API client
5. Integrate into main()
6. Add tests
7. Update documentation

## Estimated Effort

- **Core Implementation**: 4-6 hours
- **Testing**: 1-2 hours
- **Documentation**: 1 hour
- **Total**: 5-8 hours

## Success Metrics

- ✅ Works on Linux, macOS, and Windows
- ✅ No startup performance impact
- ✅ Graceful error handling
- ✅ Privacy-respecting (opt-in)
- ✅ Clear user notifications
- ✅ Comprehensive test coverage

## GitHub Releases API Quick Reference

**Endpoint**: 
```
GET https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest
```

**Required Headers**:
```
User-Agent: litra-autotoggle/1.3.0
Accept: application/vnd.github+json
```

**Key Response Fields**:
```json
{
  "tag_name": "v1.4.0",
  "html_url": "https://github.com/.../releases/tag/v1.4.0",
  "prerelease": false,
  "draft": false
}
```

**Rate Limits**:
- 60 requests/hour (unauthenticated)
- Mitigated by 24-hour cache

## Configuration Examples

### Enable in Config File
```yaml
# litra-autotoggle.yml
check_updates: true
```

### Command Line Flags
```bash
# Enable for this run
litra-autotoggle --check-updates

# Disable even if configured
litra-autotoggle --no-update-check
```

## Testing Strategy

### Unit Tests
- Version comparison logic
- Cache read/write operations
- Version string parsing

### Integration Tests
- Mock GitHub API responses
- Rate limiting scenarios
- Network error handling
- Cache expiration logic

### Manual Testing
- Enable/disable via config
- Enable/disable via CLI flags
- Simulate old version (modify cache)
- Test on all platforms
- Test with network disconnected

## Security Considerations

1. ✅ **HTTPS Only** - GitHub API enforces TLS
2. ✅ **No Authentication** - Public API, no tokens stored
3. ✅ **Input Validation** - Version strings validated
4. ✅ **Timeouts** - 5-second HTTP timeout
5. ✅ **Fail Safely** - Never crash on check failure
6. ✅ **No Telemetry** - Only version check, no tracking
7. ✅ **Opt-In Default** - Privacy-respecting

## Privacy Transparency

**What Gets Sent**:
- User's IP address (inherent to HTTP request)
- User-Agent header: `litra-autotoggle/{version}`
- No user-specific identifiers
- No usage statistics
- No telemetry data

**How Often**:
- Once per 24 hours (when enabled)
- Only when user opts in

**Where**:
- Directly to GitHub's API (api.github.com)
- No intermediary services

## Platform-Specific Notes

### Linux
- Cache: `~/.config/litra-autotoggle/last_update_check`
- Respects `XDG_CONFIG_HOME`

### macOS
- Cache: `~/Library/Application Support/litra-autotoggle/last_update_check`
- Follows Apple conventions

### Windows
- Cache: `%APPDATA%\litra-autotoggle\last_update_check`
- Uses Windows APPDATA

## Error Handling Philosophy

**Principle**: Update checks should never impact core functionality

**Implementation**:
- All errors logged but not propagated
- Network failures don't block startup
- Rate limits handled gracefully
- Parse errors caught and logged
- Async execution prevents blocking

## Future Enhancements (Optional)

### Phase 2 (Low Priority)
- ETag/If-None-Match for bandwidth savings
- Display release notes in notification
- Configurable check intervals
- Platform-specific update instructions

### Phase 3 (Very Low Priority)
- Multiple update channels (stable/beta)
- Auto-download capability (with explicit user permission)
- Signature verification
- Rollback support

## References

### API Documentation
- [GitHub REST API - Releases](https://docs.github.com/en/rest/releases/releases)

### Rust Documentation
- [reqwest Crate](https://docs.rs/reqwest/)
- [serde_json Crate](https://docs.rs/serde_json/)
- [tokio Spawn](https://docs.rs/tokio/latest/tokio/fn.spawn.html)

### Best Practices
- [Rust CLI Recommendations](https://rust-cli-recommendations.sunshowers.io/)
- [Rust Cookbook - Web APIs](https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html)

### Example Implementations
- [rustup Update Checker](https://github.com/rust-lang/rustup) - Reference implementation
- [bat Update Checker](https://github.com/sharkdp/bat) - Reference implementation

## Getting Help

If you have questions during implementation:

1. **API Questions**: See `auto-update-checks-code-examples.md` section on GitHub API
2. **Design Questions**: See `auto-update-checks-design-analysis.md` for rationale
3. **Implementation Questions**: See `auto-update-checks-implementation-plan.md` for step-by-step guide

## Contributing

When implementing this feature:

1. Follow the implementation checklist in the plan document
2. Use the code examples as templates
3. Add comprehensive tests
4. Update user documentation (README, example config)
5. Test on all platforms (Linux, macOS, Windows)

## License

This documentation is part of the litra-autotoggle project and follows the same MIT license.

---

**Document Version**: 1.0  
**Last Updated**: 2026-02-06  
**Status**: Ready for Implementation
