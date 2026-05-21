# PR Body Draft — feat: render user-visible time in Beijing time (Asia/Shanghai)

> Ready to use with: `gh pr create --title "feat: render user-visible time in Beijing time (Asia/Shanghai)" --body-file .sisyphus/notepads/kanmars.req.20260508.001.plan/pr-body.md`

---

## Summary

Renders all **user/LLM-directly-visible time displays** in Beijing time (Asia/Shanghai, UTC+8) — by hardcoding `chrono_tz::Asia::Shanghai` rather than relying on the OS `TZ` env var (which the deployment host does not set).

**Storage layer stays UTC** (DB columns, API wire format, OAuth/protocol fields). Only the 4 user-visible surfaces change: CLI output, log timestamps, LLM prompts, and agent tool returns.

Closes scope creep dilemma: 3 incidentally-discovered consistency BUGs in non-display paths (runtime_trace mixed Local/Utc, hygiene state-file timestamps, skillforge metadata `Z` literal) are deliberately **not** fixed here — see Known Carve-outs below for follow-up PR pointers.

Plan: [.sisyphus/plans/kanmars.req.20260508.001.plan.md](.sisyphus/plans/kanmars.req.20260508.001.plan.md) rev3.1 (3 rounds of Momus review: rev1→rev2→rev3→rev3.1)

## What changes (4 user-visible surfaces, ~38 sites)

### 1. CLI terminal output (`zeroclaw cron list` / `zeroclaw auth list`)
- New helper `src/time_display.rs::fmt_beijing_rfc3339()` — single-purpose 1-function module
- 11 call sites in `src/cron/mod.rs` (next_run / last_run printf)
- 2 call sites in `src/main.rs::format_expiry()` (auth token expiry)
- Smoke tested: `cron list` outputs `next=2026-05-08T20:29:30+08:00`

### 2. Log timestamps (`zeroclaw.log`)
- Custom `BeijingTimer impl tracing_subscriber::fmt::time::FormatTime` in `src/main.rs`
- Wired via `.with_timer(BeijingTimer)` on the global subscriber builder
- Dual `#[cfg]` arms so the kernel-only build (`--no-default-features`) falls back to default UTC timer (chrono-tz is in `agent-runtime` feature only)
- Smoke tested: stderr emits `2026-05-08T19:28:41.881+08:00 INFO ...`

### 3. LLM prompt time injection (8 production sites)
- `crates/zeroclaw-runtime/src/agent/system_prompt.rs:283` — `## Current Date & Time` block
- `crates/zeroclaw-runtime/src/agent/prompt.rs:259` — `CRITICAL CONTEXT: CURRENT DATE & TIME` block (also dropped unused `Local` import)
- `crates/zeroclaw-runtime/src/agent/agent.rs:1094, 1271` — per-turn `[CURRENT DATE & TIME: ...]` prefix (2 sites)
- `crates/zeroclaw-runtime/src/agent/loop_.rs:2571, 2861, 3449` — per-message timestamp markers (3 sites)
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:771` — channel system-prompt hot-reload
- Test code at `orchestrator/mod.rs:9291` (inside `#[test] fn prompt_no_daily_memory_injection`) intentionally left untouched per Plan §0.

