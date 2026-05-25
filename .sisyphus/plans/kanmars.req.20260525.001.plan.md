# Plan — kanmars.req.20260525.001 (Merge upstream master into kanmars_main with C22/C23/C16 audit blind-spot mitigation)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260525.001.plan |
| 关联需求 | 用户对话需求（2026-05-25）：kanmars_main 想 merge master 拿上游修复，但 3 个文件冲突。原因诊断：`master` = `upstream/master` = `3498f50e`（含 22 个 upstream commit，其中 `3498f50eb` 是 PR #6851 lark cron delivery）；`kanmars_main` = base `c746998f6` + fork-absorb `690572176` + 3 个独立 commit。冲突全部由 PR #6851 与 kanmars 自家 lark/feishu 拆分设计（Q2=B 决策 + 7dcb38f77 builder chain + 1f5cb56d2 reaction pool 删除）撞车，同时命中 `boulder.json.audit_blind_spots_recorded` 中已记录的 C22 / C23 / C16 三类盲区。 |
| 起草日期 | 2026-05-25 |
| 修订日期 | 2026-05-25 (rev0 — 初稿，待用户审 §2 文件级合并策略 + §3 执行步骤) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `chore/merge-master-into-kanmars-main`（待创建，基于 `kanmars_main` `166e258dc`） |
| 目标 PR 标题 | _(本 merge 是本地操作，是否开 PR 由用户在 §3 Step 10 审完 diff 后决定，本 plan 不预设 PR 名)_ |
| 风险等级 | **Medium-High**（跨 Experimental tier `zeroclaw-channels` 3 个核心文件 + 触及生产 channel 逻辑 + 同时命中 4 类 audit blind spots 中的 3 类：C22 / C23 / C16；无 schema / API / DB / security trait 改动） |
| 基线 commit | `166e258dc`（[kanmars_main HEAD](file:///Users/kanmars/workspace/kanmars_zeroclaw_github)）+ `3498f50eb`（master = upstream/master HEAD，含 22 个 upstream commit） |
| 选型方案 | **方案 A — 以 `kanmars_main` 为基底，3 个冲突文件全用 `git checkout --ours` + 逐文件 audit master 非冲突增量** |
| 预计代码行数 | **冲突部分：+0 / -0**（全用 ours）；**非冲突 auto-merge**：约 +500 / -300（来自 22 个 upstream commit 的 auto-merged 部分）；merge commit 一个 |
| 预计工作量 | 约 60-90 分钟（含 Step 8 master 非冲突增量 audit + cargo build/clippy/test 验证 + 不 commit 输出 diff） |

---

## 0. 关键目标（唯一真理来源）

> **把 upstream master 的 22 个增量 commit 合进 `kanmars_main`，不丢任何 kanmars 团队的生产决策代码，不触发任何已记录的 audit blind spot 回归。**

**完成此目标即"merge 完成"**：

- merge 后 `git log kanmars_main` 包含 master 全部 22 个 commit（`c746998f..3498f50e` 区间）
- 3 个冲突文件解决后 **100% 保留 kanmars 团队的生产决策**：
  - 审批卡片系统（参见 `kanmars.req.20260515.001.plan` + `kanmars.req.20260516.001-004.plan`）
  - Draft 流式回复（参见 `kanmars.req.20260512.001.plan` PR4）
  - Reaction CRUD + GLANCE-only 入站策略（参见 `kanmars.req.20260512.001.plan` PR2 + commit `1f5cb56d2`）
  - lark/feishu 拆分 arm + 4 个 C20 builder chain（Q2=B 决策来源：[.sisyphus/notepads/kanmars.req.20260512.001.plan/issues.md](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/.sisyphus/notepads/kanmars.req.20260512.001.plan/issues.md) + commit `7dcb38f77`）
  - Inbound 中文前缀（参见 `kanmars.req.20260512.002.plan`）
  - `image_resource_url` 迁移到飞书消息资源 API（与 `file_download_url` 对称）
  - `cron_add.rs` 11-channel enum（含 `dingtalk` / `wecom`）
- 从 master 吸收的非冲突增量全部落地：
  - 22 个 upstream commit 中 auto-merge 部分（含 `expand_tilde_in_path` #6238 / `channel_strict_non_native_prompt_hides_text_tool_protocol` #6736 / OTel gen_ai.tool.* attrs #6009 / no-default feature compile #6745 / strict tool parsing #6675 等）
- `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test --workspace` 全绿
- **不 commit**，等用户审 §3 Step 10 输出的 diff 后再决定 merge commit 与否

**显式不在范围内**：

- ❌ 不修复任何上游缺陷（如 master `cron_add.rs` enum 漏 dingtalk/wecom——本来就保留 kanmars 11-channel 全集了）
- ❌ 不评估是否撤销 fork-absorb `690572176`（这是已通过的产品决策）
- ❌ 不为 #6851 PR 的 `from_config` 接口适配 kanmars 的 `from_lark_config` / `from_feishu_config`（kanmars 拆开 arm 已自带 from_xx_config 调用，不需要适配）
- ❌ 不引入 master #6851 PR 的 `"lark" | "feishu" =>` 合并 arm 设计（与 kanmars Q2=B 决策对立）
- ❌ 不恢复多语言 reaction pool（`1f5cb56d2` 显式删除并加 regression-marker `lark_inbound_ack_policy_is_glance_only_no_random_pool`，命中 C23）
- ❌ 不恢复 `image_download_url`（已被 `image_resource_url` 替换）
- ❌ 不动 commit history（不 squash、不 rebase；如最终 commit，仅做单一 merge commit）
- ❌ 不 force-push、不 amend
- ❌ 不在本 plan 阶段创建 PR（merge 是本地操作，PR 由用户在审完 diff 后决定）
- ❌ 不移植 master #6851 新增的 wiremock 集成测试（4 个 helper/test 依赖 master 的 `from_config` 接口，与 kanmars 拆开 arm 不兼容；列为 §6 F1 follow-up）

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **SINGLE SOURCE OF TRUTH 铁律**（[zeroclaw AGENTS.md ABSOLUTE RULE](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md)）：
   - 本 merge **不新增任何 struct 字段、schema 字段、config entry、runtime cache**——3 个冲突文件全部沿用 kanmars_main 既有 source of truth。✅ 合规
   - 不复制 master 上 reaction pool 的常量到 kanmars_main（已经删了不能恢复）
2. **C23 audit blind-spot mitigation**（[boulder.json](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/.sisyphus/boulder.json)）：
   - 本 merge **命中 C23 (fork-only DELETIONS — fork removed upstream code, our merge kept upstream additions)**——upstream 把 fork 删除的 reaction pool 带回
   - 缓解：commit `1f5cb56d2` 的 `lark_inbound_ack_policy_is_glance_only_no_random_pool` 回归测试必须 PASS（§4 AC-2）；3 个冲突文件 lark.rs 用 `--ours` 不接受 master 侧
3. **C16 audit blind-spot mitigation**：
   - 本 merge **命中 C16 (match-arm INTERNAL modification)**——`deliver_announcement` 的 lark/feishu arm 内部 #6851 与 kanmars Q2=B 修改冲突
   - 缓解：§3 Step 5 手工 grep 验证 4 个 C20 builder chain (`with_streaming` / `with_approval_timeout_secs` / `with_inbound_prefix` / `with_per_user_session`) 全部保留 + "Atlas decision per gloria operator request" 注释存在
4. **C22 audit blind-spot mitigation**：
   - 本 merge **命中 C22 (match expression MISSING ARM)**——master `cron_add.rs` enum 漏 `dingtalk` / `wecom` 但 `mod.rs` 实现存在
   - 缓解：保留 kanmars enum 11-channel 全集（§3 Step 4）
5. **未命中**：C21 (struct literal FIELD-LEVEL value diff)。无需缓解。
6. **不新增 `unwrap()` / `expect()`**（[AGENTS.md Anti-Patterns](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md#anti-patterns)）——本 merge 不写新代码，仅做冲突解析，自然合规
7. **不新增 `#[allow(dead_code)]`**——同上，仅冲突解析
8. **`tracing::` 日志保持英文**（RFC #5653 §4.6）——本 merge 不动现有日志
9. **不引入新依赖**——本 merge 不动 Cargo.toml（如 master `Cargo.toml` 有依赖变更通过 auto-merge 进入，需在 Step 8 audit 中确认）
10. **完整跑** `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test --workspace`（§3 Step 9 + §4 AC-9/10/11）
11. **One concern per merge**：本 merge 一个关注点 = "把 22 个 upstream commit 合进 kanmars_main"。**不混合任何 feature / refactor / fix**；§6 列出的 3 个 follow-up 必须另起 PR
12. **基线分支**：从 `kanmars_main` `166e258dc` 创建 `chore/merge-master-into-kanmars-main`；不动 `master` 分支、不动 `upstream/master`
13. **不 commit + 不 push**：§3 Step 10 输出 diff 后等用户决策；如用户决定 commit，必须是单一 merge commit（无 squash、无 rebase）

---

## 1. 现状事实复核（已实地验证，行号对齐 kanmars_main @ `166e258dc` 和 master @ `3498f50eb`）

### 1.1 分支拓扑事实

| 事实 | 验证方法 | 结果 |
|---|---|---|
| `master` 与 `upstream/master` 是否一致 | `git rev-parse master upstream/master` | ✅ 两者都是 `3498f50ebd60209d330212102f58a268fac3e45e` |
| `kanmars_main` HEAD | `git rev-parse kanmars_main` | `166e258dc` |
| merge base | `git merge-base kanmars_main master` | `c746998f6577bac04cf664c0f003a0154d1e4688`（即 PR #6816 `fix(policy)` heredocs） |
| master 上是否含 fork-absorb commit | `git branch --contains 690572176 \| grep '^\s*master$'` | ❌ 0 hits（master 是纯 upstream 状态） |
| master 上 lark.rs 是否含审批/draft/reaction CRUD | `git show master:lark.rs \| grep -c 'fn build_approval_card\|fn request_approval\|fn update_draft'` | 0 |
| `kanmars_main` 落后 master commit 数 | `git rev-list --count c746998f..master` | 22 |
| `kanmars_main` 领先 master commit 数 | `git rev-list --count c746998f..kanmars_main` | 5 |

### 1.2 冲突清单（`git merge-tree --write-tree kanmars_main master`）

报告 **3 个 CONFLICT (content)**：

| # | 文件 | kanmars 侧来源 commit | master 侧来源 commit |
|---|---|---|---|
| 1 | [crates/zeroclaw-runtime/src/tools/cron_add.rs](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-runtime/src/tools/cron_add.rs) | `690572176` fork-absorb | `3498f50eb` #6851 lark cron delivery |
| 2 | [crates/zeroclaw-channels/src/lark.rs](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/lark.rs) | `690572176` fork-absorb（审批/draft/reaction CRUD/inbound prefix/from_xx_config/patch_message/image_resource_url 全部）+ `1f5cb56d2` 删 reaction pool | `3498f50eb` #6851 集成测试（`mount_lark_token_and_send_mocks` 等 4 个 helper/test） |
| 3 | [crates/zeroclaw-channels/src/orchestrator/mod.rs](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs) | `690572176` + `bf5049e2b` merge + `045b60ac6` classifier_provider + `c4540a489` max_context_tokens + `7dcb38f77` Q2=B builder chain | `3498f50eb` #6851 + `f1f57578d` OTel #6009 + `88e8f905b` WeChat #6238 + `17891e349` no-default #6745 + `7437cd0e6` strict tool #6675 |

### 1.3 三方函数分布关键样本（base c746998f / kanmars_main 166e258d / master 3498f50e）

| 函数 | base | kanmars | master | 说明 |
|---|---|---|---|---|
| `build_approval_card` | ❌ | ✅ | ❌ | kanmars 新增（req 20260515.001 + 20260516.001-004） |
| `build_resolved_approval_card` | ❌ | ✅ | ❌ | kanmars 新增（req 20260515.001） |
| `request_approval` | ❌ | ✅ | ❌ | kanmars 新增（req 20260515.001） |
| `wait_for_decision` | ❌ | ✅ | ❌ | kanmars 新增（req 20260515.001） |
| `patch_approval_card_resolved` | ❌ | ✅ | ❌ | kanmars 新增（req 20260516.004 P0 - 客户端不刷新） |
| `patch_or_send_once` | ❌ | ✅ | ❌ | kanmars 新增（req 20260516.004） |
| `handle_card_action_event` | ❌ | ✅ | ❌ | kanmars 新增（req 20260516.004 Card 1.0/2.0 兼容） |
| `update_draft` / `update_draft_progress` / `finalize_draft` / `cancel_draft` / `send_draft` | ❌ | ✅ | ❌ | kanmars 新增（req 20260512.001 PR4 Draft 流式） |
| `supports_draft_updates` | ❌ | ✅ | ❌ | kanmars 新增（req 20260512.001 PR4） |
| `add_reaction` / `remove_reaction` | ❌ | ✅ | ❌ | kanmars 新增（req 20260512.001 PR2 + Q3 fix） |
| `unicode_to_lark_emoji_type` | ❌ | ✅ | ❌ | kanmars 新增（req 20260512.001 PR2 emoji 命名规范化）|
| `build_feishu_inbound_prefix` | ❌ | ✅ | ❌ | kanmars 新增（req 20260512.002 inbound 前缀） |
| `from_lark_config` / `from_feishu_config` | ❌ | ✅ | ❌ | kanmars 新增（req 20260512.001 + Q2=B 决策） |
| `patch_message_url` | ❌ | ✅ | ❌ | kanmars 新增（req 20260515.001 PATCH /im/v1/messages/{id}） |
| `patch_card_content` / `delete_message_reaction_url` | ❌ | ✅ | ❌ | kanmars 新增 |
| `image_resource_url(message_id, image_key)` | ❌ | ✅ | ❌ | kanmars 改名 + API 迁移：`/im/v1/messages/{id}/resources/{key}?type=image` |
| `detect_lark_ack_locale` / `lark_ack_pool` / `random_lark_ack_reaction` / `map_locale_tag` / `find_locale_hint` | ✅ | ❌ | ✅ | kanmars `1f5cb56d2` 有意删除（gloria operator 投诉哈士奇 sticker）+ regression-marker 测试 |
| `is_cjk_han` / `is_japanese_kana` / `is_simplified_only_han` / `is_traditional_only_han` | ✅ | ❌ | ✅ | 同上（locale 检测整链一起删） |
| `image_download_url(image_key)` | ✅ | ❌ | ✅ | kanmars 替换为 `image_resource_url`（旧 API `/im/v1/images/{key}` 在权限场景下拿不到图）|
| `mount_lark_token_and_send_mocks` | ❌ | ❌ | ✅ | master #6851 新增 wiremock helper |
| `assert_send_body_matches_recipient_and_text` | ❌ | ❌ | ✅ | master #6851 新增 wiremock helper |
| `lark_send_via_from_config_emits_post_to_messages_endpoint` | ❌ | ❌ | ✅ | master #6851 新增集成测试（基于 `from_config`，不适配 kanmars 拆开 arm）|
| `feishu_send_via_from_config_emits_post_to_messages_endpoint` | ❌ | ❌ | ✅ | 同上 |
| `expand_tilde_in_path` | ❌ | ❌ | ✅ | master #6238 WeChat 配置路径展开（auto-merge 应已入工作树，§3 Step 8 audit） |
| `channel_strict_non_native_prompt_hides_text_tool_protocol` | ❌ | ❌ | ✅ | master #6736 strict tool parsing 测试（同上） |
| `deliver_announcement_rejects_feishu_value_when_use_feishu_false` | ❌ | ❌ | ✅ | master #6851 测试（基于 `use_feishu` 字段 + 合并 arm，与 kanmars 拆开 arm 不兼容）|
| `deliver_announcement_routes_lark_to_lark_arm` | ❌ | ✅ kanmars 拆开 arm 版 | ✅ master 合并 arm 版 | **同名但语义不同**：kanmars 测拆开 lark arm，master 测合并 `"lark" \| "feishu"` arm |
| `deliver_announcement_routes_feishu_to_feishu_arm` | ❌ | ✅ | ❌ | kanmars 新增（仅当拆开 arm 才有意义） |
| `resolve_classifier_route` | ❌ | ✅ | ❌ | kanmars `045b60ac6` classifier_provider per-agent override |

**关键观察**：master 上的 `deliver_announcement_routes_lark_to_lark_arm` 与 kanmars 同名但**语义截然不同**，merge 后必须用 kanmars 版本（测拆开 arm），不能用 master 版本（测合并 arm）。

### 1.4 审计盲区命中分类（参考 [.sisyphus/boulder.json](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/.sisyphus/boulder.json)）

本次 merge 同时命中 4 类已记录 audit blind spots 中的 3 类（`boulder.json.audit_blind_spots_recorded`）：

| 盲区类别 | 表现 | 缓解措施 |
|---|---|---|
| **C22** (match expression MISSING ARM) | master `cron_add.rs` enum 漏 `dingtalk` / `wecom`（但 `mod.rs` 5178/5205 行实现存在） | §3 Step 4 grep 验证保留 kanmars 11-channel 全集；§4 AC-3 |
| **C23** (fork-only DELETIONS — fork removed upstream code, our merge kept upstream additions) | upstream 把 fork 删除的多语言 reaction pool（`detect_lark_ack_locale` 等 12+ 符号）带回 | §3 Step 7 grep 验证 reaction pool 仍删除；§4 AC-7；regression-marker 测试 `lark_inbound_ack_policy_is_glance_only_no_random_pool` 必须 PASS（§4 AC-2） |
| **C16** (match-arm INTERNAL modification) | `deliver_announcement` 内 lark/feishu arm 设计 #6851 (合并 arm + `use_feishu`) vs kanmars Q2=B (拆开 arm + 4 个 C20 builder chain) 互斥 | §3 Step 5 grep 验证拆开 arm + builder chain + Atlas 注释保留；§4 AC-4/5 |

**未命中**：C21 (struct literal FIELD-LEVEL value diff)。

### 1.5 master 22 个 upstream commit 全清单（c746998f..3498f50e）

```
3498f50eb feat(channels,runtime): support lark as a cron delivery channel (#6851)  ← 冲突源头
90fc018d7 fix(channels/whatsapp-web): convert LID to phone reply target for all DMs (#6845)
a8c727744 docs(contributing): add architecture contribution map (#6853)
8e12d4bf8 fix(web): align provider badge with ConfiguredOnlyPicker filter (#6828)
f8509c292 feat: Add blog RSS/Atom feeds and sitemap discovery endpoints (#6774)
884fae4b9 fix(doctor): use configured model provider credentials (#6838)
fa51898da fix(channels/whatsapp): restore Apr-2026 protocol parity (#6246) (#6706)
121bbb53b fix: fix link rendering in philosophy.md (#6769)
cab2f5ab4 ci(labeler): narrow ci label matcher scope (#6814)
d6b2c92af fix(deploy): parametrize service template user (#6804)
f1f57578d feat(obs): enrich OTel tool spans with gen_ai.tool.* semantic convention attrs (#6009)
c356f5560 chore: Optimize images (#6748)
d05c8a9b0 fix(discord): treat thread messages as belonging to parent channel (#6829)
04cd70f00 fix(runtime): transcode Windows shell output from system code page to UTF-8 (#6772)
b4d1d59b1 feat(browser): add agent-browser headed config (#6636)
88e8f905b fix(channel): persist WeChat context_tokens and expand tilde in config paths (#6238)
17891e349 fix(channels): compile no-default feature builds (#6745)
ffaef8017 fix(memory): purge_namespace deletes by namespace, not category column (#6777)
ded9abe5a fix(web): validate local provider models during onboarding (#6811)
84413ff74 fix(security): detect Groq API keys in leak scanner (#6812)
7f9271cbc fix(providers): restore Kimi Code vision capability (#6809)
7437cd0e6 fix(runtime): add strict tool parsing mode (#6675)
```

绝大多数（21 个）通过 git auto-merge 自动进入工作树，仅 #6851 在 3 个文件上撞车需手工解决。

---

## 2. 文件级合并策略

### 2.1 `cron_add.rs`（4 行冲突）

**操作**：`git checkout --ours crates/zeroclaw-runtime/src/tools/cron_add.rs` 后保留 kanmars enum 11-channel 全集。

**保留代码**（kanmars_main 侧）：

```rust
"enum": ["telegram", "discord", "slack", "mattermost", "matrix", "qq", "webhook", "feishu", "lark", "dingtalk", "wecom"],
```

**丢弃代码**（master 侧）：

```rust
"enum": ["telegram", "discord", "slack", "mattermost", "matrix", "qq", "webhook", "lark", "feishu"],
```

**理由**：
1. master 缺 `dingtalk` / `wecom` 是 PR #6851 的 scope 局限（PR 只关心 lark），不是去除决策
2. master 的 [`mod.rs:5178`](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L5178) (`"dingtalk" =>`) 与 [`mod.rs:5205`](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L5205) (`"wecom" =>`) 实际有实现，cron schema 应一致暴露给 LLM
3. 命中 C22 盲区，必须保留 enum 全集消除 schema/实现不一致

**预期文件级 diff**：保留 kanmars 现状，行数不变（仍 1111 行）。

---

### 2.2 `lark.rs`（~764 行冲突区，整文件 kanmars 6073 行 / master 4180 行）

**操作**：`git checkout --ours crates/zeroclaw-channels/src/lark.rs` 保留 kanmars 整个文件。

**保留代码**（kanmars_main 侧，完整列表）：

| 类别 | 函数 / 符号 | req 来源 |
|---|---|---|
| 审批卡片系统 | `build_approval_card` / `build_resolved_approval_card` / `request_approval` / `wait_for_decision` / `patch_approval_card_resolved` / `patch_or_send_once` / `handle_card_action_event` | req 20260515.001 + 20260516.001-004 |
| 审批卡片测试 | `build_approval_card_*` (4 个) / `handle_card_action_event_*` (3 个) | 同上 |
| Draft 流式 | `update_draft` / `update_draft_progress` / `finalize_draft` / `cancel_draft` / `send_draft` / `supports_draft_updates` | req 20260512.001 PR4 |
| Draft 流式测试 | `update_draft_*` (3 个) / `send_draft_*` / `supports_draft_updates_*` | 同上 |
| Reaction CRUD | `add_reaction` / `remove_reaction` + 缓存逻辑（`channel_message_id_*` 校验） | req 20260512.001 PR2 |
| Reaction CRUD 测试 | `add_reaction_*` / `remove_reaction_*` (4 个) | 同上 |
| Emoji 映射 | `unicode_to_lark_emoji_type` + 测试 `unicode_to_lark_emoji_type_covers_known_noreply_emojis` | req 20260512.001 PR2 + Q3 emoji 命名规范化 |
| Inbound 前缀 | `build_feishu_inbound_prefix` + 3 个测试（`inbound_prefix_format_matches_spec` / `inbound_prefix_handles_overflow_timestamp` / `inbound_prefix_prepends_cleanly_to_user_text`） | req 20260512.002 |
| 配置入口 | `from_lark_config` / `from_feishu_config` + 测试 `lark_from_feishu_config_*` (4 个) / `lark_per_user_session_*` | req 20260512.001 + Q2=B |
| URL 构造 | `patch_message_url` / `patch_card_content` / `image_resource_url` / `delete_message_reaction_url` / `file_download_url` | req 20260515.001 |
| 辅助工具 | `truncate_card_markdown` / `looks_like_uuid_v*` / `resolve_sender` | 多 req |
| Reaction pool 删除护栏 | `lark_inbound_ack_policy_is_glance_only_no_random_pool` | commit `1f5cb56d2` regression-marker |
| Builder chain | `with_streaming` / `with_approval_timeout_secs` / `with_inbound_prefix` / `with_per_user_session` + 4 个测试 | req 20260512.001 + Q2=B |
| 图片资源 | `image_resource_url` + 测试 `lark_image_resource_url_matches_region` / `lark_image_max_bytes_is_*` | kanmars 自家迁移 |

**丢弃代码**（master 侧的对应区域）：

| 不引入 | 原因 |
|---|---|
| `LARK_ACK_REACTIONS_ZH_CN` / `LARK_ACK_REACTIONS_ZH_TW` / `LARK_ACK_REACTIONS_EN` / `LARK_ACK_REACTIONS_JA` 4 个常量 | C23 命中——`1f5cb56d2` 显式删除 |
| `detect_lark_ack_locale` / `lark_ack_pool` / `random_lark_ack_reaction` / `random_from_pool` / `pick_uniform_index` | C23 |
| `map_locale_tag` / `find_locale_hint` / `detect_locale_from_post_content` / `detect_locale_from_text` | C23 |
| `LarkAckLocale` enum + 4 个 script 判定（`is_japanese_kana` / `is_cjk_han` / `is_simplified_only_han` / `is_traditional_only_han`） | C23 |
| 6 个 `lark_reaction_locale_*` 测试 + `random_lark_ack_reaction_respects_detected_locale_pool` 测试 | C23 |
| `image_download_url` (`/im/v1/images/{image_key}`) | 已被 `image_resource_url` 替换（消息资源 API 在权限场景下更稳） |
| `lark_image_download_url_matches_region` 测试 | 同上 |

**待评估（§6 F1，本 plan 不强制移植）**：

- `mount_lark_token_and_send_mocks` (helper)
- `assert_send_body_matches_recipient_and_text` (helper)
- `lark_send_via_from_config_emits_post_to_messages_endpoint` (集成测试)
- `feishu_send_via_from_config_emits_post_to_messages_endpoint` (集成测试)

**为何延迟移植**：上述 4 个集成测试基于 master 的 `LarkChannel::from_config(lk, alias, peer_resolver)` 接口（`from_config` 内部通过 `use_feishu` 字段分流 lark/feishu）；kanmars 是 `from_lark_config(lk)` + `from_feishu_config(fs)` 拆开接口。移植需要重写 helper signature 适配拆开设计，是独立的工程改造，不属于本 merge 的 scope。

---

### 2.3 `orchestrator/mod.rs`（3 块冲突，整文件 kanmars 16169 行 / master 16251 行）

**操作**：`git checkout --ours crates/zeroclaw-channels/src/orchestrator/mod.rs` 保留 kanmars 整个文件。

#### Block A（行 5256-5301，Channel 构造时的 lark/feishu arm）

**保留代码**（kanmars 侧）：

```rust
"feishu" => {
    #[cfg(feature = "channel-lark")]
    {
        let alias = "default".to_string();
        let peer_resolver: Arc<dyn Fn() -> Vec<String> + Send + Sync> = {
            let cfg_arc = config_arc.clone();
            let alias = alias.clone();
            Arc::new(move || cfg_arc.read().channel_external_peers("feishu", &alias))
        };
        if let Some(fs) = config.channels.feishu.get("default") {
            return Ok(Arc::new(
                LarkChannel::from_feishu_config(fs)
                    .with_peer_resolver(alias, peer_resolver)
                    .with_streaming(fs.stream_mode, fs.draft_update_interval_ms)
                    .with_approval_timeout_secs(fs.approval_timeout_secs)
                    .with_inbound_prefix(fs.inbound_prefix)
                    .with_per_user_session(fs.per_user_session),
            ));
        }
        // Legacy: [channels.lark.default] with use_feishu = true.
        let lk = config.channels.lark.get("default")
            .context("Feishu channel is not configured")?;
        let lark_peer_resolver: Arc<dyn Fn() -> Vec<String> + Send + Sync> = {
            let cfg_arc = config_arc.clone();
            Arc::new(move || cfg_arc.read().channel_external_peers("lark", "default"))
        };
        Ok(Arc::new(
            LarkChannel::from_config(lk, "default".to_string(), lark_peer_resolver)
                .with_streaming(lk.stream_mode, lk.draft_update_interval_ms)
                .with_approval_timeout_secs(lk.approval_timeout_secs)
                .with_inbound_prefix(lk.inbound_prefix)
                .with_per_user_session(lk.per_user_session),
        ))
    }
    #[cfg(not(feature = "channel-lark"))]
    {
        anyhow::bail!("Feishu channel requires the `channel-lark` feature");
    }
}
```

**不引入** master 的 `"lark" | "feishu" =>` 合并 arm + `use_feishu` 字段不对称错误处理。

#### Block B（行 7978-8019，`deliver_announcement` 的 lark/feishu arm）

**保留代码**（kanmars 侧）：

```rust
#[cfg(feature = "channel-lark")]
"lark" => {
    let lk = config.channels.lark.get(alias).ok_or_else(not_configured)?;
    // Q2=B: chain C20 builders so cron delivery honors per-deployment
    // stream_mode / approval_timeout / inbound_prefix / per_user_session.
    // Cron output is one-shot so most of these are no-ops today, but
    // wiring them now means future builder additions don't silently
    // miss this path (Atlas decision per gloria operator request).
    let ch = LarkChannel::from_lark_config(lk)
        .with_streaming(lk.stream_mode, lk.draft_update_interval_ms)
        .with_approval_timeout_secs(lk.approval_timeout_secs)
        .with_inbound_prefix(lk.inbound_prefix)
        .with_per_user_session(lk.per_user_session);
    zeroclaw_api::channel::Channel::send(&ch, &make_msg(&safe_output)).await?;
}
#[cfg(feature = "channel-lark")]
"feishu" => {
    let fs = config.channels.feishu.get(alias).ok_or_else(not_configured)?;
    let ch = LarkChannel::from_feishu_config(fs)
        .with_streaming(fs.stream_mode, fs.draft_update_interval_ms)
        .with_approval_timeout_secs(fs.approval_timeout_secs)
        .with_inbound_prefix(fs.inbound_prefix)
        .with_per_user_session(fs.per_user_session);
    zeroclaw_api::channel::Channel::send(&ch, &make_msg(&safe_output)).await?;
}
#[cfg(not(feature = "channel-lark"))]
"lark" | "feishu" => {
    anyhow::bail!("Lark/Feishu channel requires the `channel-lark` feature");
}
```

**不引入** master 的 `"lark" | "feishu" =>` 合并 arm + `use_feishu` 校验。

#### Block C（行 16407-16498，测试）

**保留代码**（kanmars 侧）：
- `deliver_announcement_routes_feishu_to_feishu_arm`（测拆开 feishu arm）
- `deliver_announcement_routes_lark_to_lark_arm` **(kanmars 版本——测拆开 lark arm，注意与 master 同名但语义不同)**

**不引入**：
- master 的 `deliver_announcement_routes_lark_to_lark_arm`（语义是测合并 arm + use_feishu 校验，与拆开 arm 不兼容）
- master 的 `deliver_announcement_rejects_feishu_value_when_use_feishu_false`（依赖 `use_feishu` 字段不对称语义，kanmars 拆开设计里不需要这个字段语义）

#### 非冲突区 auto-merge audit（§3 Step 8）

需 grep 确认 master 22 个 upstream commit 中下列代码已通过 auto-merge 进入工作树（不是 ours 取本地强制丢弃）：

| commit | 应进入的代码 | 验证 grep |
|---|---|---|
| #6238 `88e8f905b` | `fn expand_tilde_in_path` | `grep -c 'fn expand_tilde_in_path' mod.rs` ≥ 1 |
| #6238 | WeChat `with_persistence(config_arc.clone()).with_workspace_dir(config.data_dir.clone())` builder chain | `grep -c 'with_persistence(config_arc' mod.rs` ≥ 1 |
| #6736 `565deaf33` | `fn channel_strict_non_native_prompt_hides_text_tool_protocol` 测试 | `grep -c 'fn channel_strict_non_native_prompt' mod.rs` ≥ 1 |
| #6009 `f1f57578d` | OTel `gen_ai.tool.*` semantic convention attrs | `grep -rn 'gen_ai.tool' crates/zeroclaw-runtime/src/observability/ \| head -5` |
| #6675 `7437cd0e6` | strict tool parsing mode | `grep -rn 'strict_tool_parsing\|StrictToolParsing' crates/zeroclaw-runtime/src/ \| head -5` |
| #6745 `17891e349` | no-default feature build 编译修复 | `git log --oneline c746998f..master -- crates/zeroclaw-channels/Cargo.toml \| head -5` |

如有任何一项 grep 返回 0，需手工 `git show <commit> -- <file>` 后摘录补回（说明 `--ours` 把它误丢了）。

---

## 3. 执行步骤（原子，逐步可中断）

### Step 0: 创建工作分支

```bash
git checkout kanmars_main
git pull origin kanmars_main   # 确保 166e258dc 是最新
git checkout -b chore/merge-master-into-kanmars-main
```

### Step 1: 触发 merge（生成冲突）

```bash
git merge master --no-commit --no-ff
# 预期输出：
#   Auto-merging Cargo.lock
#   Auto-merging Cargo.toml
#   Auto-merging crates/zeroclaw-channels/Cargo.toml
#   Auto-merging crates/zeroclaw-channels/src/lark.rs
#   CONFLICT (content): Merge conflict in crates/zeroclaw-channels/src/lark.rs
#   Auto-merging crates/zeroclaw-channels/src/orchestrator/mod.rs
#   CONFLICT (content): Merge conflict in crates/zeroclaw-channels/src/orchestrator/mod.rs
#   Auto-merging crates/zeroclaw-config/src/schema.rs
#   Auto-merging crates/zeroclaw-runtime/src/agent/loop_.rs
#   Auto-merging crates/zeroclaw-runtime/src/tools/cron_add.rs
#   CONFLICT (content): Merge conflict in crates/zeroclaw-runtime/src/tools/cron_add.rs
# 工作树停在 conflicted 状态
```

### Step 2: 3 个冲突文件全部 checkout --ours

```bash
git checkout --ours crates/zeroclaw-runtime/src/tools/cron_add.rs
git checkout --ours crates/zeroclaw-channels/src/lark.rs
git checkout --ours crates/zeroclaw-channels/src/orchestrator/mod.rs
git add crates/zeroclaw-runtime/src/tools/cron_add.rs
git add crates/zeroclaw-channels/src/lark.rs
git add crates/zeroclaw-channels/src/orchestrator/mod.rs
git status   # 预期：所有 unmerged 清空
```

### Step 3: 验证 C23 缓解——reaction pool regression-marker 测试存在

```bash
grep -c 'fn lark_inbound_ack_policy_is_glance_only_no_random_pool' crates/zeroclaw-channels/src/lark.rs
# 预期：1
```

### Step 4: 验证 C22 缓解——cron_add.rs enum 含 11-channel 全集

```bash
ENUM_LINE=$(grep '"enum":.*telegram.*discord' crates/zeroclaw-runtime/src/tools/cron_add.rs)
echo "$ENUM_LINE" | grep -c 'dingtalk'   # 预期：1
echo "$ENUM_LINE" | grep -c 'wecom'      # 预期：1
echo "$ENUM_LINE" | grep -c 'lark'       # 预期：1
echo "$ENUM_LINE" | grep -c 'feishu'     # 预期：1
```

### Step 5: 验证 C16 缓解——lark/feishu 拆开 arm + builder chain + Atlas 注释

```bash
grep -c '"lark" =>' crates/zeroclaw-channels/src/orchestrator/mod.rs       # 预期：≥2
grep -c '"feishu" =>' crates/zeroclaw-channels/src/orchestrator/mod.rs     # 预期：≥2
grep -c 'Atlas decision per gloria operator request' crates/zeroclaw-channels/src/orchestrator/mod.rs   # 预期：1
grep -c 'from_lark_config\|from_feishu_config' crates/zeroclaw-channels/src/orchestrator/mod.rs    # 预期：≥4
grep -c 'with_streaming.*with_approval_timeout_secs.*with_inbound_prefix.*with_per_user_session' crates/zeroclaw-channels/src/orchestrator/mod.rs  # 该 grep 模式可能不命中，改用：
grep -cE 'with_streaming|with_approval_timeout_secs|with_inbound_prefix|with_per_user_session' crates/zeroclaw-channels/src/orchestrator/mod.rs   # 预期：≥4
```

### Step 6: 验证 image_resource_url 保留 + image_download_url 缺席

```bash
grep -c 'fn image_resource_url' crates/zeroclaw-channels/src/lark.rs    # 预期：1
grep -c 'fn image_download_url' crates/zeroclaw-channels/src/lark.rs    # 预期：0
```

### Step 7: 验证多语言 reaction pool 仍被删除

```bash
grep -cE 'fn detect_lark_ack_locale|fn lark_ack_pool|fn random_lark_ack_reaction|fn map_locale_tag' crates/zeroclaw-channels/src/lark.rs   # 预期：0
grep -cE 'LARK_ACK_REACTIONS_ZH_CN|LARK_ACK_REACTIONS_EN|enum LarkAckLocale' crates/zeroclaw-channels/src/lark.rs    # 预期：0
```

### Step 8: master 非冲突增量 audit（§2.3 表格）

```bash
# #6238 expand_tilde_in_path
grep -c 'fn expand_tilde_in_path' crates/zeroclaw-channels/src/orchestrator/mod.rs   # 预期：1
# #6736 channel_strict 测试
grep -c 'fn channel_strict_non_native_prompt_hides_text_tool_protocol' crates/zeroclaw-channels/src/orchestrator/mod.rs   # 预期：1
# #6009 OTel gen_ai.tool 属性
grep -rn 'gen_ai\.tool' crates/zeroclaw-runtime/src/ 2>/dev/null | head -5   # 预期：≥1
# #6675 strict tool parsing
grep -rnE 'strict_tool_parsing|StrictToolParsing' crates/zeroclaw-runtime/src/ 2>/dev/null | head -5   # 预期：≥1
# #6238 WeChat with_persistence builder
grep -c 'with_persistence(config_arc' crates/zeroclaw-channels/src/orchestrator/mod.rs   # 预期：≥1
# Cargo.toml 增量
git diff HEAD -- Cargo.toml crates/zeroclaw-channels/Cargo.toml | head -50
```

**如有任一项返回 0**：手工 `git show <commit> -- <file>` 补回（说明 `--ours` 把它误丢，需要从 master 摘录到 kanmars 版本）。

### Step 9: Compile + clippy + test

```bash
cargo fmt --all -- --check         # AC-9
cargo clippy --all-targets -- -D warnings   # AC-10
cargo test --workspace             # AC-11

# 关键测试白名单（必须全 PASS）：
# - lark_inbound_ack_policy_is_glance_only_no_random_pool      ← C23 regression-marker
# - deliver_announcement_routes_lark_to_lark_arm               ← kanmars 拆开 arm 测试
# - deliver_announcement_routes_feishu_to_feishu_arm           ← kanmars 拆开 arm 测试
# - build_approval_card_*                                       ← 审批系统
# - handle_card_action_event_*                                  ← 审批系统
# - update_draft_*                                              ← Draft 流式
# - send_draft_*                                                ← Draft 流式
# - add_reaction_* / remove_reaction_*                          ← Reaction CRUD
# - unicode_to_lark_emoji_type_covers_known_noreply_emojis     ← Emoji 映射
# - inbound_prefix_*                                            ← Inbound 前缀
# - lark_from_feishu_config_*                                   ← from_xx_config
# - lark_per_user_session_*                                     ← per-user session
# - lark_image_resource_url_matches_region                      ← Image API 迁移
# - expand_tilde_in_path_expands_home_prefix                    ← master #6238
# - channel_strict_non_native_prompt_hides_text_tool_protocol  ← master #6736
```

### Step 10: 不 commit，输出 diff 给用户审查

```bash
git status
git diff --cached --stat
git diff --cached crates/zeroclaw-runtime/src/tools/cron_add.rs | head -20
git diff --cached crates/zeroclaw-channels/src/lark.rs | head -100
git diff --cached crates/zeroclaw-channels/src/orchestrator/mod.rs | head -200
# 等用户决策：
#   A. 接受 → git commit （默认 merge commit message）
#   B. 修改 → 在工作树继续改
#   C. 放弃 → git merge --abort
```

---

## 4. 验证标准（AC）

| ID | 标准 | 验证方法 | Step |
|---|---|---|---|
| AC-1 | 3 个冲突文件解决，git status 不含 unmerged | `git status` | Step 2 |
| AC-2 | C23 regression-marker 测试 PASS | `cargo test lark_inbound_ack_policy_is_glance_only_no_random_pool` | Step 9 |
| AC-3 | C22 缓解：11-channel enum 保留含 dingtalk+wecom | Step 4 grep | Step 4 |
| AC-4 | C16 缓解：lark/feishu 拆开 arm 保留 | Step 5 grep | Step 5 |
| AC-5 | C16 缓解：Atlas decision 注释 + 4 个 C20 builder chain 保留 | Step 5 grep | Step 5 |
| AC-6 | image_resource_url 保留，image_download_url 缺席 | Step 6 grep | Step 6 |
| AC-7 | 多语言 reaction pool 仍被删除 | Step 7 grep | Step 7 |
| AC-8 | master 非冲突增量 6 项全部在工作树 | Step 8 grep 全部 ≥ 预期值 | Step 8 |
| AC-9 | `cargo fmt --all -- --check` exit 0 | Step 9 | Step 9 |
| AC-10 | `cargo clippy --all-targets -- -D warnings` exit 0 | Step 9 | Step 9 |
| AC-11 | `cargo test --workspace` 通过（pre-existing failures 明示） | Step 9 | Step 9 |
| AC-12 | merge 未 commit（user 审 diff 后决定） | `git log -1 --oneline` 仍是 `166e258dc init` | Step 10 |

---

## 5. 风险 & 回滚

### 5.1 风险

| 风险 | 触发条件 | 缓解 |
|---|---|---|
| **R1**: `--ours` 把 master 非冲突侧的非冲突改动也一起丢了 | 不可能。git `--ours` 只针对冲突的 hunks 取本地，auto-merged hunks 不受影响（这是 git merge 的明确语义） | §3 Step 8 audit 抽样验证 6 个 master 非冲突增量；若有遗漏走 R1 处理流程 |
| **R2**: `cargo build` 失败 | master 22 commit 中某些 commit 改了 kanmars_main 已删除的函数（如 master 引入对 `image_download_url` 的调用，kanmars 已删该函数） | §3 Step 9 cargo build 兜底；失败则 `git show <failing-commit> -- <file>` 逐个 inspect master 变更点；如确是 master 上调了 kanmars 已删函数，需要在本 plan 之外处理（追加 commit 或在 R1 流程里补） |
| **R3**: cargo test 失败因 master 引入新依赖 | master 22 commit 改了 Cargo.toml / Cargo.lock | 已知 Cargo.toml / Cargo.lock 走 auto-merge；§3 Step 9 cargo test 验证；如失败 `git diff HEAD -- Cargo.toml` 看依赖变化 |
| **R4**: kanmars 自家功能依赖 base 上某个被 master upstream commit 改动的 helper | 低概率，但 OTel #6009 / strict tool #6675 跨文件改动可能影响 kanmars 自家 reaction 调用方 | §3 Step 9 cargo test 兜底；如失败 grep `add_reaction\|remove_reaction\|try_add_ack_reaction` 在 master 是否被 rename 或 signature 变 |
| **R5**: 用户对 §3 Step 10 diff 不满意，需要重新做合并 | 中等 | 不 commit，`git merge --abort` 即可恢复到 `166e258dc init` |
| **R6**: 用户决定 commit 后，merge commit message 不规范 | 低 | 默认 git 生成的 `Merge branch 'master' into chore/merge-master-into-kanmars-main` 已符合 conventional；用户也可手工改成 `chore: merge upstream master into kanmars_main (audit blind-spots C22/C23/C16 mitigated)` |

### 5.2 回滚

```bash
# 场景 1：merge 中途放弃（Step 2-9 任一步）
git merge --abort
# 工作树恢复到 166e258dc init，无残留

# 场景 2：已提交 merge commit 但需回滚
git reset --hard ORIG_HEAD
# 注意：reset --hard 会丢工作树未提交修改，仅在 merge commit 之后立即使用

# 场景 3：已 push 到 origin/chore/merge-master-into-kanmars-main（本 plan 不要求 push）
# 不允许 force-push（AGENTS.md branch rule）。需要新分支重做。
```

---

## 6. Post-merge follow-ups（独立 PR，本 plan 不强制）

### F1: 评估移植 master #6851 wiremock 集成测试 helper

- **范围**：`mount_lark_token_and_send_mocks` + `assert_send_body_matches_recipient_and_text` + `lark_send_via_from_config_emits_post_to_messages_endpoint` + `feishu_send_via_from_config_emits_post_to_messages_endpoint`
- **挑战**：4 个 fixture 基于 master 的 `LarkChannel::from_config(lk, alias, peer_resolver)` 单一接口；kanmars 是 `from_lark_config(lk)` + `from_feishu_config(fs)` 拆开接口
- **工作量**：重写 helper signature 适配拆开 arm，新增 2 个并行测试（lark-side + feishu-side）
- **建议 PR 标题**：`test(lark): port upstream #6851 wiremock helpers to from_xx_config split-arm design`

### F2: 评估 dingtalk / wecom 作为 cron delivery target 的实际可用性

- **当前状态**：`mod.rs` 已实现（5178 / 5205 行），`cron_add.rs` enum 已含
- **缺口**：没有 cron delivery 端到端集成测试覆盖这两个 channel
- **建议 PR 标题**：`test(channels): add cron delivery integration tests for dingtalk/wecom announce`

### F3: 更新 boulder.json 记录本次 merge 命中的 audit blind spots

- **范围**：在 `.sisyphus/boulder.json` 的 `audit_blind_spots_recorded` 数组中追加本次命中记录：
  - C22 (cron_add.rs enum schema vs mod.rs 实现 drift)
  - C23 (reaction pool fork-only DELETION 在 merge 中被 upstream 带回)
  - C16 (deliver_announcement lark/feishu arm 内部 #6851 vs Q2=B 互斥)
- **建议 PR 标题**：`chore(boulder): record C22/C23/C16 hits in audit_blind_spots from kanmars.req.20260525.001 merge`

---

## 7. Plan 决策记录

| 决策 ID | 选项 | 选择 | 理由 |
|---|---|---|---|
| **D1**: 合并方向 | A. 以 `kanmars_main` 为基底 + `--ours` / B. 以 master 为基底 + `--theirs` + 手工补回 fork 代码 | **A** | kanmars 有 6 个 req plan + 2 个 fix commit (`1f5cb56d2` / `7dcb38f77`) 的生产决策代码；master 是纯 upstream 状态，无 fork 内容。A 方案保留量大于丢失量 10:1 |
| **D2**: 是否恢复多语言 reaction pool | A. 恢复 / B. 保持删除 | **B** | `1f5cb56d2` 显式删除并加 regression-marker `lark_inbound_ack_policy_is_glance_only_no_random_pool`；gloria operator 投诉哈士奇 sticker；commit message 明示 "will be flagged in code review if any of LARK_ACK_REACTIONS_* ever come back" |
| **D3**: lark/feishu arm 设计 | A. 合并 arm (master `"lark" \| "feishu" =>`) / B. 拆开 arm + builder chain (kanmars `"lark" =>` + `"feishu" =>`) | **B** | Q2=B 决策来源 `.sisyphus/notepads/kanmars.req.20260512.001.plan/issues.md` + commit `7dcb38f77` 修真实生产 bug（gloria cron delivery "飞书渠道不支持定时推送"）+ 防御未来 outbound builder 扩展（"future-proof against new outbound-affecting builder fields landing without the cron path getting them"） |
| **D4**: Image API | A. `image_download_url(image_key)` / B. `image_resource_url(message_id, image_key)` | **B** | 与同文件 `file_download_url(message_id, file_key)` 模式对称（都用 `/im/v1/messages/{id}/resources/{key}`）；飞书消息资源 API 在权限场景下比独立 image_key 下载更稳 |
| **D5**: `cron_add.rs` enum | A. 9-channel (master) / B. 11-channel (kanmars 含 dingtalk + wecom) | **B** | master 的 `mod.rs` 5178 / 5205 实际有 dingtalk/wecom 实现，cron schema 应一致暴露给 LLM；C22 盲区缓解 |
| **D6**: 是否 commit | A. 自动 commit / B. 不 commit，输出 diff 给用户审 | **B** | 用户在原始对话明确要求"先告诉我差异，我确认之后再修改" |
| **D7**: 目标 PR | A. 现在创 PR / B. 不创 PR | **B** | 这是本地 merge 操作；PR 由用户在 §3 Step 10 审完 diff 后自主决定（如要 PR 则走 `feature/chore-merge-master-...` → `kanmars_main` 流程） |
| **D8**: master #6851 wiremock 集成测试 | A. 本 merge 同时移植适配 / B. 列为 §6 F1 follow-up | **B** | 适配需重写 helper signature 解决拆开 arm vs `from_config` 接口分歧；属于独立工程改造，违反 §0.5 第 11 条 "One concern per merge" |
| **D9**: master 22 个 upstream commit 中是否选择性丢弃某些 | A. 选择性丢弃（如 OTel）/ B. 全部接受（auto-merge） | **B** | 全部 22 个都是 upstream 已 release 的 bugfix/feature，无理由选择性丢；选择性丢会偏离 §0 "把 22 个 upstream commit 合进 kanmars_main" 的关键目标 |

---

修订日期：2026-05-25 (rev0 — 初稿，待用户审 §2 文件级合并策略 + §3 执行步骤 Step 0-10 + §4 AC + §6 follow-up)
