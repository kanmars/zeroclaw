# Plan — kanmars.req.20260516.003 (Lark "thinking" reaction fires on inbound, not after precheck — B1)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260516.003.plan |
| 关联需求 | 用户对话需求（2026-05-16）：『现在是先发 Done 再发哈士奇再撤回。应该是消息一到就有"思考中"，做完才换 Done。让用户立刻知道 bot 收到了。』 |
| 起草日期 | 2026-05-16 |
| 修订日期 | 2026-05-16 (rev0 — 初稿) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `fix/lark-implement-remove-reaction`（**复用 002 同一个分支，不开新分支**） |
| 风险等级 | **Low-Medium**（Lark 通道层语义改动 + 一个共享 orchestrator 测试需更新；不动 trait / schema / 边界） |
| 基线 commit | `0a8acc17`（002 计划已交付的提交，本计划在其之上叠加） |
| 选型方案 | **B1-a — Lark 通道层在消息到达瞬间发"思考中"（GLANCE），orchestrator 停止为 lark 加 👀；其他 channel 行为不变** |
| 预计代码行数 | +60 / -30（含 1 个新单测 + 1 个旧单测调整） |
| 预计工作量 | 约 60 分钟 |

---

## 0. 关键目标（唯一真理来源）

> **让飞书/Lark 用户在消息发出的瞬间（≤200ms）就在自己的消息上看到一个"思考中"反应（哈士奇 / GLANCE），并在 bot 完成回复时被 ✅ DONE 替换。期间不再有任何其他随机表情（鼓掌 / OK / 笑脸等）干扰，也不再出现"DONE 在哈士奇之前显示"的视觉时序错位。**

**完成此目标即"功能完成"**：

- 用户在飞书 DM 或群（@bot）发任意一条消息：
  - **T+0~200ms**：消息上出现 ① **哈士奇（GLANCE）"思考中"** 反应（来自 lark 通道层 `try_add_ack_reaction`）
  - **T+完成**：哈士奇被 ② **DONE / 故障标** 替换（来自 orchestrator 完成态收尾）
  - 全程**只有 1 个反应可见**（替换语义），不再是当前的"鼓掌 + 哈士奇 + DONE 三个并排"
- 当前 PR 002 的 `remove_reaction` 实现继续生效，是本计划得以工作的前提
- 主对话流程、approval card 流程、流式输出 draft 流程**完全不变**
- 其他 channel（discord / slack / telegram / matrix 等）的反应行为**完全不变**（仍走 orchestrator 的 👀 → ✅ pattern）

**显式不在范围内**：

