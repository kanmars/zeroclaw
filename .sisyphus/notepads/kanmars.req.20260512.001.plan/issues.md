# Issues — kanmars.req.20260512.001

## 🔴 PR2 emoji 映射错误（2026-05-12 Librarian Q3 发现）

### Bug
PR2 commit `b2aa5604` 在 `unicode_to_lark_emoji_type` 用了 4 个**飞书 API 不认识**的 emoji_type 名字：

| PR2 使用 | 状态 | 正确名字 |
|---|---|---|
| `"THUMBSUP"` | ✅ 有效 | — |
| `"NO_ENTRY"` | ❌ **无效** | `"No"` (PascalCase) |
| `"WARNING"` | ❌ **无效** | `"Alarm"` 或 `"ERROR"` |
| `"EYES"` | ❌ **无效** | `"GLANCE"` |
| `"DONE"` | ✅ 有效 | — |
| `"DONE"` (for ✔️) | ✅ 有效 | — |
| `"HEART"` | ✅ 有效 | — |
| `"CELEBRATE"` | ❌ **无效** | `"PARTY"` |

### 影响
- 飞书用户**看不到 🚫 / ⚠️ / 👀 / 🎉 反应** —— 飞书 API 返回 `code != 0` + warn log
- Reply-Intent Precheck 的 `🚫`（Refused）和 `⚠️`（Failed）**全部静默**
- G2 AC-2.1 "Bot 决定不回复时...👍/🚫/⚠️ emoji 反应" 未真实满足

### 根因
我起草 PR2 delegation prompt 时推测 `NO_ENTRY/WARNING/EYES/CELEBRATE` 是合理猜测（这些是 Unicode 名称），但**没 cite 官方 emoji_type 表**。Librarian 查到真实表：飞书用 `No/Alarm/GLANCE/PARTY` 等**非一致命名**（ALL_CAPS + PascalCase + MixedCase 混用），必须逐字匹配。

### 修法
在 `fix/feishu-reply-intent-reactions` 分支上再做一个 atomic commit（或 amend） 修正映射：

```rust
fn unicode_to_lark_emoji_type(emoji: &str) -> Option<&'static str> {
    match emoji {
        "👍" => Some("THUMBSUP"),   // ✅ verified
        "🚫" => Some("No"),         // ← fixed (was NO_ENTRY)
        "⚠️" => Some("Alarm"),      // ← fixed (was WARNING)
        "👀" => Some("GLANCE"),     // ← fixed (was EYES)
        "✅" => Some("DONE"),        // ✅ verified
        "✔️" => Some("DONE"),       // ✅ verified
        "❤️" => Some("HEART"),      // ✅ verified
        "🎉" => Some("PARTY"),      // ← fixed (was CELEBRATE)
        "👎" => Some("ThumbsDown"), // optional: PascalCase
        _ => None,
    }
}
```

同时 **单元测试必须更新**（断言改 canonical 名字），不然会断。

### Amend vs New Commit
Per MEMORY.md §4.1 + AGENTS.md git safety protocol: 
- PR2 commit `b2aa5604` **尚未 push 到 gitee**（用户还没帮 push）
- 未 push 可以 amend ✅
- 但 subagent 约定每 PR 独立 commit，追加 fix commit 更符合审计。**决策：追加 commit** `fix(lark): correct emoji_type casing per Lark canonical table`

## ⚠️ Librarian Q1/Q2/Q3 完整研究成果

### Q1: Card PATCH API (PR4 reference)
- `PATCH {base}/open-apis/im/v1/messages/{message_id}`
- Body: `{"content": "<JSON-stringified card>"}`  
- Rate limit: **5 QPS per message_id** (hard cap; error 230020)
- Max body: **30 KB** (error 230025)
- Max age: **14 days** (error 230031)
- Error codes: 230011 (recalled), 230110 (deleted), 230020 (rate), 230025 (size), 230027 (perm), 230028 (DLP), 230031 (age), 230099 (malformed), 232009 (chat dissolved)
- Scope: `im:message:send_as_bot`（ZeroClaw 已有）

### Q2: card.action.trigger event (PR3 reference)
- Event name: `card.action.trigger`（schema=2.0；v1 是 `card.action.trigger_v1` deprecated）
- Full event JSON: operator.open_id / action.value / action.tag / event.context.{open_message_id, open_chat_id}
- `value` 字段 round-trip verbatim，但顶层 key 必须是 String
- **Size limit NOT doc'd，推荐 < 1KB**；大数据存自己 DB 传 opaque id
- 响应格式 3 种：`{}` 裸 ack / `{toast:{type,content}}` / `{toast,card}` 替换卡片
- **必须 200 OK + 3 秒内响应**（否则 error 200341）
- toast.type = info/success/error/warning
- WS 和 webhook 模式都通，WS 用现有 `get_ws_endpoint` 共用 `im.message.receive_v1` 同通道，仅需按 `header.event_type` 分发
- **任何群成员都能点** — 没有内建"仅原接收者可点"保护；必须在 `value` 里带 `intended_approver_open_id` 自己校验
- 按钮 `confirm` 属性客户端对话框，适合 Deny 等破坏操作

### Q3: emoji_type 命名混乱
- 飞书 API 的 emoji_type 字符串**case-sensitive**
- 同一表内既有 ALL_CAPS (OK, THUMBSUP, HEART, PARTY, FIRE) 也有 PascalCase (ThumbsDown, CheckMark, CrossMark, MeMeMe, Yes, No, Alarm)
- 有的在用户直觉命名下**根本不存在**（如 WARNING/NO_ENTRY）
- **必须查官方 emoji 表逐字对照**，不能猜

### Source URLs
- PATCH doc: https://open.larksuite.com/document/server-docs/im-v1/message-card/patch
- Card callback doc: https://open.larksuite.com/document/uAjLw4CM/ukzMukzMukzM/feishu-cards/card-callback-communication
- emoji 表: https://open.larksuite.com/document/server-docs/im-v1/message-reaction/emojis-introduce
- Button component: https://open.feishu.cn/document/uAjLw4CM/ukzMukzMukzM/feishu-cards/card-components/interactive-components/button
- Go SDK: https://github.com/larksuite/oapi-sdk-go/blob/v3_main/event/dispatcher/callback_dispatch.go
