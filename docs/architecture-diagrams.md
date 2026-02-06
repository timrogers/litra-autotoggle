# Implementation Flow Diagram

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         User Runs CLI                            │
│                    $ litra-autotoggle                            │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Parse Arguments & Config                      │
│  • Read config file (if specified)                              │
│  • Parse CLI flags                                               │
│  • Merge configuration                                           │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
                     ┌──────────────┐
                     │ Update Check │
                     │   Disabled?  │
                     └──────┬───────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
           YES│                           │NO
              ▼                           ▼
    ┌──────────────────┐      ┌──────────────────────┐
    │  Skip Update     │      │  Check Cache         │
    │     Check        │      │  Expired?            │
    └────────┬─────────┘      └──────┬───────────────┘
             │                       │
             │         ┌─────────────┴─────────────┐
             │         │                           │
             │      YES│                           │NO (Cache Valid)
             │         ▼                           ▼
             │  ┌──────────────────┐    ┌──────────────────┐
             │  │ Query GitHub     │    │ Use Cached       │
             │  │ Releases API     │    │ Result           │
             │  │ (async, 5s max)  │    └────────┬─────────┘
             │  └────────┬─────────┘             │
             │           │                        │
             │           ▼                        │
             │  ┌──────────────────┐             │
             │  │ Update Cache     │             │
             │  │ (write to disk)  │             │
             │  └────────┬─────────┘             │
             │           │                        │
             │           └────────────────────────┘
             │                    │
             │                    ▼
             │         ┌──────────────────┐
             │         │ New Version      │
             │         │ Available?       │
             │         └──────┬───────────┘
             │                │
             │     ┌──────────┴──────────┐
             │     │                     │
             │  YES│                     │NO
             │     ▼                     ▼
             │  ┌──────────────┐  ┌──────────────┐
             │  │ Display      │  │ Continue     │
             │  │ Update       │  │ Silently     │
             │  │ Message      │  │              │
             │  └──────┬───────┘  └──────┬───────┘
             │         │                 │
             └─────────┴─────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│              Continue Normal Application Execution               │
│  • Find Litra devices                                            │
│  • Watch webcam events                                           │
│  • Toggle lights                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Update Check Flow (Detailed)

```
┌─────────────────────────────────────────────────────────────────┐
│                    check_for_updates()                           │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
                 ┌────────────────────┐
                 │ Check Environment  │
                 │ Variables:         │
                 │ • NO_UPDATE_CHECK  │
                 │ • CI               │
                 └─────────┬──────────┘
                           │
                           ▼
                 ┌────────────────────┐
                 │ Get Cache Path     │
                 │ Platform-specific: │
                 │ • Linux: ~/.cache  │
                 │ • macOS: ~/Library │
                 │ • Windows: %LOCAL% │
                 └─────────┬──────────┘
                           │
                           ▼
                 ┌────────────────────┐
                 │ Read Cache File    │
                 │ (JSON)             │
                 └─────────┬──────────┘
                           │
              ┌────────────┴────────────┐
              │                         │
       Exists │                         │ Not Exists or Expired
              ▼                         ▼
    ┌──────────────────┐    ┌──────────────────────┐
    │ Cache Valid &    │    │ Call update-informer │
    │ Not Expired?     │    │ • GitHub API         │
    └────────┬─────────┘    │ • 5 second timeout   │
             │               └──────────┬───────────┘
          YES│                          │
             │                          ▼
             │               ┌──────────────────────┐
             │               │ Parse Response       │
             │               │ • Latest version     │
             │               │ • Compare semver     │
             │               └──────────┬───────────┘
             │                          │
             │                          ▼
             │               ┌──────────────────────┐
             │               │ Write Cache          │
             │               │ • Current version    │
             │               │ • Latest version     │
             │               │ • Timestamp          │
             │               └──────────┬───────────┘
             │                          │
             └──────────────────────────┘
                            │
                            ▼
                 ┌────────────────────┐
                 │ Return Result      │
                 │ • None (up to date)│
                 │ • Some(version)    │
                 └────────────────────┘
```

## Cache Structure

```
Platform-Specific Cache Directory:
├── Linux:   ~/.cache/litra-autotoggle/
├── macOS:   ~/Library/Caches/litra-autotoggle/
└── Windows: %LOCALAPPDATA%\litra-autotoggle\cache\
    │
    └── update-cache.json
        {
          "last_checked": "2026-02-06T14:13:56Z",
          "latest_version": "1.4.0",
          "current_version": "1.3.0"
        }
```

## Configuration Priority

```
┌─────────────────────────────────────┐
│     Configuration Priority          │
│     (Highest to Lowest)             │
└─────────────────────────────────────┘
           │
           ▼
    ┌─────────────┐
    │ CLI Flags   │  --no-update-check, --check-update
    │ (Highest)   │
    └──────┬──────┘
           │
           ▼
    ┌─────────────┐
    │ Environment │  LITRA_AUTOTOGGLE_NO_UPDATE_CHECK=1
    │ Variables   │  LITRA_AUTOTOGGLE_UPDATE_CHECK_INTERVAL=48
    └──────┬──────┘
           │
           ▼
    ┌─────────────┐
    │ Config File │  disable_update_check: false
    │ (YAML)      │  update_check_interval_hours: 24
    └──────┬──────┘
           │
           ▼
    ┌─────────────┐
    │ Defaults    │  Check enabled, 24-hour interval
    │ (Lowest)    │
    └─────────────┘
```

