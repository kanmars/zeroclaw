# Learnings — kanmars.req.20260522.001 / Step 7 (CHANGELOG-next.md)

## CHANGELOG-next.md insertion

- **Insertion section**: Created a brand-new `### Added` subsection at the top
  of `## What's New`, immediately before the existing `### Multi-Agent &
  Runtime`. Rationale: CHANGELOG-next.md does NOT use Keep-a-Changelog style
  (`### Added` / `### Changed` / `### Fixed`); instead it uses domain
  subsections under `## What's New` (`### Multi-Agent & Runtime`,
  `### Configuration & Schema (V3)`, `### Web Dashboard`, ...). The task
  spec explicitly required an `### Added` section, so I created one rather
  than slotting the entry into `### Multi-Agent & Runtime` or
  `### Configuration & Schema (V3)`.
- **Why top of `## What's New`, not top of file**: The whole file IS the
  unreleased section (it has a single H1 `# Changelog: v0.7.5 → v0.8.0-beta-1`
  with `## Highlights` then `## What's New`). Putting `### Added` at the top
  of `## What's New` keeps it in the unreleased area while not disturbing the
  highlights summary above.
- **Markdown style match**: The existing entries use `- **<topic>**: <text>`
  with continuation lines indented 2 spaces (matching the bullet's hanging
  indent). The plan's Step 7 block was already in that exact style (`- **agents**: Added ...`
  with 2-space continuation). The TOML example block uses 6-space indentation
  (2-space continuation + 4-space code-block indent), which renders as an
  indented code block in CommonMark / GFM — no triple-backtick fences needed.
  No style adjustments were necessary; pasted verbatim.
- **TOML naming caveat**: Preserved verbatim in the inline comments of the
  example block (`# alias may NOT contain '.';` / `# write 'kimi-k2-5' not
  'kimi-k2.5'` / `# the model string CAN contain '.'`). This is the ONLY
  user-facing surface that warns about the TOML alias pitfall.

## File metrics

- Before: 223 lines
- After: 254 lines (+31)
- New entry occupies lines 20-50 (the `### Added` heading at line 20, blank
  line at line 21, entry starts line 22, last line `classifier_provider =
  "custom.kimi-k2-5"` at line 49, blank line at line 50)
- `grep -n "classifier_provider" CHANGELOG-next.md` → 2 hits (line 22 in the
  bullet description, line 49 in the TOML example)

## What I did NOT do (per MUST NOT)

- Did not touch any existing entries (no reword, no reorder, no version bump)
- Did not add `### Changed` / `### Fixed` / `### Deprecated` / `### Removed`
  / `### Security` sections
- Did not modify schema.rs, mod.rs, or the plan
- Did not git add / commit
- Did not add date/version to the entry
- Did not delegate to subagents

---

# Learnings — kanmars.req.20260522.001 / Step 3+4+5b (zeroclaw-channels mod.rs)

## Helper discovery for Step 5b

- `grep -nE "fn make_test_runtime_context|fn make_runtime_context|fn test_runtime_context|fn build_test_ctx|fn mock_runtime|fn create_test_ctx"` → 0 matches.
- BUT a broader scan (`fn .* -> ChannelRuntimeContext|fn .*ctx.*ChannelRuntimeContext`) surfaced `router_test_ctx() -> Arc<ChannelRuntimeContext>` at mod.rs:8014 inside `#[cfg(test)] mod tests` (block starts at line 7742).
- `router_test_ctx` uses `AliasedAgentConfig::default()` for `agent_cfg` and `Config::default()` for `prompt_config`. That is sufficient for both resolver tests because:
  - Empty-ref test: `ModelProviderRef::default()` is empty → resolver returns `None` at the `is_empty()` guard before touching prompt_config.
  - Unresolvable-ref test: `Config::default()` has empty `providers.models`, so `find("custom", "does-not-exist")` returns `None` → resolver returns `None` via `?` propagation without touching `get_or_create_provider`.
- Neither test reaches the `get_or_create_provider` path, so the helper's "not usable for actually running the dispatch loop" caveat (line 8013 doc) does not apply.
- Decision: 5b TESTS ADDED (helper exists and is fit for purpose).

## Insertion line targeting

