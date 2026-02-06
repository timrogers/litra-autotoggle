# Plan: Auto Update Checks Powered by GitHub Releases

## Overview

Add an opt-out update check that runs once on startup, queries the GitHub Releases API for the latest release, compares it against the running version, and logs a message if a newer version is available. The check is non-blocking and best-effort — network failures are silently ignored and never affect normal operation.

## Design Principles

- **Non-intrusive**: Update check runs asynchronously on startup and never delays or blocks the main webcam-monitoring loop.
- **Best-effort**: Network errors, rate limits, and parse failures are silently swallowed (logged at `debug` level only). The tool must never crash or degrade due to an update check.
- **Opt-out**: Enabled by default, with a `--skip-update-check` CLI flag and `skip_update_check` config file option to disable.
- **No auto-download**: The feature only *notifies* — it never downloads or replaces binaries. Users choose how to update (Homebrew, Cargo, manual download).
- **Minimal dependencies**: Use `ureq` (a small, blocking HTTP client) run inside `tokio::task::spawn_blocking` to avoid pulling in a large async HTTP stack.

## Current State

| Aspect | Details |
|---|---|
| Version source of truth | `Cargo.toml` `version = "1.3.0"` — embedded at compile time via `clap`'s `#[clap(version)]` |
| GitHub repo | `timrogers/litra-autotoggle` |
| Release tag format | `v1.3.0`, `v1.2.0`, etc. |
| GitHub Releases API | `GET https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest` |
| Existing HTTP dependencies | None |
| Async runtime | `tokio` with `full` features |

## Implementation Steps

### Step 1: Add dependencies to `Cargo.toml`

```toml
[dependencies]
semver = "1"
serde_json = "1"
ureq = "3"
```

- **`semver`** — for robust semver parsing and comparison.
- **`serde_json`** — for parsing the GitHub API JSON response (only the `tag_name` and `html_url` fields are needed).
- **`ureq`** — a minimal, blocking HTTP client. It will be called from `spawn_blocking` to avoid blocking the tokio runtime. Chosen over `reqwest` to avoid pulling in `hyper`/`h2`/`rustls` and significantly increasing compile times and binary size.

### Step 2: Add configuration support

**CLI argument** (in the `Cli` struct in `src/main.rs`):

```rust
#[clap(
    long,
    action,
    help = "Skip checking for updates on startup"
)]
skip_update_check: bool,
```

**Config file field** (in the `Config` struct in `src/main.rs`):

```rust
#[serde(skip_serializing_if = "Option::is_none")]
skip_update_check: Option<bool>,
```

**Config merge** (in `merge_config_with_cli`):

```rust
if !cli.skip_update_check {
    cli.skip_update_check = config.skip_update_check.unwrap_or(false);
}
```

**Example config** (add to `litra-autotoggle.example.yml`):

```yaml
# By default, the tool checks for updates on startup. You can disable this by setting
# the option below to true.
#
# skip_update_check: true
```

### Step 3: Implement the update check function

Add a new function in `src/main.rs`:

```rust
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest";

/// Checks GitHub for a newer release and logs a warning if one is available.
/// Returns silently on any error (network, parse, rate-limit, etc.).
fn check_for_updates() {
    let response = match ureq::get(GITHUB_RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", concat!("litra-autotoggle/", env!("CARGO_PKG_VERSION")))
        .call()
    {
        Ok(resp) => resp,
        Err(e) => {
            log::debug!("Update check failed: {e}");
            return;
        }
    };

    let body: String = match response.body_mut().read_to_string() {
        Ok(b) => b,
        Err(e) => {
            log::debug!("Update check: failed to read response body: {e}");
            return;
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            log::debug!("Update check: failed to parse JSON: {e}");
            return;
        }
    };

    let tag = match json["tag_name"].as_str() {
        Some(t) => t,
        None => {
            log::debug!("Update check: no tag_name in response");
            return;
        }
    };

    let html_url = json["html_url"].as_str().unwrap_or(
        "https://github.com/timrogers/litra-autotoggle/releases/latest",
    );

    // Strip leading 'v' from tag to get semver string
    let latest_version_str = tag.strip_prefix('v').unwrap_or(tag);

    let current = match semver::Version::parse(CURRENT_VERSION) {
        Ok(v) => v,
        Err(e) => {
            log::debug!("Update check: failed to parse current version: {e}");
            return;
        }
    };

    let latest = match semver::Version::parse(latest_version_str) {
        Ok(v) => v,
        Err(e) => {
            log::debug!("Update check: failed to parse latest version '{latest_version_str}': {e}");
            return;
        }
    };

    if latest > current {
        log::warn!(
            "A new version of litra-autotoggle is available: v{latest} (you have v{current}). \
             Download it at: {html_url}"
        );
    } else {
        log::debug!("litra-autotoggle is up to date (v{current})");
    }
}
```

### Step 4: Call the check from `main()`

In each platform's `main()` function, after logger initialization but before calling `handle_autotoggle_command`, spawn the update check on a background thread:

```rust
if !args.skip_update_check {
    tokio::task::spawn_blocking(check_for_updates);
}
```

This ensures:
- The check runs concurrently with normal startup.
- It does not delay the webcam monitoring loop.
- Since `spawn_blocking` runs on the tokio blocking thread pool, the synchronous `ureq` call won't block the async runtime.
- The `JoinHandle` is intentionally dropped (fire-and-forget). If the check is still in-flight when the program exits, it is silently cancelled.

