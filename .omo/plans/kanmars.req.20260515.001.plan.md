# Plan — kanmars.req.20260515.001 (Feishu Approval Card Resolved-State Update)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260515.001.plan |
| 关联需求 | 无独立 req 文档（用户对话需求："飞书输入框审批按钮点击后没反应还能继续点，期望有状态变化"） |
| 起草日期 | 2026-05-15 |
| 修订日期 | 2026-05-15 (rev1 — **已交付**) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `fix/feishu-approval-card-resolved-state` |
| 风险等级 | **Medium**（`zeroclaw-channels` Experimental tier 内行为变更，无 trait / 边界 / 安全影响） |
| 基线 commit | `f384bd86` (master, 2026-05-15 — `feat(cli): show build time in Beijing time (+0800) instead of UTC`) |
| **最终方案** | **方案 A → 修订为方案 A'（Card 2.0 schema）**：rev0 初版用 Card 1.0（与 send-time 卡对齐），生产联调发现飞书 PATCH endpoint 对 Card 1.0 envelope 接受但客户端不渲染；rev1 切到 Card 2.0 schema（与生产已验证的 `patch_card_content` 对齐）+ 加 `info!` 观测性日志 |
| 实际代码行数 | +457 / -88（含 3 个 wiremock 单测、Card 2.0 schema、5 条 info/warn 观测日志） |
| 实际工作量 | 约 4 小时（含 rev1 生产诊断与修复） |
| **状态** | ✅ **已交付** (2026-05-15 16:43 CST 生产验证：飞书客户端卡片正确渲染) |
| **交付 commits** | `589ab381` (rev0 — Card 1.0，部署后验证失败) + `2300498a` (rev1 — Card 2.0 修复 + 观测性，生产验证通过) |

---

## 0. 关键目标（唯一的真理来源）

> **让飞书 / Lark 的工具审批卡片在用户点击 Approve / Deny / Always 任一按钮后，立即在原消息位置 PATCH 成"已决议"状态——按钮组消失、出现彩色横幅（绿色 Approved / 红色 Denied）——使审批结果对用户视觉上即时可见、并消除"按钮还能继续点"的误操作面。**

**完成此目标即"功能完成"**：

- 用户在飞书 / Lark 群或单聊点击审批卡片任一按钮后：
  - 等待中的 `request_approval` future 仍按现行行为正确返回 `Approve` / `Deny` / `AlwaysApprove`（**唤醒路径不被破坏**）
  - 卡片在 ≤ 1s 内被 PATCH 成新状态：header 颜色变绿 / 红、title 改为 `✅ Tool approval — Approved` / `❌ Tool approval — Denied`、原 3 个按钮被一行 `note` 元素替换
- WebSocket 路径（[lark.rs:1066-1071](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1066-L1071)）和 HTTP webhook 路径（[lark.rs:2457-2464](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2457-L2464)）共用 `handle_card_action_event`，**两条路径同时生效**
- PATCH 任意失败路径（token 失效、5 QPS 频控 230020、HTTP 错误）一律 `tracing::warn!` 软失败，**绝不影响审批主流程的 oneshot 唤醒**
- Feishu (`open.feishu.cn`) 与 Lark (`open.larksuite.com`) 两个 platform 行为对称（共用 `LarkChannel` 实现）

**显式不在范围内**：

