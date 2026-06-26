# Plan — kanmars.req.20260512.001 (Feishu/Lark Channel Feature Parity with Telegram)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260512.001.plan |
| 关联需求 | 无独立 req 文档（用户直接对话需求：飞书 channel 不支持 cron_add，全面评估能力缺失） |
| 起草日期 | 2026-05-12 |
| 修订日期 | 2026-05-12 (rev0 — 初稿，待 Momus 审查) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支（顺序执行） | PR1: `fix/feishu-cron-delivery-enum` → PR2: `feat/feishu-reply-intent-reactions` → PR3: `feat/feishu-request-approval` → PR4: `feat/feishu-draft-streaming` → PR5: `fix/feishu-group-session-per-user` → PR6: `chore/channels-cron-tool-descs-parity` → PR7: `refactor/lark-unify-ws-http-parser` |
| 风险等级 | PR1=Low / PR2=Low / PR3=Medium / PR4=Medium / PR5=Medium / PR6=Low / PR7=Low |
| 选型方案 | **7 个独立 PR 顺序发车**（遵循 AGENTS.md "One concern per PR"），P0 两个先合即可解决用户痛点 |

---

## 0. 关键目标（唯一的真理来源）

> 让飞书 (Lark / Feishu) channel 支持以 `cron_add` 为代表的"定时任务 + 主动推送"能力，并在 `Channel` trait 覆盖面上对齐 telegram（12/18 方法覆盖）。**消除"用户在飞书说『5 分钟后提醒我喝水』无反应"的体验断层。**

**完成此目标即"功能完成"**：一个真实飞书用户能在群聊/私聊里通过自然语言触发 `cron_add` 定时任务，定时到点后收到飞书卡片提醒；bot 决定不回复时能看到 emoji 反馈；medium-risk 工具能被审批；流式消息能 in-place 更新而非刷屏。

**显式不在范围内**：
- ❌ 改 `cron_add` / `cron_update` 工具之外的 cron 基础设施（scheduler 循环、`CronJob` schema、`DeliveryConfig` 结构、`DELIVERY_FN` OnceLock 机制）—— 这些在 telegram 已跑通，本计划仅把飞书接入现有管道
- ❌ 改 `zeroclaw-runtime` / `zeroclaw-config` / `zeroclaw-api` 的公共 trait 签名 —— 所有改动限制在 `zeroclaw-channels` crate 内（PR1/PR6 除外，它们改 `cron_add.rs` 的 schema 字符串和 `build_channel_tool_descs`）
- ❌ 改 DingTalk / WeCom / WhatsApp 等其他 channel（虽然 PR1 的 enum 扩展顺手包含它们，但 trait 覆盖缺失不在本计划修）
- ❌ 引入新依赖（不动 `Cargo.toml`）
- ❌ 改 Reply-Intent Precheck 本身（MEMORY.md §6.16 硬编码问题留给独立 RFC）
- ❌ 实现飞书语音消息接入（lark.rs:986-988 的 audio skip 是独立工作量，非本计划）
- ❌ 实现 `pin_message` / `unpin_message` / `redact_message` / `request_choice`（P3 级别，留待后续）
- ❌ 改 start_typing/stop_typing（飞书卡片模拟 typing 工程量大且不是用户痛点）
- ❌ Gateway 模式下的 cron delivery（PR1 Path B fallback match 加飞书臂是防御性改动，优先级低）

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不新增 `unwrap()` / `expect()`**（AGENTS.md Anti-Pattern #9）。所有 callback 回调、卡片解析、HTTP 调用错误必须 propagate 或 `?`。
2. **不新增 `#[allow(dead_code)]`**（AGENTS.md Anti-Pattern #8）。每个新 helper 必须立即被 ≥1 处生产代码调用。
3. **不动 `compatible.rs` / `loop_.rs` / `channel.rs` trait 定义**。本计划的改动边界 = `cron_add.rs`（仅 enum） + `cron_update.rs`（仅 enum，如存在相同 enum） + `lark.rs` + `orchestrator/mod.rs`（build_channel_tool_descs + deliver_announcement fallback）。
4. **每个 PR 独立可合、独立可回滚**。PR2 不依赖 PR1 的 bug 已修，PR3 不依赖 PR2 的 reaction 已通 —— 7 个 PR 可任意顺序合并（推荐顺序是价值优先级排序，非依赖排序）。
5. **飞书 API 调用必须复用现有 `LarkChannel` 内部的 tenant token 刷新 / rate-limit / proxy 机制**。禁止绕过 `LarkChannel` 内部已有的 HTTP client 自建新的。
6. **每个 PR 必须独立提交 Code Review + 等用户确认再合并**（符合 MEMORY.md §4.1 "基础设施组件 5 步流程"）。
7. **安全 / 审批 PR（PR3）必须过 Oracle review**。其余 PR 根据风险等级决定是否走 Oracle。
8. **每个 PR 必须完整跑通 `cargo check -p zeroclaw-channels` + `cargo clippy --all-targets -- -D warnings` + `cargo test -p zeroclaw-channels`**。触碰 `cron_add.rs` 的 PR1 额外跑 `cargo test -p zeroclaw-runtime`。
9. **MEMORY.md §6.4 的"飞书不热重载"过时认知需在 PR1 的 commit body 或 PR 描述中明确纠正**，并更新 MEMORY.md（在一个 PR 末尾或独立 chore commit）。

