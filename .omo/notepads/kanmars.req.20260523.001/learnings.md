# Learnings — kanmars.req.20260523.001

## Task B: CHANGELOG-next.md updates (2026-05-23)

### Edits performed

- **File**: `CHANGELOG-next.md`
- **Before**: 254 lines
- **After**: 289 lines (+35)

### Section layout (after edit)

| Line | Content |
|---|---|
| 20 | `### Added` (existing heading from 0522.001 PR) |
| 22-49 | `classifier_provider` entry (0522.001) — PRESERVED unchanged |
| 51-63 | NEW: `providers/models` / `max_context_window` entry (appended to existing Added) |
| 65 | `### Changed` (NEW section) |
| 67-84 | NEW: `agents` / `max_context_tokens` type-change entry |
| 86 | `### Multi-Agent & Runtime` (existing, unchanged) |

### Verification

- `grep -c max_context_window CHANGELOG-next.md` → 6
- `grep -c max_context_tokens CHANGELOG-next.md` → 3
- `grep -c DEFAULT_MAX_CONTEXT_TOKENS CHANGELOG-next.md` → 1
- `grep -c "^### Added$" CHANGELOG-next.md` → 1 (no duplicate)
- `grep -c "^### Changed$" CHANGELOG-next.md` → 1 (new section)
- `grep -c classifier_provider CHANGELOG-next.md` → 2 (0522.001 entry intact)

### Key choices

1. **Indentation match**: classifier_provider uses 2-space continuation for bullet bodies and 6-space indent for inline TOML blocks. New entries mirror this exactly.
2. **Insertion point**: Placed both new entries before `### Multi-Agent & Runtime` (line 51 in original file), keeping Keep-a-Changelog section order: Added → Changed.
3. **Verbatim copy**: Markdown copied directly from plan §3 Step 6 (lines 553-590) including em dashes, which are already part of the existing CHANGELOG style (e.g., classifier_provider entry uses similar punctuation).
4. **No file creation**: Only edited the single target file; no schema.rs / mod.rs / plan.md touched.

---

## Task A: zeroclaw-config schema (2026-05-23)

### Session
- Branch: `feat/model-context-window-inheritance`
- Baseline: `045b60ac` (= `kanmars_main` HEAD, post 0522.001 classifier_provider merge)

### Confirmed line shifts vs plan baseline (`bf5049e2` → `045b60ac`)