- ❌ 不引入 `fl!()` / Fluent — `zeroclaw-channels` crate 没有 i18n 设施，[`build_approval_card` lark.rs:303-359](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L303) 现有 "🔧 Tool approval required" / "✅ Approve" 等英文硬编码即先例，沿用一致
- ❌ 不动 `Channel` trait（`zeroclaw-api/src/channel.rs`）— 副作用纯内置在 `LarkChannel`，不破坏 Stable 候选 trait
- ❌ 不动 telegram / discord / slack / matrix / dingtalk — 单文件 PR，只动 `lark.rs`
- ❌ 不加新配置项 — 行为对所有 Feishu / Lark 用户一致打开（点击后看到"已决议"是符合直觉的默认行为，无 opt-in 必要）
- ❌ 不实现"按钮 disabled 但保留"— Card 1.0 `disabled` 字段语义不稳，**直接删按钮换成无交互决议块更干净**
- ❌ 不并入流式输出 / 其他无关重构（流式输出已由用户在 PR4 验证 OK）
- ❌ 不动 `request_approval` 的 `approval_timeout_secs` 行为 / oneshot 通道结构
- ❌ 不在 PATCH body 里附带"who clicked"信息（事件 payload 里的 `operator.open_id` 可用，但本期不展示，留作后续增强）

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不新增 `unwrap()` / `expect()`**（[zeroclaw AGENTS.md Anti-Patterns](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#anti-patterns)）。`response.pointer("/data/message_id")` 用 `.and_then(|v| v.as_str()).map(str::to_string)` 安全提取
2. **不新增 `#[allow(dead_code)]`**（同上）。新 helper 立即被 `request_approval` + `handle_card_action_event` 调用
3. **不动 `zeroclaw-api`**。改动边界 = **仅** `crates/zeroclaw-channels/src/lark.rs` 一个文件
4. **`tracing::` 日志保持英文 + 稳定 `error_key` 风格**（RFC #5653 §4.6）。新增日志统一 `Lark: approval card PATCH …` 前缀，与 `Lark: draft PATCH …`（[lark.rs:2374-2382](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2374-L2382)）保持一致风格
5. **复用现有 PATCH 通道**：`patch_or_send_once`（[lark.rs:2387-2410](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2387-L2410)）、`should_refresh_lark_tenant_token`、`extract_lark_response_code`、`LARK_DRAFT_RATE_LIMIT_CODE` 全部复用，**不引入新 HTTP wrapper**
6. **不引入新依赖**。`uuid` / `serde_json` / `tokio` / `tracing` / `anyhow` 全已存在
7. **完整跑** `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test -p zeroclaw-channels` + `./dev/ci.sh all`
8. **按 [zeroclaw AGENTS.md "Workflow"](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#workflow) 6 条**：Read before write / One concern per PR / Implement minimal patch / Validate by risk tier / Document impact / Queue hygiene。落地：master 拉分支 → 改 → commit → push → 发 CR 地址等用户确认（**非 master 分支开 PR**，不直推 master）
9. **不触碰 `zeroclaw-runtime`**（transitional crate 边界）。本 PR 完全在 `zeroclaw-channels` 内，合规
10. **CHANGELOG-next.md 必须加一行**（`zeroclaw-channels` Experimental tier 行为变更，需在 PR 说明 + CHANGELOG 体现）

---

## 1. 现状事实复核（基于 2026-05-15 session 两路并行 explore + 本人精读结果）

### 1.1 关键代码位置（行号对齐基线 `f384bd86`）

| 事实 | 文件:行 |
|---|---|
| **审批卡片渲染（待重用 schema）** `build_approval_card` Card JSON 1.0 | [crates/zeroclaw-channels/src/lark.rs:303-359](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L303) |
| 流式 draft 用的 Card JSON 2.0 builder（**不能复用** —— schema 不同） | [crates/zeroclaw-channels/src/lark.rs:288-299](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L288) |
| **`pending_approvals` 字段定义**（待扩展类型） | [crates/zeroclaw-channels/src/lark.rs:523-530](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L523) |
| **`handle_card_action_event` 点击处理**（待追加 PATCH 调用） | [crates/zeroclaw-channels/src/lark.rs:1770-1808](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1770) |
| WebSocket 派发入口 `card.action.trigger` | [crates/zeroclaw-channels/src/lark.rs:1066-1071](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1066) |
| HTTP webhook 派发入口 `card.action.trigger` | [crates/zeroclaw-channels/src/lark.rs:2457-2464](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2457) |
| **`request_approval` 主体**（待重构 + 捕获 message_id） | [crates/zeroclaw-channels/src/lark.rs:2142-2221](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2142) |
| **`patch_card_content` 流式 PATCH 先例**（结构原样仿写） | [crates/zeroclaw-channels/src/lark.rs:2349-2385](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2349) |
| **`patch_or_send_once` HTTP wrapper**（直接复用） | [crates/zeroclaw-channels/src/lark.rs:2387-2410](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2387) |
| `patch_message_url` URL builder（直接复用） | [crates/zeroclaw-channels/src/lark.rs:745-747](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L745) |
| `should_refresh_lark_tenant_token` token 401 / code 99991663 检测 | lark.rs（已存在，本 PR 仅引用） |
| `extract_lark_response_code` 提取 `data.code` | lark.rs（已存在，本 PR 仅引用） |
| `LARK_DRAFT_RATE_LIMIT_CODE = 230020` 飞书 5 QPS/消息频控码 | [crates/zeroclaw-channels/src/lark.rs:256](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L256) |
| `send_draft` 已有 `data.message_id` 提取先例 | [crates/zeroclaw-channels/src/lark.rs:2259](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2259) |
| 现有 wiremock 测试模式（仿写参考） | [crates/zeroclaw-channels/src/lark.rs:4750-4858](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4750) |
| `ChannelApprovalResponse` enum 定义（已 derive Clone） | [crates/zeroclaw-api/src/channel.rs:19](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-api/src/channel.rs#L19) |
| Feishu factory 入口（`from_feishu_config`） | [crates/zeroclaw-channels/src/lark.rs:658-676](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L658) |

### 1.2 根因结论

**用户报告"按钮点击后没反应还能继续点"的根因 =** 当前 [`handle_card_action_event` lark.rs:1800-1802](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1800-L1802) 在拿到点击事件后**只做一件事**：从 `pending_approvals` 取出 `oneshot::Sender` 唤醒等待中的 `request_approval` future。它**完全没有**：

1. 捕获 / 持有原始审批卡片的 `message_id`（发送时丢弃了响应 body 的 `data.message_id`）
2. 调用任何"卡片更新 / 删除按钮"的 API

飞书 / Lark 的 IM 卡片在按钮 `value` 没有 `behaviors.callback.return.toast` 或服务端 PATCH 的情况下，**点击后视觉无反应是协议默认**。修复必须由**服务端主动 PATCH** `/im/v1/messages/{message_id}` endpoint 来重渲染卡片。

PATCH 通道在 PR4（2026-05-13 流式输出）已铺好，本 PR 是**第二个使用方**（第一个是 streaming draft）。

### 1.3 已有先例 / 错误参考

- **lark.rs 流式 draft（PR4）**：`patch_card_content`（[lark.rs:2349](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2349)）已铺好同样的 PATCH + token 刷新 + 230020 软失败模式 → **本 PR 仿写一份 sibling helper**
- **telegram.rs callback_query**（探索报告 §3）：仅调用 `answerCallbackQuery` 关闭按钮转圈，**不修改源消息** → 不可借鉴，飞书没有等效 ack 机制，必须 PATCH
- **lark.rs `send_draft` 提取 message_id**（[lark.rs:2259](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2259)）：`response.pointer("/data/message_id").and_then(|v| v.as_str()).map(str::to_string)` → **本 PR `request_approval` 完全照抄此 pattern**

---

## 2. 方案选型（已决策）

### 2.1 选项对比

| 方案 | 描述 | 优点 | 缺点 | 决策 |
|---|---|---|---|---|
| **A — Card 1.0 同 schema 重渲染**（删按钮 + 决议横幅） | PATCH 整张卡，`action` 元素替换为静态 `note` | 最干净，按钮彻底消失，无误点可能；schema 一致无 PATCH 失败 | 无 | ✅ **采纳** |
| B — 按钮加 `disabled: true` 标志 | 保留按钮但置灰 | 视觉上保留"曾经的选项" | Card 1.0 `disabled` 字段官方文档不稳定，部分客户端版本无效；用户仍可看到全部按钮易混淆 | ❌ |
| C — 删除整个 `action` element 但不加横幅 | 单纯删按钮 | 最简单 | 无任何决议结果反馈，用户不知道"我点的什么生效了" | ❌ |
| D — 用 Feishu 新版 Card v2 `cardkit` API | 通过 `PUT /open-apis/cardkit/v1/cards/{card_id}` 局部更新组件 | 局部更新更省带宽 | 需切换到 Card 2.0 + 重新设计 schema + 引入新 endpoint，**超出本 PR 范围** | ❌（留作后续 RFC） |
| E — 在 `Channel` trait 加 `on_approval_resolved` 钩子让 orchestrator 调用 | 跨渠道通用 | 通用性 | 改 `zeroclaw-api`（Stable 候选），违反"不动 trait"前提；其他渠道无对应 UI 概念 | ❌ |

**选 A 的核心理由**：

1. **零 schema 风险**：与 `build_approval_card` 同版本（Card 1.0），PATCH endpoint 接受相同 envelope
2. **零误点风险**：按钮元素物理移除（不依赖 `disabled` 字段渲染语义）
3. **单文件改动**：完全在 `lark.rs` 内闭环，符合"One concern per PR"

### 2.2 PATCH body 形态（关键技术细节）

飞书 `PATCH /im/v1/messages/{message_id}` 的 body schema：

```json
{
  "content": "<JSON-stringified Card 1.0 envelope>"
}
```

**注意**：`content` 必须是字符串（`serde_json::to_string(&card)?`），不是嵌套 JSON object。这与 [`patch_card_content` lark.rs:2351-2353](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2351-L2353) 已有的形态完全一致：

```rust
let body = serde_json::json!({
    "content": build_card_content(markdown),  // build_card_content 返回 String
});
```

新 helper 同款：`"content": card.to_string()` —— `card` 是 `serde_json::Value`，`.to_string()` 出 JSON 字符串。

---

## 3. 实施步骤（6 处编辑，全部在 `lark.rs`）

### 步骤 0：分支准备

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
git checkout master && git pull
git checkout -b fix/feishu-approval-card-resolved-state
```

### 步骤 1：新增 `PendingApproval` 结构体

**位置**：紧邻 `LarkChannel` 定义之前（lark.rs:495 附近，在 `pub struct LarkChannel {` 上方）

```rust
/// Context kept while waiting for a user's approval-card click.
/// Used to (a) wake the awaiting future via `sender` and (b) re-render
/// the card after the click so the buttons disappear.
struct PendingApproval {
    sender: tokio::sync::oneshot::Sender<zeroclaw_api::channel::ChannelApprovalResponse>,
    /// `data.message_id` returned by the send-card POST. Empty string is a
    /// sentinel meaning "card was sent but message_id was missing from the
    /// response" — handler will skip the post-click PATCH in that case.
    message_id: String,
    tool_name: String,
    arguments_summary: String,
}
```

### 步骤 2：修改 `pending_approvals` 字段类型（lark.rs:523-530）

```rust
// Before:
pending_approvals: Arc<
    tokio::sync::Mutex<
        std::collections::HashMap<
            String,
            tokio::sync::oneshot::Sender<zeroclaw_api::channel::ChannelApprovalResponse>,
        >,
    >,
>,

// After:
pending_approvals: Arc<
    tokio::sync::Mutex<std::collections::HashMap<String, PendingApproval>>,
>,
```

**编译器跟随项**：所有 `LarkChannel` 构造点（`new_with_platform` / `from_lark_config` / `from_feishu_config` 等）`pending_approvals` 初始化的 `HashMap::new()` 不变，但需确认无别处直接 insert 裸 `Sender`。

### 步骤 3：新增 `build_resolved_approval_card`

**位置**：紧挨现有 `build_approval_card`（lark.rs:359）之后

**关键约束**：必须沿用 **Card JSON 1.0** schema（与 `build_approval_card` 一致），**不能** 用 `build_card_content`（Card 2.0）。

```rust
/// Render the approval card after the user clicked, with the
/// action element replaced by a static decision banner. Re-uses the
/// same Card JSON 1.0 envelope as `build_approval_card` so PATCH stays
/// schema-compatible with the original message.
fn build_resolved_approval_card(
    tool_name: &str,
    arguments_summary: &str,
    decision: zeroclaw_api::channel::ChannelApprovalResponse,
) -> serde_json::Value {
    use zeroclaw_api::channel::ChannelApprovalResponse;

    let (banner_emoji, banner_text, header_template) = match decision {
        ChannelApprovalResponse::Approve       => ("✅",  "Approved",          "green"),
        ChannelApprovalResponse::AlwaysApprove => ("✅✅", "Approved (always)", "green"),
        ChannelApprovalResponse::Deny          => ("❌",  "Denied",            "red"),
    };

    serde_json::json!({
        "config": { "wide_screen_mode": true },
        "header": {
            "template": header_template,
            "title": {
                "tag": "plain_text",
                "content": format!("{banner_emoji} Tool approval — {banner_text}")
            }
        },
        "elements": [
            {
                "tag": "div",
                "text": {
                    "tag": "lark_md",
                    "content": format!("**Tool:** `{tool_name}`\n\n{arguments_summary}")
                }
            },
            { "tag": "hr" },
            {
                "tag": "note",
                "elements": [{
                    "tag": "plain_text",
                    "content": format!("{banner_emoji} {banner_text}")
                }]
            }
        ]
    })
}
```

### 步骤 4：新增 `patch_approval_card_resolved` 方法

**位置**：紧挨 `patch_card_content`（lark.rs:2385）之后，同一 `impl LarkChannel { ... }` 块内

```rust
/// PATCH an approval card to its resolved state. Soft-fails on every error
/// path (transport / token refresh / rate-limited / non-zero code) — never
/// propagates to the caller, since the user-visible decision is already
/// delivered via the oneshot.
async fn patch_approval_card_resolved(
    &self,
    message_id: &str,
    tool_name: &str,
    arguments_summary: &str,
    decision: zeroclaw_api::channel::ChannelApprovalResponse,
) {
    let card = build_resolved_approval_card(tool_name, arguments_summary, decision);
    let url = self.patch_message_url(message_id);
    let body = serde_json::json!({
        "content": card.to_string(),
    });

    let (status, response) = match self.patch_or_send_once(&url, &body, true).await {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!(
                "Lark: approval card PATCH transport error \
                 (message_id={message_id}): {e}"
            );
            return;
        }
    };

    let final_body = if should_refresh_lark_tenant_token(status, &response) {
        self.invalidate_token().await;
        match self.patch_or_send_once(&url, &body, true).await {
            Ok((retry_status, retry_response)) => {
                if should_refresh_lark_tenant_token(retry_status, &retry_response) {
                    tracing::warn!(
                        "Lark: approval card PATCH still unauthorized after token refresh \
                         (message_id={message_id}, body={retry_response})"
                    );
                    return;
                }
                retry_response
            }
            Err(e) => {
                tracing::warn!(
                    "Lark: approval card PATCH retry transport error \
                     (message_id={message_id}): {e}"
                );
                return;
            }
        }
    } else {
        response
    };

    let code = extract_lark_response_code(&final_body).unwrap_or(0);
    if code == LARK_DRAFT_RATE_LIMIT_CODE {
        tracing::debug!(
            "Lark: approval card PATCH rate-limited (code=230020) for message {message_id}"
        );
    } else if code != 0 {
        tracing::warn!(
            "Lark: approval card PATCH soft-failed for {message_id}: \
             code={code}, body={final_body}"
        );
    }
}
```

### 步骤 5：重构 `request_approval`（lark.rs:2142-2221）

**核心变更**：发送成功 → 提取 `data.message_id` → **才** insert `PendingApproval`（消除 race）。提取 `wait_for_decision` helper 让主体可读。

```rust
async fn request_approval(
    &self,
    recipient: &str,
    request: &zeroclaw_api::channel::ChannelApprovalRequest,
) -> anyhow::Result<Option<zeroclaw_api::channel::ChannelApprovalResponse>> {
    use zeroclaw_api::channel::ChannelApprovalResponse;

    let approval_id = Uuid::new_v4().to_string();
    let card =
        build_approval_card(&approval_id, &request.tool_name, &request.arguments_summary);

    let token = self.get_tenant_access_token().await?;
    let url = self.send_message_url();
    let body = serde_json::json!({
        "receive_id": recipient,
        "receive_id_type": "chat_id",
        "msg_type": "interactive",
        "content": serde_json::to_string(&card)?,
    });

    // Send + token-refresh retry — same logic as before, but capture response_body
    // for message_id extraction.
    let response_body = {
        let (status, resp) = self.send_text_once(&url, &token, &body).await?;
        if should_refresh_lark_tenant_token(status, &resp) {
            self.invalidate_token().await;
            let new_token = self.get_tenant_access_token().await?;
            let (retry_status, retry_body) =
                self.send_text_once(&url, &new_token, &body).await?;
            ensure_lark_send_success(retry_status, &retry_body, "approval retry")?;
            retry_body
        } else {
            ensure_lark_send_success(status, &resp, "approval")?;
            resp
        }
    };

    // Extract message_id from data.message_id (same shape as send_draft @ lark.rs:2259).
    // Empty-string sentinel means "PATCH on click will be skipped" — see
    // PendingApproval doc comment.
    let message_id = response_body
        .pointer("/data/message_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            tracing::warn!(
                "Lark: approval card sent but no data.message_id in response — \
                 post-click card update will be skipped (approval_id={approval_id})"
            );
            String::new()
        });

    let (tx, rx) = tokio::sync::oneshot::channel();
    self.pending_approvals.lock().await.insert(
        approval_id.clone(),
        PendingApproval {
            sender: tx,
            message_id,
            tool_name: request.tool_name.clone(),
            arguments_summary: request.arguments_summary.clone(),
        },
    );

    Ok(Some(self.wait_for_decision(rx, &approval_id).await))
}

/// Wait for the user's click (or timeout). Mirrors the original
/// tokio::time::timeout block in `request_approval`.
async fn wait_for_decision(
    &self,
    rx: tokio::sync::oneshot::Receiver<zeroclaw_api::channel::ChannelApprovalResponse>,
    approval_id: &str,
) -> zeroclaw_api::channel::ChannelApprovalResponse {
    use zeroclaw_api::channel::ChannelApprovalResponse;
    match tokio::time::timeout(Duration::from_secs(self.approval_timeout_secs), rx).await {
        Ok(Ok(response)) => response,
        _ => {
            self.pending_approvals.lock().await.remove(approval_id);
            ChannelApprovalResponse::Deny
        }
    }
}
```

> **说明**：原代码在每个 `Err` 分支手动 `pending_approvals.remove`。新版"先发送成功再 insert"，发送失败路径 `?` 直接传播即可，无需清理。逻辑更简洁、消除了"已 insert 但发送失败导致 leak"的可能。

### 步骤 6：改造 `handle_card_action_event`（lark.rs:1770-1808）

```rust
async fn handle_card_action_event(
    &self,
    event_payload: &serde_json::Value,
) -> anyhow::Result<()> {
    use zeroclaw_api::channel::ChannelApprovalResponse;

    let value = event_payload
        .pointer("/action/value")
        .ok_or_else(|| anyhow::anyhow!("card.action.trigger: missing event.action.value"))?;

    let approval_id = value
        .get("approval_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("card.action.trigger: missing approval_id in value"))?;

    let decision_str = value
        .get("decision")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("card.action.trigger: missing decision in value"))?;

    let decision = match decision_str {
        "approve" => ChannelApprovalResponse::Approve,
        "deny"    => ChannelApprovalResponse::Deny,
        "always"  => ChannelApprovalResponse::AlwaysApprove,
        other => {
            tracing::warn!("Lark: unknown approval decision '{other}' — treating as deny");
            ChannelApprovalResponse::Deny
        }
    };

    // Pop pending entry before both the oneshot wake and the PATCH so the
    // map is freed even if PATCH hangs.
    let pending = self.pending_approvals.lock().await.remove(approval_id);
    let Some(pending) = pending else {
        tracing::debug!(
            "Lark: card action for unknown/expired approval_id {approval_id}"
        );
        return Ok(());
    };

    // (a) Wake the awaiting request_approval future first. User-visible decision
    //     delivery must not depend on PATCH success.
    let _ = pending.sender.send(decision.clone());

    // (b) Best-effort card mutation. Skip if message_id was missing from the
    //     send response (sentinel = empty string).
    if !pending.message_id.is_empty() {
        self.patch_approval_card_resolved(
            &pending.message_id,
            &pending.tool_name,
            &pending.arguments_summary,
            decision,
        )
        .await;
    }

    Ok(())
}
```

> **`decision.clone()`**：`ChannelApprovalResponse` 已 `derive(Clone)` ([channel.rs:19](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-api/src/channel.rs#L19))，clone 成本可忽略（C-style enum）。

---

## 4. 单元测试（仿现有 wiremock 模式）

**位置**：`#[cfg(test)] mod tests` 内，紧挨 [`update_draft_*` 测试 lark.rs:4750-4858](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4750)

### 测试 1：`approval_click_patches_card_with_resolved_state`

```rust
#[tokio::test]
async fn approval_click_patches_card_with_resolved_state() {
    // Setup: token mock + send-message mock (returns data.message_id="om_appr_1")
    //        + PATCH mock (asserts body.content contains "Approved" and NOT "✅ Approve" button text).
    // Drive: spawn request_approval future; once pending_approvals has the entry,
    //        directly call ch.handle_card_action_event(&simulated_event).
    // Assert: future returns ChannelApprovalResponse::Approve;
    //         PATCH mock was called exactly once with the resolved card body.
}
```

### 测试 2：`approval_click_handler_tolerates_patch_failure`

```rust
#[tokio::test]
async fn approval_click_handler_tolerates_patch_failure() {
    // Setup: token + send-message succeed; PATCH returns code=230020 (rate-limited).
    // Drive: same as test 1.
    // Assert: future STILL returns user's decision (Deny in this case).
    //         No panic, no error propagation.
}
```

### 测试 3：`approval_click_for_unknown_id_does_not_patch`

```rust
#[tokio::test]
async fn approval_click_for_unknown_id_does_not_patch() {
    // Setup: token mock only; PATCH mock with .expect(0) — must NOT be called.
    // Drive: directly call handle_card_action_event with a fabricated event
    //        whose approval_id was never inserted into pending_approvals.
    // Assert: handler returns Ok(()); PATCH mock count == 0; debug log emitted.
}
```

> 完整 wiremock 设置照抄 [lark.rs:4750-4786](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4750-L4786) 模式，把 `path_regex("/im/v1/messages/om_draft_rl")` 换成 `om_appr_1`（POST /im/v1/messages 的发送 + PATCH /im/v1/messages/om_appr_1 的更新）。

---

## 5. 验证 & 风险

### 5.1 验证步骤

1. **本地静态检查**：
   ```bash
   cd /home/admin/workspace-public/kanmars/zeroclaw
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   ```
2. **单元测试**：
   ```bash
   cargo test -p zeroclaw-channels --features channel-lark
   ```
   预期：3 个新测试通过；现有 5 个 streaming 测试（`supports_draft_updates_reflects_stream_mode` / `update_draft_*` / `send_draft_extracts_message_id_from_response`）不受影响仍通过。
3. **完整 CI**：
   ```bash
   ./dev/ci.sh all
   ```
4. **联调（可选 / 用户侧）**：在用户已验证流式输出 OK 的飞书实例上：
   - 触发任意需要审批的工具调用
   - 在飞书客户端点击 ✅ Approve
   - 观察卡片是否在 1 秒内变绿、按钮组消失、出现 `✅ Approved` 横幅
   - 重复测试 ❌ Deny / ✅✅ Always

### 5.2 风险点 & 决策

| # | 风险 | 决策 / 缓解 |
|---|---|---|
| R1 | **Race**：用户在 `request_approval` insert pending_approvals 之前点了卡片 → handler 找不到 approval_id | **已规避**：发送成功 → 拿到 `message_id` → **才** insert。点击只可能晚于发送响应（按钮在卡片渲染前根本不存在）。即便发生（极端时序），handler 走 `tracing::debug!` 路径，无 panic |
| R2 | **Card schema 不匹配**：误用 Card 2.0 envelope PATCH Card 1.0 卡片 → 飞书报错 | **已规避**：`build_resolved_approval_card` 顶层无 `"schema":"2.0"` 字段、用 `header`/`elements` 1.0 结构。**Code review 检查点**：审 PR 时确认未引入 `"schema": "2.0"` |
| R3 | **PATCH 失败**：token 失效 / 5 QPS 频控（230020）/ HTTP 错误 | **已规避**：内部全部软失败（`tracing::warn!` / `debug!`）。oneshot 唤醒先于 PATCH，用户决策不会丢 |
| R4 | **`data.message_id` 缺失**（API 响应异常） | **已规避**：empty-string sentinel + `tracing::warn!`，handler 跳过 PATCH。审批流程仍正常工作（仅"按钮无状态变化"问题回退到现状） |
| R5 | **`pending_approvals` 字段类型变更影响其它构造路径** | **已规避**：编译器全部捕获，按 compiler error 修。Worst case 多动 1-2 个构造函数 |
| R6 | **decision.clone() 性能** | **不适用**：`ChannelApprovalResponse` 是 C-style enum，`Clone` ≈ memcpy 1 byte |
| R7 | **`Drop` 顺序**：`patch_approval_card_resolved` 持有 mutex 期间被 cancel | **不适用**：`patch_approval_card_resolved` 不持有 `pending_approvals` mutex（已在调用前 `remove`），完全独立 await |
| R8 | **Webhook 与 WS 重复触发**：飞书是否会同时通过两条路径推同一 click 事件 | **已知**：`pending_approvals.remove(approval_id)` 是原子操作（mutex 保护），第二次拿到的是 `None`，走 debug log 路径，不会 PATCH 两次 |

### 5.3 回退方案

风险等级 Medium、影响范围 = 飞书审批 UI 反馈。如线上发现严重问题：

1. **快速 revert**：`git revert <commit_sha>` 单 commit 即可（PR 单文件）
2. **回退影响**：审批流程功能不变，仅"按钮点击后无视觉反馈"回退到当前状态
3. **无 schema / 配置 / 数据迁移**

---

## 6. 工作量估算 & 时间线

| 阶段 | 行数 | 时长 |
|---|---|---|
| 步骤 1（PendingApproval struct） | +12 | 5 min |
| 步骤 2（字段类型变更 + 构造点修复） | -3/+2 | 10 min |
| 步骤 3（build_resolved_approval_card） | +35 | 15 min |
| 步骤 4（patch_approval_card_resolved） | +50 | 15 min |
| 步骤 5（request_approval 重构 + wait_for_decision） | -55/+55 | 20 min |
| 步骤 6（handle_card_action_event 改造） | -10/+25 | 10 min |
| 单测 ×3 | +120 | 30 min |
| 本地验证（fmt/clippy/test/ci.sh） | — | 10 min |
| commit + push + 写 PR description + 改 CHANGELOG-next.md | — | 10 min |
| **合计** | **+299 / -68** | **≈125 min** |

---

## 7. 提交流程（依 zeroclaw AGENTS.md "Workflow"）

1. **分支**：`fix/feishu-approval-card-resolved-state`（**非 master**）
2. **commit 信息（conventional）**：
   ```
   fix(channels): patch feishu/lark approval card to show resolved state

   After a user clicks a tool-approval card button (Approve/Deny/Always),
   the card is now PATCHed in place to a resolved-state rendering — the
   action buttons are removed and replaced with a colored decision banner
   (green for Approved/AlwaysApprove, red for Denied). Failures are
   soft-logged; the approval-decision delivery via oneshot is unaffected.

   This addresses the user-reported issue that approval buttons remained
   clickable with no visual change after the first click on Feishu.

   Risk: Medium (zeroclaw-channels Experimental tier, behavior change,
   single file, no trait/security/boundary impact).
   ```
3. **CHANGELOG-next.md**（必加，`zeroclaw-channels` Experimental tier 行为变更）：
   ```
   ### Changed
   - **lark/feishu**: After a user clicks a tool-approval button, the card
     now re-renders with the action buttons removed and a status banner
     (Approved / Denied / Approved (always)). PATCH failures soft-fail with
     a warning; approval-decision delivery is unaffected. (#TBD)
   ```
4. **PR 标题**：`fix(channels): patch feishu/lark approval card to show resolved state`
5. **size**：`size: S`（≈300 行 含测试，单文件）
6. **PR body** 按 `.github/pull_request_template.md` 全填，重点：
   - **What**：上述 commit message 内容
   - **Why**：用户报告"按钮点击后没反应还能继续点"
   - **Risk**：Medium
   - **Validation**：`./dev/ci.sh all` 通过 + 3 新测试 + 流式输出 5 现有测试不回归
   - **Rollback**：`git revert <sha>`，无副作用
7. **流程**：push 分支 → 发 CR 地址给用户确认 → 用户审完合 master → 不直推 master

---

## 8. 待用户决策项（开工前需确认）

| # | 项 | 默认 | 备选 |
|---|---|---|---|
| Q1 | 决议横幅文案"Approved (always)"措辞 | 沿用本计划 | 改成 "Approved (auto-approve future)" 等 |
| Q2 | 是否在决议块加点击者信息（`event.operator.open_id`） | **不加**（本期范围外） | 加（追加 `Approved by ou_xxx`） |
| Q3 | 是否加点击时间戳 | **不加**（本期范围外） | 加（北京时间 `HH:MM:SS`） |
| Q4 | header 颜色：Always 是否区分（如蓝色） | **绿色**（与 Approve 同） | 蓝色 / 紫色 |
| Q5 | 是否同步给 `LarkPlatform::Lark`（国际版）启用 | **同步启用**（共用 `LarkChannel` 代码自然生效） | 仅 Feishu |

---

## 9. 关联文档 / 参考

- 上一个相关 PR：PR4（2026-05-13 流式输出，建立了 PATCH 通道 + `LARK_DRAFT_RATE_LIMIT_CODE` 常量）
- [zeroclaw AGENTS.md](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md) — Workflow / Anti-Patterns / Stability Tiers
- [`Channel` trait 定义](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-api/src/channel.rs) — `request_approval` 契约
- [`build_approval_card` 现状](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L303-L359)
- [`patch_card_content` 流式 PATCH 先例](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2349-L2385)
- 飞书官方 API：`PATCH /open-apis/im/v1/messages/{message_id}` — Update message content
- 飞书 Card JSON 1.0 schema 参考：`https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/feishu-cards/`

---

**草拟完成 — 等用户审计 / 批准后进入实施。**

---

## 10. 交付总结 (2026-05-15 16:50 CST，rev1 补录)

### 10.1 时间线

| 时间 | 事件 |
|---|---|
| 2026-05-15 ~10:00 | rev0 初稿，Momus 评审通过"无阻塞问题，可执行" |
| 2026-05-15 ~11:00 | commit `589ab381` 落地，单测全绿（97/97 lark），CHANGELOG 同步更新 |
| 2026-05-15 ~11:52 | 用户部署到生产 `Malorian-3516`（`zeroclaw --version` 确认 `git-commit: 589ab381`） |
| 2026-05-15 13:52 | 用户报告："点 Always 没反应"。"喝水提醒" cron 任务实际已创建 → 后端正常但 UI 不变 |
| 2026-05-15 ~14:00-15:30 | 多轮诊断：沙箱 → 远端 WS 不通（IP 白名单），通过用户日志 grep 二分缩小嫌疑 |
| 2026-05-15 ~15:30 | 关键发现：默认日志级别 `info`，`tracing::warn!` 写到 stderr → 用户日志文件捕获 stderr → 但相关关键字 0 匹配 + cron 任务触发 → 高置信度推断 PATCH 调用了且 `code: 0` 但客户端不渲染 |
| 2026-05-15 ~16:00 | commit `2300498a` 落地：Card 2.0 schema + 5 条 info/warn 观测日志 + 测试加固 schema 2.0 断言 |
| 2026-05-15 ~16:30 | 用户重新部署 |
| 2026-05-15 16:43:42 | 生产日志确认：`Lark: approval card PATCH succeeded (status=200 OK)`，飞书客户端卡片正确渲染 |
| 2026-05-15 ~16:50 | 用户确认 "现在好像可以了"，任务完成；本计划文档归档 |

### 10.2 真凶

**飞书 IM `PATCH /im/v1/messages/{id}` 对 Card JSON 1.0 envelope 是"接受但不渲染"**：
- HTTP 200 + `{"code": 0, "data": {}}` ← 协议层完美成功
- 客户端不重新渲染卡片 ← UI 层静默失败

只有 Card JSON 2.0 envelope（带 `"schema": "2.0"`、用 `body.elements` 而非 `elements`）才能让飞书客户端真正刷新已发送的卡片。生产已验证的 `patch_card_content`（流式输出）一直用 Card 2.0，是这次诊断的关键参照系。

### 10.3 rev0 的盲点（已记录在 `.sisyphus/notepads/.../learnings.md`）

1. Plan §5.2 R2 假设 "原卡 1.0 → PATCH 用 1.0 即可"，**未做生产联调**
2. wiremock 单测 mock 永远返回 `code: 0` → 单测过 ≠ 客户端真的渲染
3. Plan §5.1 step 4 "联调（可选 / 用户侧）" 标记 "可选" → 本应对 IM UX bug 设为强制

### 10.4 rev1 防御性加固

1. **测试加 schema 断言**：`body_string_contains("\\\"schema\\\":\\\"2.0\\\"")` 让任何未来误改回 1.0 的 PR 直接红
2. **生产可观测性**：3 条 `info!` 覆盖 click 接收 / PATCH dispatch / PATCH succeeded 三个关键节点，下次任何审批问题用 `tail -f log | grep "approval card"` 立即定位
3. **`build_resolved_approval_card` 加 doc 注释**：解释"为什么必须 Card 2.0、Card 1.0 静默失败的坑"，防止后续维护者"清理"回 1.0

### 10.5 最终交付

- 分支：`fix/feishu-approval-card-resolved-state`（领先 master 2 commits）
- Commits：
  - `589ab381` fix(channels): patch feishu/lark approval card to show resolved state
  - `2300498a` fix(channels): use Card 2.0 schema for resolved approval card
- CHANGELOG-next.md：已更新，包含 Card 2.0 schema 选型说明和 PATCH 失败软降级行为
- 验证：clippy clean、97/97 lark 测试通过、1037/1039 全 channels 测试通过（2 pre-existing telegram failures，与本 PR 无关）
- 生产：已部署 `Malorian-3516`，2026-05-15 16:43 验证客户端正确渲染

### 10.6 后续动作（不在本计划范围）

合并到 master 由用户自行处理（沙箱无 gitee push 凭证，且 master merge 是用户决策）。

