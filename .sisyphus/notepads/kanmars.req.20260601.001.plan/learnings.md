# Learnings — kanmars.req.20260601.001

## [2026-06-03 Step 0] 前置事实全部验证通过

| 子项 | 结果 |
|---|---|
| Working tree | 仅 `.sisyphus/boulder.json` (本次 start-work) + plan 文件 untracked,代码无 unstaged 改动 |
| HEAD | `167751c1` on `kanmars_main` — 与 plan 基线一致 |
| Baseline FeishuConfig 引用计数 | **91 处**(plan §0.4 估 ~50;实际多 80%) |
| `from_config@845` 存在 | ✅(Momus 审查后实地验证)→ Step 5 纯删除 |
| `display_name@6728` 存在 | ✅(Momus 审查后实地验证)→ Step 6.4 跳过 |
| `.sisyphus/` 是否 gitignored | ❌ 否(34 files tracked)— Step 13 commit 需分两个 atomic commit:(A) plan + boulder.json,(B) 代码改动 |
| `target/release/zeroclaw` binary | 已编(6/1 17:07, 32M)— Step 12.7 dry-run 可直接复用,无需重新 cargo build |

## [2026-06-03 R11 Step 5 prep] 三函数比对结果

**结论:三函数 body 有显著差异,但删除 wrapper 是 STRICT IMPROVEMENT。**

### 差异矩阵

| 维度 | `from_config@845` | `from_lark_config@877` | `from_feishu_config@899` |
|---|---|---|---|
| 接收 alias 参数 | ✅ `impl Into<String>` | ❌ `String::new()` 占位 | ❌ `String::new()` 占位 |
| 接收 peer_resolver 参数 | ✅ `Arc<dyn Fn ...>` | ❌ `Arc::new(\|\| Vec::new())` 空 | ❌ `Arc::new(\|\| Vec::new())` 空 |
| use_feishu 分流 | ✅ 读 `config.use_feishu` | ✅ 读 `config.use_feishu` | ❌ hardcoded `LarkPlatform::Feishu` (FeishuConfig 无 use_feishu 字段) |
| receive_mode 字段赋值 | ✅ `ch.receive_mode = config.receive_mode.clone()` | ❌ **漏**(用 default) | ❌ **漏**(用 default) |
| proxy_url 字段赋值 | ✅ `ch.proxy_url = config.proxy_url.clone()` | ❌ **漏**(用 default = None) | ❌ **漏**(用 default = None) |
| Config 类型 | `&LarkConfig` | `&LarkConfig` | `&FeishuConfig`(不同类型!) |
| 完整性 | 完整 | 缩水 + 2 字段 bug | 缩水 + 2 字段 bug + 类型不同 |

### R11 风险定性

- **R11 触发?** 是,三函数 body 不完全等价(漏 2 字段 + 缺 alias/resolver/use_feishu 处理)
- **R11 缓解?** 极简 —— 删 wrapper 改用 `from_config` 反而 **修复** 了 receive_mode + proxy_url 漏字段的 bug
- **Step 5 工作量?** plan 估 5 min,实际可能 +5 min(orchestrator 的 "lark" arm 调用要从 `from_lark_config(lk).with_peer_resolver(alias, resolver)` 改成 `from_config(lk, alias, resolver)`,少一层 builder,更简洁)
- **Step 5 净代码 LOC?** 仍是 `-32`(plan 估算准确)

### 关键洞察:删 wrapper = 自带 bugfix

`from_lark_config` 和 `from_feishu_config` 是 fork 在引入 FeishuConfig 时新增的两个简化 wrapper,但**没复制 receive_mode + proxy_url 处理**。这意味着 fork 当前用户配 `receive_mode = "webhook"` 或 `proxy_url = "..."` 时,这两个值**根本没被读到**(走的是 wrapper path,然后 orchestrator 用 builder chain 也没补回来)。

删除 wrapper 强制统一 `from_config` 后,这两个字段自动生效。这是 **silent bugfix**,应该写入 Step 13 commit message + CHANGELOG。

### 对 orchestrator "lark" arm 影响