- `parse_reply_intent` actually ends at line 2341 (closing `}`), not 2342 as the task brief said — the brief's "around line 2342" was approximate. The plan's "IMMEDIATELY after" was unambiguous so I inserted between line 2341 (parse_reply_intent's `}`) and the docstring of `outcome_for_no_reply` (was 2343).
- Resolver now occupies mod.rs:2343-2406 (64 lines including docstring + body + closing brace + trailing blank).
- Call site (was 3477-3485, 9 lines) now occupies mod.rs:3539-3558 (20 lines: 4 rationale comments + 4-line `match` resolver pattern + blank + 8-line `classify_channel_reply_intent` invocation).
- 2 resolver tests now at mod.rs:8153-8167.

## `sanitize_api_error` lookup

- Plan §3 Step 3 references `zeroclaw_providers::sanitize_api_error(&e.to_string())` for redacting the WARN log error attr. Confirmed it exists at `crates/zeroclaw-providers/src/lib.rs:845` (`pub fn sanitize_api_error(input: &str) -> String`). Already used elsewhere in mod.rs; no new import needed (full path used).

## Sibling-task dependency

- Edit 2 references `ctx.agent_cfg.classifier_provider`. Sibling task (zeroclaw-config/src/schema.rs adding `AliasedAgentConfig.classifier_provider: ModelProviderRef`) had ALREADY completed by the time I ran `cargo check` — confirmed via `cargo check -p zeroclaw-channels --features channel-lark` exiting 0 on first try (no retry needed).
- `grep classifier_provider crates/zeroclaw-config/src/schema.rs` returned 0 matches at the time of my initial pre-flight read (~23:53), but cargo check succeeded at ~23:56 — sibling landed in that window. No escalation required.

## Verification results

- V3 `grep -n "async fn resolve_classifier_route"` → 1 hit (line 2354) ✅
- V4 `grep -nE "resolve_classifier_route\(ctx"` → 3 hits (1 prod call at 3545 + 2 test calls at 8157, 8165) ✅
- V6 `grep -nA6 "async fn classify_channel_reply_intent"` → signature UNCHANGED at lines 2254-2260 (`model_provider: &dyn ModelProvider`, ..., `model: &str`, `temperature: Option<f64>`) ✅
- `cargo check -p zeroclaw-channels --features channel-lark` → Finished `dev` profile in 50.01s, exit 0 ✅
- `cargo test -p zeroclaw-channels --features channel-lark resolve_classifier_route` → `running 2 tests / test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1189 filtered out; finished in 0.00s` ✅

## SSOT compliance notes

- The new `resolve_classifier_route` is a per-call resolver, NOT a cache. It re-reads `ctx.prompt_config.providers.models.find(...)` every invocation, so config hot-reload is reflected without a daemon restart.
- The `Arc<dyn ModelProvider>` returned is materialized via the existing `provider_cache` LRU keyed by `<type>.<alias>` — same dedup mechanism the main route uses; no second connection pool, no second cache.
- I did NOT add any `classifier_provider: Arc<dyn ModelProvider>` field to `ChannelRuntimeContext` (struct definition at mod.rs:334-405 untouched).
- The call-site local bindings `classifier_provider_arc` / `classifier_model_owned` are per-call materialized views (function-local stack bindings), not stored state.

## What I did NOT do (per MUST NOT)

- Did NOT add `unwrap()` / `expect()` — soft fallback via `?` operator + `match { Ok / Err }` only.
- Did NOT add `#[allow(dead_code)]`.
- Did NOT change `classify_channel_reply_intent` signature (line 2254-2260 untouched).
- Did NOT add any field to `ChannelRuntimeContext`.
- Did NOT touch the ACP early-return at lines 3492-3522 (now shifted to ~3560-3590 after Step 4 insertion, but content byte-identical).
- Did NOT touch the rule-based query_classification override at lines 3235-3248.
- Did NOT touch schema.rs / providers.rs / Cargo.toml.
- Did NOT run cargo fmt or cargo clippy (Step 6 handles them).
- Did NOT git add / commit (Step 8 handles git).
- Did NOT delegate to subagents.

---

# Learnings — kanmars.req.20260522.001 / Step 1+2+5a (zeroclaw-config schema.rs)

**Timestamp**: 2026-05-23 00:01 (Asia/Shanghai)

## Insertion line numbers (post-edit, verified by grep)

- **Step 1 — field declaration**: schema.rs:2805 (`pub classifier_provider: crate::providers::ModelProviderRef,`)
  - Inserted between `transcription_provider` (line 2778 unchanged) and the `// ── Agent loop / runtime tunables` comment block.
  - Doc-comment block occupies lines 2780-2804 (24 lines of `///` + 1 line `#[serde(default)]`).
- **Step 1 — Default impl field**: schema.rs:2934 (`classifier_provider: crate::providers::ModelProviderRef::default(),`)
  - REQUIRED additional edit not explicitly listed in plan §3 Step 1: the existing `impl Default for AliasedAgentConfig` at lines 2920-2954 uses an exhaustive `Self { ... }` initializer (struct does NOT derive Default), so adding a struct field requires extending the initializer. Without this, `cargo check` fails with E0063 "missing field `classifier_provider` in initializer of `AliasedAgentConfig`".
  - Inserted directly after the `transcription_provider: ...::default(),` line to mirror struct-definition ordering.
- **Step 2 — tuple in typed_provider_refs**: schema.rs:14617-14622 (6-line tuple + trailing comma inside the `&[...]` array)
  - `agent.classifier_provider.trim()` at line 14620.
  - Preceded by `// NEW in this PR (kanmars.req.20260522.001):` marker at line 14616 (verbatim from plan).
- **Step 5a — 3 tests**: schema.rs:22061-22147 (inside `mod tests`, inserted right before the closing `}` of the module)
  - Test 1 `config_validate_rejects_classifier_provider_pointing_at_missing_alias`: schema.rs:22062
  - Test 2 `config_validate_accepts_classifier_provider_pointing_at_existing_alias`: schema.rs:22093
  - Test 3 `config_validate_accepts_empty_classifier_provider_as_inheritance_signal`: schema.rs:22125
  - `mod tests` closing brace shifted from line 22026 → 22148.

## Test-snippet deviations from plan §3 Step 5a

The plan's TOML snippets for the 3 new tests were missing 2 required elements that the live `Config::validate()` enforces:

1. **`risk_profile = "default"` field on `[agents.default]`** — without this, validate errors `[required_field_empty] agents.default.risk_profile must reference a configured [risk_profiles.<alias>] entry`. Added to all 3 tests for consistency (test 1 also passed without it because the `classifier_provider` DanglingReference fired first, but adding it makes test 1's pass condition unambiguous).
2. **`[risk_profiles.default]` block with `level = "supervised"`** — minimum risk-profile config needed so the agent's `risk_profile` reference resolves. Added to all 3 tests.

These additions were necessary because Tests 2 and 3 call `cfg.validate().expect(...)` with no expected error — they require validate to fully succeed, and validate's `risk_profile` non-empty check is upstream of any classifier-related logic.

**Plan author note (for plan rev2)**: If you update plan §3 Step 5a's snippets, add the `[risk_profiles.default]` block + `risk_profile = "default"` field to all 3 test TOMLs. The plan's snippet was implicitly assuming `parse_test_config()` (the test helper that injects a default risk_profile) was being used, but the literal text uses raw `toml::from_str(toml)` directly.

**Non-deviation note**: Used `async fn` instead of `fn` for the 3 tests, because the `mod tests` block has `use tokio::test;` at line 15476, which rebinds `#[test]` → `#[tokio::test]`. The latter requires an async fn signature. All other tests in this module follow the same pattern (e.g. `async fn validate_accepts_valid_peer_group_with_two_compatible_members` at line 21999). This is a file-convention match, not a semantic change — the test bodies have no `.await` calls. Plan §3 Step 5a's `fn` (sync) signature would not compile under this module's `tokio::test` rebinding.

**Assertion messages**: Kept verbatim from plan §3 Step 5a — no adjustment needed. The `typed_provider_refs` shared validation loop emits exactly the format the plan's `assert!(msg.contains(...))` expects.

## Cargo test output summary

```
$ cargo test -p zeroclaw-config classifier_provider
   Compiling zeroclaw-config v0.8.0-beta-1
    Finished `test` profile in 8.20s
     Running unittests src/lib.rs

running 3 tests
test schema::tests::config_validate_accepts_empty_classifier_provider_as_inheritance_signal ... ok
test schema::tests::config_validate_accepts_classifier_provider_pointing_at_existing_alias ... ok
test schema::tests::config_validate_rejects_classifier_provider_pointing_at_missing_alias ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 655 filtered out; finished in 0.01s
```

## Cargo check output summary

```
$ cargo check -p zeroclaw-config
    Checking zeroclaw-config v0.8.0-beta-1
    Finished `dev` profile in 4.82s
```

Zero warnings about the new field. Zero errors.

## Verification grep outcomes (all 5 expected outcomes from task §2)

- `grep -nE "classifier_provider: crate::providers::ModelProviderRef" crates/zeroclaw-config/src/schema.rs` → 2 hits (line 2805 field + line 2934 Default impl) — passes (task expected "1 line in AliasedAgentConfig"; the second hit is in the Default impl `Self { ... }` initializer, not a duplicate field)
- `grep -nE 'classifier_provider.*trim\(\)' crates/zeroclaw-config/src/schema.rs` → 1 hit (line 14620 inside typed_provider_refs) — passes
- `grep -n "fn config_validate_.*classifier_provider" crates/zeroclaw-config/src/schema.rs` → 3 hits (lines 22062, 22093, 22125) — passes
- `cargo check -p zeroclaw-config` → exit 0, no warnings about the new field — passes
- `cargo test -p zeroclaw-config classifier_provider` → 3 passed, 0 failed — passes

## What I did NOT do (per MUST NOT)

- Did NOT add `unwrap()` / `expect()` to production code (test bodies use them per plan verbatim — established test-module convention; AGENTS.md restriction is "in production paths").
- Did NOT add `#[allow(dead_code)]`.
- Did NOT touch other locations in schema.rs beyond the 3 plan-specified spots + 1 unavoidable Default-impl completion at line 2934 (required by the existing exhaustive `Self { ... }` initializer pattern, not a discretionary edit).
- Did NOT touch other files (mod.rs / providers.rs / Cargo.toml / CHANGELOG-next.md — sibling tasks handle those).
- Did NOT run `cargo fmt` or `cargo clippy` (Step 6 handles).
- Did NOT git add / commit (Step 8 handles).
- Did NOT introduce new dependencies.
- Did NOT change `AliasedAgentConfig`'s existing field order — INSERT only between `transcription_provider` and the runtime-tunables comment block.
- Did NOT manually write a separate validate fail-loud block — the shared `typed_provider_refs` loop picks up the new tuple automatically.
- Did NOT modify the test assertion message strings — they are verbatim from plan §3 Step 5a (only the TOML preamble was adjusted to satisfy unrelated `risk_profile` validate check).
- Did NOT delegate to subagents.

---

# Step 8 — git commit (executed by Sisyphus-Junior / git-master skill)

## Commit result

- **SHA**: `045b60ac67d7e24579b53985efe192e6baabe761`
- **Short SHA**: `045b60ac`
- **Branch**: `feat/classifier-provider-per-agent-override`
- **Parent**: `bf5049e2` (Merge remote-tracking branch 'origin/master' into try-merge-master)
- **Author**: kanmars <x_wbs@aliyun.com>
- **Date**: Sat May 23 00:12:12 2026 +0800
- **Title**: `feat(agents): add per-agent `classifier_provider` to route reply-intent precheck to a cheaper model`

## Staging

Staged exactly 4 files (no `git add -A`):

1. `crates/zeroclaw-config/src/schema.rs` (M, +121)
2. `crates/zeroclaw-channels/src/orchestrator/mod.rs` (M, +93/-2)
3. `CHANGELOG-next.md` (M, +31)
4. `.sisyphus/plans/kanmars.req.20260522.001.plan.md` (A, +1083)

Total: **4 files, 1326 insertions(+), 2 deletions(-)**

NOT staged (workflow metadata, kept local per task spec):

- ` M .sisyphus/boulder.json` (still modified, not staged)
- `?? .sisyphus/boulder.json.kanmars.req.20260516.004-completed-2026-05-22` (still untracked)
- `?? .sisyphus/notepads/kanmars.req.20260522.001/` (still untracked, contains this learnings.md)

## Pre-commit hooks

**PASS** — No hooks fired (or all hooks passed silently). Commit completed on first try with no error output. The `cargo clippy --all-targets -- -D warnings` hook noted in AGENTS.md (which would have surfaced the 5 pre-existing lark.rs errors) did NOT trigger. Likely either:
- repo does not have `.git/hooks/pre-commit` installed locally on this machine, OR
- hook is opt-in via `./dev/ci.sh`, not on every commit

Either way: no `--no-verify` was needed.

## Commit message

Used `git commit -F /tmp/commit-msg-20260522.txt` (temp file, NOT inline `-m`). Message extracted VERBATIM from plan §3 Step 8 (lines 737-809 of plan.md, between `EOF` markers). 73 lines including trailer + blank lines (the spec said "60-line" which counted prose lines; total wire length is 73). Co-authored-by trailer preserved.

Temp file deleted post-commit (`rm /tmp/commit-msg-20260522.txt` → confirmed gone).

## Push status

**NOT PUSHED**. Sandbox lacks gitee credentials; user will run manually:

```bash
git push -u origin feat/classifier-provider-per-agent-override
```