---

## 1. 现状事实复核（基于 2026-05-12 分析会话四路并行 explore 结果）

### 1.1 Channel trait 覆盖对比（精确统计）

| 方法 | Telegram | Lark/Feishu | 备注 |
|---|---|---|---|
| `name` | ✅ | ✅ | 都实现 |
| `send` | ✅ | ✅ | 都实现 |
| `listen` | ✅ | ✅ | 都实现 |
| `health_check` | ✅ | ✅ | 都实现 |
| `start_typing` | ✅ | ❌ default no-op | P2 — 不在本计划 |
| `stop_typing` | ✅ | ❌ default no-op | P2 — 不在本计划 |
| `supports_draft_updates` | ✅ 返回 `stream_mode != Off` | ❌ default false | **PR4 修** |
| `supports_multi_message_streaming` | ❌ default | ❌ default | — |
| `multi_message_delay_ms` | ❌ default 800 | ❌ default 800 | — |
| `send_draft` | ✅ | ❌ default `Ok(None)` | **PR4 修** |
| `update_draft` | ✅ | ❌ default `Ok(())` | **PR4 修** |
| `update_draft_progress` | ❌ default | ❌ default | — |
| `finalize_draft` | ✅ | ❌ default | **PR4 修** |
| `cancel_draft` | ✅ | ❌ default | **PR4 修** |
| `add_reaction` | ❌ default（telegram 有内部 `try_add_ack_reaction_nonblocking` 但不覆盖 trait） | ❌ default（同上：`try_add_ack_reaction` 是内部函数） | **PR2 修飞书**（telegram 同样的问题留给独立 PR） |
| `remove_reaction` | ❌ default | ❌ default | — |
| `pin_message` | ❌ default | ❌ default | P3 — 不在本计划 |
| `unpin_message` | ❌ default | ❌ default | P3 — 不在本计划 |
| `redact_message` | ❌ default | ❌ default | P3 — 不在本计划 |
| `request_approval` | ✅ inline keyboard | ❌ default `Ok(None)` → auto-deny | **PR3 修** |
| `request_choice` | ❌ default | ❌ default | P3 — 不在本计划 |
| `supports_free_form_ask` | ✅ default true（继承） | ✅ default true（继承） | — |
| **合计覆盖** | **12/18** | **4/18** | **目标：提升到 10/18** |

### 1.2 关键代码位置（行号对齐 HEAD）

