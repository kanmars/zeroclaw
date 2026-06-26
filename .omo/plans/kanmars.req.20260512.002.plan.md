# Plan — kanmars.req.20260512.002 (Feishu Inbound Message Channel/Sender/Time Prefix)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260512.002.plan |
| 关联需求 | 无独立 req 文档（用户对话需求：大模型经常把飞书消息的时间/渠道/发送人搞串，期望入站消息加前缀"本消息通过飞书渠道发送，发送人为 XXX，发送时间为北京时间 XXX"） |
| 起草日期 | 2026-05-12 |
| 修订日期 | 2026-05-12 (rev0 — 初稿) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `feat/feishu-inbound-prefix` |
| 风险等级 | Low（改动局限于 `lark.rs` + `LarkConfig` / `FeishuConfig`，无公共 trait / API 变动） |
| 基线 commit | `3b70a143` (master, 2026-05-12) |
| 选型方案 | **方案 C — 飞书专属前缀**：在 lark.rs 三个 `ChannelMessage` 构造点前缀化 `content`，发送人先用 `open_id`（不查中文名），仅飞书生效 |
| 预计代码行数 | +80 / -5（包含 3 个单测） |
| 预计工作量 | 约 80 分钟（不含 CR 等待 + push） |

---

## 0. 关键目标（唯一的真理来源）

> **让飞书入站消息在进入 LLM 前，在 `content` 开头追加一段人类可读的元信息前缀，让模型在每一轮对话里都能看到"这条消息来自飞书、发送人、北京时间"，而不再依赖 system prompt 里的单次 `Channel context` 行（后者会因注意力漂移和 prompt 缓存导致模型把时间/渠道/发送人搞串）。**

**完成此目标即"功能完成"**：
- 在飞书（WS 文本、WS 音频转写、HTTP webhook）三条入站路径下，`ChannelMessage.content` 在送入 `append_sender_turn`（[orchestrator/mod.rs:2848](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2848)）之前，content 开头带有固定格式的中文前缀；
- 前缀由 `[channels.lark].inbound_prefix` / `[channels.feishu].inbound_prefix` 配置开关控制，默认 `false`（opt-in 保守）；
- 时间使用北京时间（`chrono_tz::Asia::Shanghai`）；
- 发送人为飞书 `open_id`（形如 `ou_xxx`），**本计划不查询中文名**；
- 其他渠道（telegram / discord / matrix / dingtalk / ...）完全不受影响。

