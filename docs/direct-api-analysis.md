# Direct GitHub API vs update-informer: Analysis and Comparison

## Question
"Would it be an option to call the GitHub API directly from our code?"

## Answer: YES - And It May Be the Better Option!

After careful analysis, **calling the GitHub API directly is absolutely a viable option** and may actually be **preferable** for this project. Here's why:

## Comparison Analysis

### Option 1: Using `update-informer` Crate (Original Recommendation)

**What it does:**
- Abstracts away GitHub API calls
- Provides built-in caching
- Handles multiple registries (GitHub, crates.io, npm, PyPI)

**Pros:**
- ✅ Less code to write
- ✅ Built-in caching logic
- ✅ Well-tested by community
- ✅ Abstracts API details

**Cons:**
- ❌ Additional dependency (~6 dependencies transitively)
- ❌ Less control over caching strategy
- ❌ Less control over error handling
- ❌ Opinionated about how checks work
- ❌ May have features we don't need
- ❌ Dependency maintenance burden

**Dependency footprint:**
```toml
update-informer = "1.1"  # Adds: ureq, serde_json, dirs, etc.
```

---

### Option 2: Calling GitHub API Directly (Alternative)

**What it does:**
- Direct HTTP calls to GitHub Releases API
- Custom caching implementation
- Full control over behavior

**Pros:**
- ✅ **Minimal dependencies** (just reqwest + serde_json)
- ✅ **Full control** over caching, timing, error handling
- ✅ **Simpler** - only implement what we need
- ✅ **Transparent** - easy to understand and debug
- ✅ **Flexible** - easy to customize behavior
- ✅ **No dependency maintenance** for update-checking-specific crate
- ✅ **Smaller binary size**

**Cons:**
- ❌ More code to write (~100-150 lines)
- ❌ Need to implement caching ourselves
- ❌ Need to handle API quirks ourselves

**Dependency footprint:**
```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde_json = "1.0"  # Already in project
dirs = "5.0"
```

## Recommendation: Call GitHub API Directly

### Why This is Better for litra-autotoggle

1. **Already Has HTTP Client Needs**
   - Project may need HTTP client for other features in future
   - reqwest is more general-purpose and useful
   - Avoids a niche dependency

2. **Minimal Code Required**
   - GitHub Releases API is simple and well-documented
   - Only need one endpoint: `/repos/:owner/:repo/releases/latest`
   - Caching is straightforward with JSON file

3. **Better Dependency Hygiene**
   - Fewer transitive dependencies
   - More control over dependency tree
   - Easier to audit for security

4. **More Maintainable**
   - Code is in our control
   - No need to track update-informer updates
   - Easier to debug issues

5. **Rust Ecosystem Practice**
   - Many successful Rust CLI tools call APIs directly
   - Common pattern in the ecosystem
   - Not overengineering

## Implementation: Direct GitHub API

### Required Dependencies

```toml
[dependencies]
# ... existing dependencies ...
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde_json = "1.0"  # Already may be present
dirs = "5.0"
chrono = { version = "0.4", features = ["serde"] }
```

**Why rustls-tls?**
- Avoids OpenSSL dependency (better cross-platform)
- Pure Rust implementation
- Smaller binary size

### API Endpoint

```
GET https://api.github.com/repos/timrogers/litra-autotoggle/releases/latest
```

**Response Format:**
```json
{
  "tag_name": "v1.4.0",
  "name": "v1.4.0",
  "published_at": "2026-02-06T14:00:00Z",
  "body": "Release notes...",
  // ... other fields
}
```

### Implementation Code Example

```rust
use reqwest;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const GITHUB_API_BASE: &str = "https://api.github.com";
const REPO_OWNER: &str = "timrogers";
const REPO_NAME: &str = "litra-autotoggle";
const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    published_at: String,
}

/// Check for updates by querying GitHub Releases API
async fn check_github_releases() -> Result<Option<String>, Box<dyn std::error::Error>> {
    let url = format!(
        "{}/repos/{}/{}/releases/latest",
        GITHUB_API_BASE, REPO_OWNER, REPO_NAME
    );

    let client = reqwest::Client::builder()
        .timeout(UPDATE_CHECK_TIMEOUT)
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let release: GitHubRelease = response.json().await?;
    
    // Strip 'v' prefix if present
    let version = release.tag_name.trim_start_matches('v').to_string();
    
    Ok(Some(version))
}

/// Compare versions and determine if update is available
async fn check_for_updates(current_version: &str) -> Option<String> {
    match check_github_releases().await {
        Ok(Some(latest_version)) => {
            if latest_version != current_version {
                Some(latest_version)
            } else {
                None
            }
        }
        Ok(None) => None,
        Err(e) => {
            log::debug!("Failed to check for updates: {}", e);
            None
        }
    }
}
```

### Caching Implementation

Same as before - simple JSON file:

```rust
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    last_checked: DateTime<Utc>,
    latest_version: Option<String>,
    current_version: String,
}

fn get_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|mut path| {
        path.push("litra-autotoggle");
        path.push("update-cache.json");
        path
    })
}

fn read_cache() -> Option<UpdateCache> {
    let path = get_cache_path()?;
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_cache(cache: &UpdateCache) -> Result<(), std::io::Error> {
    let path = get_cache_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Cache directory not found")
    })?;
    
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let json = serde_json::to_string_pretty(cache)?;
    fs::write(path, json)
}
```

## GitHub API Considerations

### Rate Limiting

**Unauthenticated requests:**
- 60 requests per hour per IP
- Sufficient for our use case (max 1 check per 24 hours)