- ❌ 不引入 3 阶段（"已收到 → 思考中 → Done"）—— 那是方案 A2，复杂度高且边际收益小
- ❌ 不修改其他 channel 的反应行为 —— B1-a 仅动 lark 通道
- ❌ 不引入"短任务跳过 GLANCE"优化 —— 一旦 lark 通道层主动发，就不再有"短任务"的 race 问题
- ❌ 不动 [`LARK_ACK_REACTIONS_ZH_CN`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L19-L21) ack pool 数据 —— 池本身没问题，问题在于"何时用 + 用什么"，本计划改成固定用 GLANCE，pool 沦为死代码（§5 R7 处理）
- ⚠️ **rev0→rev1 修订**：原计划"留作后续清理 PR"，但 §0.5 #2 + AGENTS.md 硬规则禁止 `#[allow(dead_code)]`，且 `cargo clippy --features channel-lark --all-targets -- -D warnings` 把 dead-code 提为 error。所以本 PR 必须**同时删除**：`LARK_ACK_REACTIONS_*`(×4)、`LarkAckLocale` enum、`pick_uniform_index` / `random_from_pool` / `lark_ack_pool` / `map_locale_tag` / `find_locale_hint` / `detect_locale_from_post_content` / `is_japanese_kana` / `is_cjk_han` / `is_traditional_only_han` / `is_simplified_only_han` / `detect_locale_from_text` / `detect_lark_ack_locale` / `random_lark_ack_reaction`，以及关联测试 `lark_reaction_locale_*`(×4) + `random_lark_ack_reaction_respects_detected_locale_pool`。删除是机械的（无设计决策），与"思考中信号搬到通道层"主题强相关 —— 不算扩大范围
- ❌ 不动 trait / config / schema
- ❌ 不引入新依赖

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不新增 `unwrap()` / `expect()`**（[zeroclaw AGENTS.md Anti-Patterns](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#anti-patterns)）
2. **不新增 `#[allow(dead_code)]`**。`random_lark_ack_reaction` 等辅助函数仍被单测调用，不是 dead code
3. **不动 `zeroclaw-api`**。改动边界 = `crates/zeroclaw-channels/src/lark.rs` + `crates/zeroclaw-channels/src/orchestrator/mod.rs`
4. **`tracing::` 日志保持英文**（RFC #5653 §4.6）
5. **不引入新依赖**
6. **完整跑** `cargo fmt`（仅本 PR 涉及行）+ `cargo clippy --features channel-lark` + `cargo test --features channel-lark`
7. **One concern per PR**：本 patch 一个关注点 = 把"思考中"信号从 orchestrator 后置位（precheck 之后）挪到 lark 通道层前置位（消息到达瞬间）。不与 ack pool 清理 / 3 阶段方案 / 其他 channel 改动混合
8. **CHANGELOG-next.md 必须更新**（修改的是 002 已经发布过的行为）
9. **复用 PR 002 的 `reaction_ids` 缓存机制**：`try_add_ack_reaction` 也需写缓存，让 orchestrator 的 `remove_reaction("👀")` 能命中。这是与 002 plan §3 Step 2b 决策"`try_add_ack_reaction` 不写缓存"的**有意推翻**——B1 把"思考中"语义从 orchestrator 搬到 lark 层，缓存 key 一致性变成必须

---

## 1. 现状事实复核（基于 2026-05-16 实地代码读取，行号对齐 commit `0a8acc17`）

### 1.1 关键代码位置

| 事实 | 文件:行 |
|---|---|
| Lark WS 入站 ack 调用点（待改成固定 GLANCE） | [lark.rs:1271-1278](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1271-L1278) |
| Lark webhook 入站 ack 调用点（待改成固定 GLANCE） | [lark.rs:2784-2790](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2784-L2790) |
| `try_add_ack_reaction` 函数定义（待加 cache 写入） | [lark.rs:875-948](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L875) |
| `random_lark_ack_reaction` 池随机选择（本 PR 不再调用，但保留） | [lark.rs:3106](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L3106) |
| **Orchestrator 添加 👀（待加 lark 跳过守卫）** | [orchestrator/mod.rs:3179-3186](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3179-L3186) |
| Orchestrator 完成态 remove + add | [orchestrator/mod.rs:3795-3804](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3795-L3804) |
| Orchestrator NO_REPLY 反应分支 | [orchestrator/mod.rs:3055-3068](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3055-L3068) |
| **现有测试 `process_channel_message_adds_and_swaps_reactions`**（断言 orchestrator 加 👀；本 PR 改后该测试需调整） | [orchestrator/mod.rs:9117-9219](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L9117-L9219) |
| `reaction_ids` 缓存字段（PR 002 添加） | [lark.rs:589-604](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L589) |
| `add_reaction` 写缓存的实现（PR 002 添加，本 PR 仿照） | [lark.rs:2266-2276](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2266-L2276) |
| `Channel::name()` 返回值（用于 orchestrator 区分 lark） | `LarkChannel` 名为 `"lark"` 或 `"feishu"` 取决于 `LarkPlatform` |

### 1.2 用户实测证据（铁证）

来自 `/home/admin/workspace-public/kanmars/gloria/A/logs/zeroclaw.log`（2026-05-16 02:57，"三分钟后提醒我喝水"）：

| 时刻 | 事件 | 距 T0 |
|---|---|---|
| 02:57:19.992 | Lark WS 收到用户消息 | 0 ms |
| 02:57:19.996 | Memory recall 完成 | 4 ms |
| 02:57:20.001 | **第 1 次 LLM（精分类）调用开始** | 9 ms |
| 02:57:22.844 | 精分类返回 `REPLY` | 2852 ms |
| 02:57:22.844 | **第 2 次 LLM（主 agent）调用开始** | 2852 ms |
| 02:57:39.112 | 主 agent + 工具完成 | 19120 ms |

**关键事实**：当前 orchestrator 的 [add 👀 调用](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3182) 位置在 [precheck 完成之后](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3028)（约 T+2.85s），而 Lark 通道层的 ack 在 [`lark.rs:1276`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1276) 通过 `tokio::spawn` 异步在 T+0 触发 —— 两条 POST 并发飞向飞书 API，到达顺序不可控，而且当主 agent 跑得快（短任务）时，DELETE GLANCE + POST DONE 可能在 POST GLANCE 还未被飞书客户端渲染前就到达，造成"DONE 先出现，哈士奇后出现"的视觉错位。

### 1.3 根因结论

**当前架构有两个独立的入站反应触发器**：

1. **Lark 通道层** ([`lark.rs:1276`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1276) + [`L2789`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2789))：在消息到达 lark WS / webhook 的瞬间 spawn 一个异步任务发"随机 ack 表情"（APPLAUSE / OK / SMILE 等，从 [ZH_CN pool](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L19-L21) 抽）。语义是"通道收到了"。
2. **Orchestrator** ([`mod.rs:3182`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3182))：在 precheck 完成后（仅 REPLY 路径）顺序 await 加 👀（GLANCE）。语义是"orchestrator 开始处理"。

两个触发器在用户视觉上**语义重复**（都是"我看到了"），但却产生了**两个不同的反应**（鼓掌 + 哈士奇）。再加上后续 orchestrator 的 ✅ DONE，就是用户看到的 3 个表情。即便 PR 002 修复了 `remove_reaction` 让哈士奇能被撤掉，"鼓掌"仍永远留下，"DONE 先于哈士奇出现"的 race 仍存在。

**B1 的本质**：让 lark 通道层成为"思考中"信号的**唯一发起者**，让 orchestrator 在 lark channel 上不重复加 👀。这样：

- 只有一条 POST GLANCE（在 T+0 立刻发出，无并发）
- orchestrator 完成态时仍走 `remove_reaction("👀")` + `add_reaction("✅")`，但 `remove_reaction` 现在去删的是 lark 通道层加的那一个 GLANCE（前提：缓存写入）
- 短任务的 race condition 自动消失：因为 GLANCE 是消息一到就发的，等 LLM 跑完（哪怕只跑 1 秒），飞书已经把 GLANCE 推到客户端了

---

## 2. 方案选型（已决策）

### 2.1 选项对比

| 方案 | 描述 | 哪些 channel 受影响 | 改动量 |
|---|---|---|---|
| **B1-a（采纳）** | Lark 通道层固定发 GLANCE（不再随机），orchestrator 检测 `channel.name()` 是 lark/feishu 时跳过 add 👀 | 仅 lark | 低 |
| B1-b | 同 B1-a，但 orchestrator 不区分 channel —— 让所有 channel 的 lark/feishu 内部协调（lark 层加 + orchestrator 也加 → 用 cache 检测重复 → 跳过）| 仅 lark（其他 channel 行为也通过同一守卫保护） | 中 |
| B1-c | 把 lark 通道层 ack 整个删掉，让 orchestrator 把 add 👀 时机从 precheck 之后**挪到 precheck 之前** | 所有 channel（precheck 期间也有 👀） | 高（要重排 orchestrator 状态机）|
| B2（弃选） | 仅删 lark 通道层 ack，保留 orchestrator 现状 | 仅 lark（但前 2.85 秒无视觉反馈） | 低 |

**选 B1-a 的核心理由**：

1. **直接命中用户痛点**：消息到达瞬间就有"思考中"信号
2. **只动 lark 一个 channel**：其他 channel 完全不受影响，blast radius 最小
3. **orchestrator 改动极小**：只加一行"如果是 lark/feishu 则跳过 add 👀"的守卫
4. **避免 B1-c 的状态机重排风险**：orchestrator 当前 add 👀 位置在 NO_REPLY return 之后，是有意为之（NO_REPLY 直接走 [`L3055-3068`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3055-L3068) 加 👍/🚫/⚠️，不需要中间状态）。把 add 挪前面会让 NO_REPLY 路径要先 add 👀 再 remove 再 add 👍，多 2 次 API 调用还引入新 race
5. **避免 B1-b 的 cache 跨层依赖**：B1-b 需要 orchestrator 调用 `add_reaction` 后判断"是不是已经存在"，但 `Channel` trait 没有 `has_reaction` 方法，要么加 trait（违反"不动 trait"）要么靠错误码区分（飞书没有"已存在"明确码）

### 2.2 B1-a 的"思考中"表情选定：固定 GLANCE

当前 lark 通道层从 [`LARK_ACK_REACTIONS_ZH_CN`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L19-L21) pool 随机抽：`OK / JIAYI / APPLAUSE / THUMBSUP / MUSCLE / SMILE / DONE`。

**选 GLANCE 的理由**：

1. **与 orchestrator 完成态的 ✅ DONE 形成清晰对照**："眼睛/瞅"（GLANCE 飞书美术 = 哈士奇）→ "完成印章"（DONE）→ 用户视觉上是"思考态"→"完成态"的明确切换
2. **借用现有 unicode 映射**：[`unicode_to_lark_emoji_type("👀") = "GLANCE"`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L63)，cache key 用 unicode `"👀"` 与 orchestrator `remove_reaction("👀")` 天然对齐
3. **避免与 DONE 撞名**：现有 pool 里居然包含 `DONE`，意味着随机抽到 DONE 时 ack 阶段就显示"完成"——这本身就是设计 bug，B1 顺便消除该问题
4. **不要 APPLAUSE / THUMBSUP**：这些是"赞同"语义，与"思考中"语义不符；用户上一轮反馈"鼓掌"看着像故障图标，已经投过反对票
5. **不引入新 emoji_type**：GLANCE 已经在 unicode 映射里有，无需扩展

### 2.3 cache key 一致性（关键技术细节）

PR 002 §3 Step 2b 决策："`try_add_ack_reaction` 不写缓存"。理由是当时 ack 反应永不被删，缓存键还是 emoji_type 不是 unicode，与 orchestrator 路径不一致。

**B1 推翻这个决策**：

- B1 之后，"思考中"信号由 lark 通道层发起，但**仍由 orchestrator 触发删除**（[`mod.rs:3799 remove_reaction("👀")`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3799)）
- orchestrator 用的 cache key 是 unicode `"👀"`（来自 [`add_reaction` impl](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2266-L2276)，PR 002 已实现）
- **`try_add_ack_reaction` 必须用同一个 unicode key 写缓存**，否则 orchestrator 的 `remove_reaction("👀")` 永远是 cache miss

**实现方法**：

让 `try_add_ack_reaction` 接收一个新参数 `unicode_emoji: &str`（用于 cache key），与原 `emoji_type: &str`（用于 POST body）解耦。或者：让 `try_add_ack_reaction` 内部反查 `emoji_type → unicode`，但这需要新映射函数。**前者更简单**。

---

## 3. 实施步骤（4 处编辑，分 lark.rs + orchestrator/mod.rs 两文件）

### Step 0 — 验证当前分支（30 sec）

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
git rev-parse --abbrev-ref HEAD     # 必须是 fix/lark-implement-remove-reaction
git log --oneline -1                 # 必须是 0a8acc17 (PR 002 commit)
```

如果不在该分支：`git checkout fix/lark-implement-remove-reaction`。

### Step 1 — `try_add_ack_reaction` 加 unicode 参数 + cache 写入（10 min）

**位置**：[`lark.rs:877`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L877) 函数签名 + 函数末尾 `code == 0` 分支

```rust
// Before:
async fn try_add_ack_reaction(&self, message_id: &str, emoji_type: &str) {
    if message_id.is_empty() { return; }
    // ... POST + token retry + error handling ...
    let code = payload.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code != 0 {
        let msg = payload.get("msg").and_then(|v| v.as_str()).unwrap_or("unknown error");
        tracing::warn!("Lark: add reaction returned code={code} for {message_id}: {msg}");
    }
    return;
}

// After:
/// `unicode_emoji` is the unicode key used for the in-memory `reaction_ids`
/// cache so a later `remove_reaction(unicode_emoji)` from the orchestrator
/// can find the reaction_id Feishu returned. `emoji_type` is the Feishu
/// internal name (e.g. `"GLANCE"`) used in the POST body.
async fn try_add_ack_reaction(
    &self,
    message_id: &str,
    emoji_type: &str,
    unicode_emoji: &str,
) {
    if message_id.is_empty() { return; }
    // ... unchanged POST + token retry + error handling ...
    let code = payload.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    if code != 0 {
        let msg = payload.get("msg").and_then(|v| v.as_str()).unwrap_or("unknown error");
        tracing::warn!("Lark: add reaction returned code={code} for {message_id}: {msg}");
    } else if let Some(reaction_id) = payload
        .pointer("/data/reaction_id")
        .and_then(|v| v.as_str())
    {
        // Write under the unicode key so orchestrator's
        // `remove_reaction("👀")` can find this entry.
        self.reaction_ids.lock().await.insert(
            (message_id.to_string(), unicode_emoji.to_string()),
            reaction_id.to_string(),
        );
    }
    return;
}
```

**验证**：
- `grep -n "fn try_add_ack_reaction" crates/zeroclaw-channels/src/lark.rs` 命中 1 行（签名变成 3 个参数）
- `cargo check --features channel-lark` 立刻报 2 处调用方不匹配（[L1276](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1276), [L2789](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2789)）—— 进入 Step 2 修

### Step 2 — 两个 ack 调用点改成固定 GLANCE + 传 unicode key（10 min）

#### 2a. WS 路径 [`lark.rs:1270-1278`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1270-L1278)

```rust
// Before:
let ack_emoji =
    random_lark_ack_reaction(Some(&event_payload), &text).to_string();
let reaction_channel = self.clone();
let reaction_message_id = lark_msg.message_id.clone();
tokio::spawn(async move {
    reaction_channel
        .try_add_ack_reaction(&reaction_message_id, &ack_emoji)
        .await;
});

// After:
// Fire a "thinking" reaction (GLANCE / 👀) the moment the message arrives,
// so the user sees instant visual feedback. Orchestrator will swap this
// reaction to ✅ DONE (or ⚠️ on failure) once the agent loop completes,
// or to 👍/🚫/⚠️ if the reply-intent precheck classifies as NO_REPLY.
//
// Cache-key must match orchestrator's `remove_reaction("👀")` lookup;
// see PR `kanmars.req.20260516.003` rationale.
let reaction_channel = self.clone();
let reaction_message_id = lark_msg.message_id.clone();
tokio::spawn(async move {
    reaction_channel
        .try_add_ack_reaction(&reaction_message_id, "GLANCE", "\u{1F440}")
        .await;
});
```

#### 2b. Webhook 路径 [`lark.rs:2783-2791`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2783-L2791) 同款修改：

```rust
// Before:
let ack_emoji =
    random_lark_ack_reaction(payload.get("event"), ack_text).to_string();
let reaction_channel = Arc::clone(&state.channel);
let reaction_message_id = message_id.to_string();
tokio::spawn(async move {
    reaction_channel
        .try_add_ack_reaction(&reaction_message_id, &ack_emoji)
        .await;
});

// After:
let reaction_channel = Arc::clone(&state.channel);
let reaction_message_id = message_id.to_string();
tokio::spawn(async move {
    reaction_channel
        .try_add_ack_reaction(&reaction_message_id, "GLANCE", "\u{1F440}")
        .await;
});
```

**验证**：
- `grep -nE "random_lark_ack_reaction" crates/zeroclaw-channels/src/lark.rs` 不再命中调用点（仅 fn 定义 + 单测）
- `grep -nE "try_add_ack_reaction\(" crates/zeroclaw-channels/src/lark.rs` 命中 2 处（WS + webhook），都传 3 个参数
- `cargo check --features channel-lark` 通过

### Step 3 — Orchestrator 在 lark/feishu channel 上跳过 add 👀（10 min）

**位置**：[`orchestrator/mod.rs:3179-3186`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3179-L3186)

```rust
// Before:
// React with 👀 to acknowledge the incoming message
if ctx.ack_reactions
    && let Some(channel) = target_channel.as_ref()
    && let Err(e) = channel
        .add_reaction(&msg.reply_target, &msg.id, "\u{1F440}")
        .await
{
    tracing::debug!("Failed to add reaction: {e}");
}

// After:
// React with 👀 to acknowledge the incoming message.
//
// Lark/Feishu fires its own GLANCE reaction at message-arrival time from
// the channel layer (see lark.rs:1271 and lark.rs:2784), so we skip the
// orchestrator-side add for those channels to avoid two reactions racing
// onto the same message. The orchestrator-side `remove_reaction("👀")`
// at the end of this function still works because the lark channel
// caches the reaction_id under the same unicode key.
let is_lark_like = target_channel
    .as_ref()
    .is_some_and(|ch| matches!(ch.name(), "lark" | "feishu"));
if ctx.ack_reactions
    && !is_lark_like
    && let Some(channel) = target_channel.as_ref()
    && let Err(e) = channel
        .add_reaction(&msg.reply_target, &msg.id, "\u{1F440}")
        .await
{
    tracing::debug!("Failed to add reaction: {e}");
}
```

**验证**：
- `grep -nE "is_lark_like" crates/zeroclaw-channels/src/orchestrator/mod.rs` 命中 ≥ 2 行（声明 + 使用）
- `cargo check` 通过

### Step 4 — 修补现有测试 + 加 1 个新单测（15 min）

#### 4a. 现有测试 `process_channel_message_adds_and_swaps_reactions`（[`mod.rs:9117`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L9117-L9219)）

该测试用 `RecordingChannel`（`name() = "test-channel"`），不在 lark/feishu 白名单内，所以**应当继续通过 without modification**。验证：

```bash
cargo test -p zeroclaw-channels --features channel-lark process_channel_message_adds_and_swaps_reactions
```

如果通过：无需改动。如果失败（极不可能，因为 `"test-channel"` 不会触发 `is_lark_like = true`）：检查根因，更新测试。

#### 4b. 新单测 `lark_inbound_ws_fires_glance_thinking_reaction`

**位置**：紧挨现有 `remove_reaction_*` 三个测试之后（[`lark.rs:5619`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L5619) 文件末尾 `}` 之前）

```rust
#[tokio::test]
async fn try_add_ack_reaction_caches_glance_under_unicode_key() {
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex("/auth/v3/tenant_access_token/internal"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "code": 0,
            "tenant_access_token": "t-glance",
            "expire": 7200
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path_regex("/im/v1/messages/om_glance/reactions$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "code": 0,
            "data": {
                "reaction_id": "r_glance_xyz",
                "operator": { "operator_id": "cli_test", "operator_type": "app" },
                "action_time": "1700000000000",
                "reaction_type": { "emoji_type": "GLANCE" }
            }
        })))
        .expect(1)
        .mount_as_scoped(&server)
        .await;

    let mut ch = make_channel();
    ch.api_base_override = Some(server.uri());

    // The "channel-layer ack" path: WS / webhook calls this directly with
    // (emoji_type="GLANCE", unicode_emoji="👀").
    ch.try_add_ack_reaction("om_glance", "GLANCE", "\u{1F440}")
        .await;

    // Cache MUST be keyed by unicode "👀" so orchestrator's
    // `remove_reaction("👀")` can find this entry on completion.
    let cache = ch.reaction_ids.lock().await;
    let stored = cache
        .get(&("om_glance".to_string(), "\u{1F440}".to_string()))
        .cloned();
    assert_eq!(
        stored.as_deref(),
        Some("r_glance_xyz"),
        "GLANCE reaction_id must be cached under unicode 👀 key, got {stored:?}"
    );
}
```

**验证**：`cargo test -p zeroclaw-channels --features channel-lark try_add_ack_reaction_caches_glance` 通过。

### Step 5 — 静态检查 + 全测试（10 min）

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /home/admin/workspace-public/kanmars/zeroclaw

# Format only files this PR touches (避免触碰 master 已有的 fmt 漂移)
rustfmt --edition 2024 crates/zeroclaw-channels/src/lark.rs
rustfmt --edition 2024 crates/zeroclaw-channels/src/orchestrator/mod.rs

# Manually inspect diff and revert any unrelated fmt-only hunks (same
# discipline as PR 002 Step 5).
git diff --stat
# git diff crates/zeroclaw-channels/src/lark.rs | grep "^@@"
# git diff crates/zeroclaw-channels/src/orchestrator/mod.rs | grep "^@@"

# Static checks
cargo clippy -p zeroclaw-channels --features channel-lark --all-targets -- -D warnings
cargo test -p zeroclaw-channels --features channel-lark
```