## Error Handling Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                      Error Occurs                            │
└───────────────────────────┬─────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
    Network Error                   Cache Error
              │                           │
              ▼                           ▼
    ┌──────────────────┐      ┌──────────────────┐
    │ Log Debug Msg    │      │ Log Debug Msg    │
    │ "Update check    │      │ "Cache operation │
    │  failed"         │      │  failed"         │
    └────────┬─────────┘      └────────┬─────────┘
             │                         │
             ▼                         ▼
    ┌──────────────────┐      ┌──────────────────┐
    │ Don't show user  │      │ Fall back to     │
    │ (unless verbose) │      │ no cache         │
    └────────┬─────────┘      └────────┬─────────┘
             │                         │
             └─────────────┬───────────┘
                           │
                           ▼
              ┌────────────────────────┐
              │ Application Continues  │
              │ Normally (no crash)    │
              └────────────────────────┘
```

## Module Structure

```
src/
├── main.rs
│   ├── mod update_checker;
│   ├── Cli struct (with new flags)
│   ├── Config struct (with new options)
│   └── main() function
│       ├── Parse args
│       ├── Check for updates (if enabled)
│       └── Run main logic
│
└── update_checker.rs
    ├── check_for_updates()       [Main entry point]
    ├── is_update_check_disabled() [Check env vars]
    ├── get_cache_dir()            [Platform paths]
    ├── read_cache()               [Read from disk]
    ├── write_cache()              [Write to disk]
    ├── is_cache_expired()         [Time check]
    ├── check_for_updates_internal() [GitHub API]
    ├── format_update_message()    [UI formatting]
    └── UpdateCache struct         [Data structure]
```

## Data Flow

```
User Input
    │
    ▼
┌─────────────┐
│ CLI Parser  │
└──────┬──────┘
       │
       ▼
┌─────────────┐       ┌──────────────┐
│ Config File │ ───── │ Config       │
└─────────────┘       │ Merger       │
       │              └──────┬───────┘
       │                     │
       ▼                     ▼
┌─────────────┐       ┌──────────────┐
│ Env Vars    │ ───── │ Final Config │
└─────────────┘       └──────┬───────┘
                             │
                             ▼
                      ┌──────────────┐
                      │ Update       │
                      │ Checker      │
                      └──────┬───────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
       ┌───────────┐  ┌───────────┐ ┌───────────┐
       │ Cache     │  │ GitHub    │ │ User      │
       │ (Read)    │  │ API       │ │ Output    │
       └───────────┘  └───────────┘ └───────────┘
              │              │              │
              └──────────────┴──────────────┘
                             │
                             ▼
                      ┌──────────────┐
                      │ Main App     │
                      │ Logic        │
                      └──────────────┘
```

## Timeline Visualization

```
Week 1
├── Day 1-2: Phase 1 - Core Infrastructure
│   ├── Add dependencies
│   ├── Create update_checker.rs
│   ├── Implement caching
│   └── Basic version checking
│
├── Day 3: Phase 2 - Integration
│   ├── Config struct updates
│   ├── CLI flag additions
│   └── main() integration
│
├── Day 4: Phase 3 - Polish
│   ├── Error handling
│   ├── UX refinement
│   └── Cross-platform testing
│
└── Day 5: Phase 4 - Testing & Docs
    ├── Unit tests
    ├── Integration tests
    └── Documentation updates
```

## Testing Strategy

```
Testing Pyramid

       ┌────────────┐
       │ Manual     │  Manual testing on all platforms
       │ Testing    │  Real GitHub API
       └────┬───────┘
            │
       ┌────┴────────┐
       │ Integration │  Full flow tests
       │ Tests       │  Mock GitHub API
       └────┬────────┘
            │
       ┌────┴─────────┐
       │  Unit Tests  │  Individual functions
       │              │  Comprehensive coverage
       └──────────────┘
```

## Success Criteria Checklist

```
Performance
  ├─ ✓ Startup time impact < 50ms
  ├─ ✓ Network timeout = 5 seconds
  └─ ✓ No blocking of main functionality

Functionality
  ├─ ✓ Checks GitHub releases
  ├─ ✓ Caches results (24 hour default)
  ├─ ✓ Respects disable options
  └─ ✓ Shows clear notification

Cross-Platform
  ├─ ✓ Works on Linux (x86_64 & ARM64)
  ├─ ✓ Works on macOS (Intel & Apple Silicon)
  └─ ✓ Works on Windows (x86_64 & ARM64)

Error Handling
  ├─ ✓ Handles network failures
  ├─ ✓ Handles cache corruption
  ├─ ✓ Handles API rate limiting
  └─ ✓ Never crashes application

Documentation
  ├─ ✓ README updated
  ├─ ✓ Config examples added
  ├─ ✓ CLI help text updated
  └─ ✓ Troubleshooting guide
```
