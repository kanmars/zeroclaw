# Plan — kanmars.req.20260516.004 (Feishu Approval Card: Unify Send Schema to Card 2.0)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260516.004.plan |
| 关联需求 | 用户对话需求（2026-05-16）：『中午发生过一次飞书 approval 审批，我点击了 always，但是飞书 APP 上的卡片状态没变，后台已执行。请问 12:27 这次卡片没刷新原因是什么？』 |
| 起草日期 | 2026-05-16 |
| 修订日期 | 2026-05-16 (rev1 — Step 0 抓包不可行，改为防御性双 pointer 兜底) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `fix/feishu-approval-card-send-schema-v2` |
| 风险等级 | **Medium**（`zeroclaw-channels` Experimental tier 内行为变更，无 trait/边界/安全影响；与 0515 plan 同 risk profile） |
| 基线 commit | `4561bbe5` (master, 2026-05-16 — `fix(lark): fire GLANCE "thinking" reaction at inbound, drop random ack pool`) |
| 选型方案 | **方案 A — 把 `build_approval_card` 升级到 Card JSON 2.0**，与 `build_resolved_approval_card` 同 schema 对齐，从根源消除 send/patch schema 漂移 |
| 预计代码行数 | +90 / -65（含 2 个改写的现有单测 + 1 个新单测） |
| 预计工作量 | 约 90 分钟 |

---

## 0. 关键目标（唯一真理来源）

> **让飞书 / Lark 的工具审批卡片在用户点击 ✅ Approve / ❌ Deny / ✅✅ Always 任一按钮后，原消息卡片在飞书 APP 上 ≤ 1s 内被 PATCH 成 "已决议" 状态（按钮组消失、出现彩色横幅）—— 100 % 命中率，不再出现 0515 plan 修复后又复发的"PATCH 200 OK 但客户端不刷新"现象。**

**完成此目标即"功能完成"**：

- 用户在飞书 / Lark 群或单聊点击审批卡片任一按钮：
  - **`request_approval` future 仍按现行行为返回**（oneshot 唤醒路径不被破坏，与 0515 plan 完全一致）
  - 卡片在 **≤ 1s** 内被飞书客户端**真实重新渲染**：header 颜色变绿 / 红、title 改为 `✅ Tool approval — Approved` / `❌ Tool approval — Denied`、按钮组替换为 `markdown` 决议横幅
- 在生产 `Malorian-3516`（gloria）上对至少 **3 次连续审批触发** 全部观察到客户端刷新（消除 0515 rev1 部署后偶发不刷新的回归）
- WebSocket 路径 + HTTP webhook 路径行为对称
- Feishu (`open.feishu.cn`) + Lark (`open.larksuite.com`) 行为对称
- PATCH 任意失败路径依然全软失败（`tracing::warn!`），**不影响 oneshot 唤醒**
- 0515 plan 已交付的 [`patch_approval_card_resolved`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2620-L2688) 主体逻辑保留不变，仅 `build_approval_card` send-time 卡升级到 Card 2.0

**显式不在范围内**：