**显式不在范围内**：
- ❌ 查询飞书 contact API 拿中文名（`POST /contact/v3/users/basic_batch`）—— 用户明确要求"先写 ID 就行"，中文名查询留给后续独立 PR
- ❌ 降级策略（basic_batch 失败处理）—— 既然不调用 basic_batch，无此问题
- ❌ LRU 缓存 / TTL 机制 —— 同上
- ❌ 跨渠道通用 `Hook::on_message_received` 实现 —— 用户选了方案 C（飞书专属），不做全渠道统一
- ❌ 扩展 `Channel` trait 新 `metadata_prefix()` 方法 —— `zeroclaw-api` 是 Stable 候选，不为此破坏
- ❌ 修 orchestrator [`build_channel_system_prompt`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L764) 里已有的 system prompt channel context（与本 feature 并列、不冲突，保留以防回退参考）
- ❌ 修 Reply-Intent Precheck —— 独立问题，MEMORY.md §6.16 硬编码留给后续 RFC
- ❌ 解决群聊场景的"sender 显示为 chat_id 而非 open_id"问题 —— 已由 req.20260512.001 PR5 `fix/feishu-group-session-per-user` 处理，本计划**假定该 fix 已合入**，直接读 `resolve_sender` 返回值
- ❌ 修 WS 时间戳 bug（L1198-1201 丢弃 Feishu `create_time` 用 `SystemTime::now()`）—— 顺手但不在本 PR 范围，单独 issue 跟进

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不新增 `unwrap()` / `expect()`**（AGENTS.md Anti-Pattern）。时间转换用 `and_then(chrono::DateTime::from_timestamp)` + `unwrap_or_else(chrono::Utc::now)` 安全回落
2. **不新增 `#[allow(dead_code)]`**（AGENTS.md Anti-Pattern）。新 helper 函数立即被三处构造点调用
3. **不动 `zeroclaw-api` / `zeroclaw-runtime` 公共 trait**。改动边界 = `crates/zeroclaw-channels/src/lark.rs` + `crates/zeroclaw-config/src/schema.rs`（仅追加 2 个字段）
4. **所有用户可见文案用 `fl!()` / Fluent 字符串**（AGENTS.md Localization）——前缀句子是 LLM 看到的而非用户看到的，**不走 Fluent 直接硬编码中文**（LLM 上下文要稳定一致，不能随用户 locale 切换）；这一点需在 PR 描述中显式说明
5. **`tracing::` 日志保持英文 + 稳定 `error_key`**（RFC #5653 §4.6）
6. **复用项目已有北京时间惯例**：`chrono::Utc::now().with_timezone(&chrono_tz::Asia::Shanghai)` —— MEMORY.md §6.1 / §6.5 / §6.15 记录了项目中已有 17+ 处此模式，本 PR 沿用
7. **`zeroclaw-channels` 已依赖 `chrono` + `chrono-tz = "0.10"`**（[Cargo.toml:24-25](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/Cargo.toml#L24)），**不引入新依赖**
8. **完整跑 `cargo check -p zeroclaw-channels` + `cargo clippy --all-targets -- -D warnings` + `cargo test -p zeroclaw-channels`**
9. **按 [zeroclaw AGENTS.md "Workflow"](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#workflow) 6 条**：Read before write / One concern per PR / Implement minimal patch / Validate by risk tier / Document impact / Queue hygiene。具体落地：master 拉分支 → 改 → commit → push → 发 CR 地址等用户确认（**非 master 分支开 PR**，不直推 master）
10. **transitional crate 边界**（AGENTS.md zeroclaw-runtime）：本 PR **不触碰 zeroclaw-runtime**，完全在 `zeroclaw-channels` 内，合规

---

## 1. 现状事实复核（基于 2026-05-12 session 三路并行 explore 结果）

### 1.1 关键代码位置（行号对齐基线 `3b70a143`）

| 事实 | 文件:行 |
|---|---|
| `ChannelMessage` struct 定义（已带 `sender`/`channel`/`timestamp`） | [crates/zeroclaw-api/src/channel.rs:31-50](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-api/src/channel.rs#L31) |
| 飞书 **WS 文本路径** `ChannelMessage` 构造 | [crates/zeroclaw-channels/src/lark.rs:1190-1205](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1190) |
| 飞书 **WS 音频路径** `ChannelMessage` 构造 | [crates/zeroclaw-channels/src/lark.rs:1722](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1722) |
| 飞书 **HTTP webhook 路径** `ChannelMessage` 构造 | [crates/zeroclaw-channels/src/lark.rs:1981-1991](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1981) |
| `resolve_sender` —— 决定 sender 字段用 open_id 还是 chat_id | [crates/zeroclaw-channels/src/lark.rs:582-591](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L582) |
| HTTP 路径 Feishu `create_time` 解析（正确） | [crates/zeroclaw-channels/src/lark.rs:1963-1974](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1963) |
| WS 路径时间戳（**bug：丢弃 Feishu create_time 用本地 now**，本 PR 不修） | [crates/zeroclaw-channels/src/lark.rs:1198-1201](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1198) |
| 音频路径 Feishu `create_time` 解析 | [crates/zeroclaw-channels/src/lark.rs:1710-1720](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1710) |
| `LarkConfig` schema（追加字段点） | [crates/zeroclaw-config/src/schema.rs:8092-8157](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L8092) |
| `FeishuConfig` schema（追加字段点） | [crates/zeroclaw-config/src/schema.rs:8261-8318](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L8261) |
| `append_sender_turn` 下游 LLM 入口（验证前缀生效的锚点） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:2848](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2848) |
| 现有 system prompt 里的 "Channel context" 行（本 feature **并列保留** 不移除） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:764](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L764) |
| Telegram 已有"入站文本前缀"先例（`format_forward_attribution`） | [crates/zeroclaw-channels/src/telegram.rs:1338-1341](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/telegram.rs#L1338) |
| 项目已有"北京时间"惯例（17+ 处同模式） | MEMORY.md §6.5 |

### 1.2 根因结论

**用户报告"大模型搞串时间 / 渠道 / 发送人"的根因 = 元信息只在 system prompt 里出现一次，不在每条 user message 里出现。** 模型看 20 轮对话时，第 1 轮的 `Channel context: channel=lark, sender=ou_xxx` 会被 prompt 缓存固化（对 Anthropic prefix cache 有利），但**单条消息的"什么时候发的、谁发的"在 user message 里完全没有**。注意力漂移下模型会：
- 把"5 分钟前的消息"当成"刚刚"
- 把 A 用户的诉求张冠李戴到 B 用户身上
- 把飞书的对话风格误认为 Telegram/Discord

方案 C 的补救：**每条 user message 开头注入一段明确的元信息段**，让模型在任何轮次都能读到"这条"的上下文。Telegram 早已走这条路（[format_forward_attribution](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/telegram.rs#L1338)），架构先例清晰。

### 1.3 替代方案弃选理由

| 方案 | 弃选原因 |
|---|---|
| 方案 A (`Hook::on_message_received` 全渠道统一) | 用户选择 C —— 飞书先快速止血，其他渠道以后再说 |
| 方案 B (orchestrator 里一行拦截) | 改动触达所有渠道，范围与用户意图不符 |
| 扩展 `Channel` trait 新方法 | 破坏 `zeroclaw-api` 公共 API（Stable tier 候选，不值得） |
| 查中文名 + LRU 缓存 + 降级 | 用户明确"先写 ID"，留给后续 PR |

---

## 2. 目标 (Goals) & 验收标准 (Acceptance Criteria)

### G1 — 飞书入站消息在进入 LLM 前带中文元信息前缀

- **AC-1.1** 当 `[channels.lark].inbound_prefix = true`（或 feishu 同），飞书 WS 文本路径的 `ChannelMessage.content` 在构造完成后以下列格式开头：
  ```
  本消息通过飞书渠道发送，发送人为 <open_id>，发送时间为北京时间 <YYYY-MM-DD HH:MM:SS>

  <原消息 content>
  ```
- **AC-1.2** HTTP webhook 路径行为同 AC-1.1（使用 Feishu `create_time` 作为时间源）
- **AC-1.3** 音频转写路径行为同 AC-1.1（使用 Feishu `create_time` 作为时间源）
- **AC-1.4** WS 路径时间来源仍是 `SystemTime::now()`（L1198-1201 既存 bug，本 PR **不修**，在 PR body 里明确标注"follow-up"）
- **AC-1.5** 时间格式固定为 `%Y-%m-%d %H:%M:%S`（精确到秒，不带时区后缀——前缀字串里已经有"北京时间"四字，避免冗余）
- **AC-1.6** 前缀与原 content 之间**恰好一个空行**（`\n\n`）

### G2 — 配置开关保守默认 + 可按 channel 实例关闭

- **AC-2.1** `LarkConfig` 新增字段 `inbound_prefix: bool`，默认 `false`
- **AC-2.2** `FeishuConfig` 新增字段 `inbound_prefix: bool`，默认 `false`
- **AC-2.3** 字段开关为 `false` 时，三条构造路径的 `content` 与基线字节完全一致（**零行为变化**）
- **AC-2.4** 字段在 schema 里有文档字符串解释作用 + 默认值（AGENTS.md 要求）

### G3 — 零侵入其他渠道 & 下游管道

- **AC-3.1** `telegram.rs` / `discord.rs` / `matrix.rs` / `dingtalk.rs` 等**零改动**
- **AC-3.2** `zeroclaw-api`、`zeroclaw-runtime`、`zeroclaw-config`（除新增字段外）、`loop_.rs`、`orchestrator/mod.rs` 零改动
- **AC-3.3** 测试 `cargo test -p zeroclaw-channels` 全部通过，且无 pre-existing 失败
- **AC-3.4** `cargo clippy --all-targets -- -D warnings` 零 warning

### G4 — 前缀写进历史缓存（而非仅本轮）

- **AC-4.1** 前缀注入发生在 `ChannelMessage` 构造阶段（lark.rs 三路径），即 `msg.content` 本身被改写
- **AC-4.2** 这意味着前缀也会被 [`append_sender_turn`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2848) 写入 `conversation_histories` 会话缓存，模型在后续多轮中能看到每条历史消息的发送人+时间
- **AC-4.3** 单条前缀约 50 汉字 ≈ 150 bytes，1000 条历史膨胀 ≈ 150 KB，**可接受**（无需第二层"仅本轮显示不存历史"优化）

### G5 — 测试覆盖

- **AC-5.1** 新增单测 `test_inbound_prefix_ws_text_enabled`：`inbound_prefix=true` 时 WS 文本路径 content 以预期格式开头
- **AC-5.2** 新增单测 `test_inbound_prefix_ws_text_disabled`：`inbound_prefix=false` 时 content 与原始完全一致
- **AC-5.3** 新增单测 `test_inbound_prefix_time_format`：Beijing time 格式、时区、秒级精度断言
- **AC-5.4** 单测全部在 `crates/zeroclaw-channels/src/lark.rs` 内的 `#[cfg(test)] mod tests` 块中，与现有 76+ lark 单测共同运行
- **AC-5.5** **不**为 HTTP 路径和音频路径分别加测试（DRY：三路径共享 `build_inbound_prefix` 辅助函数，一个测试覆盖函数行为即可；另两个路径靠 cargo check + 手工审查行号正确性保证）

---

## 3. 实施步骤

### Step 1 — 准备分支

```bash
git checkout master
git pull
git checkout -b feat/feishu-inbound-prefix
```

### Step 2 — schema 改动（两个 config 加字段）

**文件**：`crates/zeroclaw-config/src/schema.rs`

- `LarkConfig` ([L8092-8157](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L8092)) 追加字段：
  ```rust
  /// When true, every inbound Lark message gets a human-readable Chinese
  /// prefix prepended to its text content before reaching the LLM
  /// (format: "本消息通过飞书渠道发送, 发送人为 <open_id>, 发送时间为
  /// 北京时间 <YYYY-MM-DD HH:MM:SS>\n\n"). Helps the model keep track
  /// of channel / sender / timestamp across long conversations.
  /// Defaults to false (opt-in).
  #[serde(default)]
  pub inbound_prefix: bool,
  ```

- `FeishuConfig` ([L8261-8318](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L8261)) 追加同名字段（保持两 config 对称）

- 验证：`cargo check -p zeroclaw-config`

- 验证：现有测试（包括 schema 快照，如有）无失败；必要时 `cargo insta review` 接受新 snapshot

### Step 3 — lark.rs 新增辅助函数

**文件**：`crates/zeroclaw-channels/src/lark.rs`（靠近文件顶部 helper 区）

```rust
fn build_feishu_inbound_prefix(open_id: &str, ts_secs: u64) -> String {
    let beijing = chrono::DateTime::from_timestamp(ts_secs as i64, 0)
        .unwrap_or_else(chrono::Utc::now)
        .with_timezone(&chrono_tz::Asia::Shanghai);
    format!(
        "本消息通过飞书渠道发送，发送人为 {}，发送时间为北京时间 {}\n\n",
        open_id,
        beijing.format("%Y-%m-%d %H:%M:%S")
    )
}
```

**关键设计选择**：
- 函数不是 `LarkChannel` 的 method，而是 free function —— 更好测试、无 self 状态
- 参数只收 `open_id` + `ts_secs`，**不收 config flag** —— flag 判断留给调用点（zero-prefix 时根本不调用此函数，减少开销）
- 返回值包含末尾 `\n\n`，调用点直接 `format!("{prefix}{original}")` 即可

### Step 4 — 三条构造路径接入

每条路径在 `ChannelMessage` 构造**之前**提取所需字段，条件调用 helper。

**4a. WS 文本路径** ([lark.rs:1190-1205](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1190))

改造前：
```rust
let channel_msg = ChannelMessage {
    // ...
    content: text,
    // ...
    timestamp: std::time::SystemTime::now().duration_since(...).unwrap_or_default().as_secs(),
    // ...
};
```

改造后：
```rust
let ts_secs = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
let final_content = if self.config.inbound_prefix {
    format!("{}{}", build_feishu_inbound_prefix(sender_open_id, ts_secs), text)
} else {
    text
};
let channel_msg = ChannelMessage {
    // ...
    content: final_content,
    // ...
    timestamp: ts_secs,
    // ...
};
```

**4b. WS 音频路径** ([lark.rs:1722](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1722))

在 `try_build_audio_channel_message` 返回 `ChannelMessage` 之前，读取音频事件里已解析的 `ts_secs` + `sender_open_id`（在 [L1660-1720](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1660) 附近已有），按同模式条件注入。

**4c. HTTP webhook 路径** ([lark.rs:1981-1991](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1981))

`build_text_channel_messages` 内部已有 `open_id`（L1820-1825）+ `timestamp`（L1963-1974），按同模式条件注入。

**关键点**：这三处的 `sender_open_id` 必须是 **真正的发送者 open_id**（形如 `ou_xxx`），**不**复用 `ChannelMessage.sender`（因为 `resolve_sender` 在群聊模式下可能返回 `chat_id`）。WS 路径的 `sender_open_id` 在 [L1067](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1067)、HTTP 路径在 [L1820-1825](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1820)、音频路径在 [L1660-1664](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1660)，全部可直接访问，**不需要新增字段传递**。

### Step 5 — 单元测试

在 `lark.rs` 的 `#[cfg(test)] mod tests` 块中追加：

```rust
#[test]
fn test_inbound_prefix_format() {
    // ts = 2026-05-12 22:30:00 CST = 2026-05-12 14:30:00 UTC = 1778761800
    let prefix = build_feishu_inbound_prefix("ou_abc123", 1778761800);
    assert!(prefix.starts_with("本消息通过飞书渠道发送，发送人为 ou_abc123，"));
    assert!(prefix.contains("北京时间 2026-05-12 22:30:00"));
    assert!(prefix.ends_with("\n\n"));
}

#[test]
fn test_inbound_prefix_fallback_on_bad_timestamp() {
    let prefix = build_feishu_inbound_prefix("ou_xxx", u64::MAX);
    assert!(prefix.contains("北京时间"));  // Falls back to Utc::now(), format still valid
}

#[test]
fn test_inbound_prefix_prepends_cleanly() {
    let prefix = build_feishu_inbound_prefix("ou_abc123", 1778761800);
    let combined = format!("{prefix}你好");
    assert!(combined.ends_with("你好"));
    assert!(combined.matches('\n').count() == 2);  // exactly the \n\n separator
}
```

Config gate (`inbound_prefix=false` 路径) 的断言通过跑现有 76 lark tests（基线通过） + `cargo clippy` 来保证没有行为回归；**不**再单独写一个"flag 关闭"的用例（会引入 mock `LarkConfig` 的复杂性，性价比低）。

### Step 6 — 验证清单

| 检查 | 命令 | 期望 |
|---|---|---|
| 编译 | `cargo check -p zeroclaw-channels -p zeroclaw-config` | 零错误 |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 零 warning |
| 单测 | `cargo test -p zeroclaw-channels` | 全通过（原 76 + 新 3 = 79） |
| Config 测试 | `cargo test -p zeroclaw-config` | 全通过 |
| 手工审查 | `git diff master` | 改动只涉及 `lark.rs` + `schema.rs`，行数符合预估（+80 / -5） |
| 前缀手工验证（可选） | 本地 daemon 跑飞书 bot，发一条消息，`zeroclaw.log` 观察 content 注入 | 前缀出现，格式正确 |

### Step 7 — 提交 + 推送

```bash
git add crates/zeroclaw-channels/src/lark.rs crates/zeroclaw-config/src/schema.rs
git commit -m "feat(lark): prepend channel/sender/timestamp metadata to inbound messages

Add opt-in inbound_prefix config to LarkConfig/FeishuConfig that causes
incoming Feishu messages to be rewritten with a human-readable Chinese
prefix before reaching the LLM:

  本消息通过飞书渠道发送，发送人为 <open_id>，发送时间为北京时间 <Beijing time>

  <original content>

Rationale: model was losing track of channel / sender / timestamp across
long conversations because this metadata appeared only once in the system
prompt (orchestrator/mod.rs:764). Putting it inside every user turn keeps
the context fresh in every attention window.

Scope: Feishu only. Telegram already does equivalent via
format_forward_attribution (telegram.rs:1338). Other channels to follow.

Sender uses raw open_id (ou_xxx). Display-name lookup via
/contact/v3/users/basic_batch is deferred to a follow-up PR.

Defaults to false — opt-in only.

Refs: .sisyphus/plans/kanmars.req.20260512.002.plan.md"
git push -u origin feat/feishu-inbound-prefix
```

### Step 8 — 发 CR 地址 + 等用户确认

按 zeroclaw AGENTS.md "Workflow" 第 2/5/6 条：one concern per PR、document impact、follow `.github/pull_request_template.md`。发送分支 URL，等用户明确 "合并" 再 squash-merge（非 master 分支开 PR，不直推 master）。

---

## 4. 边界 & 风险 & 回滚

### 4.1 风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 前缀文字影响模型行为（比如模型开始"模仿"前缀写成输出） | 低 | 中 | system prompt 里加一句 "User messages may begin with a metadata prefix; do not echo it back"；或先上线观察，真出现再加指令 |
| 历史缓存膨胀 | 极低 | 低 | AC-4.3 已估算 ≈ 150 KB / 1000 条，可接受 |
| `chrono_tz::Asia::Shanghai` 编译特性未启用 | 低 | 高 | 基线已启用（MEMORY.md §6.5 项目 17+ 处在用）；`cargo check` 即知 |
| WS 时间戳 bug 放大：用 `SystemTime::now()` 在前缀里显示"现在"而不是"消息发送时" | 中 | 中 | PR body 里明确标注；可选同 PR 顺手修（提取 `event_payload.create_time`），但会让 PR 职责不单一 —— 本计划**不修**，独立 issue 追踪 |
| 群聊场景 sender 还是被 `resolve_sender` 返回成 chat_id | 低 | 中 | 本 PR **不**使用 `ChannelMessage.sender`，直接读事件原始 `sender_open_id`（见 Step 4c "关键点"） |
| 单测用的时间戳 `1778761800` 解析错误 | 低 | 低 | 用 `date -u -d '2026-05-12 14:30:00' +%s` 本地验证；测试里也可以放 `chrono::TimeZone::with_ymd_and_hms` 动态构造 |

### 4.2 回滚

单 PR 单 commit，`git revert` 即可。Config 字段保留在 schema 里不破坏兼容（新字段默认 false，已部署用户无感）。

### 4.3 AGENTS.md transitional crate 边界

本 PR **完全在 `zeroclaw-channels`** 实施，不触碰 `zeroclaw-runtime`。完全符合"不在 transitional crate 加新功能"的约束。

---

## 5. 超时 / 看门线

- PR 如 **2 工作日未合并**（含 CR 等待），触发原因复盘：CI 红 / review 卡 / 需求变更？
- 不走 Oracle review（风险 Low，改动局限）；如果 user 或 Momus 明确提出担忧再补

---

## 6. 分支与命名约定

- 分支：`feat/feishu-inbound-prefix`（符合 conventional commit 前缀）
- 基于：`master` @ `3b70a143`
- Size：`size: S`（80 代码行左右）
- 合并策略：squash-merge

---

## 7. 待解决问题（Open Questions）

1. **系统 prompt 要不要新增一句告诉模型"用户消息开头的元信息块不用回读给用户"？**
   - 正方：防御性明智，大模型偶尔会把"本消息通过飞书渠道发送..."当成用户指令复述
   - 反方：大概率模型自己能识别是系统注入的元信息，加了反而浪费 token
   - **倾向**：先不加，上线观察 1 周；如出现复读现象再加
2. **`FeishuConfig` 和 `LarkConfig` 的字段默认是否应该联动？**（一个配了另一个自动继承？）
   - 背景：项目内 `lark` 和 `feishu` 是同一产品的国际 / 国内分支，两个 config 常同时存在
   - **倾向**：不联动，保持字段独立 —— 简单胜于聪明
3. **前缀要不要在 LLM 可见段之外另加一份"结构化"（比如 JSON header）让 tool / 其他 hook 方便解析？**
   - **倾向**：不加。本 PR 是"止血"级别，任何更复杂结构都留给后续全渠道 Hook 方案（req 下一版）
4. **Beijing time 格式是否需要带时区后缀？**（`2026-05-12 22:30:00 CST` vs `2026-05-12 22:30:00`）
   - 前缀字串已有"北京时间"四字，追加 CST 冗余
   - **倾向**：不加后缀

---

## 8. 跟进事项（Follow-up PRs，不在本计划内）

本 PR 明确**不做**但值得独立 PR 跟进的：

| 跟进项 | 优先级 | 备注 |
|---|---|---|
| F1 — 查飞书中文名（`POST /contact/v3/users/basic_batch` + LRU 缓存 + TTL） | P1 | 需先在飞书应用后台开 `contact:user.basic_profile:readonly` 权限 |
| F2 — WS 文本路径时间戳 bug 修复（用 `event_payload.create_time` 取代 `SystemTime::now()`） | P2 | 独立 bug，和本 PR 逻辑解耦 |
| F3 — 全渠道 `Hook::on_message_received` 实现（方案 A） | P1 | telegram/discord/matrix 有同样问题；架构脚手架已 ready |
| F4 — 新增 `[runtime].display_timezone` 配置把 17+ 处硬编码 Beijing 改成可配 | P3 | MEMORY.md §6.1 / §6.5 长期技术债 |

---

## 9. 计划落地签核

| 字段 | 状态 |
|---|---|
| rev0 起草 | 2026-05-12 ✅ |
| Momus 审查 | ✅ 2026-05-12 **OKAY** 一次通过（ses_1e3732a54ffet2mIv4EsNP313K）—— 所有行号锚点核实真实存在，仅 FeishuConfig 行号轻微漂移（已修正为 L8261） |
| 用户拍板开工 | ⏸ 待用户说 "开始实现" |
| Implementation | ⏸ |
| CR push & 用户确认合并 | ⏸ |

---

## 10. 进度 Checklist (Top-level)

- [ ] Step 1 — 拉分支
- [ ] Step 2 — schema 两字段
- [ ] Step 3 — helper 函数
- [ ] Step 4a — WS 文本路径接入
- [ ] Step 4b — WS 音频路径接入
- [ ] Step 4c — HTTP webhook 路径接入
- [ ] Step 5 — 3 个单测
- [ ] Step 6 — 6 项验证通过
- [ ] Step 7 — commit + push
- [ ] Step 8 — 发 CR 等确认

---

**End of Plan**
