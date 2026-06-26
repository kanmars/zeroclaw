# Plan — kanmars.req.20260516.001 (Strip Image Markers from Reply-Intent Precheck)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260516.001.plan |
| 关联需求 | 用户对话需求（2026-05-16）：『刚才下载图片并大模型理解成功之后，重新发起其他的语句，看请求日志报文，发现报文长度特别长，把刚才的图片内容也加到对话历史中发送给大模型了』 |
| 起草日期 | 2026-05-16 |
| 修订日期 | 2026-05-16 (rev0 — 初稿) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `fix/orchestrator-strip-image-markers-from-precheck` |
| 风险等级 | **Low**（仅 `zeroclaw-channels::orchestrator::classify_channel_reply_intent` 一处剥离 + `zeroclaw-memory::strip_media_markers` 暴露为 `pub(crate)` → 跨 crate 改 `pub`，无 trait/schema/边界变化） |
| 基线 commit | `f384bd86` (master HEAD, 2026-05-15) |
| 当前工作分支 | `fix/lark-image-download-restore` HEAD `6436a991`（综合修复合并提交） — **本 plan 在该分支基础上派生新分支** |
| 选型方案 | **Proposal A — 仅在 reply-intent precheck 处剥离 marker**（不改 main agent 路径）。详见 §1.4 |
| 预计代码行数 | +12 / -3（含 1 个新单测） |
| 预计工作量 | 约 25 分钟 |

---

## 0. 关键目标（唯一真理来源）

> **让飞书/Lark 的 reply-intent precheck（Decide whether the assistant should send any visible reply...）调用 LLM 时，不再在 user content 里携带任何历史 `[IMAGE:data:base64,...]` marker。precheck 是 3 分类（REPLY / SKIP / REACT）路由器，根本不需要看图。**

**完成此目标即"功能完成"**：

- 复现场景：用户在飞书发了一张图（被 vision provider 成功理解）→ 紧接着发一句无关文字（如"今天期货新闻"）→ 检查 zeroclaw 日志中下一次发给 provider 的 `DEBUG_CHAT request body` 不再含 `image_url` part 或 base64 data URI（即 `body_bytes` 从 ~242 KB 跌到 ~30 KB 以内）；
- 主 agent 路径行为**不变**（`prepare_messages_for_provider` 仍按现状传图，本 PR 不动主路径）；
- `consolidation::strip_media_markers` 行为**不变**（已用于 memory consolidation 提示，本 PR 仅把它对外暴露 `pub(crate)` 或 `pub`）；
- 现有 `zeroclaw-channels` / `zeroclaw-memory` 测试全部通过；
- 不动 `zeroclaw-api` / `zeroclaw-runtime` / `zeroclaw-providers` / `zeroclaw-config`。

**显式不在范围内**：