当前:
```rust
LarkChannel::from_lark_config(lk)
    .with_peer_resolver(alias, peer_resolver)  // 补 alias + resolver
    .with_streaming(lk.stream_mode, lk.draft_update_interval_ms)
    .with_approval_timeout_secs(lk.approval_timeout_secs)
    .with_inbound_prefix(lk.inbound_prefix)
    .with_per_user_session(lk.per_user_session)
```

改为:
```rust
LarkChannel::from_config(lk, alias, peer_resolver)  // 一步到位,含 receive_mode + proxy_url
    .with_streaming(lk.stream_mode, lk.draft_update_interval_ms)
    .with_approval_timeout_secs(lk.approval_timeout_secs)
    .with_inbound_prefix(lk.inbound_prefix)
    .with_per_user_session(lk.per_user_session)
```

更简洁 + 修 bug。

### 对 orchestrator "feishu" arm 影响(Step 6.1 整体删)

当前 fork "feishu" arm 调 `from_feishu_config(fs).with_peer_resolver(alias, resolver).with_*()`。删 arm 后:
- 用户写 `[channels.feishu.<alias>]` 在 V3 load 时无对应 HashMap 字段 → V2→V3 fold migration 兜底,自动 fold 成 `[channels.lark.<alias>] use_feishu=true`
- 后续走 "lark" arm 的 `from_config`(LarkConfig + use_feishu=true → LarkPlatform::Feishu)
- 同样修了 receive_mode + proxy_url bug

## [2026-06-03 AC-4] V2→V3 fold migration 测试结果

**结论:V2→V3 自动 fold 在 fork 当前就 100% 工作 —— D1 决策"operator 零感知"承诺已被实证。**

### 跑测命令
```bash
cargo test -p zeroclaw-config --test migration feishu 2>&1 | tail -30
```

### 测试结果(0.04s wall-clock)

```
running 3 tests
test feishu_and_lark_blocks_become_two_aliases ... ok
test feishu_only_block_folds_into_lark_feishu_alias ... ok
test feishu_block_with_same_app_id_as_lark_still_lands_under_feishu_alias ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 86 filtered out; finished in 0.04s
```

### 三测试对应 plan §4 AC-4 + §1.2 fold 矩阵

| 测试名 | 覆盖场景 | 对应 plan §1.2 矩阵行 |
|---|---|---|
| `feishu_only_block_folds_into_lark_feishu_alias` | 用户仅有 `[channels.feishu.default]` → 自动 fold 成 `[channels.lark.feishu] use_feishu=true` | 第 1 行 |
| `feishu_and_lark_blocks_become_two_aliases` | 双 bot:用户同时配 `[channels.lark.default]` + `[channels.feishu.default]` → fold 后变 `lark.default + lark.feishu`,两 bot 都活 | 第 3 行 |
| `feishu_block_with_same_app_id_as_lark_still_lands_under_feishu_alias` | 同 app_id 冲突时 feishu 端保留为 `lark.feishu` alias,**不被 drop**(注意:测试名暗示比 plan §1.2 第 5 行"后者被 drop"更宽容) | 第 5 行(语义略有出入,需 §3 Step 11 CHANGELOG 时核对) |

### 对 §3 Step 12.4 的影响

Plan §3 Step 12.4 设计在所有 §3 Step 2-11 完成后再跑这 3 个测试作为回归检查。**当前预先跑一次(在 fork 未改源码状态)** 的价值:
- ✅ 证明 V2→V3 fold path 在 fork 已就位且正确(降低 R4 风险概率从 Low → ~0)
- ✅ 给 D1 决策提供"operator 零感知"的实证(不是承诺,是测试)
- ✅ Step 12.4 在 plan 完成后再跑一次,确认 fold path 不被破坏

### 实测细节澄清 §1.2 矩阵第 5 行

测试 `feishu_block_with_same_app_id_as_lark_still_lands_under_feishu_alias` 的名字暗示 **feishu 端不会被 drop**,而是落到 `lark.feishu` alias —— 这与 plan §1.2 第 5 行写的"后者被 drop + WARN"语义有差异。

