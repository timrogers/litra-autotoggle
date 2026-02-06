# Quick Implementation Checklist

This checklist provides a step-by-step guide for implementing automatic update checks in litra-autotoggle.

## Pre-Implementation

- [ ] Review the full [Auto Update Plan](./auto-update-plan.md)
- [ ] Understand current codebase structure
- [ ] Set up development environment with all platforms

## Phase 1: Core Infrastructure (1-2 days)

### Dependencies
- [ ] Add `update-informer = "1.1"` to Cargo.toml
- [ ] Add `dirs = "5.0"` to Cargo.toml
- [ ] Add `serde_json = "1.0"` to Cargo.toml (if not already present)
- [ ] Add `chrono = { version = "0.4", features = ["serde"] }` to Cargo.toml
- [ ] Optional: Add `colored = "2.1"` for formatted output
- [ ] Run `cargo build` to verify dependencies

### Create Update Checker Module
- [ ] Create `src/update_checker.rs`
- [ ] Implement `UpdateCache` struct with serde support
  ```rust
  #[derive(Serialize, Deserialize)]
  struct UpdateCache {
      last_checked: String,
      latest_version: Option<String>,
      current_version: String,
  }
  ```
- [ ] Implement `is_update_check_disabled()` - check environment variables
- [ ] Implement `get_cache_dir()` - use `dirs` crate for platform paths
- [ ] Implement `read_cache()` - read and deserialize cache file
- [ ] Implement `write_cache()` - serialize and write cache file
- [ ] Implement `is_cache_expired()` - check if cache needs refresh

### Implement Version Checking
- [ ] Implement `check_for_updates_internal()` using `update-informer`
- [ ] Implement version comparison logic
- [ ] Handle network errors gracefully
- [ ] Implement timeout (5 seconds)
- [ ] Add proper error handling

### Format Output
- [ ] Implement `format_update_message()` - create notification text
- [ ] Make message informative but non-intrusive
- [ ] Include update instructions for all installation methods
- [ ] Add instructions to disable checks

## Phase 2: Integration (1 day)

### Update Config Struct
- [ ] Add `disable_update_check: Option<bool>` to Config struct
- [ ] Add `update_check_interval_hours: Option<u64>` to Config struct
- [ ] Update config validation if needed
- [ ] Update example config file with new options

### Update CLI Struct
- [ ] Add `--no-update-check` flag to Cli struct
- [ ] Add `--check-update` flag to Cli struct (force check)
- [ ] Update help text
- [ ] Test CLI parsing

### Environment Variables
- [ ] Check `LITRA_AUTOTOGGLE_NO_UPDATE_CHECK` environment variable
- [ ] Check `LITRA_AUTOTOGGLE_UPDATE_CHECK_INTERVAL` environment variable
- [ ] Check `LITRA_AUTOTOGGLE_FORCE_UPDATE_CHECK` environment variable
- [ ] Check `CI` environment variable (disable in CI by default)

### Integrate into main()
- [ ] Add `mod update_checker;` to main.rs
- [ ] Call update check after config parsing
- [ ] Make check async/non-blocking
- [ ] Display notification if update available
- [ ] Ensure main functionality isn't delayed

## Phase 3: Polish (1 day)

### Error Handling
- [ ] Handle network unavailable
- [ ] Handle GitHub API rate limiting
- [ ] Handle cache directory permission errors
- [ ] Handle corrupted cache files
- [ ] Ensure all errors are logged (verbose mode only)
- [ ] Ensure application never crashes due to update check

### User Experience
- [ ] Test notification display on different terminals
- [ ] Ensure message is clear and actionable
- [ ] Test with colored output (if implemented)
- [ ] Test in verbose mode
- [ ] Ensure no output in quiet/normal mode (only when update available)

### Cross-Platform Testing
- [ ] Test on Linux x86_64
- [ ] Test on Linux ARM64
- [ ] Test on macOS Intel
- [ ] Test on macOS Apple Silicon
- [ ] Test on Windows x86_64
- [ ] Test on Windows ARM64
- [ ] Verify cache directory creation on all platforms
- [ ] Verify file permissions on all platforms