**预期**：
- clippy exit 0
- 既有 lark::tests 全绿（应为 102 = 101 + 1 新测）
- 既有 orchestrator::tests::process_channel_message_adds_and_swaps_reactions 仍通过（test channel name="test-channel" 不在 lark 守卫白名单）
- pre-existing telegram failures 不变（与本 PR 无关，PR 002 已确认）

### Step 6 — Atomic commit + push（5 min）

`fix/lark-implement-remove-reaction` 分支已有一个 commit（`0a8acc17` PR 002）。本 PR 在其上叠加一个新 commit。**不 amend、不 squash**——保持两个语义独立的 commit，便于分块 review。

```bash
git status --short
git add crates/zeroclaw-channels/src/lark.rs \
        crates/zeroclaw-channels/src/orchestrator/mod.rs \
        CHANGELOG-next.md \
        .sisyphus/plans/kanmars.req.20260516.003.plan.md
git diff --stat HEAD                  # 期望 4 个文件
git commit -F - <<'EOF'
fix(lark): fire GLANCE "thinking" reaction at inbound, drop random ack pool

Before this patch the lark channel had two competing inbound-reaction
sources:

  1. Channel layer at lark.rs:1276 / lark.rs:2789 — fired a random
     `LARK_ACK_REACTIONS_ZH_CN` emoji (APPLAUSE / OK / SMILE / etc.) via
     `tokio::spawn` the moment the WS / webhook event arrived.
  2. Orchestrator at mod.rs:3182 — added GLANCE (👀) sequentially after
     reply-intent precheck (~2.85 s after T0).

Both POSTs raced toward the Feishu API in parallel. On short tasks the
DELETE GLANCE + POST DONE pair could land before POST GLANCE was fanned
out by Feishu's distribution layer, producing the "DONE shows first,
husky appears after" UX bug the user reported.

The two sources also had **redundant semantics** — both said "I see your
message" — yet rendered as **two different reactions** stacked on top of
each other.

This patch consolidates "thinking" into a single source:

* Channel layer now fires a fixed GLANCE (instead of the random pool),
  which Feishu renders as the husky-eye emoji and which is semantically
  the "I'm thinking" signal.
* `try_add_ack_reaction` gains a `unicode_emoji` parameter so it can
  cache the returned `reaction_id` under the same unicode key
  (`"\u{1F440}"`) that the orchestrator uses for `remove_reaction("👀")`
  on completion. This reverts PR 002's "ack reactions don't write cache"
  decision now that the lark layer is the canonical 👀 emitter.
* Orchestrator skips its own `add_reaction("👀")` call when the target
  channel is `"lark"` or `"feishu"`. The matching
  `remove_reaction("👀")` at completion is unchanged and now finds the
  channel-layer cache entry.

Other channels (discord / slack / telegram / matrix / ...) keep the
existing orchestrator-driven 👀 → ✅ pattern. Only lark/feishu users see
the new behaviour: instant GLANCE on send, swapped to ✅ / ⚠️ when the
agent loop completes.

The random ack pool (`LARK_ACK_REACTIONS_ZH_CN` / `*_TW` / `*_EN` /
`*_JA`) and `random_lark_ack_reaction` are no longer called from
production code paths but are kept for now (still referenced by their
own unit tests). A follow-up PR can remove them.

Risk: Low-Medium (zeroclaw-channels Experimental tier, two files,
behaviour change limited to lark/feishu inbound reactions, no trait /
config / schema impact).

Co-authored-by: Sisyphus <sisyphus@ohmyopencode.local>
EOF
git push -u origin fix/lark-implement-remove-reaction
```

