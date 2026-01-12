# Disable telemetry and prompt uploads by default

## Summary

This PR implements a privacy-focused fork policy that **disables all outbound telemetry and prompt uploads** by default. All changes are minimal, reversible early returns that preserve existing code structure while ensuring complete data privacy.

## What's Disabled

✅ **All telemetry (Sentry + PostHog)**
- `flush-logs` command exits immediately with a message
- No HTTP requests sent even if DSNs or API keys are present
- Background telemetry flush processes never spawned

✅ **All prompt uploads (CAS API)**
- Prompt enqueuing to CAS disabled at source
- `flush-cas` command exits immediately with a message
- Background CAS flush processes never spawned
- API upload calls return early with error

## What Still Works

✅ **Full local functionality**
- AI authorship tracking and attribution
- Statistics and visualization
- Git notes storage
- SQLite database for local prompt storage
- All agent integrations (Cursor, Claude Code, etc.)

## Technical Implementation

### 1. Central Policy Gate

Added `outbound_network_reporting_disabled()` function in `src/config.rs`:
- Returns `true` by default in this fork
- Single source of truth for all network reporting decisions
- Well-documented for easy auditing

### 2. Gated Telemetry Paths

**Modified files:**
- `src/observability/flush.rs`: Early exit in `handle_flush_logs()`
- `src/observability/mod.rs`: Early return in `spawn_background_flush()`

**Effect:** No Sentry/PostHog events sent, no background processes spawned.

### 3. Gated Prompt Upload Paths

**Modified files:**
- `src/authorship/post_commit.rs`: Early return in `enqueue_prompt_messages_to_cas()`
- `src/commands/flush_cas.rs`: Early exit in `handle_flush_cas()` and `spawn_background_cas_flush()`
- `src/api/cas.rs`: Early error return in `upload_cas()`

**Effect:** No prompt data uploaded to CAS or any remote API, regardless of configuration.

### 4. Tests Added

- Unit test for `outbound_network_reporting_disabled()` (always returns `true`)
- Tests for flush-logs early exit behavior
- Tests for URL sanitization (existing security feature)
- All 227 existing tests pass

### 5. Documentation

Updated `README.md` with prominent **Fork Policy** section explaining:
- What's disabled and why
- What still works locally
- Technical implementation details
- Privacy guarantees

## Files Modified

| File | Changes | Purpose |
|------|---------|---------|
| `README.md` | Added Fork Policy section | User-facing documentation |
| `src/config.rs` | Added policy gate function + test | Central control point |
| `src/observability/flush.rs` | Gated handler + tests | Disable telemetry flush |
| `src/observability/mod.rs` | Gated spawn function | Prevent background telemetry |
| `src/authorship/post_commit.rs` | Gated enqueue function | Prevent CAS queuing |
| `src/commands/flush_cas.rs` | Gated handlers + spawn | Disable CAS flush |
| `src/api/cas.rs` | Gated upload function | Block API calls |

## Verification

### Build and Test
```bash
cargo test --lib
# Result: ok. 227 passed; 0 failed; 0 ignored
```

### Manual Verification

**No outbound traffic during:**
1. `git-ai checkpoint` - Local tracking only
2. `git commit` - Prompts stored locally, no uploads
3. `git push` - No background flush processes spawned
4. `git-ai flush-logs` - Exits with "Telemetry disabled" message
5. `git-ai flush-cas` - Exits with "CAS uploads disabled" message

**Policy gate test:**
```rust
#[test]
fn test_outbound_network_reporting_disabled() {
    assert!(outbound_network_reporting_disabled());
}
```

## Privacy Guarantee

This fork will **never** upload prompts, telemetry, or any repository data to external servers, regardless of:
- Login state
- API keys or DSNs configured
- Custom `api_base_url` settings
- Environment variables

To re-enable network reporting, the source code must be modified and the project rebuilt.

## Migration Path

Users can verify the policy by:
1. Inspecting `src/config.rs::outbound_network_reporting_disabled()`
2. Running tests to confirm it returns `true`
3. Monitoring network traffic during normal usage (should be none)

## Compatibility

- ✅ All existing local features work unchanged
- ✅ Agent integrations continue to function
- ✅ Statistics and visualization intact
- ✅ Git operations unaffected
- ✅ No breaking changes to user workflows

## Reversibility

All changes are minimal early returns. To restore upstream behavior:
1. Change `outbound_network_reporting_disabled()` to return `false`
2. Rebuild the project
3. All telemetry and uploads will resume

## Questions?

See the **Fork Policy** section in README.md for complete documentation.