**Best practices:**
- Include User-Agent header (required by GitHub)
- Cache results appropriately
- Handle 403 (rate limit) gracefully

### API Stability

- GitHub Releases API is stable and versioned
- Widely used across the ecosystem
- Very unlikely to break

### Error Handling

```rust
async fn check_with_error_handling() -> Option<String> {
    match check_github_releases().await {
        Ok(Some(version)) => Some(version),
        Ok(None) => None,
        Err(e) => {
            // Log but don't show user unless verbose
            log::debug!("Update check failed: {}", e);
            
            // Could also check specific error types:
            // - Network errors: ignore
            // - Rate limit: cache and retry later
            // - Parse errors: log warning
            
            None
        }
    }
}
```

## Updated Implementation Plan

### Phase 1: Core Infrastructure (Revised)

1. ✅ Add `reqwest` dependency to Cargo.toml
2. ✅ Add `dirs`, `chrono`, `serde_json` (if not present)
3. ✅ Create `src/update_checker.rs` module
4. ✅ Implement GitHub API call function
5. ✅ Implement cache management
6. ✅ Implement version comparison

**Key difference:** Write ~100 lines of straightforward code instead of integrating a library.

### Benefits Over Original Plan

1. **Fewer Dependencies**
   - update-informer → reqwest (more general-purpose)
   - Reduces dependency tree complexity

2. **More Control**
   - Custom error messages
   - Custom caching strategy
   - Custom rate limit handling

3. **Better Fit**
   - reqwest is useful for future features
   - More aligned with Rust ecosystem practices

4. **Learning Value**
   - Team understands the code fully
   - No "magic" from third-party crate

## Code Comparison

### With update-informer:
```rust
use update_informer::{registry, Check};

let informer = update_informer::new(
    registry::GitHub, 
    "timrogers", 
    "litra-autotoggle", 
    env!("CARGO_PKG_VERSION")
).timeout(Duration::from_secs(5));

let version = informer.check_version().ok().flatten();
```

**Lines of code:** ~5 lines, but adds dependency

### With direct API:
```rust
async fn check_github_releases() -> Result<Option<String>, Box<dyn std::error::Error>> {
    let url = format!("{}/repos/{}/{}/releases/latest", 
        GITHUB_API_BASE, REPO_OWNER, REPO_NAME);
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .build()?;
    
    let release: GitHubRelease = client.get(&url).send().await?.json().await?;
    Ok(Some(release.tag_name.trim_start_matches('v').to_string()))
}
```

**Lines of code:** ~15-20 lines, no extra dependency

## Real-World Examples

Many popular Rust CLI tools call GitHub API directly:

1. **rustup** - Uses custom GitHub API integration
2. **cargo-edit** - Direct API calls for crate info
3. **bat** - Direct GitHub API for release checking
4. **ripgrep** - Custom update checking

This is a **common and accepted pattern** in the Rust ecosystem.

## Security Considerations

### With update-informer:
- Trust the crate maintainer
- Trust transitive dependencies
- Limited audit capability

### With direct API:
- ✅ Full code visibility
- ✅ Easy to audit
- ✅ No supply chain risk from update-checking crate
- ✅ Only depend on well-vetted HTTP client (reqwest)

## Performance Comparison

Both approaches have similar performance:
- Network call: ~100-500ms (same for both)
- Parsing: Negligible difference
- Caching: Similar strategies

**Verdict:** No significant performance difference

## Maintenance Comparison

### update-informer:
- Need to track crate updates
- Potential breaking changes
- Dependent on maintainer

### Direct API:
- ✅ Code is stable once written
- ✅ No breaking changes unless GitHub changes API
- ✅ Full control over maintenance

## Final Recommendation

**Use Direct GitHub API Calls with reqwest**

### Reasons:
1. ✅ Minimal additional code (~100-150 lines)
2. ✅ Fewer dependencies (better for binary size and security)
3. ✅ More control and transparency
4. ✅ reqwest is more general-purpose (useful for future)
5. ✅ Common pattern in Rust ecosystem
6. ✅ Easier to customize and debug
7. ✅ No dependency on niche crate

### When to Use update-informer Instead:
- Need to check multiple registries (GitHub, crates.io, npm)
- Want absolute minimal code
- Don't want to maintain caching logic
- Team lacks HTTP client experience

### For litra-autotoggle:
Direct API is the better choice because:
- Only need GitHub releases
- Team can maintain simple HTTP code
- Better dependency hygiene
- More flexible for future needs

## Updated Documentation

I'll update the implementation documentation to reflect this approach:
- Replace update-informer with reqwest
- Provide complete implementation examples
- Update dependency list
- Keep all other planning intact

## Implementation Timeline (Updated)

Still **4-5 days**, but with this breakdown:

### Phase 1: Core Infrastructure (1-2 days)
- Add reqwest, dirs, chrono
- Implement GitHub API call function (~30 lines)
- Implement cache management (~50 lines)
- Implement version comparison (~20 lines)

### Phase 2: Integration (1 day)
- Same as before

### Phase 3: Polish (1 day)
- Same as before

### Phase 4: Testing (1 day)
- Same as before, plus:
  - Test API response parsing
  - Test rate limit handling
  - Test network errors

## Conclusion

**YES, calling the GitHub API directly is not just an option - it's the RECOMMENDED approach** for this project because:

1. More control
2. Fewer dependencies
3. Better maintainability
4. Common Rust pattern
5. Easier to understand and debug

The original recommendation of update-informer was good for rapid development, but the direct API approach is better for production code in this case.