### Step 5: Add tests

Add unit tests for version comparison logic. The network call itself is not unit-tested (it depends on external state), but the parsing and comparison logic can be extracted into a testable helper:

```rust
/// Parses a GitHub release tag and compares it to the current version.
/// Returns `Some((latest_version, html_url))` if an update is available, `None` otherwise.
fn parse_update_from_release_json(json_body: &str) -> Option<(String, String)> {
    let json: serde_json::Value = serde_json::from_str(json_body).ok()?;
    let tag = json["tag_name"].as_str()?;
    let html_url = json["html_url"]
        .as_str()
        .unwrap_or("https://github.com/timrogers/litra-autotoggle/releases/latest")
        .to_string();

    let latest_version_str = tag.strip_prefix('v').unwrap_or(tag);
    let current = semver::Version::parse(CURRENT_VERSION).ok()?;
    let latest = semver::Version::parse(latest_version_str).ok()?;

    if latest > current {
        Some((latest.to_string(), html_url))
    } else {
        None
    }
}
```

Test cases:

```rust
#[test]
fn test_parse_update_available() {
    let json = r#"{"tag_name": "v99.0.0", "html_url": "https://github.com/timrogers/litra-autotoggle/releases/tag/v99.0.0"}"#;
    let result = parse_update_from_release_json(json);
    assert!(result.is_some());
    let (version, url) = result.unwrap();
    assert_eq!(version, "99.0.0");
    assert!(url.contains("v99.0.0"));
}

#[test]
fn test_parse_no_update_same_version() {
    let json = format!(
        r#"{{"tag_name": "v{}", "html_url": "https://example.com"}}"#,
        CURRENT_VERSION
    );
    assert!(parse_update_from_release_json(&json).is_none());
}

#[test]
fn test_parse_no_update_older_version() {
    let json = r#"{"tag_name": "v0.0.1", "html_url": "https://example.com"}"#;
    assert!(parse_update_from_release_json(&json).is_none());
}

#[test]
fn test_parse_invalid_json() {
    assert!(parse_update_from_release_json("not json").is_none());
}

#[test]
fn test_parse_missing_tag_name() {
    let json = r#"{"html_url": "https://example.com"}"#;
    assert!(parse_update_from_release_json(json).is_none());
}

#[test]
fn test_parse_invalid_semver_tag() {
    let json = r#"{"tag_name": "not-semver", "html_url": "https://example.com"}"#;
    assert!(parse_update_from_release_json(json).is_none());
}
```

### Step 6: Update documentation

Add a section to `README.md` under a new heading:

```markdown
### Update checks

By default, `litra-autotoggle` checks for new versions on startup by querying
GitHub Releases. If a newer version is available, a message is logged.

This check is non-blocking and will not delay startup. If the check fails
(e.g., due to no internet connection), it is silently ignored.

To disable this check, use the `--skip-update-check` flag or set
`skip_update_check: true` in your configuration file.
```

## File Changes Summary

| File | Change |
|---|---|
| `Cargo.toml` | Add `semver`, `serde_json`, `ureq` dependencies |
| `src/main.rs` | Add `Config.skip_update_check` field |
| `src/main.rs` | Add `Cli.skip_update_check` argument |
| `src/main.rs` | Update `merge_config_with_cli` to merge the new field |
| `src/main.rs` | Add `CURRENT_VERSION` and `GITHUB_RELEASES_URL` constants |
| `src/main.rs` | Add `check_for_updates()` function |
| `src/main.rs` | Add `parse_update_from_release_json()` helper (testable) |
| `src/main.rs` | Call `tokio::task::spawn_blocking(check_for_updates)` in each platform's `main()` |
| `src/main.rs` | Add 6 unit tests for the parse/compare logic |
| `litra-autotoggle.example.yml` | Add commented `skip_update_check` option |
| `README.md` | Add "Update checks" documentation section |

## Considerations

### Why `ureq` over `reqwest`?

`reqwest` pulls in a large dependency tree (`hyper`, `h2`, `tower`, `rustls` or `native-tls`), significantly increasing compile times and binary size. Since the update check is a single GET request that doesn't need connection pooling, streaming, or async I/O, `ureq` is a better fit. It's wrapped in `spawn_blocking` to avoid blocking the tokio runtime.

### Why not check periodically during the daemon's lifetime?

The daemon can run for days or weeks. However:
- Adding periodic checks adds complexity (timers, state for "already notified").
- The user won't see the log message unless they're watching logs in real-time.
- Startup is the natural point when a user might act on an update notification.
- This matches the behavior of tools like `gh` (GitHub CLI) and `npm`.

A periodic check could be added later if there's demand.

### Rate limiting

The GitHub API allows 60 unauthenticated requests/hour per IP. Since this tool checks once per startup, and users typically start the daemon once per session (or once per boot), this is well within limits. The `User-Agent` header is set as required by GitHub's API policy.

### Pre-release versions

The `/releases/latest` endpoint only returns non-prerelease, non-draft releases, so pre-release tags (e.g., `v2.0.0-beta.1`) are automatically excluded.

### Homebrew users

Homebrew users may prefer to update via `brew upgrade`. The update check message directs users to the GitHub Releases page, which is appropriate regardless of installation method. The message does not prescribe a specific update command.