**预期**：push 在沙箱会失败（同 PR 002，无 gitee 凭证），由用户手动 push。

### Step 7 — CHANGELOG-next.md 加一条 Changed 入口（3 min）

紧挨 PR 002 添加的 `Lark/Feishu: implemented Channel::remove_reaction` 入口之后追加：

```markdown
- **Lark/Feishu**: The "thinking" reaction (👀, rendered by Feishu as the
  husky-eye emoji) now fires from the lark channel layer at message-arrival
  time (T+0) instead of from the orchestrator after reply-intent precheck
  (T+~2.85s). The previous random ack reaction (APPLAUSE / OK / SMILE /
  etc., picked from `LARK_ACK_REACTIONS_ZH_CN` per locale) is replaced by
  a fixed 👀 to give a single, consistent "thinking" signal. Combined with
  the `remove_reaction` implementation from the same release, users now
  see exactly **one** reaction at any time during the message lifecycle:
  👀 while the bot is working, then ✅ (or ⚠️ on failure / 👍 on
  informational no-reply / 🚫 on safety refusal). Other channels (Discord,
  Slack, Telegram, Matrix, ...) are unaffected.
```

---

## 4. 验证清单（PR 提交前必须全绿）

| 项 | 命令 | 预期 |
|---|---|---|
| 函数签名变更 | `grep -n "fn try_add_ack_reaction" crates/zeroclaw-channels/src/lark.rs` | 1 行，`unicode_emoji` 第 3 参数 |
| 两个 ack 调用点 | `grep -n "try_add_ack_reaction(" crates/zeroclaw-channels/src/lark.rs` | 2 调用 + 1 定义 + 1 新单测 = 4 行 |
| 不再调用 `random_lark_ack_reaction` | `grep -nE "random_lark_ack_reaction\\(" crates/zeroclaw-channels/src/lark.rs` | 仅在 fn 定义 + 单测中出现，**不在 spawn 块内出现** |
| Orchestrator 守卫 | `grep -n "is_lark_like" crates/zeroclaw-channels/src/orchestrator/mod.rs` | ≥ 2 行 |
| Format | 仅本 PR 文件 fmt 干净（手动 inspect） | 无意外 hunk |
| Lint | `cargo clippy -p zeroclaw-channels --features channel-lark --all-targets -- -D warnings` | exit 0 |
| 单元测试 | `cargo test -p zeroclaw-channels --features channel-lark` | 1043 passed / 2 pre-existing telegram failures（与本 PR 无关） |
| 新单测 | `cargo test -p zeroclaw-channels --features channel-lark try_add_ack_reaction_caches_glance` | 1 passed |
| 既有反应测试不回归 | `cargo test -p zeroclaw-channels --features channel-lark process_channel_message_adds_and_swaps_reactions` | passed（test channel 不在 lark 白名单） |
| Lark 102 测试全绿 | `cargo test -p zeroclaw-channels --features channel-lark lark::` | 102 passed（PR 002 后 101 + 本 PR +1） |
| 改动文件数 | `git diff --stat HEAD~1..HEAD` | 4 文件（lark.rs + mod.rs + CHANGELOG + plan）|