| Symbol | Plan line | Actual line | Drift |
|---|---|---|---|
| `pub struct ModelProviderConfig` | 635 | 635 | 0 |
| `pub max_tokens: Option<u32>` (output cap) | ~666 | 666 | 0 |
| `pub max_context_tokens: usize` (pre-edit) | 2825 | 2825 | 0 (the warned +25 drift did NOT happen — 0522.001's classifier_provider field was inserted earlier in the struct but the relative position of max_context_tokens stayed) |
| `impl Default for AliasedAgentConfig` | 2920 | 2920 | 0 |
| `impl AliasedAgentConfig { is_dispatchable }` | — | 2960 | — (existing block extended) |
| `fn model_provider_for_agent` | 3074 | 3102 | **+28** (drift confirmed here) |
| `fn default_agent_max_context_tokens` | 4407 | 4407 | 0 |
| `#[cfg(test)] mod tests` | — | 15502 | — |
| `use tokio::test;` | — | 15510 | — |

**Insight**: Drift varies by location. Always grep — don't assume uniform drift across the file.

### Final landing positions (post-PR Task A)

| Edit | Symbol | Final line |
|---|---|---|
| 1a | `pub max_context_window: Option<usize>` | 691 |
| 1b | `pub max_context_tokens: Option<usize>` | 2860 |
| 1c | `pub const DEFAULT_MAX_CONTEXT_TOKENS` | 637 |
| 1c | `fn default_agent_max_context_tokens` | DELETED |
| 1d | `max_context_tokens: None` (Default impl) | 2973 |
| 1e | `AliasedAgentConfig::resolved_max_context_tokens` | 3011 |
| 1e | `Config::resolved_max_context_tokens_for_agent` | 3166 |
| 1f | 3 unit tests | 22213, 22225, 22236 |
| Step 4 | 3 integration tests | 22248, 22269, 22292 |

**V2 note (max_context_tokens Option<usize>)**: 2 grep hits — line 2860 (this PR's AliasedAgentConfig field) and line 8974 (pre-existing `RuntimeProfileConfigOverride.max_context_tokens` dead-config field, unrelated).

**V5 note (max_context_tokens: None)**: 2 grep hits — line 2973 (this PR's Default impl) and 9005 (pre-existing in `RuntimeProfileConfigOverride::default`).

### Lessons

1. **Plan line numbers are advisory.** Always grep first. Drift is non-uniform: 0 lines around line 2825, +28 lines around line 3074.
2. **COMMENT/DOCSTRING hook fires on every `///`.** Each block must be justified per priority 3 (necessary public API docs). All docstrings here are plan-mandated verbatim.
3. **Test count assumption was wrong.** Task brief said "88 + 6 = 94"; actual schema test module has 658 pre-existing → post-PR total 664 schema tests + 88 migration.rs tests + 1 ignored doctest = 752 passed, 0 failed.
4. **`use tokio::test;` rebinding requires `#[test] async fn`.** All 6 new tests follow this convention; bodies have no `.await` calls.
5. **TOML test preambles must include `risk_profile = "default"` + `[risk_profiles.default] level = "supervised"`** (inherited 0522.001 lesson). Without these, `Config::validate()` rejects.
6. **`impl AliasedAgentConfig` block already existed** at line 2960 with `is_dispatchable`. Extended rather than created new.
7. **`#[derive(...,Default)]` on `ModelProviderConfig`** enabled clean `..Default::default()` syntax in tests with `Some(N)` overrides.

### Verification snapshot

| V# | Expected | Actual |
|---|---|---|
| V1 | 1 | 1 (line 691) ✅ |
| V2 | ≥1 (in AliasedAgentConfig) | 2 (2860 new + 8974 pre-existing) ✅ |
| V3 | 1 | 1 (line 637) ✅ |
| V4 | 0 | 0 ✅ |
| V5 | ≥1 | 2 (2973 + 9005) ✅ |
| V6 | ≥2 | 5 (2 helpers + 3 unit tests) ✅ |
| `cargo check -p zeroclaw-config` | exit 0 | exit 0 ✅ |
| `cargo test -p zeroclaw-config resolved_max_context_tokens` | 6 passed | 6 passed ✅ |
| `cargo test -p zeroclaw-config` (full) | green | 664 + 88 + 0/1 ignored = 752 passed, 0 failed ✅ |

### Deviations from plan snippet

None. All 6 edits + 6 tests applied verbatim per plan §3 Step 1a–1f and Step 4.

### Handoff to Task C

Public API now available:
- `zeroclaw_config::schema::AliasedAgentConfig::resolved_max_context_tokens(model_cfg: Option<&ModelProviderConfig>) -> usize`
- `zeroclaw_config::schema::Config::resolved_max_context_tokens_for_agent(agent_alias: &str) -> usize`
- `zeroclaw_config::schema::DEFAULT_MAX_CONTEXT_TOKENS: usize` (= 32_000)

Task C call sites:
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:7467` — has `&Config` + agent_alias → use `config.resolved_max_context_tokens_for_agent(&agent_alias)`.
- `crates/zeroclaw-runtime/src/agent/loop_.rs:3520, 3904, 3972, 4042` — scope analysis pending; helper preferred, fallback `agent.max_context_tokens.unwrap_or(zeroclaw_config::schema::DEFAULT_MAX_CONTEXT_TOKENS)`.

---

## Task C: zeroclaw-channels + zeroclaw-runtime call-site migration (2026-05-23)

### Confirmed line shifts vs plan baseline (`bf5049e2` → `045b60ac`)

| Call site (logical) | Plan line | Actual line | Drift |
|---|---|---|---|
| orchestrator/mod.rs `context_token_budget:` | 7467 | 7467 | 0 (no 0522.001 drift around this site, despite warned +60) |
| loop_.rs run() tool_loop CLI mode  | 3520 | 3520 | 0 |
| loop_.rs run() tool_loop interactive CLI | 3904 | 3904 | 0 |
| loop_.rs run() context overflow recovery (`let mut compressor`) | 3972 | 3972 | 0 |
| loop_.rs run() compress before hard trim (`let compressor`) | 4042 | 4042 | 0 |

**Insight**: Drift in channels/orchestrator + runtime/loop_.rs was 0 across all 5 sites — opposite of the warned ~+60 drift from 0522.001. The `resolve_classifier_route` helper that 0522.001 added lives at mod.rs:2354 and only added lines around 2343-2418 + rewrote the original call site, which is far above all our target sites. Task A's pattern (drift varies by location, always grep) held.

### Scope analysis (each site)

| Site | Enclosing function | `&Config` in scope? | `agent_alias` in scope? | Form chosen |
|---|---|---|---|---|
| orchestrator/mod.rs:7467 | `start_channels(config: Config, ...)` (line 6700); `for agent_alias in &enabled_agents` (line 6813) makes `agent_alias` a `&String` | YES (`config` owned, line 6701) | YES (`&String` from for-loop) | `config.resolved_max_context_tokens_for_agent(agent_alias)` — Config helper preferred |
| loop_.rs:3520 | `pub async fn run(config: Config, agent_alias: &str, ...)` (line 2799) → async move closure (line 2851); `agent_alias` rebound to `&str` at line 2852 | YES (owned, captured by move) | YES (`&str`) | `agent.resolved_max_context_tokens(config.model_provider_for_agent(agent_alias))` — AliasedAgentConfig helper preferred |
| loop_.rs:3904 | same as 3520 | YES | YES | same preferred form |
| loop_.rs:3972 | same as 3520 (inside `is_context_window_exceeded` recovery branch) | YES | YES | same preferred form |
| loop_.rs:4042 | same as 3520 (compress-before-trim block) | YES | YES | same preferred form |

**No fallback (`unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS)`) was needed — all 5 sites had both `&Config` (or owned `Config`) and `agent_alias` reachable.** Zero TODO comments added.

### Final landing positions (post-PR Task C)

| File | Line | Form |
|---|---|---|
| orchestrator/mod.rs | 7467 | `context_token_budget: config.resolved_max_context_tokens_for_agent(agent_alias),` |
| loop_.rs | 3520 | `agent.resolved_max_context_tokens(config.model_provider_for_agent(agent_alias)),` |
| loop_.rs | 3904 | same form |
| loop_.rs | 3972 | same form |
| loop_.rs | 4042 | same form |

### Verification snapshot

| Check | Expected | Actual |
|---|---|---|
| `grep -nE "agent\.max_context_tokens" channels/runtime` | 0 | 0 ✅ |
| `grep -nE "resolved_max_context_tokens" orchestrator/mod.rs` | ≥1 | 1 (line 7467) ✅ |
| `grep -nE "resolved_max_context_tokens" loop_.rs` | ≥4 | 4 (3520, 3904, 3972, 4042) ✅ |
| `grep -nE "DEFAULT_MAX_CONTEXT_TOKENS" loop_.rs` | 0 (no fallback used) | 0 ✅ |
| `cargo check -p zeroclaw-channels --features channel-lark` | exit 0 | exit 0 (9.47s incremental after runtime check) ✅ |
| `cargo check -p zeroclaw-runtime` | exit 0 | exit 0 (35.64s clean) ✅ |
| `cargo test -p zeroclaw-channels --features channel-lark --lib` | ≥1191 passed, 0 failed | 1191 passed, 0 failed, 0 ignored (7.80s) ✅ |
| `cargo test -p zeroclaw-runtime --lib` | green | 1830 passed, 0 failed, 1 ignored (24.19s) ✅ |

### Lessons

1. **Drift warning was over-cautious.** Plan brief warned ~+60 line drift due to 0522.001 PR adding `resolve_classifier_route` (mod.rs:2354). Reality: that PR's insertions were *above* line 2418, so all 5 target sites (orchestrator:7467, loop_.rs:3520–4042) had **zero drift**. Mirrors Task A's "non-uniform drift" lesson — drift is local, not file-wide. Always grep, never assume.
2. **Cross-crate incremental cargo check.** `cargo check -p zeroclaw-channels --features channel-lark` re-checks the `zeroclaw-runtime` dep, so it surfaces loop_.rs errors before you've even run the runtime check. Useful for catching downstream type-mismatch fallout (here: `Option<usize>` vs `usize` at `ContextCompressor::new`).
3. **`agent_alias` type is `&str` inside `run()` after rebind.** `pub async fn run(config: Config, agent_alias: &str, ...)` outer; `let agent_alias: &str = __zc_alias.as_str();` (line 2852) inside the async move. Passing `agent_alias` directly (without extra `&`) to `model_provider_for_agent(&str)` is the cleanest form — no double-reference deref noise.
4. **`agent_alias` type is `&String` in `start_channels()` for-loop.** `for agent_alias in &enabled_agents` (where `enabled_agents: Vec<String>`) → `&String`. Passing `agent_alias` directly to `resolved_max_context_tokens_for_agent(&str)` works via deref coercion (`&String` → `&str`). No `.as_str()` needed.
5. **`config: Config` (owned) still allows `.method()` on borrowed receivers.** Inside `run()`, `config` is owned (moved into async closure). Calling `config.model_provider_for_agent(agent_alias)` where the method takes `&self` auto-borrows — no need to write `(&config).method()` or restructure.
6. **No SSOT violation introduced.** All 5 sites resolve `max_context_tokens` on-demand from the canonical `Config` + `AliasedAgentConfig` + `ModelProviderConfig` triple. No new state cached anywhere; matches AGENTS.md "Patterns that are NOT duplicate state" (helpers materialize views on-demand from canonical state).
7. **No formatting / dependency changes.** Single-concern PR: pure call-site rewrite. No `cargo fmt`, no `cargo clippy`, no `git add`, no new deps. Atlas handles those.

### Deviations from plan snippet

None. All 5 edits used the preferred helper form; no fallback (`unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS)`) needed; no TODO comments added.

Minor style choice: pass `agent_alias` directly (no leading `&`) to the helpers rather than the brief's literal `&agent_alias`, since `agent_alias` is already `&str` in loop_.rs and `&String` in orchestrator — both auto-deref to `&str` for the helper signature. This avoids `&&str` / `&&String` noise. Functionally identical.

### Handoff to atlas (Step 5–7)

All implementation work complete. Atlas should run:
1. `cargo fmt --all` (no manual fmt was done)
2. `cargo clippy --all-targets -- -D warnings` (not run; needs central verification)
3. `dev/ci.sh dry-check` (SSOT enforcement)
4. `dev/ci.sh all` (full PR validation)
5. `git add` + commit per git-master skill conventions
6. PR open per github-pr skill

Branch: `feat/model-context-window-inheritance` (still active).
Total commits in this feature branch should be: Task A (schema.rs + tests) + Task B (CHANGELOG-next.md) + Task C (5 call-site edits) = atomic squash candidates.

---

## Task D: clippy field-reassign-with-default fix (2026-05-23)

- **File**: `crates/zeroclaw-config/src/schema.rs` lines 22212-22224 (test `resolved_max_context_tokens_uses_explicit_agent_value`)
- **Fix**: replaced `let mut agent = AliasedAgentConfig::default(); agent.max_context_tokens = Some(50_000);` with struct literal `AliasedAgentConfig { max_context_tokens: Some(50_000), ..AliasedAgentConfig::default() }` (explicit type for readability since `ModelProviderConfig` literal below uses `..Default::default()`).
- **Verification**: `cargo clippy -p zeroclaw-config --all-targets -- -D warnings` → exit 0 (16.38s); `cargo test -p zeroclaw-config resolved_max_context_tokens_uses_explicit_agent_value` → 1 passed, 663 filtered. Other 2 unit tests + 3 integration tests untouched. `#[test] async fn` signature preserved (tokio rebinding intact).

---

## Step 7 — Atomic commit + push (executed 2026-05-23 01:54 +0800)

### Commit
- SHA: `c4540a489b07ef41954aa6b1aaecf2c13b6c29d1` (short: `c4540a48`)
- Title: `feat(config): inherit agent.max_context_tokens from model.max_context_window`
- Files: 5 (schema.rs, mod.rs, loop_.rs, CHANGELOG-next.md, plan.md)
- Diff: +1152 / -13 (plan.md is 938 new lines + 35 CHANGELOG + ~180 schema/wiring)
- Pre-commit hooks: PASS (clean exit, no hook intervention)
- Commit message: 80-line verbatim from plan §3 Step 7

### Push
- Command: `git push -u origin feat/model-context-window-inheritance`
- Exit: 0
- Remote: `https://github.com/kanmars/zeroclaw.git`
- Branch tracking: `origin/feat/model-context-window-inheritance` set up
- GitHub PR URL: https://github.com/kanmars/zeroclaw/pull/new/feat/model-context-window-inheritance

### Post-push working tree
- 4 metadata files remaining (not staged, intentional):
  - `M .sisyphus/boulder.json` (active state)
  - `?? .sisyphus/boulder.json.kanmars.req.20260516.004-completed-2026-05-22` (0522 archive)
  - `?? .sisyphus/notepads/kanmars.req.20260522.001/` (0522 learnings)
  - `?? .sisyphus/notepads/kanmars.req.20260523.001/` (this PR's learnings — including this entry)

### Process notes
- Used temp file `/tmp/commit-msg-20260523.txt` (deleted post-commit) — heredoc approach worked cleanly
- No `--no-verify`, no `--force`, no `--amend` invoked
- No deviations from plan; commit message identical to plan §3 Step 7 lines 603-686