| 事实 | 文件:行 |
|---|---|
| `cron_add` tool schema `delivery.channel` enum **漏飞书** | [crates/zeroclaw-runtime/src/tools/cron_add.rs:147-150](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/cron_add.rs#L147) |
| `LarkChannel::send` 已实现（卡片 + token 刷新） | [crates/zeroclaw-channels/src/lark.rs:1787](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1787) |
| `LarkChannel` impl Channel 只覆盖 4 方法 | [crates/zeroclaw-channels/src/lark.rs:1782-1830](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1782) |
| `try_add_ack_reaction` 内部函数（反应 API 已通） | [crates/zeroclaw-channels/src/lark.rs:625](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L625) |
| `CRON_CHANNEL_REGISTRY` 按 `ch.name()` 注册 | [crates/zeroclaw-channels/src/orchestrator/mod.rs:115](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L115) + [:5644](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L5644) |
| `deliver_announcement` Path A 注册表查询（飞书能 work） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:5903](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L5903) |
| `deliver_announcement` Path B fallback match **无飞书臂** | [crates/zeroclaw-channels/src/orchestrator/mod.rs:5909-5996](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L5909) |
| `build_channel_tool_descs` 仅列 `"schedule"` 不列 `cron_*` | [crates/zeroclaw-channels/src/orchestrator/mod.rs:689](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L689) |
| `loop_::process_message` 路径宣传 6 个 `cron_*` | [crates/zeroclaw-runtime/src/agent/loop_.rs:2367-2384](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/agent/loop_.rs#L2367) |
| 飞书群聊 sender = chat_id（历史串话） | [crates/zeroclaw-channels/src/lark.rs:1021-1022](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1021) + [:1551](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1551) |
| WS 解析器 | [crates/zeroclaw-channels/src/lark.rs:918-1042](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L918) |
| HTTP 解析器（近乎重复） | [crates/zeroclaw-channels/src/lark.rs:1647-1730](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1647) |
| `FeishuConfig` schema | [crates/zeroclaw-config/src/schema.rs:8240-8291](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L8240) |
| `LarkConfig` schema | [crates/zeroclaw-config/src/schema.rs:8092-8158](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L8092) |

### 1.3 根因结论

**用户报告 "飞书不支持 cron_add" 的直接根因 = [`cron_add.rs:149`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/cron_add.rs#L149) 的 JSON schema `delivery.channel` enum 没有 `"feishu"` / `"lark"`**。严格 schema 模式的 LLM（Anthropic tool_use、OpenAI strict mode）发不出工具调用；非严格模式即使发出，`serde_json::from_value::<DeliveryConfig>` 也会在运行期失败。其他所有基础设施（`CRON_CHANNEL_REGISTRY`、`LarkChannel::send`、`register_delivery_fn`、`process_channel_message` 热重载）**都已正确工作**。

---

## 2. 目标 (Goals) & 验收标准 (Acceptance Criteria)

### G1 — 飞书可调 cron_add，定时任务到点通过飞书卡片推送
- **AC-1.1** LLM 能在飞书 channel 的 system prompt 下发出 `cron_add` tool call，`delivery.channel="feishu"` 通过 JSON schema 校验
- **AC-1.2** 定时任务到点后，飞书用户在原 chat 收到卡片消息（复用 `LarkChannel::send` 路径）
- **AC-1.3** `cron_list` 能列出飞书用户创建的定时任务
- **AC-1.4** 在 daemon 正常运行的场景下，`CRON_CHANNEL_REGISTRY` 查到 `"feishu"` → 用活跃 channel 实例推送
- **AC-1.5** 在 gateway-only 或 registry 冷启动场景下，`deliver_announcement` fallback match 能从 `FeishuConfig` / `LarkConfig` 构造新 `LarkChannel` 推送（PR1 加 fallback 臂）

### G2 — Reply-Intent Precheck 的 emoji 反馈在飞书可见
- **AC-2.1** Bot 决定"不回复"时，飞书原消息上出现 👍（informational） / 🚫（refuse） / ⚠️（fail） emoji 反应
- **AC-2.2** `LarkChannel::add_reaction` 复用现有 `try_add_ack_reaction` 逻辑，调用飞书 `/im/v1/messages/<id>/reactions` API
- **AC-2.3** 飞书 API 失败（token 过期、消息已撤回、权限不足）时降级 warn log，不 panic 不阻塞主流程

### G3 — 飞书支持 medium-risk 工具审批
- **AC-3.1** `LarkChannel::request_approval` 发送带 Approve/Deny 按钮的交互卡片
- **AC-3.2** 用户点击按钮 → 飞书 callback webhook 路由 → 唤醒 `pending_approvals: HashMap<approval_id, oneshot::Sender>`
- **AC-3.3** 超时（默认 120s）未点击 → `Ok(None)` 自动 deny
- **AC-3.4** 审批结果（Approve / Deny / Timeout）在卡片上以视觉状态反映（按钮变灰 + 显示结果文字）
- **AC-3.5** 不实现 "Approve Always" 快捷键（MVP，后续补）

### G4 — 飞书流式消息体验升级
- **AC-4.1** `LarkChannel::supports_draft_updates()` 返回 `stream_mode != Off`
- **AC-4.2** `send_draft` 发送占位卡片，返回 `message_id`
- **AC-4.3** `update_draft` / `update_draft_progress` 调用飞书 `PATCH /im/v1/messages/<id>` 更新卡片内容，rate-limit 默认 1000ms
- **AC-4.4** `finalize_draft` 把占位卡片替换成最终内容卡片
- **AC-4.5** `cancel_draft` 把卡片更新为"已取消"状态或删除（按飞书能力选）
- **AC-4.6** 长消息（>LARK_CARD_MARKDOWN_MAX_BYTES）chunking 逻辑在 draft 路径同样生效

### G5 — 群聊会话不再串话
- **AC-5.1** 飞书群聊中，`ChannelMessage.sender` 使用 `open_id`（或 `union_id`） 而非 `chat_id`
- **AC-5.2** `reply_target` 保持 `chat_id`（回复目标不变）
- **AC-5.3** 单聊场景下行为向后兼容（sender 变化不破坏现有历史；若必要，加 migration 说明）
- **AC-5.4** `conversation_history_key = channel + chat_id + sender` 能区分群内多人

### G6 — Channels 路径与 loop_ 路径工具文档对齐
- **AC-6.1** `build_channel_tool_descs` 列出完整 6 个 `cron_*` 工具（与 `loop_.rs:2367-2384` 一致）
- **AC-6.2** 或抽公共 `fn channel_cron_tool_descs() -> Vec<(&str,&str)>` 供两路复用
- **AC-6.3** 不引入重复：如果抽公共函数，`loop_.rs` 改为调用之

### G7 — 降低 lark.rs 维护成本
- **AC-7.1** WS 与 HTTP 两路解析器共用 `fn parse_lark_event(content: &LarkMsgContent) -> Option<ChannelMessage>`
- **AC-7.2** 单元测试覆盖 text/post/image/file/audio 5 种消息类型
- **AC-7.3** 行数净减（签名需要：PR7 diff stat `- > +`）

### G-Memory — 纠正 MEMORY.md §6.4 过时认知
- **AC-M.1** 在 PR1 的 commit body 或独立 chore commit 中说明 `req kanmars.req.20260506.001` 已修复飞书热重载
- **AC-M.2** 下次会话我会手动更新 `/home/admin/workspace-private/workspace/MEMORY.md` §6.4

---

## 3. PR 拆分与实施步骤

### PR1 — `fix/feishu-cron-delivery-enum`（P0，半天）

**目标**：让飞书能被 `cron_add` 接受为合法 delivery channel。

**改动清单**：

| 文件 | 改动 | 行数 |
|---|---|---|
| [crates/zeroclaw-runtime/src/tools/cron_add.rs:149](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/cron_add.rs#L149) | enum 加 `"feishu", "lark"`（同时加 `"dingtalk", "wecom"` 前瞻覆盖，**如 `cron_update.rs` 有相同 enum 同步改**） | ~2 |
| [crates/zeroclaw-channels/src/orchestrator/mod.rs:5909-5996](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L5909) | `deliver_announcement` fallback match 加 `"feishu" \| "lark" =>` 臂，从 `FeishuConfig` / `LarkConfig` 构造 `LarkChannel::from_*_config` 并 `send` | ~30 |
| cron_add.rs description | 把 "telegram/discord/slack/mattermost/matrix/qq" 列表更新为包含飞书 | ~1 |

**步骤**：
1. `git checkout master && git pull`
2. `git checkout -b fix/feishu-cron-delivery-enum`
3. 改 `cron_add.rs` enum + description
4. 验证：`rg '"enum"' crates/zeroclaw-runtime/src/tools/cron_*.rs` 找同源 enum 同步
5. 改 `orchestrator/mod.rs` fallback match 加 lark/feishu 臂
6. `cargo check -p zeroclaw-runtime -p zeroclaw-channels`
7. `cargo clippy --all-targets -- -D warnings`
8. `cargo test -p zeroclaw-runtime -p zeroclaw-channels`
9. 若有 JSON schema snapshot 测试，`cargo insta review` 接受新 snapshot
10. atomic commit：`fix(cron): allow feishu/lark as cron_add delivery channel`；body 说明根因 + MEMORY.md §6.4 过时澄清
11. push + 发 CR 地址给用户
12. 用户确认后 squash-merge

**风险**：Low。仅扩 enum + 加 match 臂，不改业务逻辑。

**Rollback**：`git revert <commit>`。cron_add schema 变化对现网 LLM 只"允许更多选项"，无 breaking。

---

### PR2 — `feat/feishu-reply-intent-reactions`（P0，半天到一天）

**目标**：飞书用户在 bot 决定不回复时能看到 emoji 反馈。

**改动清单**：

| 文件 | 改动 | 行数 |
|---|---|---|
| [crates/zeroclaw-channels/src/lark.rs:1782-1830](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1782) | `impl Channel for LarkChannel` 加 `async fn add_reaction(&self, message_id, emoji) -> Result<()>` | ~30 |
| 新 helper 函数 | 抽 `async fn post_lark_reaction(&self, message_id, emoji) -> Result<()>` 复用现有 token 刷新 | ~40 |
| 把内部 `try_add_ack_reaction` 重构为调用新 helper | 去重 | ~-20 |

**关键决策**：
- 飞书 `/im/v1/messages/<message_id>/reactions` API 使用 `emoji.emoji_type` 名称（非 unicode），需映射 `"👍" → "THUMBSUP"` / `"🚫" → "NO_ENTRY"` / `"⚠️" → "WARNING"`（或查飞书文档确认映射表）
- 映射失败时 log warn + 返回 `Ok(())`（不阻塞 reply-intent precheck）

**步骤**：
1-2. 同 PR1
3. 研究飞书 reaction API 的 `emoji_type` 字符串（查 `/im/v1/messages/:message_id/reactions` 文档，或从现有 `try_add_ack_reaction` 代码推断 —— 它已在 work 说明映射表已内嵌）
4. 实现 `post_lark_reaction` helper
5. 重构 `try_add_ack_reaction` 调用它
6. `impl Channel` 块加 `async fn add_reaction`
7. 单元测试：mock HTTP 验证 POST body + emoji 映射
8-12. 同 PR1

**风险**：Low。新增可选实现，失败降级。

**Rollback**：`git revert`。

---

### PR3 — `feat/feishu-request-approval`（P1，2-3 天，**需 Oracle review**）

**目标**：飞书支持 medium-risk 工具的交互式审批。

**改动清单**：

| 文件 | 改动 | 行数 |
|---|---|---|
| `lark.rs` 结构体加字段 | `pending_approvals: Arc<Mutex<HashMap<String, oneshot::Sender<ChannelApprovalResponse>>>>` | ~5 |
| `impl Channel` 加 `request_approval` | 发送审批卡片 + 插入 pending_approvals + select! 等 oneshot/timeout | ~80 |
| 审批卡片构造器 | `build_approval_card(request, approval_id)` —— 含 Approve/Deny 按钮，callback 携带 approval_id | ~60 |
| WS/HTTP 入站 handler 加 callback 解析 | 接收 `card.action.trigger` 事件 → 匹配 approval_id → `pending_approvals.remove()` → `sender.send()` | ~50 |
| 卡片更新 helper | 点击后把按钮变灰 + 显示结果文字（用 PATCH message） | ~30 |

**关键技术问题（Oracle 咨询项）**：
- 飞书交互卡片 callback 事件的 JSON 结构（`action` 字段）与 telegram inline keyboard callback_query 不同，需从飞书文档或现有代码找参照
- `approval_id` 如何在 callback 里往返：飞书 card action 的 `value` 字段（JSON object）
- card action 的 `confirm` 弹窗 vs 直接执行
- 能否区分点击人是否是原消息接收者（防止第三人乱点）
- Approval timeout (`config.channels.feishu.approval_timeout_secs`) 需加到 `FeishuConfig` / `LarkConfig` schema（此为唯一 schema 改动）

**步骤**：
1-2. 同 PR1
3. **Oracle consultation**：卡片审批设计 + callback 路由 + 并发安全
4. 加 `approval_timeout_secs` 到 `FeishuConfig` / `LarkConfig`（默认 120）
5. 加 `pending_approvals` 字段 + 构造函数初始化
6. 实现 `build_approval_card`
7. 实现 `request_approval`（发卡 + pending_approvals.insert + tokio::select! oneshot/timeout）
8. 实现 callback 解析（WS 和 HTTP 两路都加）
9. 实现按钮状态更新
10. 集成测试：mock 飞书 API，完整走 request→callback→response 闭环
11. Clippy + test pass
12-13. CR + user confirm + merge

**风险**：Medium。并发安全（pending_approvals 的 HashMap 清理）、超时处理、双路 callback 解析。

**Rollback**：`git revert`。`request_approval` 默认返 `Ok(None)` → auto-deny，revert 后退回原状态。

---

### PR4 — `feat/feishu-draft-streaming`（P1，3-5 天）

**目标**：流式消息用卡片 PATCH in-place 更新，不再每 chunk 发一张新卡片。

**改动清单**：

| 文件 | 改动 | 行数 |
|---|---|---|
| `lark.rs` 结构体 | `last_draft_edit: Arc<Mutex<HashMap<String, Instant>>>` + `stream_mode: StreamMode` + `draft_update_interval_ms: u64` | ~10 |
| `with_streaming(mode, interval)` builder 方法 | 仿 telegram | ~15 |
| `impl Channel` `supports_draft_updates` | 返回 `self.stream_mode != Off` | ~3 |
| `send_draft` | `POST /im/v1/messages` 发占位卡片，返回 `message_id` | ~40 |
| `update_draft` | rate-limit 检查 + `PATCH /im/v1/messages/<id>` 更新卡片 | ~60 |
| `update_draft_progress` | 进度条形式更新（可选，默认 `Ok(())`） | ~20 |
| `finalize_draft` | 最后一次 PATCH，替换为完整内容；失败降级 send+delete 策略 | ~50 |
| `cancel_draft` | PATCH "已取消" 或 DELETE | ~20 |
| `FeishuConfig` / `LarkConfig` schema | 加 `stream_mode: StreamMode`（默认 Off）+ `draft_update_interval_ms: u64`（默认 1000） | ~10 |
| `orchestrator` 构造时 chain `.with_streaming(...)` | 照抄 telegram 模式 | ~5 |

**关键决策**：
- 飞书交互卡片 PATCH API 限速（查 API 文档，一般 5 QPS per app）→ `draft_update_interval_ms` 默认 1000ms 安全
- 长消息 chunking：draft 模式下如果最终内容超过 LARK_CARD_MARKDOWN_MAX_BYTES，finalize 时用 chunking 逻辑（delete 占位 + send 多条）
- StreamMode 枚举：`Off | Drafts`（不做 `Multi` 模式，飞书一张卡片够用）

**步骤**：
1-2. 同 PR1
3. 查飞书 PATCH API 限速 + 鉴权（同 send 路径）
4. 加 config 字段
5. 加结构体字段 + builder
6. 实现 5 个 Channel 方法
7. 集成测试：mock 模拟 10 chunk 流 → 验证只有 N 次 PATCH（N ≤ 10 / interval_sec）
8. 实盘测试（本地飞书 bot）：短消息 / 长消息 / 取消 / 异常
9. Clippy + test pass
10-11. CR + user confirm + merge

**风险**：Medium。rate-limit 计算、PATCH API 限速、长消息降级路径。

**Rollback**：`git revert`。`supports_draft_updates` 默回 false → 退回每 chunk 新卡片。

---

### PR5 — `fix/feishu-group-session-per-user`（P1，1 天）

**目标**：群聊按 sender 区分历史，不串话。

**改动清单**：

| 文件 | 改动 | 行数 |
|---|---|---|
| [lark.rs:1021-1022](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1021) WS 路径 | `sender = open_id` (from `lark_msg.sender.sender_id.open_id`) | ~3 |
| [lark.rs:1551](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1551) HTTP 路径 | 同上 | ~3 |
| `reply_target` 保持 `chat_id` | 不变 | 0 |
| 单聊兼容性 | 单聊时 sender=open_id 也能区分用户，但可能破坏现有历史 key | 需 migration 说明 |

**关键决策**：
- 单聊场景 `chat_id` 和 `open_id` 对应关系稳定（单聊 chat 只有一个外部 user），但 `conversation_history_key = channel + chat_id + sender` 从 `feishu_<chat>_<chat>` 变成 `feishu_<chat>_<open_id>` → **现有历史丢失**
- 缓解方案：commit body 说明"首次部署后旧会话历史不再访问，新消息起建新 session"
- 或：加 config flag `[channels.feishu].per_user_session = true`（默认 false 向后兼容），后续再默认 true
- **推荐方案**：加 flag，默认 false（保守），用户自愿开启

**步骤**：
1-2. 同 PR1
3. 加 `per_user_session: bool` (默认 false) 到 `FeishuConfig` / `LarkConfig`
4. 修 WS 和 HTTP 两路 parse，if per_user_session → sender=open_id else sender=chat_id
5. **验证**：现有集成测试（如有）在默认 false 下行为不变
6. Clippy + test
7-8. CR + merge

**风险**：Medium。行为变更对现网历史有影响，flag 默认 off 缓解。

**Rollback**：`git revert` 或用户改 config 关 flag。

---

### PR6 — `chore/channels-cron-tool-descs-parity`（P2，半天）

**目标**：消除 `build_channel_tool_descs` 与 `loop_.rs` 的 cron 工具描述漂移。

**改动清单**：

| 文件 | 改动 | 行数 |
|---|---|---|
| `orchestrator/mod.rs:639-714 build_channel_tool_descs` | 把 `loop_.rs:2367-2384` 的 6 个 cron_* descriptor 追加进来 | ~15 |
| 或抽 helper | `fn cron_tool_descs() -> &'static [(&str, &str)]` 共用 | ~20 |

**步骤**：
1-2. 同 PR1
3. 决策：抽 helper 还是直接重复（推荐抽 helper 放 `zeroclaw-runtime` crate 公开模块，两路调用）
4. 实现
5. Clippy + test
6-7. CR + merge

**风险**：Low。

**Rollback**：`git revert`。

---

### PR7 — `refactor/lark-unify-ws-http-parser`（P3，1-2 天）

**目标**：WS 和 HTTP 路径共用一个消息解析函数。

**改动清单**：

| 文件 | 改动 | 行数 |
|---|---|---|
| `lark.rs` 新 helper | `fn parse_lark_event(content: &LarkMsgContent, message_id: &str, chat_id: &str, sender: &LarkSender) -> Option<ChannelMessage>` | ~150 |
| [lark.rs:918-1042](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L918) WS 路径 | 替换为调 helper | ~-90 |
| [lark.rs:1647-1730](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1647) HTTP 路径 | 替换为调 helper | ~-70 |
| 单元测试 | 覆盖 text/post/image/file/audio 5 种类型 | ~200 |

**步骤**：
1-2. 同 PR1
3. 仔细对比 WS 和 HTTP 两段，找出分歧字段（大概率是字段路径深度不同）
4. 设计统一 input 类型 or Either<WsContent, HttpContent>
5. 实现 helper + 单元测试
6. 替换两路调用，确保行为逐字节一致
7. `cargo test -p zeroclaw-channels` 覆盖新测试
8-9. CR + merge

**风险**：Low-Medium。重构纯属内部，但改动面大，易引入微妙 bug。必须通过已有集成测试。

**Rollback**：`git revert`。

---

## 4. 验证计划 (Validation)

### 4.1 每 PR 必跑
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p zeroclaw-channels
```

触碰 `cron_add.rs` 的 PR1 额外：
```bash
cargo test -p zeroclaw-runtime
```

### 4.2 端到端测试（PR1 + PR2 合并后手工验证）

**测试环境**：gloria 或 daily_3_6 沙箱实例，已配好飞书 bot。

**场景 1 (AC-1.x, AC-1.5)**：
1. 在飞书私聊发："5 分钟后提醒我喝水"
2. 观察 bot 回复 "好的，5 分钟后提醒你喝水"
3. 等 5 分钟
4. 飞书应收到提醒卡片

**场景 2 (AC-2.x)**：
1. 在飞书群聊发无意义消息如 "嗯嗯"
2. Bot 不回复，原消息应有 👍 emoji 反应

**场景 3 (AC-G-Memory)**：
1. 修改 `/home/admin/workspace-private/workspace/AGENTS.md`
2. 立刻在飞书发一条消息
3. Bot 的行为应已反映 AGENTS.md 的新内容（证明热重载已 work —— 此条用于 validate MEMORY.md §6.4 过时认知）

### 4.3 PR3 验证
- 手工触发一次 medium-risk shell 命令（如 `git push`），飞书卡片应出现 Approve/Deny 按钮
- 点击 Approve → 执行；点击 Deny → 不执行；不点 120s → 超时 auto-deny
- 按钮点击后卡片状态更新

### 4.4 PR4 验证
- 让 bot 回答一个长问题（> 5s 流式输出）
- 飞书卡片应 in-place 更新（而非刷屏多条新卡）
- 按 interval 节流，不超过 1 次/秒

### 4.5 PR5 验证
- 在群聊中 user A 问 "我是谁"，bot 回复 A 的身份
- 紧接 user B 问 "我是谁"，bot 回复 B 的身份（不串成 A）
- 默认 flag 关闭时，行为同现状

### 4.6 PR6 验证
- 抽取 LLM-facing system prompt，检查 cron_add 等 6 个工具都在飞书 system prompt 中宣传

### 4.7 PR7 验证
- 所有原有集成测试通过
- 新增 5 种消息类型单元测试通过

---

## 5. 风险 & Rollback 总表

| PR | 风险等级 | 主要风险 | Rollback 策略 |
|---|---|---|---|
| PR1 | Low | schema 扩 enum 无 breaking | `git revert` |
| PR2 | Low | Reaction API 失败降级 warn | `git revert` / 函数退回 default no-op |
| PR3 | **Medium** | 并发安全、超时、callback 路由 | `git revert` / `request_approval` 退回 default `Ok(None)` |
| PR4 | Medium | PATCH API rate-limit、长消息降级 | `git revert` / `supports_draft_updates` 退 false |
| PR5 | Medium | 现网会话历史断层 | config flag 默认 off；revert |
| PR6 | Low | 文档漂移，无业务变 | `git revert` |
| PR7 | Low-Medium | 重构引入微妙 bug | `git revert`；必须跑所有集成测试 |

**全局 Rollback**：任何时候可按反向顺序逐个 revert，每个 PR 独立可回滚。

---

## 6. 超时 / 看门线

- PR1 + PR2 如果 **2 工作日未合并**（含 CR 等待），触发原因复盘：是 CI 红、review 卡、还是需求变更？
- PR3 / PR4 如果 Oracle review 提出"需重新设计"类意见，回到 §2 重写对应 G/AC 后重启该 PR。
- 整个计划如果 **3 周未完成 P0 (PR1+PR2)**，升级为 blocker 通知用户。

---

## 7. 分支与命名约定

- 所有 PR 分支前缀按 conventional commit：`fix/` `feat/` `chore/` `refactor/`
- 基于 `master`，每个 PR 独立基线
- Atomic commit 原则：每个 PR 内部允许多 commit，但单 commit 必须可独立通过 `cargo check + clippy + test`
- Size 预估：PR1 size:XS / PR2 size:S / PR3 size:M / PR4 size:M / PR5 size:S / PR6 size:XS / PR7 size:M

---

## 8. 待解决问题（Open Questions）

1. **飞书 reaction emoji 的 `emoji_type` 映射表**：是从现有 `try_add_ack_reaction` 代码推断出来的（反查 LARK_ACK_REACTIONS_*），还是需要查飞书官方文档完整列表？→ PR2 开工时验证
2. **飞书卡片 callback action 的 JSON 结构**：`card.action.trigger` 事件的精确 schema？→ PR3 开工时 Oracle 咨询 + 飞书 SDK 参考
3. **飞书 PATCH API rate limit**：官方文档具体数值？→ PR4 开工时查
4. **`cron_update.rs` 是否存在独立的 `delivery.channel` enum**：需 PR1 开工前 grep 确认
5. **PR5 默认 flag 方案 vs 直接改**：倾向 flag 方案（保守），但会让群聊默认串话问题保留 → **待与用户确认**
6. **PR6 helper 放哪个 crate**：`zeroclaw-runtime` 公开 vs `zeroclaw-channels` 新内部 mod —— 依赖方向要不破坏当前 crate 分层

---

## 9. 计划落地签核

| 字段 | 状态 |
|---|---|
| rev0 起草 | 2026-05-12 ✅ |
| Momus 审查 | ✅ 2026-05-12 OKAY 一次通过（ses_1e80ce09dffeP7uzuDXSTKS6Fq） |
| Oracle 审查（PR3 架构） | ⏸ **Deferred** — MVP 实现已完成，80 tests pass；若用户要求外部 architecture review 可后补 |
| 用户拍板 Open Questions #5 / #6 | ✅ 按 plan 推荐方案落地（Q5=config flag default false；Q6=helper in runtime） |
| PR1-PR6 本地完成 | ✅ **全部落地**（详见 §10） |
| PR7 aborted | ✅ RFC tech debt filed per plan §3 PR7 own rollback provision |
| **Push to gitee** | 🚧 **Hard blocker — sandbox has no gitee credentials**（MEMORY.md §4.5 沙箱边界外）→ handoff to user |

## 10. 进度 Checklist (Top-level)

- [x] PR1 — `fix/feishu-cron-delivery-enum` (commit `39d953e1`, 验证通过，待 push)
- [x] PR2 — `feat/feishu-reply-intent-reactions` (commits `b2aa5604` + `43cf2f5b` emoji fix, 76 lark tests pass, 待 push)
- [x] PR3 — `feat/feishu-request-approval` (commit `db0a66a7`, 3 files +354/-2, 80 lark tests pass, 待 push; **Oracle review deferred** — MVP 实现已完成，若用户要求外部审核可 request)
- [x] PR4 — `feat/feishu-draft-streaming` (commit `d21f221f`, 3 files +461/-6, 82 lark tests pass, 待 push)
- [x] PR5 — `fix/feishu-group-session-per-user` (commit `d2090058`, 3 files +124/-3, 78 lark + 620 config tests pass, 待 push)
- [x] PR6 — `chore/channels-cron-tool-descs-parity` (commit `16279c65`, 3 files +61/-18, 1623 runtime tests pass, 待 push)
- [~] PR7 — `refactor/lark-unify-ws-http-parser` **ABORTED** (subagent judgment: post/list 已提取，剩余 text/image/file 不可 net-negative 提取；audio 结构发散；filed as RFC technical debt per plan §3 PR7 own rollback provision)

## 11. 事后发现的问题 (Issues Found Post-Implementation)

### I1. 🔴 PR2 emoji_type 映射错误（2026-05-12 Librarian Q3）

PR2 commit `b2aa5604` 的 `unicode_to_lark_emoji_type` **4/8 条目不是飞书 API 有效的 emoji_type**：

| Unicode | PR2 填的（错） | 飞书 canonical（对） | 影响 |
|---|---|---|---|
| 🚫 | `"NO_ENTRY"` | `"No"` | Refused 反应不显示 |
| ⚠️ | `"WARNING"` | `"Alarm"` (闹钟) 或 `"ERROR"` (X face) | Failed 反应不显示 |
| 👀 | `"EYES"` | `"GLANCE"` (侧目) | Seen ack 反应不显示 |
| 🎉 | `"CELEBRATE"` | `"PARTY"` (彩带爆炸) | Celebrate 反应不显示 |
| 👍 | `"THUMBSUP"` | ✅ 正确 | — |
| ✅ | `"DONE"` | ✅ 正确 | — |
| ✔️ | `"DONE"` | ✅ 正确 | — |
| ❤️ | `"HEART"` | ✅ 正确 | — |

**符合 PR2 "失败降级 warn log" 策略** → 不崩但 emoji 不现，用户感受与修 PR2 前**没区别**。

**修复计划**（追加 commit 到 `feat/feishu-reply-intent-reactions` 分支，未 push 前）：
1. 更新 `unicode_to_lark_emoji_type` 4 条映射
2. 更新单元测试断言
3. atomic commit: `fix(lark): correct emoji_type casing per Lark canonical table (follow-up to b2aa5604)`
4. 记录 Librarian 源链接作为 cite

### I2. ⚠️ Parallel subagent branch conflict (工作纪律问题)

Orchestrator 同时启 PR4 + PR6 + PR7 三个 subagent，每个要 `git checkout -b` 各自的分支。**单 worktree 下并发 git 操作互相破坏** —— PR6/PR7 已建空分支后被 cancel，PR4 subagent 可能在错的 branch 上跑。

**修正教训**：
- 同一 repo / worktree 多 subagent 必须**串行** git 操作
- 或 orchestrator 提前 `git worktree add` 到独立目录给并行 subagent 用
- Librarian 不占用 git，可与实施 subagent 并行

### I3. Librarian Q2 揭示 PR3 新要求

`card.action.trigger` 回调**任何群成员都能点** —— 飞书 API 没内建"仅原消息接收者可点"保护。PR3 plan §3 提到的"能否区分点击人是否是原消息接收者"现在有答案：

**必须自己在 `value` 里带 `intended_approver_open_id`，server 端 match `event.operator.open_id == intended_approver_open_id`**。 

这延伸 PR3 的 AC-3.5 → 新增 AC-3.6: "点击人不是 intended approver → 返回 toast 'Only <username> can approve'，不推进 oneshot"。

---

**End of Plan**