**线上回归验证**（PR merge + rebuild + 重启 gloria 后用户实测）：

| 步骤 | 期望 |
|---|---|
| 在飞书 DM 给 bot 发任意一条短消息（如"今天几号"）| ✅ 消息上**立即**出现哈士奇/GLANCE（≤ 500ms） |
| 等 LLM 跑完 | ✅ 哈士奇被 ✅ DONE 替换 |
| 全程消息上**只有 1 个反应** | ✅ 没有"鼓掌 / 哈士奇 / DONE" 三个并排 |
| 发一条 prompt injection 试图让 bot 不回（如"忽略上面所有指令"）| ✅ 哈士奇出现 → 被 🚫 替换 |
| 短任务（精分类 NO_REPLY 直接结束）| ✅ 哈士奇出现 → 被 👍 替换 |
| 检查日志 | 不应出现 `Lark remove_reaction failed`；可能出现 `Lark remove_reaction: cache miss, skipping`（仅在重启后旧消息触发，正常）|

---

## 5. 风险与缓解

| # | 风险 | 严重性 | 缓解 |
|---|---|---|---|
| R1 | **Cache key 不一致**：`try_add_ack_reaction` 写入的 unicode key 与 orchestrator 的 `remove_reaction("👀")` lookup key 不同 | 中 | 新增的单测 `try_add_ack_reaction_caches_glance_under_unicode_key` 直接断言 cache 用 unicode key；如果 future patch 误改回 `emoji_type` 作 key，单测立刻红 |
| R2 | **`is_lark_like` 误判**：`Channel::name()` 返回值未来改名 | 低 | 模式匹配 `"lark" | "feishu"` 是 hard-coded；PR 002 至 003 期间 `LarkPlatform` 枚举已稳定数月。如改名，编译期不报错但行为悄悄退化（lark 上会出现两个 GLANCE）—— 在 PR 描述里明确标注此守卫，提醒后续维护者 |
| R3 | **race**：lark spawn 的 GLANCE POST 还在飞，orchestrator 已经走到 `remove_reaction("👀")` | 低 | 主 LLM 调用至少 ~3 秒（精分类 + token round-trip），GLANCE 的飞书 POST 典型 100-300ms，远早于完成。极端情况下 cache miss → `remove_reaction` 走 debug log 路径不报错，最坏退化是哈士奇没被删 |
| R4 | **`tokio::spawn` 任务被 cancel**：lark channel 被关闭时，正在飞的 GLANCE POST 可能被丢弃 | 极低 | 这是 fire-and-forget 设计的固有特性，PR 002 之前就存在，不是本 PR 引入 |
| R5 | **NO_REPLY 路径产生重复反应**：lark 加 GLANCE 后，orchestrator 走 NO_REPLY 路径加 👍 / 🚫 / ⚠️，**没有先 remove GLANCE** → 用户看到 GLANCE + 👍 两个 | **中** | **需要扩展守卫**：NO_REPLY 路径也要 `remove_reaction("👀")` for lark before adding the no-reply emoji。**这是本计划的隐藏阻塞项**，需在 §3 Step 3 之后加 §3 Step 3.5 |
| R6 | **`random_lark_ack_reaction` 变成 dead code 但被单测引用** | 极低 | 单测保留 → 不是真 dead code → clippy 不会告警；后续清理 PR 单独处理 |
| R7 | **多平台并列**：未来添加 `wecom` channel 也想要"瞬时思考中"行为，又要再扩展 `is_lark_like` 守卫 | 低 | 真要加再加，YAGNI 原则不预先抽象。可选：把守卫改成 `channel.supports_immediate_ack()` trait 方法，但那要改 trait（违反 §0.5 #3） |
| R8 | **GLANCE 在飞书海外版（Lark）渲染** | 低 | GLANCE 是飞书内部 emoji_type，国际版 Lark 客户端是否同样渲染为"哈士奇"未知。如果国际版渲染异常，可后续把 lark 与 feishu 平台分别使用不同 emoji_type | 