需要 §3 Step 11 CHANGELOG 写之前实地读测试代码确认 fold 行为。当前推测:
- plan §1.2 第 5 行可能描述的是 V3-V3 冲突(用户已经手写了 `[channels.lark.feishu]` + 又有 `[channels.feishu.default]` 同 app_id),不是 V2-V2 冲突
- 实际测试覆盖的是 V2-V2 冲突(两个 V2 block 同 app_id)
- 行为可能是:V2 时期 lark + feishu 并存正常,fold 后都活,V3 reload 时如果 user 又手写 `[channels.lark.feishu]` 才发生 drop

**Action**: Step 11 CHANGELOG 写之前,读 `crates/zeroclaw-config/tests/migration.rs:1350` 那个测试的实际断言,确认 fold 行为描述准确。

## [2026-06-04 Step 4] 发现 silent default-value change(R12 新风险)

### 事实

- `default_feishu_approval_timeout_secs() -> u64 { 120 }` (fork 私有,L10595)
- `default_channel_approval_timeout_secs() -> u64 { 300 }` (上游通用,L10585,fork 也有)

### 当前 LarkConfig 行为

`LarkConfig.approval_timeout_secs` 字段使用 `#[serde(default = "default_feishu_approval_timeout_secs")]`(L11765)。即用户**未显式配** `approval_timeout_secs` 时,默认值 = **120**。

### Plan 完成后行为

`LarkConfig.approval_timeout_secs` 改用 `#[serde(default = "default_channel_approval_timeout_secs")]`。默认值 = **300**(2.5x increase)。

### 影响范围

- **fork 现网部署**:gloria / atlas / songjiang 等所有用 LarkConfig 但**未显式配** `approval_timeout_secs` 的 bot,审批 card 超时窗口从 2 min 静默变 5 min
- **行为差异**:延长操作员响应窗口(对 operator 友好);但若有自动化脚本依赖 2 min 失败重试,会受影响
- **回滚**:operator 在 `[channels.lark.<alias>] approval_timeout_secs = 120` 显式恢复

### 决策(跟 plan §0 ❌ 隐含)

接受默认值 120 → 300 的 silent change:
- Plan §0 显式说"删掉 default_feishu_approval_timeout_secs 后改用通用函数"
- 与上游对齐(上游 LarkConfig 也用 default_channel_approval_timeout_secs)
- 行为变化对 operator 是改善(更长容忍窗口)

### Step 11 CHANGELOG 必须显式标注

```markdown
- **lark/feishu**: `[channels.lark.<alias>].approval_timeout_secs` default value
  changed from `120` (Feishu's historical hardcoded value) to `300` (the
  channel-wide default). Operators wanting the prior 2-minute window must
  set `approval_timeout_secs = 120` explicitly.
```

## [2026-06-04 T11] CHANGELOG-next.md entry inserted

### Insertion point

- File: `CHANGELOG-next.md`
- Section: existing `### Changed` (line 65)
- Position: lines **86-107** (22 lines, under cap of 25)
- After: previous `agents.max_context_tokens` entry (lines 67-84)
- Before: `### Multi-Agent & Runtime` H3 (now line 109)
- No existing entries modified; insert-only.

### Entry covers all 3 facets per T11

1. **Schema consolidation** (top paragraph): `[channels.feishu.<alias>]` removed
   as V3 first-class → folds to `[channels.lark.<alias>] use_feishu = true` via
   existing V2→V3 migration. `.toml.backup-<ts>` written on commit. Calls out
   the 5 preserved features (stream_mode, draft_update_interval_ms,
   approval_timeout_secs, inbound_prefix, per_user_session).
2. **Silent default change (R12)**: `approval_timeout_secs` 120→300, with
   explicit rollback hint (`= 120`).
3. **Silent bugfix (R11)**: `receive_mode` and `proxy_url` previously dropped
   by `from_feishu_config` / `from_lark_config` wrappers; unified `from_config`
   honors them.

### Style match

- Lead pattern `**channels/lark**:` matches existing `**agents**:` and
  `**providers/models**:` entries in `### Changed`.
- Sub-emphasis labels `**Silent default change**:` / `**Silent bugfix**:` mirror
  the existing `**Backward compatibility**:` label in the prior entry.
- No code-fence used (the 3 facets are all prose-style; consistent with the
  agents entry which only has paragraphs, not code blocks). Inline code via
  backticks only.
- English-only per AGENTS.md localization rule. No emojis. No em dashes.

