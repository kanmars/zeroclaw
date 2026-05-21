# Plan — kanmars.req.20260516.002 (Implement `LarkChannel::remove_reaction` to clear stuck 👀 ack reaction)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260516.002.plan |
| 关联需求 | 用户对话需求（2026-05-16 凌晨 02:02）：『发一条飞书消息，bot 回复了 3 个表情（鼓掌 / 哈士奇 / DONE）一直留在消息上没被清理，期望"处理中"那个表情在完成后被自动删除』 |
| 起草日期 | 2026-05-16 |
| 修订日期 | 2026-05-16 (rev0 — 初稿) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `fix/lark-implement-remove-reaction` |
| 风险等级 | **Low**（仅在 `LarkChannel` 内部新增方法 + 一个 in-memory 缓存字段；不动 trait / schema / 边界；本 crate `Experimental` tier） |
| 基线 commit | `cc98b8ce` (master HEAD, 2026-05-16) |
| 当前工作分支 | `master` HEAD `cc98b8ce`（与运行中的 gloria binary 同 commit） |
| 选型方案 | **方案 A — POST 时缓存 reaction_id**。详见 §2.1 |
| 预计代码行数 | +180 / -5（含 3 个 wiremock 单测） |
| 预计工作量 | 约 90 分钟 |

---

## 0. 关键目标（唯一真理来源）

> **让 `LarkChannel::remove_reaction(message_id, emoji)` 真正调用飞书 `DELETE /im/v1/messages/{id}/reactions/{reaction_id}` 端点删除 bot 自己加过的反应；最直接的可观察效果：orchestrator 在 LLM 完成后调用 `remove_reaction("👀")` 能让飞书消息上的"处理中"表情真正消失，使最终用户在飞书消息上只看到 ① ack 表情（鼓掌）+ ③ DONE 两个表情，而不是当前的三个（鼓掌 / 哈士奇 / DONE）。**

**完成此目标即"功能完成"**：

- 用户在飞书发任意一条消息 → bot 回复完成后，飞书消息上的反应表情数 = 2（① ack + ③ DONE），而非 3
- 现有 [`mod.rs:3179-3186`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3179-L3186) 添加 👀 / [`mod.rs:3798-3803`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3798-L3803) 移除 👀 + 添加 ✅ 的调用顺序**完全不变**
- 现有 `Channel::add_reaction` / `try_add_ack_reaction` / `random_lark_ack_reaction` 行为**完全不变**（继续把 ack 表情写到飞书）
- 删除调用任意失败路径（缓存 miss / 401 / 404 / 飞书错误码 231007/231011）一律 `tracing::warn!` 或 `tracing::debug!` 软失败，**绝不影响主对话流程**
- 已开通的飞书机器人权限（`im:message` 或 `im:message.reactions:write_only`，二选一即可）够用，**不需要用户去后台改权限**
- Feishu (`open.feishu.cn`) 与 Lark (`open.larksuite.com`) 行为对称（共用 `LarkChannel` 实现，端点路径完全一致）

**显式不在范围内**：