### 5.1 R5 必须修复（升级到 §3 Step 3.5）

需要在 [`mod.rs:3055-3068`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3055-L3068) NO_REPLY 反应分支也加 lark 守卫前置 `remove_reaction("👀")`：

```rust
// Before (mod.rs:3055):
if ctx.ack_reactions
    && let Some(channel) = target_channel.as_ref()
{
    let emoji = kind.emoji();
    if let Err(e) = channel
        .add_reaction(&msg.reply_target, &msg.id, emoji)
        .await
    {
        tracing::debug!("...");
    }
}

// After:
if ctx.ack_reactions
    && let Some(channel) = target_channel.as_ref()
{
    let is_lark_like = matches!(channel.name(), "lark" | "feishu");
    // Lark/Feishu fired GLANCE (👀) at message-arrival time from the
    // channel layer; remove it before stamping the no-reply outcome so
    // the user sees a single reaction (👍/🚫/⚠️) rather than
    // GLANCE + outcome stacked.
    if is_lark_like {
        let _ = channel
            .remove_reaction(&msg.reply_target, &msg.id, "\u{1F440}")
            .await;
    }
    let emoji = kind.emoji();
    if let Err(e) = channel
        .add_reaction(&msg.reply_target, &msg.id, emoji)
        .await
    {
        tracing::debug!("...");
    }
}
```