### What was NOT touched

- All other CHANGELOG body preserved verbatim (highlights, added, multi-agent,
  channels, providers, ACP, skills, breaking changes, bug fixes, contributors).
- No new section added; reused existing `### Changed`.
- Plan §6 follow-ups (F1-F11) NOT mentioned per MUST NOT.

## [2026-06-04 T3+T4+T7-partial] schema.rs FeishuConfig cleanup completed

### Deletions applied (8 distinct edits, bottom-up to preserve line numbers)

| # | Target | Lines removed | Method |
|---|---|---|---|
| 1 | 3 FeishuConfig serde tests at end (`feishu_config_serde` / `_toml_roundtrip` / `_deserializes_without_optional_fields`) | ~64 lines | single edit, preserved `// ── LINE ──` separator |
| 2 | `config.channels.feishu.insert(...)` block in `load_or_init_decrypts_feishu_channel_secrets` test (was L19596) | ~20 lines | single edit, lark.insert("feishu",...) above still satisfies the assertion |
| 3 | `config.channels.feishu.insert(...)` block in earlier test (was L17510) | ~20 lines | single edit |
| 4 | `feishu: HashMap::new(),` in 12-indent context (3 occurrences at L10530/18054/18482) | 3 lines | `replaceAll=true` on `lark/feishu/line` trio |
| 5 | `feishu: HashMap::new(),` in 16-indent context (L16738) | 1 line | separate edit (unique indent) |
| 6 | LarkConfig.approval_timeout_secs serde attr 120→300 | 3-line docstring rewrite | R12 documented |
| 7 | `default_feishu_approval_timeout_secs()` function + 6-line doc comment | 11 lines | single edit, preserved next fn `default_matrix_draft_update_interval_ms` |
| 8 | `ChannelsConfig.feishu` field + doc comment + serde attrs | 4 lines | single edit |

### Verification

- `cargo check -p zeroclaw-config` → **PASSED** (Finished dev profile, 2m 35s)
- `grep -nE "FeishuConfig|channels\.feishu|default_feishu_approval_timeout_secs"` on schema.rs → **0 hits** (exit code 1 from grep)

### Anchor disambiguation lesson

4 occurrences of `feishu: HashMap::new(),` had only 2 distinct anchor patterns once you broaden to surrounding lines:
- 12-space indent (3 occurrences) → all share `lark→feishu→line` trio, so `replaceAll=true` handles all 3 at once
- 16-space indent (1 occurrence) → handled separately with the wider `lark/feishu/line` trio at deeper indent

This is faster than 4 separate edits with hand-crafted unique anchors.

### R12 docstring evolution

Initial edit referenced `default_feishu_approval_timeout_secs` in the new docstring → would have left a stale FeishuConfig reference and broken the 0-hit verification. Revised to "previously `120` in the deprecated Feishu fork" — preserves the operator-facing migration recipe without naming the deleted function. Lesson: when deleting symbols, scrub their names from any new docstrings too.


## T8 + T9 + T10 Subagent Findings (2026-06-04)