- ❌ Proposal B（一并剥离主 agent 路径历史 marker，仅保留最末轮）—— 涉及 vision 多轮追问 UX，独立 RFC
- ❌ Proposal C（新增 `[multimodal] precheck.include_images` 配置开关）—— AGENTS.md 反对"speculative config flags"
- ❌ ContextCompressor 的 marker 剥离逻辑增强（已经会剥离，本 PR 不动）
- ❌ 修改 `multimodal::trim_old_images` 语义 → 独立 RFC
- ❌ 全渠道审计（telegram / slack / discord 等同样可能有问题）—— 调研显示同代码路径，但本 PR 仅修 orchestrator 一处即对全部渠道生效，无需逐渠道动
- ❌ 任何 OpenSpec 提案 —— 改动 ~12 行，按 ZeroClaw `AGENTS.md` 规则直接小 PR

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不新增 `unwrap()` / `expect()`**（[AGENTS.md Anti-Pattern](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md)）。`strip_media_markers` 内部 `LazyLock` 已经 `unwrap()` 但那是项目既有代码，不视作本 PR 引入。
2. **不新增 `#[allow(dead_code)]`**。
3. **不动 `zeroclaw-api` 公共 trait**。
4. **不动 `zeroclaw-config` schema**。
5. **`tracing::` 日志保持英文**（RFC #5653 §4.6）。本 PR 不新增 log。
6. **复用既有基础设施**：`zeroclaw-memory::consolidation::strip_media_markers`（已有 regex `\[(?:IMAGE|DOCUMENT|FILE|VIDEO|VOICE|AUDIO):[^\]]*\]` → `[media attachment]`），仅暴露 visibility，不复制实现。
7. **不引入新依赖**。
8. **完整跑 `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test -p zeroclaw-channels -p zeroclaw-memory`**。
9. **按 [zeroclaw AGENTS.md "Workflow"](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#workflow) 5 步流程**：从 master 拉 `fix/orchestrator-strip-image-markers-from-precheck` → 改 → commit → push → 发 CR 等用户确认。
10. **One concern per PR**：本 PR 一个关注点 = 一个 user concern（precheck 不应携带历史图）。不与昨天的 lark image fix / approval card fix 混合。

---

## 1. 现状事实复核（基于 2026-05-16 实地代码读取）

### 1.1 关键代码位置（行号对齐基线 `f384bd86`）

| 事实 | 文件:行 |
|---|---|
| `classify_channel_reply_intent` 函数定义 | [orchestrator/mod.rs:2211](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2211) |
| `convo: String` 拼接初始化 | [orchestrator/mod.rs:2218](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2218) |
| **漏点：原始 `msg.content` 直接 writeln 进 convo** | [orchestrator/mod.rs:2241](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2241) |
| 既有 marker 剥离守卫（仅在 `!supports_vision` 时触发） | [orchestrator/mod.rs:2886](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2886) |
| `strip_media_markers` regex helper | [zeroclaw-memory/src/consolidation.rs:47](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-memory/src/consolidation.rs#L47) |
| `zeroclaw-memory` workspace 依赖在 channels | [zeroclaw-channels/Cargo.toml](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/Cargo.toml) (`zeroclaw-memory.workspace = true`) |
| `parse_image_markers` （备选实现源） | [zeroclaw-providers/src/multimodal.rs:95](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/multimodal.rs#L95) |

### 1.2 用户实测日志（铁证）

来自 `/home/admin/workspace-public/kanmars/gloria/A/logs/zeroclaw.log`（2026-05-16）：

| 行 | 时间 | 类型 | image_url 大小 | 用户当前发的 |
|---|---|---|---|---|
| L47 | 01:04:45 | precheck #1 | **214 KB** ✅ 应该带 | 第一次发图 |
| L65 | 01:05:22 | memory consolidation | 0 ✅ 已被 `strip_media_markers` 替换为 `[media attachment]` | n/a |
| **L74** | **01:05:44** | **precheck #2** | **214 KB ❌ 不该带** | 第二次问别的（无关图片） |
| L87 | 01:06:34 | 主 agent reply | 0 ✅ | 期货新闻 |

L47 和 L74 的 image_url payload 前缀完全相同（`'data:image/jpeg;base64,/9j/4AAQSkZJRgABA...'`）—— 同一张图被重复编码、重复发送。

### 1.3 根因结论

`classify_channel_reply_intent` 把 `history` 里**每个 `ChatMessage` 的原始 `content` 字段**（其中含 `[IMAGE:data:image/jpeg;base64,...]` marker）拼接进 `convo: String`。然后通过 `Provider::chat_with_system(convo)` 提交给 OpenAI 兼容 provider，[`compatible.rs::to_message_content`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L1550) 在 `convo` 整串上跑 `parse_image_markers` 把每个 marker 升格为 `image_url` part。

[L2886](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2886) 的"历史 marker 剥离"块**仅在 `!active_provider.supports_vision()` 时执行**。qwen-3.6-plus 支持 vision → 守卫不触发 → 历史 marker 全保留 → 每次 precheck 都把所有历史图重新 base64 发一次。

### 1.4 替代方案弃选理由

| 方案 | 描述 | 弃选原因 |
|---|---|---|
| **A（本计划）** | 仅在 `classify_channel_reply_intent` 拼接 convo 时跑 `strip_media_markers` | ⭐ 改动最小，0 风险（precheck 不需要图），立刻生效 |
| B | 把 [L2886](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2886) 的 `!supports_vision()` 守卫去掉，让所有 provider 历史都剥离 marker（仅保留最末轮）| 影响主 agent 多轮 vision UX（追问图片需重发）；单独 RFC 讨论 |
| C | 新增 `[multimodal] precheck.include_images: bool` 配置开关 | AGENTS.md 反 "speculative config flag" |
| D | 把 marker 剥离推到 `to_message_content`（compatible.rs）一层，按"是否在 precheck context"判断 | 跨 crate 改造 + 需要 context 透传，体量超出 user concern |

---

## 2. 目标 (Goals) & 验收标准 (Acceptance Criteria)

### G1 — `classify_channel_reply_intent` 不再传图给 LLM（P0，核心）

- **AC-1.1** 在 [orchestrator/mod.rs:2240](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2240) 的循环体内，`writeln!(convo, "[{role}] {}", msg.content)` 改为先用 `strip_media_markers(&msg.content)` 再 writeln。
- **AC-1.2** 用户实测日志中：发完图之后下一条无关消息触发的 `DEBUG_CHAT request body` 大小 < 50 KB（之前是 242 KB），且 `body` 中不再含 `data:image/`。
- **AC-1.3** 历史中第一次包含 marker 的那次 precheck（即用户**真在发图的那一轮**）也被剥离 —— 但因为 precheck 是 3 分类路由不需要看图，丢失视觉信息**不影响 REPLY/SKIP/REACT 决策**。这是预期行为。

### G2 — `strip_media_markers` 对外暴露最小可见性（P0，配合 G1）

- **AC-2.1** [memory/src/consolidation.rs:47](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-memory/src/consolidation.rs#L47) 的 `fn strip_media_markers` 改为 `pub fn strip_media_markers`（最小化暴露：channels crate 是同 workspace 的兄弟 crate，没有更细的可见性可用）。
- **AC-2.2** 不改函数签名 / 行为 / regex。

### G3 — 单测覆盖

- **AC-3.1** 在 `orchestrator/mod.rs` 的 tests 模块（如有；否则新建 `#[cfg(test)] mod precheck_tests`）添加 1 个单测：构造 history 含 `[IMAGE:data:image/png;base64,FAKE]`，调用一个最小重构出的 helper（或直接 inline 测 `strip_media_markers` 在 convo 拼接里的应用），断言结果不含 `IMAGE:` / `base64`。
  - **如果 inline 测不便**（`classify_channel_reply_intent` 是 `async fn` 且依赖 provider，难以单测）→ 退化为：`strip_media_markers` 已有公共测试覆盖（在 `consolidation.rs` 测试里），仅在 channels 加个 `assert!(strip_media_markers("...[IMAGE:data:...]...").contains("[media attachment]"))` 的烟测验证 visibility 改对了。

### G4 — 零侵入其他渠道 & 主 agent 路径

- **AC-4.1** 不动 telegram / discord / matrix / dingtalk / slack / email / lark / 任一 channel 实现（修复在 orchestrator 共享逻辑，自动覆盖所有渠道）。
- **AC-4.2** 不动 `prepare_messages_for_provider` 主 agent 调用路径。
- **AC-4.3** 不动 [orchestrator/mod.rs:2886](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2886) 的既有 `!supports_vision()` 守卫（保留向后兼容）。

### G5 — 静态检查 + 测试全绿

- **AC-5.1** `cargo fmt --all -- --check` exit 0
- **AC-5.2** `cargo clippy -p zeroclaw-channels -p zeroclaw-memory --all-targets -- -D warnings` exit 0
- **AC-5.3** `cargo test -p zeroclaw-channels -p zeroclaw-memory` 全绿
- **AC-5.4** LSP diagnostics on `orchestrator/mod.rs` + `consolidation.rs` 0 error

---

## 3. 实施步骤（按提交顺序）

### Step 1 — 环境与分支准备（5 min）

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
# 当前在 fix/lark-image-download-restore（含两个综合修复）
# 本 fix 与 image / approval 修复完全独立，应基于 master 起新分支
git checkout master
git pull --ff-only origin master                       # 应仍是 f384bd86 或更新
git checkout -b fix/orchestrator-strip-image-markers-from-precheck
```

**验证**：`git branch --show-current` = `fix/orchestrator-strip-image-markers-from-precheck`；`git status` 干净。

### Step 2 — 暴露 `strip_media_markers`（2 min）

文件：`crates/zeroclaw-memory/src/consolidation.rs:47`

```rust
// Before:
fn strip_media_markers(text: &str) -> String {

// After:
pub fn strip_media_markers(text: &str) -> String {
```

**验证**：`grep "pub fn strip_media_markers" crates/zeroclaw-memory/src/consolidation.rs` 命中 1 行。

### Step 3 — 在 precheck convo 拼接处剥离 marker（5 min）

文件：`crates/zeroclaw-channels/src/orchestrator/mod.rs`

#### 3a. 在 use 块加 import

找到该文件顶部的 use 块（约 1-50 行），加上：
```rust
use zeroclaw_memory::consolidation::strip_media_markers;
```
（位置：跟其他 `use zeroclaw_memory::...` 同组；如果没有就独立一行加在 zeroclaw-* crate 区域）

#### 3b. 改 [L2241](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2241) 拼接逻辑

```rust
// Before (L2235-2242):
for msg in history.iter().filter(|m| m.role != "system") {
    let role = match msg.role.as_str() {
        "assistant" => "assistant",
        _ => "user",
    };
    let _ = writeln!(convo, "[{role}] {}", msg.content);
}

// After:
for msg in history.iter().filter(|m| m.role != "system") {
    let role = match msg.role.as_str() {
        "assistant" => "assistant",
        _ => "user",
    };
    // Strip [IMAGE:data:...] / [DOCUMENT:...] / [FILE:...] / [VIDEO:...] /
    // [VOICE:...] / [AUDIO:...] markers before sending to LLM.
    // The reply-intent precheck is a 3-class router (REPLY/SKIP/REACT) and
    // never needs raw media bytes; without this strip every base64 image
    // in history gets re-sent on every inbound message (~200 KB × N turns,
    // ~1600 image tokens × N).
    let scrubbed = strip_media_markers(&msg.content);
    let _ = writeln!(convo, "[{role}] {scrubbed}");
}
```

**验证**：
- `grep -n "strip_media_markers" crates/zeroclaw-channels/src/orchestrator/mod.rs` 命中 use + 调用 = 2 行
- LSP diagnostics 0 error

### Step 4 — 加单测（10 min）

#### 4a. 在 `orchestrator/mod.rs` 的 `#[cfg(test)] mod tests` 末尾加（如果不存在 mod 则新建）：

```rust
#[test]
fn precheck_strip_media_markers_is_wired() {
    // Smoke test: ensure strip_media_markers is reachable from orchestrator
    // and that it actually strips [IMAGE:data:base64,...] payloads.
    let raw = "Hello [IMAGE:data:image/png;base64,iVBORw0KGgoAAA] world";
    let cleaned = strip_media_markers(raw);
    assert!(!cleaned.contains("IMAGE:"), "expected no IMAGE: marker, got {cleaned}");
    assert!(!cleaned.contains("base64"), "expected no base64 payload, got {cleaned}");
    assert!(cleaned.contains("[media attachment]"), "expected placeholder, got {cleaned}");
}
```

**验证**：`cargo test -p zeroclaw-channels precheck_strip_media_markers_is_wired` 通过。

### Step 5 — 静态检查（5 min）

```bash
cargo fmt --all -- --check
cargo clippy -p zeroclaw-channels -p zeroclaw-memory --all-targets -- -D warnings
cargo test -p zeroclaw-channels -p zeroclaw-memory
```

**验证**：三条全部 exit 0。

### Step 6 — Atomic commit + push + CR（5 min）

```bash
git add crates/zeroclaw-memory/src/consolidation.rs \
        crates/zeroclaw-channels/src/orchestrator/mod.rs
git status --short                                    # 应仅 2 个文件
git diff --stat HEAD                                  # 预期 +12 / -3
git commit -F - <<'EOF'
fix(channels): strip media markers from reply-intent precheck convo

The reply-intent precheck (`classify_channel_reply_intent` in
orchestrator/mod.rs) feeds raw `ChatMessage.content` strings — including
historical `[IMAGE:data:image/...;base64,...]` markers — into a flat
`convo: String` that is sent to the OpenAI-compatible provider. The
provider's `to_message_content` then re-parses the markers and emits one
`image_url` content part for every historical image.

Result: every inbound message after a Feishu/Lark image upload re-sends
the full ~200 KB base64 payload (~1600 image_tokens) on each precheck,
even when the new message is unrelated text ("today's futures news").

Fix: scrub each turn's content through `consolidation::strip_media_markers`
(which already replaces media markers with `[media attachment]`) before
appending to convo. Precheck is a 3-class router (REPLY/SKIP/REACT) and
never needs raw media bytes; main-agent path is unchanged.

The previously-existing strip block at orchestrator/mod.rs:2886 is gated
on `!provider.supports_vision()` and therefore inactive for vision-capable
providers like qwen3.6-plus. This PR closes the gap for that case.

Side effects:
- Saves ~200 KB body + ~1600 image_tokens per precheck call when image is
  in history.
- Exposes `consolidation::strip_media_markers` as `pub` (was private).
EOF
git push -u origin fix/orchestrator-strip-image-markers-from-precheck
```

**验证**：push 成功，发 CR 链接给用户。

### Step 7 — Optional CHANGELOG-next.md entry

```markdown
### Fixed

- **Channels orchestrator**: Reply-intent precheck no longer re-sends
  historical media (image/audio/video) base64 payloads to the LLM on every
  inbound message. Saves ~200 KB request body and ~1600 image tokens per
  call after a Feishu/Lark image upload.
```

---

## 4. 验证清单（PR 提交前必须全绿）

| 项 | 命令 | 预期 |
|---|---|---|
| 格式 | `cargo fmt --all -- --check` | exit 0 |
| Lint | `cargo clippy -p zeroclaw-channels -p zeroclaw-memory --all-targets -- -D warnings` | exit 0 |
| 单元测试 | `cargo test -p zeroclaw-channels -p zeroclaw-memory` | 全绿 |
| 新单测 | `cargo test -p zeroclaw-channels precheck_strip_media_markers_is_wired` | 通过 |
| LSP diagnostics | `lsp_diagnostics` on `orchestrator/mod.rs` + `consolidation.rs` | 0 error |
| Grep 验证 use | `grep -n "use zeroclaw_memory::consolidation::strip_media_markers" crates/zeroclaw-channels/src/orchestrator/mod.rs` | 1 行 |
| Grep 验证调用 | `grep -nE "let scrubbed = strip_media_markers|strip_media_markers\(&msg.content\)" crates/zeroclaw-channels/src/orchestrator/mod.rs` | ≥ 1 行 |
| Grep 验证 visibility | `grep -n "pub fn strip_media_markers" crates/zeroclaw-memory/src/consolidation.rs` | 1 行 |
| 改动文件数 | `git diff --stat HEAD~1` | 仅 2 个文件 |

**线上回归验证**（PR merge + 部署后用户实测）：
- 发图给 bot，bot 成功理解；
- 紧接着发一条无关文字（如"今天天气如何"）；
- 检查 zeroclaw 日志 `DEBUG_CHAT request body bytes=` 不再是 ~242 KB；
- 检查响应 `prompt_tokens_details.image_tokens` 缺失或为 0。

---

## 5. 风险与缓解

| 风险 | 严重性 | 缓解 |
|---|---|---|
| `pub fn strip_media_markers` 暴露给整个 workspace（更广的可见性） | 极低 | 这是个纯 regex helper，无副作用；`pub(crate)` 不可用因为是跨 crate 调用；最小化原则下 `pub` 是合理选择 |
| 用户在飞书发图时第一轮 precheck 也被剥离图 → 模型看不到图判断"是否回复" | 低 | precheck 是 3 分类（REPLY/SKIP/REACT），只用 text 上下文 + 是否 @bot / DM 等元信号；图与该决策无关。生产数据无证据表明图影响 REPLY 决策。 |
| 主 agent 路径仍会重传同图（context 还会被另一处膨胀） | 低 | 这是已知的 [L2886 守卫](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2886) 限制（仅 vision-capable 时未启用 strip）；本 PR 仅修 precheck 这条**最高频**的路径（每条入站消息 1 次）。主 agent 路径每"会话轮次"才 1 次，影响小且涉及 vision UX 设计选择 → 留给独立 RFC。 |
| `strip_media_markers` regex 漏匹配某些 marker 变体 | 极低 | regex `\[(?:IMAGE|DOCUMENT|FILE|VIDEO|VOICE|AUDIO):[^\]]*\]` 已在 production memory consolidation 路径上跑了数月，覆盖了项目中所有 channel 用过的 marker 类型 |
| `convo` 中 `[media attachment]` 占位文本本身被 LLM 误解为指令 | 极低 | precheck 系统提示已经是稳定的 3 分类提示词，不会因为多一个 `[media attachment]` 占位改变行为；和 memory consolidation 共用同一占位文本，已有先例 |

---

## 6. Rollback Plan

如 PR merge 后线上出现问题：

1. **优先**：`git revert <commit-sha>` 在 master 上发回滚 PR
2. 如果只是 visibility 问题（`pub` 引发不该有的依赖），改回 `pub(crate)` + 在 channels 复制 6 行 regex —— 仍不动 precheck 行为
3. 如果 precheck 决策质量下降（罕见）：可 hotfix 改回原始 `msg.content` 透传 + 加 `strip_media_markers` 仅作用于"非最后一条"消息（保留当前轮图，剥离历史图）

---

## 7. 后续（不在本 PR 内，单独 issue）

| Follow-up | 优先级 | 描述 |
|---|---|---|
| 主 agent 路径多轮历史 marker 剥离（Proposal B） | 🟡 P1 | 去掉 [L2886](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2886) 的 `!supports_vision()` 守卫；保留最末轮图，剥离历史图。需要 RFC 讨论 vision 多轮追问 UX |
| `multimodal::trim_old_images` 语义改进 | 🟢 P2 | 当前是"超过 max_images 才裁"，可考虑"始终只保留最近 1 张图" |
| 全渠道 precheck token 监控 | 🟢 P2 | 加 `tracing::debug!` 记录 precheck convo 长度 + image_tokens，便于回归监控 |

---

## 7.5 Boulder task checklist（continuation hook 可数）

- [ ] Step 1: 建分支 `fix/orchestrator-strip-image-markers-from-precheck`
- [ ] Step 2: `consolidation.rs` 暴露 `pub fn strip_media_markers`
- [ ] Step 3a: orchestrator/mod.rs use 块加 `use zeroclaw_memory::consolidation::strip_media_markers`
- [ ] Step 3b: orchestrator/mod.rs L2241 改用 `strip_media_markers(&msg.content)`
- [ ] Step 4: 加 `precheck_strip_media_markers_is_wired` 单测
- [ ] Step 5: cargo fmt --check + clippy + test 全绿
- [ ] Step 6a: atomic commit
- [ ] Step 6b: push 到 origin（可能需要用户接力）
- [ ] Step 7: CHANGELOG-next.md 加 1 条 Fixed 条目

---

## 8. Sign-off

| 角色 | 状态 |
|---|---|
| 起草人（组件管理员）| ✅ 已完成（rev0） |
| 计划审阅人（Momus Plan Critic）| ⏳ 待审阅 |
| 计划审阅人（用户）| ⏳ 待审阅 |
| 实施授权 | ⏳ 待用户明确 "execute" / "go" / "开始改" 才会动代码 |

**当前模式**：plan 起草完毕，等用户审阅后授权实施。

**注意：本 PR 与 `fix/lark-image-download-restore`（昨天的综合修复 merge commit `6436a991`）独立，应基于 master 起新分支。两个 PR 可并行 review，无序合并。**