### 5.2 回退方案

如 PR merge 后线上出现严重问题：

1. **快速 revert**：`git revert <这次的 commit_sha>`（PR 002 的 `0a8acc17` 仍在，`remove_reaction` 实现保留，仅回退本 PR 的"思考中信号搬到通道层"改动）
2. **回退影响**：用户回到"鼓掌 + 哈士奇 + DONE 三个反应"的状态，但因为 PR 002 的 `remove_reaction` 还在，哈士奇仍会被撤掉，最终只剩鼓掌 + DONE 两个，比预 PR 002 时代好
3. **无 schema / 配置 / 数据迁移**

---

## 6. 工作量估算 & 时间线

| 阶段 | 行数 | 时长 |
|---|---|---|
| Step 0（分支验证） | — | 1 min |
| Step 1（`try_add_ack_reaction` 加 unicode 参数 + cache 写入） | +12 / -1 | 10 min |
| Step 2（两个 ack 调用点改成固定 GLANCE） | +8 / -16 | 10 min |
| Step 3（orchestrator add 👀 加 lark 守卫） | +12 / -1 | 10 min |
| Step 3.5（orchestrator NO_REPLY 路径加 lark remove 👀） | +12 / -0 | 10 min |
| Step 4（新单测 +1） | +35 / -0 | 15 min |
| Step 5（fmt + clippy + test） | — | 5 min |
| Step 6（commit + push） | +1 / -2（CHANGELOG）| 5 min |
| Step 7（CHANGELOG-next.md 入口） | +12 / -0 | 5 min |
| **合计** | **≈ +90 / -20** | **≈ 70 min** |