### T8 — channel-feishu Cargo feature alias
- **Status**: DONE (1 line deleted)
- **File**: `Cargo.toml:318` — `channel-feishu = ["channel-lark"]`
- **Cross-check grep**: `channel-feishu` only referenced in (a) deleted line, (b) historical archive `excision-v0.8.0-incidents.md:78` (item #8 of phase 6 audit, must NOT modify), (c) `README.kanmars.md:55` (fork doc — describes alias's existence; will become stale post-deletion, but not in T10 scope since it's a cargo feature row not a `[channels.feishu]` config block). Plan artifact mentions ignored.
- **Verification**: `cargo metadata --no-deps` confirms `channel-feishu` gone; manifest parses cleanly.

### T9 — V3_CHANNEL_TYPES + TYPE_NAMES feishu entries
- **Status**: NO-OP (target absent)
- **Ground truth**: `V3_CHANNEL_TYPES` lives at `crates/zeroclaw-config/src/schema/v2.rs:82-115` (33 entries). `"feishu"` NOT present (only `"lark"` line 98). `TYPE_NAMES` constant does not exist anywhere in `crates/zeroclaw-config/`.
- **Conclusion**: Upstream v0.8.0 Phase 6 already cleaned this; fork didn't reintroduce.

### T10 — docs `channels.feishu` / `FeishuConfig` references
- **Status**: NO-OP (all hits in protected historical archives)
- **Hits found** (9 lines across 2 files):
  - `docs/maintainers/excision-v0.8.0-line-count-report.md` lines 93, 94 — Phase 6 line-count summary, past-tense historical record describing the v0.8.0 excision work itself. Companion to incidents.md.
  - `docs/maintainers/excision-v0.8.0-incidents.md` lines 67, 71, 72, 73, 77, 79, 81 — explicitly excluded by task spec (historical archive, verbatim).
- **Both files are historical artifacts** documenting what was deleted; no user-facing config docs present `[channels.feishu]` as a V3 first-class option.

### Follow-up / deferred (not blocking this plan)
- `README.kanmars.md:55` documents the `channel-feishu` cargo alias as "等价别名". Post-T8, that row references a deleted line. **Not modified per conservative rule** — outside T10's stated scope (alias != V3 config block). Suggest follow-up issue to refresh fork README to point users at `channel-lark` only.

### Cargo verification (final 5 lines of `cargo check --workspace 2>&1 | tail`)
```
error[E0599]: no function or associated item named `from_feishu_config` found for struct `LarkChannel` in the current scope
error: could not compile `zeroclaw-channels` (lib) due to 7 previous errors
```
**Pre-existing errors** caused by parallel subagent B (lark.rs refactor in flight) + subagent A (ChannelsConfig.feishu field removal). NOT introduced by T8. Confirmed by error signatures: `from_lark_config`, `from_feishu_config`, `field 'feishu' on ChannelsConfig` — all are downstream of subagent A/B/C's still-uncommitted code surface. T8's change (deleting one Cargo.toml feature alias line) cannot produce E0599/E0609 errors of that shape.

### Lesson
- Plan §1.6 marked items as "(待 grep)" precisely because the author wasn't sure they existed. 2/3 turned out to be no-ops. Conservative grep-first approach saved manufacturing fictitious changes.
- When parallel subagents touch related code, "compile clean" gate is unreliable until all parallel work merges. Use shape-of-error inspection (E-codes + symbol names) to disambiguate "my error" vs "their error".

## [2026-06-04 T5] from_lark_config + from_feishu_config wrappers deleted

### Deletions performed (lark.rs only)

| Item | Original lines | Lines removed |
|---|---|---|
| `pub fn from_lark_config(...)` + doc comment | L870-L893 | 24 lines |
| `pub fn from_feishu_config(...)` + doc comment | L895-L910 | 16 lines |
| Test `lark_from_feishu_config_sets_feishu_platform` | L4486-L4513 | 28 lines |
| **Total** | | **~68 lines net removal** |

### `from_config@845` preserved verbatim — verified post-edit.

### T5 verification gap (spec vs reality)

Spec MUST DO: "after deletion, cargo check MUST pass" + "grep ... returns 0 hits".

**Reality**: orchestrator/mod.rs L5462, L6760, L8627, L8641 still call
`from_lark_config` / `from_feishu_config` (4 call sites, lib code not test).
Until task C (orchestrator cleanup) runs, `cargo check -p zeroclaw-channels`
yields 7 errors (4 missing-method + 3 missing-field on ChannelsConfig.feishu).

Plus several test bodies (lark.rs L4471, 4515, 4551, 4653, 4931) still call
the deleted wrappers — but these are in `#[cfg(test)]` and don't break
`cargo check` (only `cargo check --tests` / `cargo test`).

**Atomic unit done correctly per spec literal scope** (delete 2 fns + 1 test
in lark.rs only); spec's "0 errors" verification expectation requires task C
to also land. Confirmed by spec dependency note: "Orchestrator cleanup (C)
runs AFTER you — will change `from_lark_config(lk).with_peer_resolver(...)`
callsites to `from_config(lk, alias, peer_resolver)`."

### Silent bugfix unlocked

Per R11: when C switches orchestrator to `from_config(lk, alias, resolver)`,
`receive_mode` and `proxy_url` are no longer dropped (fork wrappers ignored
those fields). Wrapper deletion is bugfix-bearing.

### lark.rs grep state after T5 (informational, not failure)

```
grep -nE "from_lark_config|from_feishu_config|FeishuConfig" lark.rs
```
returns ~30 hits, all in test bodies (and 6 doc-comment refs). These are
NOT in T5 scope per plan §3 Step 7.2 (which only names one test). Cleanup
of the remaining test body references will happen in a later task or the
broader Step 7.4 grep sweep.


## [2026-06-04 T6 + T7-orchestrator-portion] orchestrator/mod.rs cleanup completed

### Final state of orchestrator/mod.rs

`grep -nE 'feishu|FeishuConfig|from_lark_config|from_feishu_config'` returns 2 hits:
- L6686 `display_name = if lk.use_feishu { "Feishu" } else { "Lark" }` — Step 6.4 SKIP per plan §3 (LarkConfig.use_feishu field still in place, this drives the display label for V3 fold path)
- L17912 `// C22: Verify "lark.<alias>" channel string is routed to the lark arm.` — surviving lark test comment, originally `// C22: Mirror of feishu test for "lark.<alias>"`; retitled to remove stale "Mirror of feishu test" phrasing now that feishu test is gone

Both are intentional. Zero `from_*_config`, zero `channels.feishu`, zero `FeishuConfig`.

### Edits applied (top-down summary, but executed bottom-up to preserve line numbers)

| # | Sub-task | Location | Lines removed/changed |
|---|---|---|---|
| 1 | T7.3 — delete `deliver_announcement_routes_feishu_to_feishu_arm` test | was L17992-18014 | -22 lines |
| 2 | T6.5 — delete `"feishu" =>` arm in `deliver_announcement` + clean `"lark" \| "feishu" =>` not-feature merge to plain `"lark" =>` | was L8635-8650 | -19 lines (merged with cleanup of lark arm peer_resolver wiring) |
| 3 | T6.5a — `deliver_announcement` "lark" arm: introduced inline empty `peer_resolver` (cron path doesn't need real peer resolution; matches semantic of prior `from_lark_config` which baked empty resolver) | L8627-8628 | +3 lines (peer_resolver decl), changed `from_lark_config(lk).with_*()` → `from_config(lk, alias.to_string(), peer_resolver).with_*()` |
| 4 | T6.1 — delete `"feishu" =>` arm in `build_channel_by_id` | was L5451-5492 | -42 lines |
| 5 | T6.2 NO-OP — `"lark" =>` arm at L5425 already used `from_config` (likely landed earlier in plan) | L5425-5450 | unchanged |
| 6 | T6.3 — delete `for (alias, fs) in &config.channels.feishu` health-check loop | was L6701-6727 | -27 lines |
| 7 | T6.3b — clean `is_empty()` guard | L6730 | `lark.is_empty() \|\| feishu.is_empty()` → `lark.is_empty()` |
| 8 | T6 docstring — L5810 channel list comment | L5810 | removed `feishu, ` |
| 9 | T7.4 — comment retitle in surviving lark_to_lark test | L17912 | removed "Mirror of feishu test" stale phrasing |

Net delta: orchestrator/mod.rs from ~18074 → 17993 lines (-81 lines).

### Silent bugfix wired through (per R11)

Both `deliver_announcement` "lark" arm (L8627) and `build_channel_by_id` "lark" arm (L5439) now use `LarkChannel::from_config(...)` instead of the deleted `from_lark_config(...)` wrapper. Per learnings R11: `from_config` honors `receive_mode` + `proxy_url` config fields that the old wrapper silently dropped. Operators with `receive_mode = "webhook"` or `proxy_url = "..."` in their `[channels.lark.<alias>]` block now actually get those values applied. CHANGELOG entry already covers this (per T11).

### `deliver_announcement` cron-path peer_resolver semantic note

The new `"lark" =>` arm constructs `peer_resolver: Arc<dyn Fn() -> Vec<String> + Send + Sync> = Arc::new(Vec::new)` (empty). This matches the **prior** semantic exactly — `from_lark_config(lk)` baked an empty resolver inside (per learnings R11 matrix row "peer_resolver: ❌ `Arc::new(|| Vec::new())` 空"). No behavioral change vs pre-patch.

### Verification gates

| Gate | Result |
|---|---|
| `cargo check -p zeroclaw-channels --features channel-lark` | ✅ PASS in 26.67s, 0 errors |
| `cargo check` for 13 source crates (zeroclaw-channels, -config, -runtime, -providers, -tools, -memory, -api, -log, -infra, -gateway, -plugins, -tool-call-parser, -macros) | ✅ PASS in 2m 33s, 0 errors |
| `cargo check --workspace` | Blocked by sandbox env (`gio-2.0 >= 2.70` missing — system pkg-config issue, not a code issue). Affects only `zeroclaw-tui` and `zeroclaw-hardware` (GTK-dependent crates, not in our scope). |
| `cargo test -p zeroclaw-channels --features channel-lark --lib` | ❌ FAILS — but the 11 errors are 100% in `crates/zeroclaw-channels/src/lark.rs` test bodies (L4446-L4931, all `#[cfg(test)]`). Pre-existing per learnings.md L283-291 (T5 left lark.rs test bodies stale; spec scope was prod code only). Orchestrator's own test (`deliver_announcement_routes_lark_to_lark_arm`) compiles fine — these errors are sibling test bodies referencing deleted symbols. |

### Action item for Atlas / Step 12

Before Step 12 verification gate can pass, somebody (likely Atlas as part of Step 12 or a fast follow-up subagent) must clean `crates/zeroclaw-channels/src/lark.rs` test bodies of `from_lark_config` / `from_feishu_config` / `FeishuConfig` references. That is **not** in T6/T7 scope per spec MUST NOT clause. Lines to clean (per cargo test output): 4446, 4471, 4472, 4496, 4515, 4531, 4551, 4629, 4653, 4914, 4931. Estimated ~50-100 lines of test body deletions/migrations.

### Hook behavior in this run

Two comment-write hook invocations:
1. Initial verbose 2-line "silent bugfix" explanatory comment in `deliver_announcement` `"lark"` arm — removed per Priority 4 (unnecessary; learnings.md already captures R11 context permanently)
2. Test comment retitle (L17912) — pre-existing comment edited in place (Priority 1), kept

Lesson: when a hook fires, the cleanest path is to remove the new comment if `learnings.md` already permanently documents the rationale. Code-reviewer reading the diff can find R11 in notepad; bloating the source with rationale is duplicate state.

## Final Cleanup Sweep (Phase A + B) — lark.rs orphaned FeishuConfig/from_*_config refs

After T6 deleted `FeishuConfig` struct + `from_feishu_config` + `from_lark_config` wrappers, **6 sibling tests + 6 doc-comment fragments remained** that still referenced the deleted symbols. They didn't surface until `cargo check --tests` ran with the channel-lark feature enabled.

### Deleted tests (Phase A, all in `crates/zeroclaw-channels/src/lark.rs` test module)

1. `lark_from_feishu_config_propagates_mention_only` (39 lines, ~L4444-4482) — exercised `from_feishu_config`
2. `lark_per_user_session_propagates_from_lark_config` (40 lines, ~L4484-4522) — exercised `from_lark_config`
3. `lark_per_user_session_propagates_from_feishu_config` (35 lines, ~L4524-4558) — exercised `from_feishu_config`
4. `lark_from_feishu_config_initializes_5_new_fields_to_constructor_defaults` (42 lines, ~L4620-4661) — exercised `from_feishu_config`
5. `lark_image_resource_url_matches_region` (33 lines, ~L4904-4936) — used `from_feishu_config` to build Feishu channel instance for URL parity check; deleted in full per task spec (the Feishu URL routing assertion lost coverage but `lark_reaction_url_matches_region` still covers the same parity via `LarkConfig{use_feishu:true}` + `from_config`)

Total: 5 tests removed (~189 lines).

### Doc comments rewritten (Phase B)

- L643: `Lifted to LarkConfig/FeishuConfig` → `Lifted to LarkConfig`
- L646: `after Self::from_lark_config / Self::from_feishu_config` → `after Self::from_config`
- L654: `Lifted to LarkConfig/FeishuConfig in C20` → `Lifted to LarkConfig in C20`
- L800: `LarkConfig/FeishuConfig exposed this` → `LarkConfig exposes this`
- L820: `C20 lifts it to LarkConfig/FeishuConfig` → `C20 lifts it to LarkConfig`
- L831: `Self::from_feishu_config / Self::from_lark_config which default` → `Self::from_config which defaults`
- L4472 (test-module comment): `from FeishuConfig/LarkConfig` → `from LarkConfig`

### Verification

- `cargo check -p zeroclaw-channels --features channel-lark --tests` → 0 errors, 10.42s
- `cargo test -p zeroclaw-channels --features channel-lark --lib` → **1278 passed; 0 failed; 0 ignored**, 11.14s
- `grep -nE "FeishuConfig|from_feishu_config|from_lark_config" crates/zeroclaw-channels/src/lark.rs` → 0 hits

### Learning

When deleting symbols (struct/fn/wrapper), `cargo check` of `--lib` alone doesn't surface test bodies that referenced them. Must run `cargo check --tests` (or `cargo check --all-targets`) explicitly. Also: a "feature-gated" symbol's referents may all be feature-gated too — when in doubt, run the check with the canonical feature flag enabled (`--features channel-lark` here).

Also: when a single test mixed a still-valid assertion (e.g., Lark URL parity) with a deleted-symbol assertion (Feishu via `from_feishu_config`), the explicit task directive was to **delete in full** rather than rewrite. The lost Feishu coverage was acceptable because a parallel test (`lark_reaction_url_matches_region`) exercised the same URL-base-selection logic via the unified `from_config(LarkConfig{use_feishu:true}, ...)` path.

## Atlas Step 12 follow-up: src/config/mod.rs Feishu cleanup (post-subagent)

The 5 parallel cleanup subagents missed `src/config/mod.rs` because plan §3 scope was `crates/*`; `src/` (binary entry) wasn't enumerated. Atlas verification caught it at fmt --check stage.

**Changes** (src/config/mod.rs, -19 lines):
- L17 re-export list: removed `FeishuConfig, ` (1 line edit)
- Test block: removed `let feishu = FeishuConfig {...};` (17 lines) + `assert_eq!(feishu.app_id, ...);` (1 line)

**Verification**:
- `cargo fmt --all -- --check` → exit 0
- `cargo check -p zeroclaw-config` → exit 0
- `grep -rn "FeishuConfig|channels.feishu|from_feishu_config" src/ Cargo.toml` → 0 hits
- Note: full `cargo check --features agent-runtime` fails on PRE-EXISTING `zeroclaw-channels` orchestrator feature-gating bugs (unconditional `pub use crate::{wati,webhook,wecom};` referencing `#[cfg(feature="...")]` modules). These are unrelated to Feishu cleanup and outside scope.

**Lesson**: Future binary↔library refactor plans must enumerate `src/` re-export shims explicitly when deleting library types; "delete X" subagents that scan only `crates/` will leave binary-side re-exports as dangling references.

---

## T13 Execution Record (2026-06-04)

Two atomic commits created on `chore/consolidate-lark-feishu-to-upstream-schema`, **NOT pushed** (per plan §0 — awaiting user sign-off after diff review).

| Commit | Hash | Scope | Files | Net |
|---|---|---|---|---|
| A | `be34225d` | sisyphus state (plan + notepad + boulder) | 4 | +1373/-17 |
| B | `ddf35809` | source consolidation (Lark/Feishu unification) | 7 | +59/-633 |

**Commit B file list** (explicit, no `git add -A`):
- `crates/zeroclaw-config/src/schema.rs` (−219)
- `crates/zeroclaw-channels/src/lark.rs` (−287)
- `crates/zeroclaw-channels/src/orchestrator/mod.rs` (−125)
- `crates/zeroclaw-runtime/src/agent/loop_.rs` (cargo fmt only, +16/-16 churn)
- `src/config/mod.rs` (+21)
- `Cargo.toml` (−1, removed unused dep)
- `CHANGELOG-next.md` (+23)

**Verification**: `git status --porcelain` empty; `git log --oneline -3` shows both new commits on top of `167751c1` (previous HEAD). Pre-commit hooks ran normally (no `--no-verify` bypass).

**Next**: gloria/atlas operator + reviewer +1 → `git push -u origin chore/consolidate-lark-feishu-to-upstream-schema` → open PR.