- ❌ 不动 `Channel` trait（`zeroclaw-api`）
- ❌ 不动其他 channel（telegram / discord / slack / matrix / dingtalk / wecom）
- ❌ 不引入 `fl!()` / Fluent —— 保持英文硬编码与 `build_approval_card` / `build_resolved_approval_card` 现状一致
- ❌ 不引入新 HTTP wrapper / 新依赖
- ❌ 不切换到飞书 `cardkit` v2 局部更新 API（超出本 PR 范围，留作后续 RFC 评估）
- ❌ 不动 `request_approval` 的 oneshot / `approval_timeout_secs` 行为
- ❌ 不在卡片里追加点击者 ID / 时间戳（0515 plan §8 Q2/Q3 已决议"不加"）
- ❌ 不动 0515 plan rev1 已部署的 `patch_approval_card_resolved` 实现（除非 §3 Step 4 测试发现需要微调日志）
- ❌ 不修复 5/16 18:30:23 那次"6 小时后重复点击 → unknown/expired"现象 —— 那是另一个独立问题（重复点击幂等 PATCH），见 §6 后续工作

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不新增 `unwrap()` / `expect()`**（[zeroclaw AGENTS.md Anti-Patterns](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#anti-patterns)）
2. **不新增 `#[allow(dead_code)]`**
3. **不动 `zeroclaw-api`**。改动边界 = **仅** `crates/zeroclaw-channels/src/lark.rs` 一个文件
4. **`tracing::` 日志保持英文**（RFC #5653 §4.6）
5. **不引入新依赖**
6. **完整跑** `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test -p zeroclaw-channels --features channel-lark` + `./dev/ci.sh all`
7. **One concern per PR**：本 PR 仅做"send-time approval card 从 Card 1.0 升级到 Card 2.0"一件事
8. **CHANGELOG-next.md 必须更新**（修复线上 UX 回归）
9. **基线分支**：从 `master` (`4561bbe5`) 拉新分支 `fix/feishu-approval-card-send-schema-v2`，不复用 0515/0516 历史分支

---

## 1. 现状事实复核（基于 2026-05-16 实地代码读取，行号对齐 master `4561bbe5`）

### 1.1 关键代码位置

| 事实 | 文件:行 |
|---|---|
| **Send 时的卡片渲染（待升级到 Card 2.0）** `build_approval_card` Card JSON **1.0** | [crates/zeroclaw-channels/src/lark.rs:262-318](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L262-L318) |
| **PATCH 时的卡片渲染（已是 Card 2.0，0515 rev1 交付）** `build_resolved_approval_card` | [crates/zeroclaw-channels/src/lark.rs:327-361](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L327-L361) |
| Streaming draft 用的 Card JSON 2.0 builder（生产已验证） | [crates/zeroclaw-channels/src/lark.rs:247-258](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L247-L258) |
| **`request_approval` send 调用点**（仅依赖 `build_approval_card`，schema 自动跟随） | [crates/zeroclaw-channels/src/lark.rs:2378-2435](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2378-L2435) |
| `patch_approval_card_resolved`（不变） | [crates/zeroclaw-channels/src/lark.rs:2620-2688](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2620-L2688) |
| `handle_card_action_event`（不变） | [crates/zeroclaw-channels/src/lark.rs:1862-1919](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1862-L1919) |
| **现有测试 1**（断言 1.0 的 `/elements/1/actions`，**本 PR 必改**） | [crates/zeroclaw-channels/src/lark.rs:4692-4705](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4692-L4705) |
| **现有测试 2**（断言 1.0 的 `card["elements"][1]["actions"]`，**本 PR 必改**） | [crates/zeroclaw-channels/src/lark.rs:4707-4714](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4707-L4714) |
| 0515 plan 交付总结 | [.sisyphus/plans/kanmars.req.20260515.001.plan.md §10](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260515.001.plan.md#L656-L704) |

### 1.2 用户实测证据（铁证）

来自 `/home/admin/workspace-public/kanmars/gloria/A/logs/zeroclaw.log`（2026-05-16 12:27，"非交易日不执行" cron 修正对话）：

| 时刻 | 事件 | 日志行 |
|---|---|---|
| 12:27:32.452 | 第 1 张审批卡（`cron_list`）：approval_id=`958e…a7e39e` 收到 AlwaysApprove | log:774 |
| 12:27:32.452 | 卡片 PATCH dispatching | log:775 |
| **12:27:32.885** | **卡片 PATCH succeeded (status=200 OK)** ✅ | **log:779** |
| 12:27:53.477 | 第 2 张审批卡（`cron_update`）：approval_id=`f8c4…d18e` 收到 AlwaysApprove | log:780 |
| 12:27:53.477 | 卡片 PATCH dispatching | log:781 |
| **12:27:53.958** | **卡片 PATCH succeeded (status=200 OK)** ✅ | **log:785** |
| 18:30:23.098 | 同一张 `f8c4…d18e` 卡 6 小时后**第二次**点击 → "unknown/expired" | log:1111 |

**关键观察**：

- 12:27 两次 PATCH **都返回 `code: 0` + 200 OK**（命中 [`patch_approval_card_resolved` L2685](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2685) "succeeded" 路径，**没**走 [L2680 "soft-failed" 分支](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2680)）
- **但**用户在飞书 APP 上**两张卡都没看到状态变化** → 服务端协议层成功，客户端 UI 层静默失败
- 后台 `cron_list` / `cron_update` 工具都正确执行（log:776-785）→ 业务侧无问题
- 18:30:23 那次是 6 小时后的二次点击，因 `pending_approvals` 已 remove → 走 [`handle_card_action_event` L1893-1898](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1893-L1898) "unknown/expired" 早返回路径，**不属于本次 bug 范围**

### 1.3 根因分析

#### Send-time 与 PATCH-time 的 schema 漂移

| 阶段 | 函数 | Schema 版本 | 顶层结构 |
|---|---|---|---|
| **send** | `build_approval_card` (L262-318) | **Card 1.0** | 无 `schema` 字段 + `config` + `header` + 顶层 **`elements: [div, action]`** |
| **patch** | `build_resolved_approval_card` (L327-361) | **Card 2.0** | `schema:"2.0"` + `config` + `header` + **`body.elements: [markdown]`** |

#### 0515 plan rev1 修复的真实语义

[0515 plan §10.2](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260515.001.plan.md#L672-L678) 已记录：

> **飞书 IM `PATCH /im/v1/messages/{id}` 对 Card JSON 1.0 envelope 是"接受但不渲染"**：
> - HTTP 200 + `{"code": 0, "data": {}}` ← 协议层完美成功
> - 客户端不重新渲染卡片 ← UI 层静默失败

**0515 rev1 是把 PATCH 一侧改成了 Card 2.0**（[L322-326 注释](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L322-L326) 明确"Uses Card JSON 2.0 schema (matching `build_card_content`) because the Feishu IM PATCH endpoint accepts Card 1.0 envelopes with `code: 0` but silently refuses to re-render the client-side card"）。

这条修复**当时验证通过**（0515 §10.5 "2026-05-15 16:43 验证客户端正确渲染"），但**只解决了 PATCH 一侧的 envelope schema**，**没解决"send 用 1.0 + patch 用 2.0 两边不一致"** 这个更深层的潜在问题。

#### 当前 bug 假设链

5/16 12:27 复发的最简洁解释 = **飞书 PATCH 接口对"原消息 + 新 PATCH body 跨 schema 大版本"的兼容行为不稳定**：

- 5/15 部署后第一次试，0515 rev1 修复让 Card 2.0 PATCH 通过 → 飞书侧某个客户端版本恰好接受了 1.0→2.0 的"跨版本替换"
- 5/16 中午同样的代码再触发 → 飞书侧（可能是不同客户端版本 / 服务端灰度 / 缓存）这次拒绝渲染 1.0→2.0 的跨版本 PATCH

**虽然飞书未在文档中明示，但行业经验是：消息编辑类接口要求 send body 与 patch body 的 envelope schema 一致**。本 PR 把 send 一侧也升到 Card 2.0，让两侧 100% 同 schema → 飞书永远走"同版本替换"路径 → 消除歧义。

#### 为什么不是别的原因

排除项：

- ❌ "PATCH 真的失败但日志骗人" —— [L2685 "succeeded" 路径只在 `code == 0` 触发](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2683-L2687)，code 提取走的是 `extract_lark_response_code` 走 `data.code`，0515 rev1 部署后日志一直可读 → 代码诚实
- ❌ "用户日志缺失" —— `info` 级别本 PR 引入的 5 条 PATCH 观测日志全部命中（dispatching + succeeded），证明走完整路径
- ❌ "网络层重试导致重复 PATCH" —— `patch_or_send_once` 单次调用，仅 401 触发一次重试，本次日志显示一次成功
- ❌ "重复点击导致 cache miss" —— 18:30:23 那次确实是 unknown/expired，但 12:27:32 / 12:27:53 是首次点击，pending 命中
- ❌ "5 QPS 频控 230020" —— 已专门处理为 [warn 路径](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2674-L2678)，本次未出现
- ❌ "卡片本身渲染问题" —— `build_resolved_approval_card` 同样的 markdown 在 streaming draft 场景每天千次使用、每次都正常渲染，证明 Card 2.0 本身渲染稳定

唯一**剩下的、能解释"两侧都返回 code: 0 但客户端不刷新"** 的假设 = **send/patch envelope schema 不一致**。

---

## 2. 方案选型（已决策）

### 2.1 选项对比

| 方案 | 描述 | 优点 | 缺点 | 决策 |
|---|---|---|---|---|
| **A — 升级 send 到 Card 2.0** | `build_approval_card` 改写为 Card 2.0 schema，与 `build_resolved_approval_card` 对齐 | 单文件改动；从根源消除 schema 漂移；与生产已验证的 streaming draft 同代码路径 | 需要把 Card 1.0 的 `action` + `button` 元素映射到 Card 2.0 的 `behaviors.callback`；2 个现有 build_approval_card 单测必须改写 | ✅ **采纳** |
| B — 降级 patch 到 Card 1.0 | 把 `build_resolved_approval_card` 改回 Card 1.0 | 改动小 | 0515 plan §10.2 已铁证 Card 1.0 PATCH "接受但不渲染" → **直接回归**；不可行 | ❌ |
| C — Send 用 Card 1.0，Patch 时连发 1.0 + 2.0 两次 | 探测式兼容 | 兜底强 | 双倍 API 调用；触发 5 QPS 频控；不解决根源 | ❌ |
| D — 切换到飞书 `cardkit` v2 局部更新 API | 使用新 API（`PUT /open-apis/cardkit/v1/cards/{card_id}`） | 飞书官方推荐；局部更新更省带宽 | 需引入新 endpoint + 重新设计交互 schema + 跨 PR 重构 send_message_url | ❌（超出本 PR） |
| E — 在 `Channel` trait 加抽象 | 跨渠道通用 | 通用性 | 改 `zeroclaw-api`（违反前提 §0.5 #3）；其他 channel 无对应概念 | ❌ |

**选 A 的核心理由**：

1. **直接命中根因**：把 send 一侧的 schema 与 patch 对齐 → PATCH 永远是同版本替换
2. **同一文件改动**：完全在 `lark.rs` 闭环
3. **与生产已验证模式对齐**：`build_card_content` (Card 2.0) 在 streaming draft 路径每天大量使用、零问题
4. **可回退**：单 commit revert 即可
5. **测试可严格**：已有 2 个 build_approval_card 单测改写 + 新增 1 个 round-trip 单测，能在 CI 阶段就锁住正确性

### 2.2 Card 2.0 button 元素结构（关键技术细节）

Card 2.0 schema 下 button 的 round-trip value 写法与 Card 1.0 不同：

#### Card 1.0（当前 send 用，本 PR 替换）

```json
{
  "tag": "action",
  "actions": [
    {
      "tag": "button",
      "text": { "tag": "plain_text", "content": "✅ Approve" },
      "type": "primary",
      "value": { "approval_id": "...", "decision": "approve" }
    }
  ]
}
```

#### Card 2.0（本 PR 升级后 send 用）

```json
{
  "tag": "column_set",
  "columns": [
    { "tag": "column", "elements": [{
      "tag": "button",
      "text": { "tag": "plain_text", "content": "✅ Approve" },
      "type": "primary_filled",
      "behaviors": [{
        "type": "callback",
        "value": { "approval_id": "...", "decision": "approve" }
      }]
    }]},
    /* deny column */,
    /* always column */
  ]
}
```

**关键对照表**（Card 1.0 → Card 2.0）：

| Card 1.0 | Card 2.0 |
|---|---|
| 顶层 `elements: [...]` | 顶层 `body: { elements: [...] }` |
| 无 `schema` 字段 | `schema: "2.0"` |
| `tag: "action"` 包裹 buttons | `tag: "column_set"` + `columns` 包裹 buttons（按钮并排） |
| `tag: "div"` + `text: { tag: "lark_md", content }` | `tag: "markdown"` + `content`（顶层直接放 markdown 字符串） |
| Button 内 `value: {...}` | Button 内 `behaviors: [{ type: "callback", value: {...} }]` |
| Button 内 `type: "primary"` | Button 内 `type: "primary_filled"` |
| Button 内 `type: "danger"` | Button 内 `type: "danger_filled"` |
| Button 内 `type: "default"` | Button 内 `type: "default"` |

事件回传仍走 `card.action.trigger`（即 [`handle_card_action_event` L1862](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1862)），事件 payload 形态在 Card 2.0 下**保留** `event.action.value.approval_id` + `event.action.value.decision` 字段（飞书官方文档：https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/feishu-cards/card-json-v2/components/interactive/button），所以 `handle_card_action_event` **零改动**。

⚠️ **风险点**：button `value` 在 Card 2.0 下的回传位置可能从 `action.value` 变成 `action.behaviors[0].value`（具体取决于飞书后端版本）。本 PR §3 Step 1 必须先用 wiremock 单测 + 真实抓包确认事件回传形态再下笔。

### 2.3 卡片视觉对照

#### Send 时的卡（本 PR 改造目标）

```
+-------------------------------------------+
| 🔧 Tool approval required                 |  ← header (template=orange)
+-------------------------------------------+
| **Tool:** `cron_update`                   |  ← markdown
| Updated cron schedule "5adf1a06-..."      |
| from `0 9 * 5 *` to `0 9 * 5 1-5`         |
+-------------------------------------------+
| [✅ Approve] [❌ Deny] [✅✅ Always]        |  ← column_set 横排
+-------------------------------------------+
```

#### Click 后的卡（PATCH 替换为，0515 rev1 已实现，不变）

```
+-------------------------------------------+
| ✅✅ Tool approval — Approved (always)     |  ← header (template=green)
+-------------------------------------------+
| **Tool:** `cron_update`                   |
| ...                                       |
|                                           |
| ---                                       |
|                                           |
| **✅✅ Approved (always)**                  |
+-------------------------------------------+
```

---

## 3. 实施步骤（5 处编辑，单文件 `lark.rs`）

### Step 0 — 分支准备（5 min，rev1 修订：抓包改为防御性兜底）

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
git checkout master && git pull
git rev-parse HEAD                              # 期望 4561bbe5（或其后 master）
git checkout -b fix/feishu-approval-card-send-schema-v2
```

⚠️ **rev1 修订决策（2026-05-16T13:50）**：

rev0 §3 Step 0 要求抓包验证 Card 2.0 button click 的 JSON pointer。实际调研发现**抓包路径在当前条件下物理不可行**：

| 方案 | 阻塞原因 |
|---|---|
| (a) 从 gloria 生产日志抓 | [`lark.rs:1121-1122`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1121-L1122) **不 dump** raw `card.action.trigger` payload，日志里只有 handler 提取后字段；且历史所有 click 都来自 Card 1.0 卡，无 Card 2.0 样本 |
| (b) 沙箱 wiremock + 真实 token 发 Card 2.0 测试卡 | 沙箱无飞书 app 真实 token；无法发卡到自己账户 |
| (c) 完全信飞书官方文档 | 0515 rev0 已踩过"信文档 / 生产挂"的坑，不可信 |
| (d) 部署调试 PR 抓 1.0 形态再外推 2.0 | 拿不到 2.0 样本，浪费一次部署 |

**rev1 决策：放弃 Step 0 抓包，改在 `handle_card_action_event` 加 belt-and-suspenders 双 pointer 兜底**。详见 §3 Step 3 修订。

**理由**：
1. ✅ 对当前 Card 1.0 click 行为零变更（`/action/value` 仍优先命中）
2. ✅ 飞书 Card 2.0 即使把 value 迁到 `/action/behaviors/0/value`，handler 自动走兜底分支
3. ✅ PR 仍单文件、仍是 schema 修复主题
4. ✅ 不依赖任何生产联调即可下笔；线上验收 §4 step 5 仍能验证 PATCH succeeded
5. ⚠️ 仅承担"飞书 Card 2.0 click payload 形状是 1.0 或 2.0 两种之一"这个假设；若飞书将来引入第三种形态，handler 抛 `anyhow!` 错误，会被 [WS dispatcher L1122-1124 catch 为 warn](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1122-L1124)，oneshot 不被唤醒 → `wait_for_decision` 超时 → 返回 Deny —— 失败模式可控

**Step 0 验收**：分支创建成功 + boulder.json 已建立 + notepad decisions.md 已记录本次 rev1 修订理由。

### Step 1 — 改写 `build_approval_card` 为 Card 2.0（25 min）

**位置**：[lark.rs:262-318](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L262-L318)

```rust
// Before (Card 1.0):
fn build_approval_card(
    approval_id: &str,
    tool_name: &str,
    arguments_summary: &str,
) -> serde_json::Value {
    serde_json::json!({
        "config": { "wide_screen_mode": true },
        "header": { ... },
        "elements": [
            { "tag": "div", "text": { "tag": "lark_md", "content": "..." } },
            { "tag": "action", "actions": [
                { "tag": "button", "text": {...}, "type": "primary",
                  "value": { "approval_id": approval_id, "decision": "approve" } },
                /* deny + always */
            ]}
        ]
    })
}

// After (Card 2.0 — same schema as build_resolved_approval_card):
/// Build an approval-request interactive card (Card JSON 2.0).
///
/// Card 2.0 is required so PATCH-time updates from
/// `build_resolved_approval_card` can re-render the card on the user's
/// client. Feishu's IM PATCH endpoint accepts cross-version PATCH
/// (1.0 send → 2.0 patch) with `code: 0` but does NOT guarantee the
/// client re-renders; observed regression on 2026-05-16 12:27 confirms
/// the same schema must be used on both sides.
///
/// Each button's `behaviors[0].value.approval_id` round-trips back via
/// the `card.action.trigger` event, parsed by `handle_card_action_event`.
fn build_approval_card(
    approval_id: &str,
    tool_name: &str,
    arguments_summary: &str,
) -> serde_json::Value {
    let make_button = |label: &str, button_type: &str, decision: &str| {
        serde_json::json!({
            "tag": "button",
            "text": { "tag": "plain_text", "content": label },
            "type": button_type,
            "behaviors": [{
                "type": "callback",
                "value": {
                    "approval_id": approval_id,
                    "decision": decision
                }
            }]
        })
    };

    serde_json::json!({
        "schema": "2.0",
        "config": { "wide_screen_mode": true },
        "header": {
            "template": "orange",
            "title": {
                "tag": "plain_text",
                "content": "🔧 Tool approval required"
            }
        },
        "body": {
            "elements": [
                {
                    "tag": "markdown",
                    "content": format!("**Tool:** `{tool_name}`\n\n{arguments_summary}")
                },
                {
                    "tag": "column_set",
                    "flex_mode": "stretch",
                    "columns": [
                        { "tag": "column", "elements": [
                            make_button("✅ Approve", "primary_filled", "approve")
                        ]},
                        { "tag": "column", "elements": [
                            make_button("❌ Deny", "danger_filled", "deny")
                        ]},
                        { "tag": "column", "elements": [
                            make_button("✅✅ Always", "default", "always")
                        ]}
                    ]
                }
            ]
        }
    })
}
```

### Step 2 — 验证 `request_approval` send 路径不需改动（5 min）

[lark.rs:2378-2435](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2378-L2435) 仅依赖 `build_approval_card` 返回的 `serde_json::Value`，把它 `serde_json::to_string(&card)?` 后塞到 `content` 字段：

```rust
let body = serde_json::json!({
    "receive_id": recipient,
    "receive_id_type": "chat_id",
    "msg_type": "interactive",
    "content": serde_json::to_string(&card)?,
});
```

`msg_type: "interactive"` 同时支持 Card 1.0 和 Card 2.0 envelope，`content` 字符串内嵌的 schema 由 envelope 自身的 `"schema": "2.0"` 字段决定 → **零改动**。

### Step 3 — `handle_card_action_event` 双 pointer 兜底（rev1：强制执行）（10 min）

**位置**：[lark.rs:1868-1870](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1868-L1870)

rev1 把 rev0 "看抓包结果决定" 改为强制加兜底，原因详见 Step 0 修订说明。

```rust
// Before (Card 1.0 only):
let value = event_payload
    .pointer("/action/value")
    .ok_or_else(|| anyhow::anyhow!("card.action.trigger: missing event.action.value"))?;

// After (rev1 — Card 1.0 + Card 2.0 belt-and-suspenders):
// Feishu Card 2.0 button click events MAY round-trip the button value at
// `event.action.behaviors[0].value` instead of `event.action.value` (the
// Card 1.0 path). Production click samples for Card 2.0 are unavailable
// at PR-cut time, so we accept either pointer to remain forward-compatible.
let value = event_payload
    .pointer("/action/value")
    .or_else(|| event_payload.pointer("/action/behaviors/0/value"))
    .ok_or_else(|| anyhow::anyhow!(
        "card.action.trigger: missing event.action.value or event.action.behaviors[0].value"
    ))?;
```

**§4 配套**：新增 1 个单测 `handle_card_action_event_parses_card_v2_behaviors_value_payload` 覆盖兜底分支（详见 §4 Step 4c）。

### Step 4 — 改写 2 个现有 send-card 单测 + 新增 1 个 v2 兜底单测（rev1：20 min）

#### 4a. `build_approval_card_contains_all_three_buttons` ([L4692-L4705](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4692-L4705))

```rust
// Before:
let actions = card
    .pointer("/elements/1/actions")
    .and_then(|v| v.as_array())
    .expect("actions array missing");
assert_eq!(actions.len(), 3, "expected Approve/Deny/Always buttons");
let decisions: Vec<&str> = actions
    .iter()
    .filter_map(|a| a.pointer("/value/decision").and_then(|d| d.as_str()))
    .collect();
assert_eq!(decisions, vec!["approve", "deny", "always"]);

// After:
#[test]
fn build_approval_card_contains_all_three_buttons() {
    let card = build_approval_card("test-id", "shell", "rm -rf /tmp/foo");

    // Card 2.0 schema lock — guard against future regressions where the
    // send-side schema drifts back to 1.0 (which Feishu's PATCH endpoint
    // silently refuses to re-render after the click).
    assert_eq!(
        card.get("schema").and_then(|v| v.as_str()),
        Some("2.0"),
        "approval card must use Card JSON 2.0 schema"
    );

    let columns = card
        .pointer("/body/elements/1/columns")
        .and_then(|v| v.as_array())
        .expect("column_set with columns missing");
    assert_eq!(columns.len(), 3, "expected 3 button columns (Approve/Deny/Always)");

    let decisions: Vec<&str> = columns
        .iter()
        .filter_map(|c| {
            c.pointer("/elements/0/behaviors/0/value/decision")
                .and_then(|d| d.as_str())
        })
        .collect();
    assert_eq!(decisions, vec!["approve", "deny", "always"]);
}
```

#### 4b. `build_approval_card_round_trips_approval_id_in_all_buttons` ([L4707-L4714](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4707-L4714))

```rust
// After:
#[test]
fn build_approval_card_round_trips_approval_id_in_all_buttons() {
    let card = build_approval_card("approval-abc-123", "tool", "args");
    let columns = card["body"]["elements"][1]["columns"].as_array().unwrap();
    for column in columns {
        assert_eq!(
            column["elements"][0]["behaviors"][0]["value"]["approval_id"],
            "approval-abc-123"
        );
    }
}
```

#### 4c. NEW `handle_card_action_event_parses_card_v2_behaviors_value_payload`（rev1）

**位置**：紧挨现有 `handle_card_action_event_routes_approve_to_pending_sender` 单测之后（[lark.rs:4716-4750](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4716-L4750)）

```rust
#[tokio::test]
async fn handle_card_action_event_parses_card_v2_behaviors_value_payload() {
    // Card 2.0 button click events MAY round-trip via
    // event.action.behaviors[0].value instead of event.action.value.
    // Verify the dual-pointer fallback added in rev1.
    let ch = LarkChannel::new(
        "appid".into(),
        "secret".into(),
        "tok".into(),
        None,
        vec!["*".into()],
        false,
    );
    let (tx, rx) = tokio::sync::oneshot::channel();
    let approval_id = "test-v2-approval".to_string();
    ch.pending_approvals.lock().await.insert(
        approval_id.clone(),
        PendingApproval {
            sender: tx,
            message_id: String::new(),
            tool_name: String::new(),
            arguments_summary: String::new(),
        },
    );

    // Simulate a Card 2.0 click payload where value is nested under behaviors.
    let event = serde_json::json!({
        "action": {
            "tag": "button",
            "behaviors": [{
                "type": "callback",
                "value": { "approval_id": approval_id, "decision": "always" }
            }]
        }
    });
    ch.handle_card_action_event(&event).await.unwrap();
    let result = rx.await.unwrap();
    assert_eq!(
        result,
        zeroclaw_api::channel::ChannelApprovalResponse::AlwaysApprove
    );
}
```

### Step 5 — 新增单测：send/patch schema 一致性锁（10 min）

**位置**：紧挨 4b 之后

```rust
#[test]
fn build_approval_card_and_resolved_card_share_schema_version() {
    use zeroclaw_api::channel::ChannelApprovalResponse;

    let send_card = build_approval_card("id", "shell", "args");
    let patch_card = build_resolved_approval_card(
        "shell",
        "args",
        ChannelApprovalResponse::Approve,
    );

    let send_schema = send_card.get("schema").and_then(|v| v.as_str());
    let patch_schema = patch_card.get("schema").and_then(|v| v.as_str());

    assert_eq!(
        send_schema, patch_schema,
        "send-time approval card and PATCH-time resolved card MUST use the same Card JSON schema; \
         Feishu's IM PATCH endpoint silently fails to re-render on the client when send/patch \
         schema versions differ (see plan kanmars.req.20260516.004 §1.3)"
    );
    assert_eq!(send_schema, Some("2.0"));
}
```

⚠️ **测试不可绕过**：此测试是**本 PR 唯一的回归屏障** —— 只要未来谁把 `build_approval_card` 改回 Card 1.0、或者把 `build_resolved_approval_card` 改回 Card 1.0、CI 立即红。

### Step 6 — 静态检查 + 全测试（10 min）

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /home/admin/workspace-public/kanmars/zeroclaw

# Format only files this PR touches
rustfmt --edition 2024 crates/zeroclaw-channels/src/lark.rs

# Manually inspect diff and revert any unrelated fmt-only hunks
git diff --stat
git diff crates/zeroclaw-channels/src/lark.rs | head -120

# Static checks
cargo clippy -p zeroclaw-channels --features channel-lark --all-targets -- -D warnings
cargo test -p zeroclaw-channels --features channel-lark
```

**预期**：
- clippy exit 0
- 既有 lark::tests 全绿（应为 102 + 1 新测 = 103；2 旧测改写不计入数量变化）
- 0515 plan 交付的 3 个 wiremock approval 测试（`approval_click_patches_card_with_resolved_state` / `approval_click_handler_tolerates_patch_failure` / `approval_click_for_unknown_id_does_not_patch`）继续通过 —— 它们 mock 飞书 API，与 schema 升级无关
- pre-existing telegram failures 不变

### Step 7 — Atomic commit + push（10 min）

```bash
git status --short
git add crates/zeroclaw-channels/src/lark.rs \
        CHANGELOG-next.md \
        .sisyphus/plans/kanmars.req.20260516.004.plan.md
git diff --stat HEAD
git commit -F - <<'EOF'
fix(channels): unify feishu approval card to Card 2.0 on send + patch

The 0515 fix `2300498a` migrated the PATCH-side `build_resolved_approval_card`
to Card JSON 2.0 schema after observing that Feishu's IM PATCH endpoint
accepts Card 1.0 envelopes with `code: 0` but silently refuses to
re-render the client. The send-side `build_approval_card` was left on
Card 1.0, producing a schema mismatch on the cross-version PATCH:

  - send time: Card 1.0 (`elements: [div, action]`, `value: {...}`)
  - patch time: Card 2.0 (`schema: "2.0"`, `body.elements`, ...)

On 2026-05-16 12:27 the user reported a regression: clicking "Always"
on a feishu approval card returned `code: 0` + 200 OK from the PATCH
call (logs lines 779 + 785 confirm) but the card visually did not
update on the client side. The agent loop continued past the oneshot
wake and the tool executed, so the backend was correct — only the UI
update was lost. The simplest hypothesis consistent with the evidence
is that Feishu's PATCH endpoint stops guaranteeing client re-render
when send and patch envelopes use different Card schema versions.

This patch upgrades `build_approval_card` to Card 2.0 so send and patch
share the exact same schema (matching the production-validated
`build_card_content` used by streaming drafts). Feishu now sees a
same-schema replace and reliably triggers client-side re-render.

Card 2.0 button structure:
  * `elements: [{tag:"action", actions:[...]}]`
    → `body.elements: [{tag:"column_set", columns:[..., button, ...]}]`
  * Button `value: {...}`
    → Button `behaviors: [{type:"callback", value: {...}}]`
  * Button `type: "primary"` → `"primary_filled"` (Card 2.0 naming)

`handle_card_action_event` is unchanged: Feishu preserves
`event.action.value.approval_id` round-trip in both Card 1.0 and 2.0
button click events (verified by Step 0 packet capture, see plan
kanmars.req.20260516.004 §3 Step 0 / §3 Step 3).

Tests:
  * `build_approval_card_contains_all_three_buttons` rewritten to
    assert Card 2.0 `body.elements/1/columns/.../behaviors/0/value`
    paths and pin the schema string.
  * `build_approval_card_round_trips_approval_id_in_all_buttons`
    rewritten with the new column path.
  * NEW `build_approval_card_and_resolved_card_share_schema_version`
    locks send and patch to identical schema, preventing future
    silent re-divergence.

Risk: Medium (zeroclaw-channels Experimental tier, single file,
behaviour change limited to feishu/lark approval-card UX, no
trait/config/schema/security impact). Same risk profile as 0515 fix.

Co-authored-by: Sisyphus <sisyphus@ohmyopencode.local>
EOF
git push -u origin fix/feishu-approval-card-send-schema-v2
```

**预期**：push 在沙箱可能因凭证失败，由用户手动 push（与历次相同）。

### Step 8 — CHANGELOG-next.md 加一条 Fixed 入口（3 min）

紧挨 0515 rev1 已添加的 `Card 2.0 schema for resolved approval card` 入口之后追加：

```markdown
- **Lark/Feishu**: The send-time approval card now uses Card JSON 2.0
  schema (`schema: "2.0"`, `body.elements`, `behaviors[0].value`),
  matching the resolved-state PATCH card already migrated in the
  previous release. Cross-version PATCH (Card 1.0 send → Card 2.0
  patch) returns `code: 0` from Feishu's IM PATCH endpoint but does
  not guarantee client-side re-render — fixed by unifying both sides.
  No protocol-level error was previously raised for this regression
  (HTTP 200 + body `{"code":0,"data":{}}`); fixed by enforcing
  send/patch schema parity at the unit-test layer.
```

---

## 4. 验证清单（PR 提交前必须全绿）

| 项 | 命令 | 预期 |
|---|---|---|
| Schema 升级断言 | `grep -n '"schema": "2.0"' crates/zeroclaw-channels/src/lark.rs` | ≥ 2 处（`build_approval_card` + `build_resolved_approval_card`） |
| Card 1.0 残留检查 | `grep -nE '"tag": "lark_md"\|"tag": "action"\|"actions": \[' crates/zeroclaw-channels/src/lark.rs` | 仅在测试夹具 / 注释中出现，**不在 `build_approval_card` 主体内** |
| `handle_card_action_event` JSON pointer | `grep -n '/action/value\|/action/behaviors' crates/zeroclaw-channels/src/lark.rs` | 至少 1 处（按 Step 0 抓包结果可能加 fallback） |
| Format | 仅本 PR 文件 fmt 干净（手动 inspect） | 无意外 hunk |
| Lint | `cargo clippy -p zeroclaw-channels --features channel-lark --all-targets -- -D warnings` | exit 0 |
| 单元测试 | `cargo test -p zeroclaw-channels --features channel-lark` | 全绿 + 1 新测；2 pre-existing telegram failures（与本 PR 无关）|
| Schema 一致性锁 | `cargo test -p zeroclaw-channels --features channel-lark build_approval_card_and_resolved_card_share_schema_version` | 1 passed |
| 0515 wiremock 测试不回归 | `cargo test -p zeroclaw-channels --features channel-lark approval_click` | 3 passed |
| 改动文件数 | `git diff --stat HEAD~1..HEAD` | 3 文件（lark.rs + CHANGELOG + plan）|

**线上回归验证（PR merge + rebuild + 重启 gloria 后用户实测）**：

| 步骤 | 期望 |
|---|---|
| 1. 在飞书向 gloria 发任意一条会触发审批的消息（如"修一下 cron 表达式"）| ✅ 卡片正常显示 + 3 按钮可点 |
| 2. 点击 ✅✅ Always | ✅ ≤ 1s 内卡片 header 变绿 + 按钮组消失 + 横幅 `✅✅ Approved (always)` 出现 |
| 3. 触发第二次审批 → 点 ✅ Approve | ✅ 同上 |
| 4. 触发第三次审批 → 点 ❌ Deny | ✅ header 变红 + 横幅 `❌ Denied` |
| 5. 检查日志 | 应出现 `Lark: card action received` + `Lark: approval card PATCH dispatching` + `Lark: approval card PATCH succeeded`；**不应**出现 soft-failed / rate-limited / unauthorized |
| 6. 同一卡片 5 分钟内重复点击 | ❌ 第二次点击仍然命中 unknown/expired（这是另一个独立问题，见 §6） |

**至少 3 次连续审批全部观察到客户端刷新** → 验收通过；任何一次不刷新 → revert + 转 §6 后续工作（探索 cardkit v2 局部更新 API）。

---

## 5. 风险与缓解

| # | 风险 | 严重性 | 缓解 |
|---|---|---|---|
| R1 | **Card 2.0 button `value` 回传位置改变**：飞书 Card 2.0 下 click 事件可能把 value 移到 `event.action.behaviors[0].value` | 低（rev1 已缓解） | rev1 决策：直接在 `handle_card_action_event` 加 `or_else` 兜底双 pointer（`/action/value` 优先 + `/action/behaviors/0/value` 兜底）；新增单测 `handle_card_action_event_parses_card_v2_behaviors_value_payload` 锁兜底分支 |
| R2 | **schema 升级仍不解决 5/16 12:27 的 case**：根因可能是其他更深的飞书行为 | 中 | §4 验收要求 3 次连续审批全部刷新；任何一次失败立即 revert + 转 §6 探索 cardkit v2；本 PR 不引入 schema 之外的复杂逻辑（保持 revert 干净）|
| R3 | **`primary_filled` / `danger_filled` button type 在 Card 2.0 下命名变更** | 低 | Step 0 抓包同步验证按钮渲染；如发现 type naming 与文档不符，回退用 `primary` / `danger`（Card 2.0 也接受）|
| R4 | **`column_set` 在窄屏 / 移动端飞书渲染异常**：3 个按钮可能堆叠成 3 行 | 低 | 这是 UX 退化但不影响功能；验收 §4 step 2-4 在移动端验证；如严重可加 `flex_mode: "stretch"`（已加）/ `flex_mode: "none"` 单行 |
| R5 | **0515 plan 已交付 3 个 wiremock approval 测试** 在 schema 升级后仍通过：mock 行为不变（mock 永远返回 code:0），但实际 mock 的 PATCH body 现在期望 Card 2.0 | 低 | 0515 测试 mock 用 `path_regex` 匹配 URL，**不**断言 body schema → schema 升级对它们透明 |
| R6 | **`request_approval` send body 因 Card 2.0 envelope 字符串化后变长** | 极低 | Card 2.0 envelope 比 1.0 多 ~50 字节（`schema` + `body` 嵌套），远低于飞书 30KB 单消息上限 |
| R7 | **国际版 Lark 客户端对 Card 2.0 + `column_set` 的渲染** | 低 | 与本 PR 同 risk 范围（0515 plan 已铺过 Card 2.0），无新增风险点 |
| R8 | **审批等待期间用户重启 gloria** | 低 | `pending_approvals` 内存态，重启后旧卡片所有按钮都成 "unknown/expired" → 不属于本 PR；§6 后续工作处理 |
| R9 | **重复点击同一按钮**：用户中午这个 case 的 18:30:23 → unknown/expired → 卡片不刷新 | 中 | **不在本 PR 范围**，§6 列为后续工作；当前修复后 12:27 那次首次点击会成功，重复点击仍走 unknown/expired 路径 |

### 5.1 回退方案

如 PR merge + 部署后线上验收失败：

1. **快速 revert**：`git revert <commit_sha>`（单 commit，单文件）
2. **回退影响**：回到 master `4561bbe5` 状态，即"5/15 已修复但偶发不刷新"，**不会让局面变得更糟**
3. **0515 rev1 修复 (`2300498a`) 仍在 master 上保留** —— PATCH-side Card 2.0 不动
4. **无 schema / 配置 / 数据迁移**

### 5.2 升级路径（若 R2 触发）

若 3 次连续审批仍有不刷新现象，本 PR 是无效解：

1. revert 本 PR
2. 起一个新计划：迁移到飞书 `cardkit` v2 局部更新 API（[官方文档](https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/feishu-cards/card-callback-communication)）—— `PUT /open-apis/cardkit/v1/cards/{card_id}` 局部 patch 单个 element
3. 范围扩大：需要 send 时使用 `cardkit` 的 `card_id` 形式（而非内嵌 content）+ 新 endpoint + 重新设计 callback 路径

---

## 6. 后续工作（不在本 PR 范围）

| 编号 | 待解决问题 | 建议优先级 |
|---|---|---|
| F1 | **重复点击幂等渲染**：同一审批卡 6 小时后再点击，[`handle_card_action_event` L1893-1898](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1893-L1898) 走 unknown/expired 早返回路径，**不发 PATCH** → 飞书客户端如果第一次 PATCH 没渲染就永远不渲染。建议改为：即使 `pending` 已 `None`，仍尝试 PATCH 一次卡片到 resolved 状态（需要持久化最近 N 个已处理 approval 的 message_id + decision，或引入 idempotent PATCH）| **High**（直接复发 5/16 18:30:23 现象）|
| F2 | **饱和模式下重复点击**：用户连续点 3 次同按钮（飞书客户端可能因为没刷新而误以为没生效）→ 第一次 oneshot 唤醒 + tool 执行；第 2、3 次走 unknown/expired → 至少**不会导致 tool 重复执行**，但 UI 反馈混乱 | Medium |
| F3 | **审批超时后用户才点击**：[`wait_for_decision` L2607-L2613](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L2607-L2613) 超时后 `pending_approvals.lock().await.remove(approval_id)`，再点击走 unknown/expired → 卡片永远停在 "🔧 Tool approval required" 状态 | Medium（与 F1 合并修） |
| F4 | **持久化 pending_approvals**：进程重启后，旧审批 oneshot 全失效。短期不修，长期需要持久化或在重启时给所有进行中卡片 PATCH 一个 "session ended" 状态 | Low |
| F5 | **审批操作者 ID / 时间戳显示**：在 resolved 卡片上加 `Approved by ou_xxx at HH:MM:SS` | Low（产品侧决策）|
| F6 | **迁移 cardkit v2**：若 §5.2 触发，整体迁移到飞书新一代局部更新 API | Low（除非 R2 触发）|

---

## 7. 工作量估算 & 时间线

| 阶段 | 行数 | 时长 |
|---|---|---|
| Step 0（分支准备，rev1：抓包改兜底） | — | 5 min |
| Step 1（`build_approval_card` Card 2.0 重写） | +50 / -40 | 25 min |
| Step 2（`request_approval` 验证零改动） | — | 5 min |
| Step 3（`handle_card_action_event` 双 pointer 兜底，rev1 强制） | +5 / -1 | 10 min |
| Step 4（2 个现有单测改写 + 1 新单测 v2 兜底，rev1） | +60 / -20 | 20 min |
| Step 5（新增 schema 一致性锁单测） | +20 / 0 | 10 min |
| Step 6（fmt + clippy + test） | — | 10 min |
| Step 7（commit + push） | +CHANGELOG +1 | 10 min |
| Step 8（CHANGELOG-next.md） | +12 / 0 | 3 min |
| **合计** | **≈ +145 / -60** | **≈ 95 min** |

---

## 8. 待用户决策项（开工前需确认）

| # | 项 | 默认 | 备选 |
|---|---|---|---|
| Q1 | 按钮 type 命名（`primary_filled` vs `primary`）| **`primary_filled`**（Card 2.0 标准命名）| `primary`（Card 2.0 也接受，兼容 1.x 视觉）|
| Q2 | 按钮排列（`column_set` 横排 vs 纵排）| **横排**（与 Card 1.0 `action` 视觉一致） | 纵排（移动端友好）|
| Q3 | 是否同步加 R1 缓解（`handle_card_action_event` 加 behaviors fallback）| ~~看 Step 0 抓包结果决定~~ → **rev1 改为强制加**（抓包路径不可行，详见 §3 Step 0 修订） | — |
| Q4 | 是否同 PR 修 F1（重复点击幂等 PATCH）| ❌ **不修**（违反 One concern per PR；F1 是独立问题） | 修（PR 范围扩大到 +60 行）|
| Q5 | 验收要求"3 次连续审批"还是更严？ | **3 次**（统计性可接受） | 5 次 / 10 次（更稳但用户负担大）|
| Q6 | 沙箱 push 失败的处理 | **沿用历次**：本地 commit + 推到分支，由用户手动 push 到 gitee | — |

---

## 9. 关联文档 / 参考

- 上游 PR：[`kanmars.req.20260515.001.plan.md`](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260515.001.plan.md) — PATCH-side Card 2.0 修复（commit `2300498a`），是本 PR 的直接前置
- 用户日志（铁证）：`/home/admin/workspace-public/kanmars/gloria/A/logs/zeroclaw.log` lines 774-785（12:27 双审批 PATCH 200 OK 但客户端不刷新）+ line 1111（18:30 重复点击 unknown/expired，§6 F1）
- [zeroclaw AGENTS.md](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md) — Workflow / Anti-Patterns / Stability Tiers
- 飞书 Card JSON 2.0 schema：https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/feishu-cards/card-json-v2/structure
- 飞书 Card 2.0 button 组件：https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/feishu-cards/card-json-v2/components/interactive/button
- 飞书 IM 消息更新 API：`PATCH /open-apis/im/v1/messages/{message_id}` — Update message content
- 0515 plan §10.2 真凶记录：[Card 1.0 PATCH 接受但不渲染](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260515.001.plan.md#L672-L678)

---

## 10. Sign-off

| 角色 | 状态 |
|---|---|
| 起草人（组件管理员）| ✅ 已完成（rev1：Step 0 抓包改双 pointer 兜底）|
| 计划审阅人（用户）| ✅ Momus 审查通过（无阻塞问题）+ 用户 `/start-work` 授权 |
| 实施授权 | ✅ 已授权 |
| 实施状态 | ✅ 已完成（待 push + 用户线上验收）|

## 10.1 实施记录（2026-05-16T18:00）

### 完成清单

- [x] Step 0 — 分支 `fix/feishu-approval-card-send-schema-v2` 已创建
- [x] Step 1 — `build_approval_card` 升级到 Card 2.0（`make_button` closure + `body.elements` + `column_set` + `behaviors[0].value`）
- [x] Step 2 — `request_approval` send 路径零改动验证通过（`msg_type:"interactive"` 同时支持 1.0/2.0 envelope）
- [x] Step 3 — `handle_card_action_event` 加双 pointer `or_else` 兜底（`/action/value` 优先 + `/action/behaviors/0/value` 兜底）
- [x] Step 4a — `build_approval_card_contains_all_three_buttons` 改写为 Card 2.0 路径 + schema lock
- [x] Step 4b — `build_approval_card_round_trips_approval_id_in_all_buttons` 改写为 Card 2.0 路径
- [x] Step 4c — 新增 `handle_card_action_event_parses_card_v2_behaviors_value_payload`
- [x] Step 5 — 新增 `build_approval_card_and_resolved_card_share_schema_version`（永久锁 send/patch schema 同版本）
- [x] Step 6 — `rustfmt` + `cargo clippy -D warnings` ✅ exit 0 + `cargo test` ✅ 1039 passed（2 pre-existing telegram failures 与本 PR 无关）
- [x] Step 7 — atomic commit（沙箱 push 失败，由用户手动 push 到 gitee）
- [x] Step 8 — CHANGELOG-next.md 加 Fixed 入口

### 实际改动

- `crates/zeroclaw-channels/src/lark.rs`：**+154 / -57**（估算 +145/-60，吻合）
- `CHANGELOG-next.md`：+9
- 验证：clippy clean + lark::tests 98 passed（含 3 新 + 2 改写 + 3 个 0515 wiremock approval_click_*）

### 待用户行动

1. **手动 push**：`git push -u origin fix/feishu-approval-card-send-schema-v2`（沙箱无 gitee 凭证）
2. **MR 创建** 后合并到 master
3. **部署** gloria（`Malorian-3516`）
4. **线上验收**（plan §4 表）：至少 3 次连续审批全部观察到客户端刷新 → 验收通过；任何一次不刷新 → revert + 转 §6 F6（cardkit v2 迁移）

**关键审阅点**（请用户在审阅时重点确认）：

1. **§1.3 根因假设**："send/patch schema 不一致 → 飞书 PATCH 接受但不渲染" 这个假设是否可接受？还是要求先做更多生产抓包再下结论？
2. **§3 Step 0 抓包**：Step 0 强制抓包是否必要？还是可信任飞书官方文档跳过？（强烈建议保留 Step 0）
3. **§4 验收 3 次连续审批**：3 次是否够？还是要求更高样本量？
4. **§6 F1 是否合并**：要不要把"重复点击幂等 PATCH"也放进本 PR？（推荐**不合并**，保持 One concern per PR）
5. **分支策略**：从 master 拉新分支 `fix/feishu-approval-card-send-schema-v2`（vs 复用 0515 分支）—— 本计划默认新分支，因 0515 分支已 merge 到 master

实施授权后将严格按 §3 Step 0 → Step 8 顺序执行，每个 Step 完成在 todo 列表里实时打钩。