- ❌ **不删除 ① ack 表情**（鼓掌等）—— 这是 [`lark.rs:1249`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1249) `random_lark_ack_reaction` 主动加的"通道层 ack"，是 feature 不是 bug；用户只抱怨"哈士奇为什么没消失"，没抱怨第一个表情
- ❌ **不调整 [`LARK_ACK_REACTIONS_ZH_CN`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L19-L21) 池**（不收窄到只有 `OK`）—— 这是独立的 UX 决策，不解决 stuck reaction
- ❌ **不引入 fallback 路径"GET list 找 reaction_id 再 DELETE"**（方案 B）—— 见 §2.1 弃选理由
- ❌ **不动 [`unicode_to_lark_emoji_type`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L58)**（`👀 → "GLANCE"` 映射保持原样；"哈士奇是 GLANCE 的飞书内置渲染"是飞书美术问题，本 PR 不解决）
- ❌ **不改 orchestrator** —— `mod.rs:3798-3800` 调用 `remove_reaction("👀")` 已经是正确写法，问题在 lark 实现侧
- ❌ **不动 `Channel` trait**（`zeroclaw-api/src/channel.rs`）—— 仅实现已有 trait 方法，签名一致
- ❌ **不动其他 channel 的 `remove_reaction`**（telegram / discord / matrix / slack 等独立 PR；本 PR `One concern per PR`）
- ❌ **不加新配置项**（行为对所有 Feishu/Lark 用户一致打开，无需 opt-in）
- ❌ **不加 reaction_id 持久化** —— 进程重启后缓存丢失是 acceptable degradation（旧消息上的 stuck reaction 已经过去了，用户也不会回头看）

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不新增 `unwrap()` / `expect()`**（[zeroclaw AGENTS.md Anti-Patterns](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#anti-patterns)）。`response.pointer("/data/reaction_id")` 用 `.and_then(|v| v.as_str()).map(str::to_string)` 安全提取
2. **不新增 `#[allow(dead_code)]`**。新字段 `reaction_ids` 立即被 `add_reaction` + `try_add_ack_reaction` + `remove_reaction` 三处调用
3. **不动 `zeroclaw-api`**。改动边界 = **仅** `crates/zeroclaw-channels/src/lark.rs` 一个文件
4. **`tracing::` 日志保持英文 + 稳定 `error_key` 风格**（RFC #5653 §4.6）。新增日志统一 `Lark: remove_reaction …` 前缀，与既有 `Lark: add reaction …` / `Lark: approval card PATCH …`（[lark.rs:877](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L877) / [lark.rs:856](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L856)）保持一致风格
5. **复用现有 HTTP 通道**：[`http_client()`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L759)、[`get_tenant_access_token`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1300)、[`invalidate_token`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1356)、[`message_reaction_url`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L798)、[`api_base`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs) 全部复用，**不引入新 HTTP wrapper**
6. **不引入新依赖**。`uuid` / `serde_json` / `tokio` / `tracing` / `anyhow` 全已存在
7. **完整跑** `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test -p zeroclaw-channels`（按需 `./dev/ci.sh all`）
8. **按 [zeroclaw AGENTS.md "Workflow"](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#workflow) 5 步流程**：从 master 拉 `fix/lark-implement-remove-reaction` → 改 → commit → push → 发 CR 等用户确认（**非 master 分支开 PR**，不直推 master）
9. **不触碰 `zeroclaw-runtime`**（transitional crate 边界）。本 PR 完全在 `zeroclaw-channels` 内，合规
10. **CHANGELOG-next.md 必须加一行**（`zeroclaw-channels` Experimental tier 行为变更，符合 RFC 行为变更披露要求）

---

## 1. 现状事实复核（基于 2026-05-16 实地代码读取，行号对齐基线 `cc98b8ce`）

### 1.1 关键代码位置

| 事实 | 文件:行 |
|---|---|
| `Channel::remove_reaction` trait 默认实现（no-op `Ok(())`） | [zeroclaw-api/src/channel.rs:208-215](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-api/src/channel.rs#L208-L215) |
| `impl Channel for LarkChannel` 块开始 | [lark.rs:2136](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2136) |
| **`LarkChannel::add_reaction` 完整实现**（待加缓存写入） | [lark.rs:2185-2241](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2185-L2241) |
| **`LarkChannel::remove_reaction` 缺失** —— 走 trait 默认 no-op | （当前根本没实现） |
| `try_add_ack_reaction`（待加缓存写入） | [lark.rs:856-924](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L856) |
| `post_message_reaction_with_token`（POST 反应底层 HTTP 调用） | [lark.rs:829-852](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L829-L852) |
| `message_reaction_url`（POST 端点 URL builder，待复用 + 加 reaction_id 删除变体） | [lark.rs:798-800](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L798) |
| `unicode_to_lark_emoji_type`（emoji 映射，本 PR 仅引用） | [lark.rs:58-70](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L58) |
| `LarkChannel` struct 定义（待加缓存字段） | [lark.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs) （`pending_approvals` 同款 `Arc<Mutex<HashMap>>` 结构作模板） |
| `LarkChannel` 构造点（`pending_approvals: Arc::new(...)` 一行） | [lark.rs:634](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L634)（grep 命中唯一一处） |
| Orchestrator 调用 `add_reaction("👀")` | [orchestrator/mod.rs:3179-3186](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3179-L3186) |
| Orchestrator 调用 `remove_reaction("👀")` + `add_reaction(✅/⚠️)` | [orchestrator/mod.rs:3794-3804](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3794-L3804) |
| 现有 wiremock 测试模式（仿写参考） | [lark.rs:4244-4280](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4244)（reaction url tests）+ [lark.rs:4750-4858](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4750)（draft PATCH wiremock 模式） |

### 1.2 用户实测证据（铁证）

来自用户现场观察 + `/home/admin/workspace-public/kanmars/gloria/A/logs/zeroclaw.log`（2026-05-16 02:02）：

- 用户发"一分钟后提醒我喝水" → 飞书消息上累积出现 3 个表情：① 鼓掌（APPLAUSE，ack pool 随机抽）+ ② 哈士奇（飞书 `GLANCE` emoji_type 的内置美术渲染）+ ③ DONE（完成态 ✅）
- 日志中 grep `reaction|emoji|GLANCE|APPLAUSE|DONE` 命中均为 system prompt 字段噪音，**没有任何 `tracing::warn!` 来自 reaction 代码** —— 说明 3 次 POST 全部 200 成功，且 `remove_reaction` 走 trait 默认 no-op 根本没发请求所以也没日志
- 用户 binary `--version` 输出 `git-commit: cc98b8ce` —— 与 master HEAD 同 commit，证明 bug 在当前源码本身

### 1.3 根因结论

**[`impl Channel for LarkChannel` 块（lark.rs:2136-2241）只实现了 `send` / `listen` / `health_check` / `add_reaction` / `request_approval`，没有实现 `remove_reaction`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2136)**。Rust trait 解析回退到 `Channel` trait 在 [`zeroclaw-api/src/channel.rs:208-215`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-api/src/channel.rs#L208-L215) 定义的默认实现 `Ok(())`，**根本不发 HTTP 请求**。

因此 [orchestrator/mod.rs:3799](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3799) 的 `let _ = channel.remove_reaction(&msg.reply_target, &msg.id, "\u{1F440}")` 永远是 silent no-op，"处理中"的 👀（GLANCE/哈士奇）反应永远留在飞书消息上。

### 1.4 飞书 API 可用性（已验证，详见上一轮分析 §"飞书 API 完全支持 P0 实现"）

| API | 端点 | 方法 | 关键事实 |
|---|---|---|---|
| 添加反应 | `/open-apis/im/v1/messages/{message_id}/reactions` | POST | ✅ 已实现；**响应 `data.reaction_id` 包含删除时所需 ID** |
| 删除反应 | `/open-apis/im/v1/messages/{message_id}/reactions/{reaction_id}` | DELETE | ✅ 支持；只能删自己加的（错误码 231007） |
| 列出反应 | `/open-apis/im/v1/messages/{message_id}/reactions` | GET | ✅ 支持；本 PR 不用 |

权限：`im:message` 或 `im:message.reactions:write_only`（二选一即可），**zeroclaw 现已开通 `im:message`**（已能 send + add_reaction）。频控：1000 次/分钟、50 次/秒。区域：feishu.cn / larksuite.com 端点路径完全相同。

---

## 2. 方案选型（已决策）

### 2.1 选项对比

| 方案 | 描述 | API 调用次数 | 状态 | 决策 |
|---|---|---|---|---|
| **A — POST 时缓存 reaction_id** | `add_reaction` 解析 POST 响应 `data.reaction_id` 写入 `Arc<Mutex<HashMap<(msg_id, emoji), reaction_id>>>`，`remove_reaction` 直接 lookup 后 DELETE | 0 额外（仅原本就要发的 DELETE） | 有状态（in-memory） | ✅ **采纳** |
| B — 先 GET list 找 bot 自己的 reaction_id，再 DELETE | 每次 remove 都先 `GET .../reactions?reaction_type={emoji}` 翻页找 `operator_type=app` 的 item，提取 `reaction_id` 再 DELETE | 每次 remove 多 1 次 GET（~100ms 延迟） | 无状态 | ❌ |
| C — A + B 混合（缓存 miss fallback list+delete） | 优先走缓存，没命中再 GET list | 0 额外（命中）/ 1 额外（miss） | 有状态 + 兜底 | ❌（见下） |

**选 A 的核心理由**：

1. **零延迟开销**：飞书 POST 响应已经返回 `data.reaction_id`，是天然的输出，丢掉它是浪费
2. **零额外 API 配额消耗**：飞书每个 endpoint 1000/分 50/秒频控独立，少 1 次 GET 是真省
3. **不需要新权限**：方案 B 需要额外 `im:message.reactions:read` 才能 GET list，A 不需要
4. **代码量最小**：缓存维护逻辑约 5 行，B/C 涉及分页处理 + 身份匹配约 30 行
5. **进程重启场景可接受**：唯一缺点是重启后旧消息上的 stuck reaction 无法清理 —— 但 zeroclaw 重启时旧消息已经"过去了"，用户在飞书往下滚就看不到，**没有功能性影响**

**为何不要 C（A+B 混合 fallback）**：会被"两条路径都要测、两套错误码处理"拖累；A 的失败模式（缓存 miss）已经是良性退化（什么都不做 = 维持现状 = 用户看到一个多余表情，与 bug-fix 前一致），不值得引入复杂度兜底。**Worst case 等同于现状**，不会更差。

### 2.2 缓存形态（关键技术细节）

```rust
// LarkChannel 新增字段
reaction_ids: Arc<tokio::sync::Mutex<HashMap<(String, String), String>>>,
//                                       ↑ message_id ↑ emoji_unicode ↑ reaction_id
```

**Key 必须是 `(message_id, emoji_unicode)` 二元组**（不能只用 message_id），原因：同一条消息可能被加多个不同的反应（例如先 👀 处理中、再 ✅ 完成），需要按 emoji 区分删除哪一个。

**Eviction 策略 v0**：不主动 evict，map 无界增长。理由：

- 单条 reaction_id ≈ 50 bytes；key 二元组 ≈ 70 bytes；总计 ~120 bytes/条
- 10000 条仅占用 1.2 MB，对长期运行的 bot（几个月不重启）也完全 OK
- 每次 `remove_reaction` 成功会 `cache.remove(key)`，单条对话流的反应自然被清除（add → remove pattern）
- 流式 ack（① 鼓掌）走 `try_add_ack_reaction` 永不被 remove，会累积；但总量受单实例飞书消息总数限制
- **如果 v0 上线后真观察到内存增长问题，再做 v1 引入 LRU**（Anti-Pattern: 不为可能不发生的问题预先优化）

**线程安全**：`Arc<Mutex<HashMap>>` 与现有 `pending_approvals` 字段（[lark.rs:634](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L634)）同模式；`tokio::sync::Mutex` 而非 `std::sync::Mutex` —— 异步代码持锁不会阻塞 runtime worker。

### 2.3 飞书错误码处理矩阵

| HTTP / code | 含义 | 处理 |
|---|---|---|
| 200 + `code: 0` | DELETE 成功 | `cache.remove(key)`；不打日志 |
| 401 / `code: 99991663` | tenant_access_token 失效 | `invalidate_token` + retry once（仿现有 `add_reaction` 模式） |
| 200 + `code: 231007` | "no permission to delete this reaction"（机器人不在该会话/不是原始添加人） | `tracing::debug!`（预期可能发生，不告警）；仍 `cache.remove(key)` |
| 200 + `code: 231010` | "reaction does not belong to the message"（reaction_id 与 message_id 不匹配，缓存与服务端漂移） | `tracing::debug!`；仍 `cache.remove(key)` |
| 200 + `code: 231011` | "invalid reaction_id"（ID 已被自己/别人删过） | `tracing::debug!`；仍 `cache.remove(key)` |
| 200 + `code: 231003` | "message not found / deleted" | `tracing::debug!`；仍 `cache.remove(key)`（消息没了，缓存条目失效） |
| 4xx/5xx 其他 | 网络/未知错误 | `tracing::warn!` 留下 status + body；**不** remove from cache（下次可能能重试 — 但当前 PR 不实现重试，留待后续） |
| 缓存 miss | 没找到 (msg_id, emoji) → reaction_id 映射 | `tracing::debug!("cache miss")`；直接返回 `Ok(())`（良性退化 = 现状） |

**永不 propagate error**：`remove_reaction` 始终返回 `Ok(())`，所有失败软日志。理由：调用方 [orchestrator/mod.rs:3798](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3798) 用 `let _ =` 丢错误了，错误传不到用户；与其传播没人看的错误，不如打 warn 日志。

---

## 3. 实施步骤（4 处编辑，全部在 `lark.rs`）

### Step 0 — 环境与分支准备（5 min）

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
git checkout master
git pull --ff-only origin master                       # 当前应仍是 cc98b8ce
git checkout -b fix/lark-implement-remove-reaction
git status                                             # 应干净（无未跟踪修改）
```

**验证**：`git branch --show-current` = `fix/lark-implement-remove-reaction`；`git status` 干净（除 .sisyphus/run-continuation/* 已知忽略）。

### Step 1 — `LarkChannel` 加 `reaction_ids` 字段（10 min）

**位置**：紧挨现有 `pending_approvals: Arc<...>` 字段之后

```rust
// 在 pub struct LarkChannel { ... } 内追加：
/// In-memory map of `(message_id, emoji_unicode)` → `reaction_id` for
/// reactions this bot added via `add_reaction` / `try_add_ack_reaction`.
/// Used by `remove_reaction` to call DELETE without a preceding GET.
///
/// Lifetime: process-local, lost on restart. Reactions added before a
/// restart are unreachable (acceptable degradation — user has scrolled past
/// old messages by then).
///
/// Bounded growth: caller-driven add/remove (typical conversation pairs an
/// add with a remove). Pure-add `try_add_ack_reaction` entries accumulate;
/// total memory ≈ 120 bytes × #messages-bot-saw-since-restart.
reaction_ids: Arc<tokio::sync::Mutex<std::collections::HashMap<(String, String), String>>>,
```

**位置**：构造点 [lark.rs:634](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L634) 同 `pending_approvals` 兄弟字段，加一行：

```rust
reaction_ids: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
```

**验证**：
- `grep -n "reaction_ids" crates/zeroclaw-channels/src/lark.rs` 命中 2 处（field 定义 + Arc::new）
- `cargo check -p zeroclaw-channels` 编译通过（编译器会指出所有需要初始化 `reaction_ids` 的构造点；如有遗漏立刻报错）

### Step 2 — 抽取 reaction_id 写入缓存（在两个 add 路径）（15 min）

#### 2a. `LarkChannel::add_reaction` 在 `code == 0` 分支写入缓存

**位置**：[lark.rs:2230-2238](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2230-L2238) 现有 JSON 解析后

```rust
// Before (L2230-2239):
let payload: serde_json::Value = response.json().await?;
let code = payload.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
if code != 0 {
    let msg = payload
        .get("msg")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown error");
    tracing::warn!("Lark add_reaction returned code={code} for {message_id}: {msg}");
}
return Ok(());

// After:
let payload: serde_json::Value = response.json().await?;
let code = payload.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
if code != 0 {
    let msg = payload
        .get("msg")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown error");
    tracing::warn!("Lark add_reaction returned code={code} for {message_id}: {msg}");
} else if let Some(reaction_id) = payload
    .pointer("/data/reaction_id")
    .and_then(|v| v.as_str())
{
    self.reaction_ids
        .lock()
        .await
        .insert(
            (message_id.to_string(), emoji.to_string()),
            reaction_id.to_string(),
        );
}
return Ok(());
```

#### 2b. `try_add_ack_reaction` 在 `code == 0` 分支写入缓存

**位置**：[lark.rs:856-924](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L856) 函数末尾的 success 分支。注意此函数签名是 `try_add_ack_reaction(&self, message_id: &str, emoji_type: &str)` —— **传入的是飞书 emoji_type（如 `"APPLAUSE"`），不是 unicode emoji**。所以缓存 key 第二元素是 `emoji_type`，**不需要再走 `unicode_to_lark_emoji_type` 反查**。

调用方（orchestrator）走的是 `add_reaction(unicode)` → 内部转 emoji_type → POST；ack 路径直接传 emoji_type → POST。两路径**缓存 key 的语义不一样**：

- `add_reaction(msg_id, "👀")` 缓存 key = `(msg_id, "👀")`（unicode）
- `try_add_ack_reaction(msg_id, "APPLAUSE")` 缓存 key = `(msg_id, "APPLAUSE")`（emoji_type）

**这是有意的**：`remove_reaction` 只会被 orchestrator 调用，永远只查 unicode key；ack reaction 永不被删，缓存条目纯属副产品（占内存但不被读，只会随着 process 生命周期累积）。

**简化决策**：**`try_add_ack_reaction` 里不做缓存写入**。理由：
- 它产生的 reaction 永不被删（无 caller 调用 `remove_reaction(unicode_for_APPLAUSE)`）
- 缓存的 key 类型不一致（一边 unicode 一边 emoji_type），混入会让 remove 逻辑变复杂
- 节省内存

修订计划：**只在 `add_reaction` 写缓存（2a），跳过 `try_add_ack_reaction`（2b 取消）**。

**验证**（更新版）：
- `grep -n "reaction_ids" crates/zeroclaw-channels/src/lark.rs` 命中 3 处（field 定义 + Arc::new + add_reaction 内 insert）
- LSP diagnostics on lark.rs 0 error

### Step 3 — 实现 `LarkChannel::remove_reaction`（30 min）

**位置**：紧挨 `add_reaction` 之后（[lark.rs:2241](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2241) 函数闭合括号之后），同一 `impl Channel for LarkChannel` 块内

```rust
async fn remove_reaction(
    &self,
    _channel_id: &str,
    message_id: &str,
    emoji: &str,
) -> anyhow::Result<()> {
    if message_id.is_empty() {
        return Ok(());
    }

    // Cache lookup: do we have a reaction_id for this (message, emoji)?
    let reaction_id = {
        let mut cache = self.reaction_ids.lock().await;
        cache.remove(&(message_id.to_string(), emoji.to_string()))
    };
    let Some(reaction_id) = reaction_id else {
        // Cache miss: either we never added this reaction, or the process
        // restarted since add. Silently degrade — no API call, no error.
        // This matches pre-fix behavior (do nothing) for forgotten entries.
        tracing::debug!(
            message_id,
            emoji,
            "Lark remove_reaction: cache miss, skipping"
        );
        return Ok(());
    };

    let mut token = self.get_tenant_access_token().await?;
    let url = format!(
        "{}/im/v1/messages/{message_id}/reactions/{reaction_id}",
        self.api_base()
    );

    let mut retried = false;
    loop {
        let response = self
            .http_client()
            .delete(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await?;

        // Same token-refresh dance as add_reaction.
        if response.status().as_u16() == 401 && !retried {
            self.invalidate_token().await;
            token = self.get_tenant_access_token().await?;
            retried = true;
            continue;
        }

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            tracing::warn!(
                "Lark remove_reaction failed for {message_id}/{reaction_id}: \
                 status={status}, body={err_body}"
            );
            return Ok(());
        }

        // Parse Feishu error code; only warn for unexpected non-zero codes.
        // 231003 (message deleted) / 231007 (no permission) / 231011 (invalid
        // reaction_id) are expected drift between cache and server state —
        // log at debug, not warn.
        let payload: serde_json::Value = response.json().await?;
        let code = payload.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
        match code {
            0 => {} // success, nothing to log
            231003 | 231007 | 231010 | 231011 => {
                let msg = payload
                    .get("msg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                tracing::debug!(
                    "Lark remove_reaction: server-side stale state \
                     (code={code}, msg={msg}, message_id={message_id})"
                );
            }
            _ => {
                let msg = payload
                    .get("msg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                tracing::warn!(
                    "Lark remove_reaction returned code={code} for {message_id}: {msg}"
                );
            }
        }
        return Ok(());
    }
}
```

**验证**：
- `grep -nE "async fn remove_reaction" crates/zeroclaw-channels/src/lark.rs` 命中 1 行
- LSP diagnostics on lark.rs 0 error
- `cargo check -p zeroclaw-channels` 编译通过

### Step 4 — 单元测试（25 min）

**位置**：`#[cfg(test)] mod tests` 内，紧挨现有 `lark_reaction_url_matches_region` 测试（[lark.rs:4244](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4244)）

#### 4a. 测试 1：`remove_reaction_caches_id_from_add_and_deletes`

```rust
#[tokio::test]
async fn remove_reaction_caches_id_from_add_and_deletes() {
    // Setup wiremock: tenant_access_token (returns "test_token"),
    //                 POST /im/v1/messages/om_test/reactions
    //                   responds {"code":0,"data":{"reaction_id":"r_xyz",...}},
    //                 DELETE /im/v1/messages/om_test/reactions/r_xyz
    //                   responds {"code":0}.
    // Drive: ch.add_reaction("chat_id", "om_test", "👀").await.unwrap();
    //        ch.remove_reaction("chat_id", "om_test", "👀").await.unwrap();
    // Assert: POST mock called once + DELETE mock called once with the
    //         exact reaction_id "r_xyz"; cache is empty after remove.
}
```

#### 4b. 测试 2：`remove_reaction_silent_on_cache_miss`

```rust
#[tokio::test]
async fn remove_reaction_silent_on_cache_miss() {
    // Setup: only token mock; DELETE wiremock with .expect(0) — must NOT be
    // called.
    // Drive: ch.remove_reaction("chat_id", "om_never_added", "👀").await.unwrap();
    // Assert: returns Ok(()), DELETE mock count == 0.
}
```

#### 4c. 测试 3：`remove_reaction_tolerates_server_stale_codes`

```rust
#[tokio::test]
async fn remove_reaction_tolerates_server_stale_codes() {
    // Setup: token + POST returns reaction_id "r_stale";
    //        DELETE returns {"code":231007,"msg":"no permission"}.
    // Drive: add then remove.
    // Assert: remove returns Ok(()) (no error propagation), no panic.
    //         Cache entry was removed (next remove call would be a miss).
}
```

> 完整 wiremock 设置照抄 [lark.rs:4750-4858](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4750) 模式。Mock 路径用 `path_regex` 匹配 `/im/v1/messages/om_test/reactions` 和 `/im/v1/messages/om_test/reactions/r_xyz`。

**验证**：`cargo test -p zeroclaw-channels remove_reaction_` 命中 3 个测试全绿。

### Step 5 — 静态检查（5 min）

```bash
cargo fmt --all -- --check
cargo clippy -p zeroclaw-channels --all-targets -- -D warnings
cargo test -p zeroclaw-channels
```

**验证**：三条全部 exit 0；现有 lark 测试不回归。

### Step 6 — Atomic commit + push + CR（10 min）

```bash
git add crates/zeroclaw-channels/src/lark.rs
git status --short                                    # 应仅 1 个文件改动
git diff --stat HEAD                                  # 预期 +180 / -5
git commit -F - <<'EOF'
feat(lark): implement Channel::remove_reaction so the 👀 ack-reaction is cleared on completion

The orchestrator's ack/done reaction pattern (mod.rs:3179 add 👀, mod.rs:3799
remove 👀, mod.rs:3801 add ✅) silently degraded on Feishu/Lark because
LarkChannel never implemented `remove_reaction`. The trait default (in
zeroclaw-api/src/channel.rs:208-215) returns `Ok(())` without any HTTP call,
so every "processing" 👀 reaction stayed pinned to the user's message
forever, leaving three reactions (random ack + 👀-as-GLANCE/Husky + ✅) where
the design intended two (ack + ✅).

This patch wires the missing piece:

1. `LarkChannel` gets a new `reaction_ids: Arc<Mutex<HashMap<(message_id,
   emoji), reaction_id>>>` field — same shape as `pending_approvals`.
2. `add_reaction` extracts `data.reaction_id` from the existing POST
   response (Feishu already returns it; we were discarding it) and writes
   the cache entry on success.
3. New `remove_reaction` impl: cache lookup → `DELETE /im/v1/messages/
   {message_id}/reactions/{reaction_id}` → soft-fail with
   `tracing::debug!` for expected stale-state codes (231003 / 231007 /
   231010 / 231011) and `tracing::warn!` for unexpected errors.

Cache is process-local (no persistence). Reactions added before a restart
are unreachable — acceptable since by then the user has scrolled past
those messages.

No new dependencies, no trait change, no config flag. Permission
`im:message` (already in use) is sufficient — no need to grant
`im:message.reactions:write_only`.

Risk: Low (zeroclaw-channels Experimental tier, single file, behavior
addition only — fixes a silent no-op, breaks nothing).
EOF
git push -u origin fix/lark-implement-remove-reaction
```

**验证**：push 成功，发 CR 链接给用户。

### Step 7 — CHANGELOG-next.md 入口

```markdown
### Fixed

- **lark/feishu**: Implemented `Channel::remove_reaction` for the Feishu/Lark
  channel. Previously the trait default no-op left the orchestrator's
  "processing" 👀 reaction stuck on the user's message forever, producing
  three reactions (random ack + 👀-as-GLANCE/Husky + ✅) instead of the
  intended two (ack + ✅). The implementation caches `reaction_id` from
  the POST response so DELETE needs only one HTTP call. Cache is
  process-local; reactions from before a restart are not reachable. (#TBD)
```

---

## 4. 验证清单（PR 提交前必须全绿）

| 项 | 命令 | 预期 |
|---|---|---|
| 格式 | `cargo fmt --all -- --check` | exit 0 |
| Lint | `cargo clippy -p zeroclaw-channels --all-targets -- -D warnings` | exit 0 |
| 单元测试 | `cargo test -p zeroclaw-channels` | 全绿 |
| 新单测命中 | `cargo test -p zeroclaw-channels remove_reaction_` | 3 个全通过 |
| LSP diagnostics | `lsp_diagnostics` on `lark.rs` | 0 error |
| Grep 验证字段 | `grep -n "reaction_ids" crates/zeroclaw-channels/src/lark.rs` | 3 行（定义 + Arc::new + add_reaction insert） |
| Grep 验证 remove impl | `grep -nE "async fn remove_reaction" crates/zeroclaw-channels/src/lark.rs` | 1 行 |
| Grep 验证 endpoint | `grep -nE "/reactions/{reaction_id}" crates/zeroclaw-channels/src/lark.rs` | ≥ 1 行 |
| 改动文件数 | `git diff --stat HEAD~1` | 仅 1 个文件（lark.rs） |
| 现有测试不回归 | `cargo test -p zeroclaw-channels lark` | 既有 lark 测试全绿 |
| 完整 CI（可选但推荐） | `./dev/ci.sh all` | exit 0 |

**线上回归验证**（PR merge + 部署后用户实测）：
- 在飞书 DM 或群聊给 bot 发任意一条消息（如"今天天气如何"）
- 观察 bot 的回复消息发出后，消息上的反应表情数 = 2（① ack + ③ DONE），不再有"哈士奇"残留
- 检查 zeroclaw 日志：
  - 不应出现 `Lark remove_reaction failed`（warn 级）
  - 可能出现 `Lark remove_reaction: cache miss, skipping`（debug 级，仅当进程重启后处理旧消息时）
  - 可能出现 `Lark remove_reaction: server-side stale state`（debug 级，正常）

---

## 5. 风险与缓解

| # | 风险 | 严重性 | 缓解 |
|---|---|---|---|
| R1 | **Race**：`add_reaction` 写入缓存之前 `remove_reaction` 就被调用 | 极低 | orchestrator 是顺序调用（add 完 await，再走 LLM 完成才 remove），没有并发可能 |
| R2 | **缓存无界增长**（`try_add_ack_reaction` 未走缓存，但 `add_reaction` 的 👀 + ✅ 若不被 remove 也累积） | 低 | 设计上 👀 一定会被 remove 配对；✅ 不被 remove，但每会话仅 1 条；实际增长 ≤ #会话数。10000 条 ≈ 1.2 MB，对长跑 bot 也 OK。如真出问题 → v1 加 LRU |
| R3 | **`unicode_to_lark_emoji_type` 不识别的 emoji** | 极低 | `add_reaction` 在映射 None 时早期 return（[lark.rs:2197-2204](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2197-L2204)），根本不会进缓存；`remove_reaction` 缓存 lookup 自然 miss → debug log + return Ok。无 panic |
| R4 | **DELETE 报 231007 "no permission"**（机器人被踢出群后再尝试删除） | 低 | 走 debug log 路径（"server-side stale state"），不告警。缓存条目仍被 remove（避免重复尝试） |
| R5 | **process 重启导致缓存丢失**，残留旧消息 | 已知接受 | 文档清楚标注；用户场景中老消息已滚出视野，无可观察影响。**不为此引入持久化** |
| R6 | **`reaction_ids` 字段 type 与 `pending_approvals` 类型互不影响** | 极低 | 完全独立字段，借鉴模式 |
| R7 | **多个 LarkChannel 实例（如 feishu + larksuite 同时启用）共享 cache？** | 极低 | 每个 `LarkChannel::new` 各自构造一份，不共享。reaction_id 是端点 scope 唯一，只在自己生命周期内查得到 |
| R8 | **DELETE HTTP body 默认 `{}` vs Feishu 不接受空 body** | 极低 | DELETE 端点不要求 body（[chyroc/lark sample 验证](https://github.com/larksuite/oapi-sdk-go/blob/v3_main/sample/apiall/imv1/delete_messageReaction.go)）；reqwest 的 `.delete().send()` 不发 body 默认 OK |
| R9 | **Error 231007 在某些边角场景被错误归到 stale 类**（其实是真正的 bug） | 极低 | debug log 仍保留 code + msg，便于事后查；orchestrator 路径下 bot 永远是消息原作者 + 在会话中，几乎不可能踩到 231007 真正异常面 |

### 5.1 回退方案

如 PR merge 后线上出现严重问题：

1. **快速 revert**：`git revert <commit_sha>` 单 commit 即可（PR 单文件单 commit）
2. **回退影响**：回到当前现状 = 用户继续看到 3 个反应；ack/done 主要功能不受影响
3. **无 schema / 配置 / 数据迁移**

---

## 6. 工作量估算 & 时间线

| 阶段 | 行数 | 时长 |
|---|---|---|
| Step 0（分支准备） | — | 5 min |
| Step 1（reaction_ids 字段 + Arc::new） | +12 / -0 | 10 min |
| Step 2a（add_reaction 写缓存） | +12 / -0 | 10 min |
| Step 3（remove_reaction 实现） | +75 / -0 | 30 min |
| Step 4（单测 ×3） | +80 / -0 | 25 min |
| Step 5（本地 fmt/clippy/test） | — | 5 min |
| Step 6（commit + push + 写 PR description + CHANGELOG） | +1 / -5 | 10 min |
| **合计** | **≈ +180 / -5** | **≈ 95 min** |

---

## 7. 提交流程（依 zeroclaw AGENTS.md "Workflow"）

1. **分支**：`fix/lark-implement-remove-reaction`（**非 master**）
2. **commit 信息**：见 Step 6（conventional commit `feat(lark):` —— "添加缺失的 trait 实现"是 feature，不是 fix bug；现状是 silent no-op 而非异常行为。如审阅人偏好 `fix(lark):` 也可接受）
3. **CHANGELOG-next.md**：必加（见 Step 7）
4. **PR 标题**：`feat(lark): implement Channel::remove_reaction so the 👀 ack-reaction is cleared on completion`
5. **size**：`size: S`（≈180 行 含测试，单文件）
6. **PR body** 按 `.github/pull_request_template.md` 全填，重点：
   - **What**：上述 commit message 内容
   - **Why**：用户报告"飞书消息上 3 个表情，期望 2 个；哈士奇/GLANCE 应该被自动清理"
   - **Risk**：Low
   - **Validation**：`cargo test -p zeroclaw-channels` 全绿 + 3 新测试 + 现有 lark 测试不回归 + 用户线上验证（部署后发消息观察反应数）
   - **Rollback**：`git revert <sha>`，无副作用
7. **流程**：push 分支 → 发 CR 地址给用户确认 → 用户审完合 master → 不直推 master

---

## 8. 待用户决策项（开工前需确认）

| # | 项 | 默认 | 备选 |
|---|---|---|---|
| Q1 | commit type 用 `feat(lark):` 还是 `fix(lark):` | `feat`（添加 trait 实现是 feature） | `fix`（用户视角是修 bug） |
| Q2 | 是否同时收窄 `LARK_ACK_REACTIONS_ZH_CN` ack pool 为单一表情（如只 `OK`） | **不收窄**（独立 PR 决策） | 收窄到 1 个，让 ack 反应稳定可预期 |
| Q3 | 是否一并删除 ① ack 表情（即整个移除 `try_add_ack_reaction` 调用） | **不删除**（用户没要求） | 删除（彻底简化为只剩 ✅） |
| Q4 | 缓存上限要不要预先加 LRU | **不加**（v0 简单是美） | 加（防御性，~10 行额外代码） |
| Q5 | 是否在 `remove_reaction` 缓存 miss 时降级走 GET list+DELETE | **不降级**（方案 A 纯净） | 降级（方案 C 兜底，~30 行额外） |

---

## 9. 关联文档 / 参考

- 上一轮分析（本会话内）：组件管理员对"3 个表情"的根因调查 + 飞书 API 可用性确认
- [zeroclaw AGENTS.md](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md) — Workflow / Anti-Patterns / Stability Tiers
- [`Channel` trait 定义](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-api/src/channel.rs) — `add_reaction` / `remove_reaction` 契约 + 默认 no-op 实现
- [现有 `add_reaction` 实现 lark.rs:2185-2241](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2185-L2241)
- [`pending_approvals` 字段（仿写模板）lark.rs:523](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs)
- 飞书官方 API：`DELETE /open-apis/im/v1/messages/{message_id}/reactions/{reaction_id}`
  - 中国：https://open.feishu.cn/document/server-docs/im-v1/message-reaction/delete
  - 海外：https://open.larksuite.com/document/server-docs/im-v1/message-reaction/delete
- OSS 实现参考：
  - [larksuite/oapi-sdk-go sample](https://github.com/larksuite/oapi-sdk-go/blob/v3_main/sample/apiall/imv1/delete_messageReaction.go)
  - [go-lark/lark api_message.go](https://github.com/go-lark/lark/blob/main/api_message.go)
  - [chyroc/lark api_message_reaction_delete.go](https://github.com/chyroc/lark/blob/master/api_message_reaction_delete.go)

---

## 10. Sign-off

| 角色 | 状态 |
|---|---|
| 起草人（组件管理员）| ✅ 已完成（rev0） |
| 计划审阅人（Momus Plan Critic）| ⏳ 待审阅 |
| 计划审阅人（用户）| ⏳ 待审阅 |
| 实施授权 | ⏳ 待用户明确 "execute" / "go" / "开始改" 才会动代码 |

**当前模式**：plan 起草完毕，等用户审阅后授权实施。

**注意**：本 PR 与昨日 `fix/lark-image-download-restore` / `fix/orchestrator-strip-image-markers-from-precheck` 完全独立，应基于 master HEAD `cc98b8ce` 起新分支。三个 PR 可并行 review，无序合并。