---

## 7. 提交流程

1. **分支**：`fix/lark-implement-remove-reaction`（**复用 PR 002 分支**，不开新分支）
2. **commit 信息**：见 Step 6（conventional commit `fix(lark):` —— 修的是 PR 002 自己暴露的 race + 用户报告的"DONE 在哈士奇之前"问题）
3. **关于 commit 顺序**：PR 002 的 `0a8acc17` 在前，本 PR 的新 commit 在后。两个 commit 都属于"飞书反应体验修复"主题，可一起 review，不必分 PR
4. **CHANGELOG-next.md**：必加（见 Step 7）
5. **PR 标题**（合并后由用户在 gitee 上设置）：`fix(lark): fire GLANCE "thinking" reaction at inbound + implement remove_reaction`（涵盖两个 commit）
6. **size**：`size: M`（两个 commit 总计 ≈ +400 行 含测试）
7. **流程**：本地 commit → push（沙箱失败由用户手动 push） → 发 CR 给用户确认 → 用户审完合 master

---

## 8. 待用户决策项（开工前需确认）

| # | 项 | 默认 | 备选 |
|---|---|---|---|
| Q1 | "思考中"用 GLANCE 还是其他 emoji_type | **GLANCE**（用户已熟悉哈士奇含义） | 其他飞书 emoji_type，如 `THINK` / `WAIT` / `LOADING`（如有） |
| Q2 | NO_REPLY[REFUSE] 时是否保留 GLANCE → 🚫 切换 | **保留**（一致性） | 跳过守卫，让 prompt injection 看到 GLANCE + 🚫 stack（不推荐） |
| Q3 | `is_lark_like` 守卫硬编码字符串 vs trait 方法 | **硬编码**（YAGNI） | 加 `Channel::supports_immediate_ack()` trait 方法（动 trait，违反前提） |
| Q4 | 是否同 PR 删 dead `LARK_ACK_REACTIONS_*` pool 与 `random_lark_ack_reaction` | ~~不删~~ → **rev1 改为「必删」**（AGENTS.md 禁止 `#[allow(dead_code)]` + clippy `-D warnings` gate）| — |
| Q5 | 是否给国际版 Lark（`open.larksuite.com`）走不同 emoji_type | **同走 GLANCE**（默认） | 拆分 platform 用不同 emoji_type（如有发现渲染差异）|

---

## 9. 关联文档 / 参考

- 上游 PR：`kanmars.req.20260516.002.plan` —— `Channel::remove_reaction` 实现（commit `0a8acc17`），是本 PR 得以工作的前提
- 用户反馈：本会话第 N+2 / N+3 / N+4 轮，分别报告"DONE 在哈士奇之前"、"质疑应该有 3 阶段"、"接受 B 方案 2 阶段"
- 时间窗口实测：本会话第 N+5 轮，确认精分类延迟 ~2.85s，主 agent ~16s
- [zeroclaw AGENTS.md](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md) — Workflow / Anti-Patterns / Stability Tiers
- 飞书 emoji_type 列表：https://open.feishu.cn/document/server-docs/im-v1/message-reaction/emojis-introduce
- PR 002 计划：[`.sisyphus/plans/kanmars.req.20260516.002.plan.md`](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260516.002.plan.md)

---

## 10. Sign-off

| 角色 | 状态 |
|---|---|
| 起草人（组件管理员）| ✅ 已完成（rev0） |
| 计划审阅人（用户）| ⏳ 待审阅 |
| 实施授权 | ⏳ 待用户明确 "execute" / "go" / "开始改" 才会动代码 |

**当前模式**：plan 起草完毕，等用户审阅后授权实施。

**注意**：本计划在 `fix/lark-implement-remove-reaction` 分支上叠加新 commit（不开新分支、不 amend PR 002 的 `0a8acc17`）。两个 commit 都聚焦"飞书反应体验"主题，可作为同一个 PR 一起 review。