### 4. Agent tool JSON outputs (4 tool files, 13 sites)
- `tools/schedule.rs` (7 sites): `dt.to_rfc3339()` → `dt.with_timezone(&chrono_tz::Asia::Shanghai).to_rfc3339()`
- `tools/delegate.rs` (3 sites): same pattern for `started_at`/`finished_at`
- `tools/cron_runs.rs`: **`RunView` struct field types unchanged** (per Plan §0.5 #4); JSON reconstructed via `serde_json::json!()` at the agent-facing boundary, preserving SQLite read path & external schema expectations
- `tools/cron_add.rs`: `next_run` field converted at `json!()` call site

### 5. BUG fixes in same surfaces
- `crates/zeroclaw-runtime/src/heartbeat/engine.rs:328`: prompt was `Current time: {} UTC` with literal `UTC` token + `Utc::now()` — LLM was being told its current time was UTC, causing wrong-time-of-day decisions
- `crates/zeroclaw-runtime/src/tools/security_ops.rs:217`: MCSS report was `Generated: {} UTC` (literal "UTC")

Both now use `Utc::now().with_timezone(&Asia::Shanghai)` + `%Z` format spec.

## Why hardcode (not `chrono::Local::now()` or config field)

User has **no `TZ` env var set** on the deployment host. `chrono::Local::now()` reads `/etc/localtime` — on a stripped/container deploy this can fall back to UTC silently. So 8 prior `Local::now()` sites were susceptible to the same UTC bug they'd been fighting.

A configurable `[runtime].display_timezone` was considered but rejected — user requirement is "all Beijing", hardcoding is the smallest viable change. If multi-tz support is needed later, a single `s/chrono_tz::Asia::Shanghai/config.runtime.display_timezone/` pass replaces the constant.

`chrono_tz::Asia::Shanghai` is a compile-time const — no `unwrap()`/`expect()` introduced.

## Manual verifications (5/5 evidence)

| # | Surface | Status | Evidence |
|---|---|---|---|
| 1 | `zeroclaw cron list` shows `+08:00` | ✅ | `next=2026-05-08T20:29:30.255629112+08:00` |
| 2 | `zeroclaw auth list` shows `+08:00` | ⚠️ N/A | Test host has no OAuth provider configured. Code path verified by clippy + grep (`format_expiry()` 2 sites use helper, 0 bare `to_rfc3339`). |
| 3 | `zeroclaw.log` first line shows `+08:00` | ✅ | `2026-05-08T19:28:41.881+08:00 INFO zeroclaw_config::schema: Config loaded ...` |
| 4 | Agent reply to "what time is it" | ⏸ | Run `cargo run -- agent` and ask interactively; expect Beijing time. Code path verified by 8-site Local→Asia/Shanghai migration + workspace clippy. |
| 5 | Heartbeat decision prompt content | ⏸ | Run heartbeat trigger; check `runtime_trace.jsonl` or `RUST_LOG=debug` log; expect `+0800`/`+08:00`/`CST`, no literal ` UTC`. Code path verified by `grep -n ' UTC'` 0 hits + 40 heartbeat tests pass. |

## Test results

- `cargo build` (touched crates): ✅ Finished in 1m 05s
- `cargo clippy --all-targets -- -D warnings` (runtime + channels + main bin): ✅ 0 warnings
- `cargo test -p zeroclaw-runtime --lib`: ✅ **1622 passed; 0 failed**
- `cargo test -p zeroclaw-channels --lib`: 940 passed; **2 pre-existing failures unrelated to this PR** (`build_channel_by_id_*_telegram_*` — verified by `git checkout master && cargo test ...` reproducing the same failures; channel registry bug, not timezone)

## All grep verifications (run on current HEAD = f85ebf32)

```bash
# 1. Production code 0 residual chrono::Local::now() (covers bare + fully-qualified):
$ grep -rnE '(chrono::)?Local::now\(\)' crates/zeroclaw-runtime/src/agent/ crates/zeroclaw-channels/src/orchestrator/mod.rs
crates/zeroclaw-channels/src/orchestrator/mod.rs:9291:        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
# Only test-code hit at line 9291 — expected.

# 2. Helper called 11 times in cron/mod.rs + 2 in main.rs (13 total):
$ grep -c fmt_beijing_rfc3339 src/cron/mod.rs src/main.rs
src/cron/mod.rs:11
src/main.rs:2

# 3. cron/mod.rs zero bare to_rfc3339:
$ grep -c '\.to_rfc3339()' src/cron/mod.rs
0

# 4. BeijingTimer ≥3 hits in main.rs (struct + impl + with_timer):
$ grep -c BeijingTimer src/main.rs
3

# 5. " UTC" literal removed from heartbeat + MCSS:
$ grep -n ' UTC' crates/zeroclaw-runtime/src/heartbeat/engine.rs crates/zeroclaw-runtime/src/tools/security_ops.rs
(0 hits)

# 6. Asia::Shanghai conversion count across runtime + channels prompt files:
$ grep -rnc 'with_timezone(&chrono_tz::Asia::Shanghai)' crates/zeroclaw-runtime/src/agent/ crates/zeroclaw-channels/src/orchestrator/mod.rs
# system_prompt.rs:1, prompt.rs:1, agent.rs:2, loop_.rs:3, orchestrator/mod.rs:1 = 8 production sites

# 7. Tool files Asia::Shanghai counts (Commit 5):
$ grep -nc 'with_timezone(&chrono_tz::Asia::Shanghai)' crates/zeroclaw-runtime/src/tools/{schedule,delegate,cron_runs,cron_add}.rs
schedule.rs:7, delegate.rs:3, cron_runs.rs:2, cron_add.rs:1

# 8. RunView struct field types preserved (Plan §0.5 #4):
$ grep -A 9 'struct RunView' crates/zeroclaw-runtime/src/tools/cron_runs.rs | grep 'DateTime<chrono::Utc>'
    started_at: chrono::DateTime<chrono::Utc>,
    finished_at: chrono::DateTime<chrono::Utc>,

# 9. src/util.rs untouched:
$ diff <(cat src/util.rs) <(echo 'pub use zeroclaw_runtime::util::*;') && echo "OK"
OK

# 10. 0 new unwrap/expect/allow(dead_code):
$ git diff master..HEAD | grep '^+' | grep -E '\.unwrap\(\)|\.expect\(\(|allow\(dead_code\)'
(0 hits)
```

## Necessary deviation: `crates/zeroclaw-channels/Cargo.toml` +1 line

Plan rev3.1 §8 DoD initially asserted "`Cargo.toml` 未变". This held for 5 of 6 commits, but Commit 6's site `orchestrator/mod.rs:771` adding `chrono_tz::Asia::Shanghai` required `chrono-tz` to be linked in `zeroclaw-channels` (which previously had no dep path to it). Subagent caught this at compile time and added:

```toml
chrono-tz = "0.10"   # NEW LINE in crates/zeroclaw-channels/Cargo.toml
```

Matches the unconditional declaration already present in `crates/zeroclaw-runtime/Cargo.toml`. Documented in Commit 6 message and Plan §8 was updated to reflect honest state.

## 6 commits (atomic, bisect-friendly)

| # | SHA | Title |
|---|---|---|
| 1 | `e2c5c2ee` | feat(cli): introduce time display helper for Asia/Shanghai |
| 2 | `282010e0` | feat(cli): render cron list and auth expiry in Beijing time |
| 3 | `a9344d66` | feat(log): emit tracing logs in Beijing time |
| 4 | `eeec384a` | fix(runtime): hardcode Asia/Shanghai in heartbeat decision prompt and MCSS report |
| 5 | `ed4e292e` | fix(runtime): render schedule/cron/delegate tool outputs in Beijing time |
| 6 | `f85ebf32` | fix(runtime): hardcode Asia/Shanghai in LLM prompt time injection |

Diff: ~+85 / -45 lines across ~13 files (incl. Cargo.toml/Cargo.lock).

## Known Carve-outs (deliberately NOT in this PR — see Plan §5)

These are real issues but unrelated to user-visible time display. Each gets its own follow-up PR if/when prioritized:

- **runtime_trace.rs Local/Utc inconsistency** (line 216 vs 371/399 — but 371/399 are in `#[test] fn`, 216 writes JSONL with no user/LLM read path)
- **memory/hygiene.rs same-file Local + Utc mix** (lines 108-123 are state-file r/w + cutoff arithmetic, 0 user-visible output)
- **skillforge/integrate.rs:84 literal `Z` suffix** (writes TOML manifest `[skill.metadata].forge_timestamp` field — metadata, not display)
- **`cost/tracker.rs` `.naive_utc()` daily bucketing** (migration day would double-count or skip; needs data migration script)
- **Filename timezone inconsistency** (`backup_tool.rs`, `screenshot.rs`, `robot-kit/look.rs`, `robot-kit/listen.rs` use UTC; `migration.rs`/`main.rs:1317` use Local — affects external scripts)
- **Gateway REST/SSE 130 wire-format sites** (breaking-change risk if Z→+08:00; should be done at frontend dashboard render layer instead)
- **memory/sqlite.rs/audit.rs/response_cache.rs Local::now() writes** (storage layer — Plan §0 explicitly preserves UTC storage)

## Rollback

Single-branch linear PR. To revert after merge:

```bash
git revert <merge-sha>
```

Each of the 6 commits is independently revertable (no cross-commit dependencies beyond Commit 1's helper, which 2-6 build on).

## Plan & Process

Plan went through 3 rounds of Momus review:
- **rev1** REJECT: 3 BLOCKER + 2 MAJOR + 5 MINOR (file conflicts, line refs, struct types)
- **rev2** REJECT (anti-scope-creep): cut 3 consistency BUGs that weren't in user's actual ask
- **rev3** ACCEPT WITH MINOR PATCHES: off-by-one (12→11) + grep regex coverage
- **rev3.1** final: 6 commits, 4 user-visible surfaces, 7/10 DoD checked at orchestrator hand-off, remaining 3 require user push + manual smoke + reviewer approval

Plan, notepads, and learnings preserved in `.sisyphus/`.