## Phase 4: Testing & Documentation (1 day)

### Unit Tests
- [ ] Write tests for cache read/write
- [ ] Write tests for cache expiration
- [ ] Write tests for version comparison
- [ ] Write tests for environment variable handling
- [ ] Write tests for disabled check logic
- [ ] Run `cargo test` and ensure all pass

### Integration Tests
- [ ] Test full update check flow
- [ ] Test with different cache states
- [ ] Test with network errors
- [ ] Test with invalid cache files
- [ ] Mock GitHub API for testing

### Documentation
- [ ] Update README.md with update check section
- [ ] Document all environment variables
- [ ] Document CLI flags
- [ ] Add FAQ about update checks
- [ ] Update example config file comments
- [ ] Add troubleshooting section

### Example Config Updates
- [ ] Update `litra-autotoggle.example.yml`
- [ ] Add commented examples for new options
- [ ] Add explanation of defaults

## Final Verification

### Functionality
- [ ] Update check works on first run
- [ ] Cache is created properly
- [ ] Subsequent runs use cache
- [ ] Cache expires after interval
- [ ] Notification displays correctly when update available
- [ ] Disable flag works
- [ ] Force check flag works
- [ ] Environment variables work

### Performance
- [ ] Startup time impact < 50ms (cached)
- [ ] Network timeout works (5s max)
- [ ] No blocking of main functionality

### Code Quality
- [ ] Run `cargo clippy` and fix warnings
- [ ] Run `cargo fmt` to format code
- [ ] Review code for best practices
- [ ] Check for any TODOs or FIXMEs
- [ ] Ensure consistent error handling

### Security
- [ ] Only HTTPS connections to GitHub
- [ ] No sensitive data in cache
- [ ] Safe deserialization of cache
- [ ] Proper input validation

## Pre-Release Checklist

- [ ] All tests passing
- [ ] Documentation complete
- [ ] Tested on all supported platforms
- [ ] Performance metrics acceptable
- [ ] No known bugs
- [ ] Security review complete
- [ ] Code review complete

## Release

- [ ] Update version in Cargo.toml
- [ ] Update CHANGELOG
- [ ] Create release notes mentioning new feature
- [ ] Tag release
- [ ] Monitor for issues after release
- [ ] Update Homebrew formula if needed

## Post-Release Monitoring

- [ ] Monitor GitHub issues for update check problems
- [ ] Check if users find notifications helpful
- [ ] Gather feedback on frequency/UX
- [ ] Consider future enhancements based on feedback

## Notes

- **Order Matters**: Follow phases in sequence
- **Test Frequently**: Run tests after each major change
- **Incremental Commits**: Commit working code frequently
- **Documentation**: Update docs as you implement features
- **Ask for Help**: Don't hesitate to ask questions if stuck

## Common Pitfalls to Avoid

1. **Don't** block the main application during update checks
2. **Don't** show errors to users unless in verbose mode
3. **Don't** fail the application if update check fails
4. **Don't** check on every single run (respect cache)
5. **Don't** forget to handle offline scenarios
6. **Don't** forget cross-platform path handling
7. **Don't** ignore CI environment (should be disabled by default)
8. **Don't** make network calls without timeout

## Estimated Time Breakdown

| Phase | Tasks | Time |
|-------|-------|------|
| Phase 1 | Core Infrastructure | 1-2 days |
| Phase 2 | Integration | 1 day |
| Phase 3 | Polish | 1 day |
| Phase 4 | Testing & Docs | 1 day |
| **Total** | | **4-5 days** |

## Success Metrics

After implementation, the feature should:
- ✅ Work seamlessly on all platforms
- ✅ Not impact application performance
- ✅ Respect user preferences
- ✅ Provide clear, actionable information
- ✅ Handle errors gracefully
- ✅ Be well-documented
- ✅ Have comprehensive test coverage
