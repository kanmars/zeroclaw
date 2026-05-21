# ZeroClaw — kanmars 二创版本说明 (README.kanmars.md)

> **本文档定位**：本仓库是 [zeroclaw-labs/zeroclaw](https://github.com/zeroclaw-labs/zeroclaw) 的 kanmars 二次创作版本，运行在 Linux 服务器上，作为 [`gloria`](https://gitee.com/kanmars/gloria) 部署包的引擎核心。
>
> 本文档专门记录：①二创差异点、②Linux 编译/Windows 交叉编译命令、③Web Session API 的精确用法、④规划中的功能与可行性方案。
>
> **upstream 通用说明请看 [README.md](README.md)。本文不重复。**

---

## 目录

- [一、二创功能差异速查](#一二创功能差异速查)
- [二、编译命令（Linux 本机 / Windows zig 交叉）](#二编译命令linux-本机--windows-zig-交叉)
- [三、Web Session API 详解（核心二创点）](#三web-session-api-详解核心二创点)
- [四、Web Tool API 详解](#四web-tool-api-详解)
- [五、记忆实时读取 AGENTS.md](#五记忆实时读取-agentsmd)
- [六、规划中的新功能（可行性 + 方案）](#六规划中的新功能可行性--方案)

---

## 一、二创功能差异速查

| # | 功能 | 状态 | 来源 | 详细说明 |
|---|---|---|---|---|
| 1 | Web 端接口支持 session（7 个 REST + WS） | ✅ **已实现** | 🔵 **upstream 已追平**（曾二创，已弃） | [§3](#三web-session-api-详解) |
| 2 | Web 端接口支持 tool（GET /api/tools + WS/SSE 流式 tool_call 事件） | ✅ **已实现**（仅"列出 + 观察"） | 🔵 **upstream 已追平**（曾二创，已弃） | [§4](#四web-tool-api-详解) |
| 2.1 | Web 端 REST 直接触发 tool（`POST /api/tools/{name}/invoke`） | ❌ **未实现** | 二创需求 | [§4.3 建议方案](#43-建议方案rest-直接触发-tool) |
| 3 | 记忆实时读取 AGENTS.md/SOUL.md/MEMORY.md（channel 路径每条消息重读） | ✅ **已实现**（channel 路径，2026-05-07 by req kanmars.req.20260506.001） | 🟢 **kanmars 二创**（合并到 upstream master 47ad7766） | [§5](#五记忆实时读取-agentsmd) |
| 4 | 后台反思机制（对话 N 句后总结/生成 skill） | 🔵 **规划中** | 新需求 | [§6.1](#61-后台反思机制对话-n-句后自动总结--生成skill) |
| 5 | Skills 自动加载（"下条消息生效"语义） | ✅ **免费已有** | 🔵 **upstream 完整覆盖**（kanmars 在此从无补丁） | [§6.2](#62-skills-自动加载) |
| 5.1 | Skills 自动加载（"loop 中途感知新 skill"语义，需 file watcher） | ❌ **未实现** | upstream 也无 | [§6.2 重档方案](#623-实现路径分两档) |
| 6 | Subagent | ✅ **upstream 已完整实现**（叫 `delegate`） | upstream | [§6.3](#63-subagent) |

图例：✅ 已实现 / ⚠️ 部分实现 / ❌ 未实现 / 🔵 规划中 / 🟡 半成品 / 🔵 upstream 已追平 / 🟢 kanmars 二创已合入

> **关于"upstream 已追平"**：kanmars fork 早期（2 月份左右）基于的 upstream 还没有这些能力，当时手动加了补丁。3-4 月之后 upstream 通过以下 commit 把同样能力官方化了，且实现更完整。kanmars 二创补丁因此被上游追平、不再需要维护：
>
> - **Session 持久化**：`bd0a12ad` (@theonlyhennygod, 2026-02-28) + PR #5705 `99084185` (@dangilles, 2026-04-25 — abort + 增量保存) — 详见 [§3.10](#310-演进史kanmars-二创-→-upstream-追平)
> - **Web tool 暴露**：`80f5c184` (@shane.gg, 2026-04-10 — 抽 zeroclaw-gateway crate 时一并搬入 `/api/tools` + WS/SSE 流式 `tool_call` 事件) — 详见 [§4.4](#44-演进史kanmars-二创-→-upstream-追平)
> - **Personality / 记忆文件加载**：`9c4ecfd1` (@theonlyhennygod, 2026-03-24 — `personality` 模块) + `40980e4c` (@shane.gg, 2026-04-10 — 重构搬位置) — 详见 [§5.3](#53-演进史kanmars-二创-→-upstream-追平)
> - **Skills 加载/创建**：`02688eb1` (@theonlyhennygod, 2026-03-18 — `SkillCreator` autonomous skill creation) + 7+ 个后续 fix/feat 持续打磨 — 详见 [§6.2.4](#624-演进史kanmars-从未在此处补丁upstream-持续完善)
>
> **关键事实**：grep 整个 git 史，kanmars 在 `crates/zeroclaw-gateway/` / `crates/zeroclaw-runtime/src/agent/` / `crates/zeroclaw-runtime/src/skills/` 下的提交数 **= 0**。所有这些功能 100% 来自 upstream，无任何 kanmars 私有补丁需要维护。kanmars 全部 9 个 commit 都是 squash 后的 init/merge，原始二创代码细节已在 squash 时丢失，只能通过和 upstream 对比当前文件反推。

---

## 二、编译命令（Linux 本机 / Windows zig 交叉）

### 2.1 飞书 channel 的 feature flag

| flag | 类型 | 何时用 |
|---|---|---|
| **`channel-lark`** | 真实 feature（root [`Cargo.toml:286`](Cargo.toml#L286) → channels [`Cargo.toml:91`](crates/zeroclaw-channels/Cargo.toml#L91)） | 推荐 |
| `channel-feishu` | alias（root [`Cargo.toml:312`](Cargo.toml#L312)：`channel-feishu = ["channel-lark"]`）| 等价别名 |

实现文件：[`crates/zeroclaw-channels/src/lark.rs`](crates/zeroclaw-channels/src/lark.rs)（`FEISHU_BASE_URL = "https://open.feishu.cn/open-apis"`，`LarkPlatform` 枚举同时支持 Lark/Feishu）。

> ⚠️ **重要**：默认 features 已经包含 `channel-lark`。
> Cargo.toml: `default = ["agent-runtime", ...]`，而 `agent-runtime` 列了 `"channel-lark"`（[`Cargo.toml:262`](Cargo.toml#L262)）。
> **`cargo build --release` 已经把飞书编进去**，只有 `--no-default-features` 模式下才需要显式加 `--features channel-lark`。

### 2.2 Web dashboard 编译说明

`include = ["/web/dist/**/*"]`（[`Cargo.toml:55`](Cargo.toml#L55)）只影响 `cargo publish` 打包，**不影响 `cargo build`**。

| 场景 | 是否需要先 build web |
|---|---|
| API-only / 只用 REST + WS | ❌ 不需要，直接 `cargo build` |
| 嵌入 dashboard 到二进制（`embedded-web` feature） | ✅ 必须先 build web，否则 [`crates/zeroclaw-gateway/build.rs:73`](crates/zeroclaw-gateway/build.rs#L73) 会因找不到 `web/dist/index.html` 报错 |

### 2.3 Linux 本机编译

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw

# === 方式 A：最简（默认含飞书，纯 API + WS，无 dashboard 嵌入） ===
cargo build --release --locked
# 产物：target/release/zeroclaw

# === 方式 B：完整（含 dashboard 嵌入二进制） ===
# Step 1: 先 build web（推荐用 xtask，自动 npm install + 生成 OpenAPI client + vite build）
cargo run -p xtask --bin web -- build
# 等价于手动：cd web && npm ci && npm run build && cd ..

# Step 2: cargo build 带 embedded-web feature
cargo build --release --locked --features embedded-web
# channel-lark 已在 default 内，无需重复指定

# === 方式 C：精简自定义（只要 feishu，砍掉其他 channel） ===
cargo build --release --locked --no-default-features \
  --features "agent-runtime,channel-lark,gateway,tui-onboarding,observability-prometheus,schema-export"

# === 方式 D：用官方 installer（cargo install 而非 build） ===
./install.sh --source --features agent-runtime,channel-lark
```

### 2.4 Windows 用 zig 交叉编译

仓库内**没有**现成 zig / cargo-zigbuild 配置（已 grep 确认：无 `cargo-zigbuild` recipe / 无 `.cargo/config.toml` windows target / Justfile 无 cross recipe）。下面是标准方案：

```bash
# === 一次性安装 ===
pip install ziglang
cargo install --locked cargo-zigbuild
rustup target add x86_64-pc-windows-gnu     # 推荐 GNU（zig 配 GNU 最稳）
# 备选 MSVC：rustup target add x86_64-pc-windows-msvc  # 需要 xwin 拉 SDK，更复杂

cd /home/admin/workspace-public/kanmars/zeroclaw

# === API-only Windows 二进制（默认已含 channel-lark） ===
cargo zigbuild --release --locked --target x86_64-pc-windows-gnu
# 产物：target/x86_64-pc-windows-gnu/release/zeroclaw.exe

# === 含 dashboard 嵌入的 Windows 二进制 ===
cargo run -p xtask --bin web -- build
cargo zigbuild --release --locked --target x86_64-pc-windows-gnu --features embedded-web

# === ARM64 Windows（如有需要） ===
rustup target add aarch64-pc-windows-gnullvm
cargo zigbuild --release --locked --target aarch64-pc-windows-gnullvm
```

#### Windows 交叉编译注意点

- ✅ `rusqlite { features = ["bundled"] }`（channels [`Cargo.toml:39`](crates/zeroclaw-channels/Cargo.toml#L39)）自带 sqlite 源码，无需 host sqlite
- ✅ `rustls`（非 OpenSSL）→ 无需 host OpenSSL，zig 直接搞定
- ⚠️ `aardvark-sys` 仅在 hardware feature 下编译，默认不开，对 Windows 交叉编译无影响
- ⚠️ 二进制为 release profile（`opt-level="z" + lto="fat" + strip=true`，[`Cargo.toml:350-381`](Cargo.toml#L350)），编译时间较长（10-30 分钟视机器）

### 2.5 建议给 Justfile 加的 recipe（可选）

> 当前 [Justfile](Justfile) 无 web/cross recipe。如想统一入口，可加（**注意 [AGENTS.md](AGENTS.md) 工作流要求新分支 + PR**）：

```just
build-web:
    cargo run -p xtask --bin web -- build

build-windows:
    cargo zigbuild --release --locked --target x86_64-pc-windows-gnu

build-windows-embedded: build-web
    cargo zigbuild --release --locked --target x86_64-pc-windows-gnu --features embedded-web
```

---

## 三、Web Session API 详解

> **状态**：✅ 已完整实现 ｜ **来源**：🔵 upstream 已追平（kanmars 早期二创补丁已弃）
> **代码位置**：[`crates/zeroclaw-gateway/src/lib.rs:1090-1199`](crates/zeroclaw-gateway/src/lib.rs#L1090) + [`crates/zeroclaw-gateway/src/api.rs:858-1149`](crates/zeroclaw-gateway/src/api.rs#L858) + [`crates/zeroclaw-gateway/src/ws.rs`](crates/zeroclaw-gateway/src/ws.rs) + [`crates/zeroclaw-gateway/src/sse.rs`](crates/zeroclaw-gateway/src/sse.rs)
> **演进史**：见 [§3.10](#310-演进史kanmars-二创-→-upstream-追平)

### 3.1 默认网络绑定

来源：[`crates/zeroclaw-config/src/schema.rs:2230-2313`](crates/zeroclaw-config/src/schema.rs#L2230)

| 配置项 | 默认值 | 说明 |
|---|---|---|
| `gateway.port` | `42617` | 监听端口 |
| `gateway.host` | `127.0.0.1` | 绑公网需 `gateway.allow_public_bind = true` |
| `gateway.require_pairing` | `true` | 关闭则跳过 Bearer 鉴权（仅 dev） |
| `gateway.session_persistence` | `true` | 关闭则 list/get/delete/rename 全部返空/404 |
| `gateway.path_prefix` | `None` | 反向代理嵌套用，如 `"/zeroclaw"` 后所有路径变 `/zeroclaw/api/...` |

下文示例统一用 `127.0.0.1:42617` + `$TOKEN` 占位 Bearer。

### 3.2 鉴权：如何拿到 `$TOKEN`

```bash
# 1. 用已有的 admin token 生成一次性配对码
curl -s -X POST http://127.0.0.1:42617/api/pairing/initiate \
  -H "Authorization: Bearer $ADMIN_TOKEN"
# → {"pairing_code":"123456","message":"New pairing code generated"}

# 2. 新设备用配对码换自己的 bearer token
curl -s -X POST http://127.0.0.1:42617/api/pair \
  -H "Content-Type: application/json" \
  -d '{"code":"123456","device_name":"my-laptop","device_type":"cli"}'
# → {"token":"<TOKEN>","message":"Pairing successful"}

export TOKEN=<TOKEN>
```

首次冷启动用 `zeroclaw pair` CLI 命令解决 chicken-and-egg。

### 3.3 7 个 REST 端点（Quick Reference）

| Method | Path | Body | 用途 |
|---|---|---|---|
| GET | `/api/sessions` | – | 列出所有 session |
| GET | `/api/sessions/running` | – | 列出当前运行中 session |
| GET | `/api/sessions/{id}/messages` | – | 拉取会话历史 |
| DELETE | `/api/sessions/{id}` | – | 删除 session（**同时取消进行中的 turn**） |
| PUT | `/api/sessions/{id}` | `{"name":"..."}` | 重命名 |
| GET | `/api/sessions/{id}/state` | – | 查询状态（`idle`/`running`） |
| POST | `/api/sessions/{id}/abort` | – | 取消进行中的 turn（幂等） |

> ⚠️ **没有 `POST /api/sessions` 创建端点**。Session 创建是**隐式**的——见 §3.5 生命周期。
> ⚠️ **`{id}` 是用户面 UUID**，gateway 内部自动加 `gw_` 前缀（[`api.rs:915`](crates/zeroclaw-gateway/src/api.rs#L915)）。

### 3.4 7 个 REST 端点详解

#### 3.4.1 列出 session

```bash
curl -s http://127.0.0.1:42617/api/sessions \
  -H "Authorization: Bearer $TOKEN"
```

**响应 200**（启用持久化时）：
```json
{
  "sessions": [
    {
      "session_id": "abc-123",
      "created_at": "2026-05-06T10:00:00+00:00",
      "last_activity": "2026-05-06T10:05:00+00:00",
      "message_count": 42,
      "name": "Sprint planning"
    }
  ]
}
```
**响应 200**（持久化关闭时）：`{"sessions": [], "message": "Session persistence is disabled"}`

#### 3.4.2 列出运行中 session

```bash
curl -s http://127.0.0.1:42617/api/sessions/running \
  -H "Authorization: Bearer $TOKEN"
```

#### 3.4.3 拉取会话历史

```bash
curl -s http://127.0.0.1:42617/api/sessions/abc-123/messages \
  -H "Authorization: Bearer $TOKEN"
```

**响应 200**：
```json
{
  "session_id": "abc-123",
  "messages": [
    { "role": "user", "content": "Hello" },
    { "role": "assistant", "content": "Hi!" }
  ],
  "session_persistence": true
}
```

#### 3.4.4 删除 session（兼"kill switch"）

```bash
curl -s -X DELETE http://127.0.0.1:42617/api/sessions/abc-123 \
  -H "Authorization: Bearer $TOKEN"
```

**副作用**：[`api.rs:956-964`](crates/zeroclaw-gateway/src/api.rs#L956) 会先 cancel 该 session 任何进行中的 turn，再删除持久化数据。**这是"对该 session 立即停止一切"的安全调用**。

**响应**：
- 200 `{"deleted": true, "session_id": "abc-123"}`
- 404 `{"error": "Session not found"}` 或 `{"error": "Session persistence is disabled"}`
- 500 `{"error": "Failed to delete session: <reason>"}`

#### 3.4.5 重命名

```bash
curl -s -X PUT http://127.0.0.1:42617/api/sessions/abc-123 \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"Sprint planning"}'
```

**响应**：
- 200 `{"session_id": "abc-123", "name": "Sprint planning"}`
- 400 `{"error": "name is required"}`（空字符串或缺字段）
- 404 同上

#### 3.4.6 查询状态

```bash
curl -s http://127.0.0.1:42617/api/sessions/abc-123/state \
  -H "Authorization: Bearer $TOKEN"
```

**响应 200**：
```json
{
  "session_id": "abc-123",
  "state": "running",
  "turn_id": "turn-xyz",
  "turn_started_at": "2026-05-06T10:05:30+00:00"
}
```
（`state: "idle"` 时无 `turn_id`/`turn_started_at` 字段）

#### 3.4.7 取消进行中的 turn

```bash
curl -s -X POST http://127.0.0.1:42617/api/sessions/abc-123/abort \
  -H "Authorization: Bearer $TOKEN"
```

**幂等**，两种情况都返 200：
- 在跑：`{"status": "aborted"}`
- 没在跑：`{"status": "no_active_response"}`

### 3.5 WebSocket 实时通道 `/ws/chat`

> 来源：[`crates/zeroclaw-gateway/src/ws.rs:1-260`](crates/zeroclaw-gateway/src/ws.rs#L1)，路由注册在 [`lib.rs:1194`](crates/zeroclaw-gateway/src/lib.rs#L1194)

#### 3.5.1 URL 格式

```
ws://127.0.0.1:42617/ws/chat?session_id=<id>&name=<label>&cwd=<path>&token=<bearer>
```

**Query 参数**（[`ws.rs:64-75`](crates/zeroclaw-gateway/src/ws.rs#L64) `WsQuery`）：

| 参数 | 必填 | 用途 |
|---|---|---|
| `session_id` | 选 | 恢复已有 session。**省略则服务端自动生成 UUID 创建新 session** |
| `name` | 选 | 人类可读名 |
| `cwd`（别名 `workspaceDir` / `workspace_dir`） | 选 | 安全沙箱的项目根 |
| `token` | 选 | Bearer token（浏览器无法设 header 时的后备） |

#### 3.5.2 三档鉴权（任一即可，[`ws.rs:86-120`](crates/zeroclaw-gateway/src/ws.rs#L86)）

1. `Authorization: Bearer <token>` header（首选）
2. `Sec-WebSocket-Protocol: bearer.<token>` 子协议
3. `?token=<token>` query 参数

**子协议**：客户端可请求 `zeroclaw.v1`，服务端原样回声。

#### 3.5.3 消息协议

**Server → Client 首帧**（`session_start`，[`ws.rs:204-216`](crates/zeroclaw-gateway/src/ws.rs#L204)）：
```json
{
  "type": "session_start",
  "session_id": "f3c1...",
  "resumed": true,
  "message_count": 4,
  "name": "Sprint planning"
}
```

**Client → Server 可选握手**（[`ws.rs:218-260`](crates/zeroclaw-gateway/src/ws.rs#L218)）：
```json
{
  "type": "connect",
  "session_id": "abc-123",
  "device_name": "my-laptop",
  "capabilities": [],
  "cwd": "/home/me/project"
}
```
服务端回 `{"type":"connected","message":"Connection established"}`。**此握手可省略，第一帧直接发 message 也可以**。

**Client → Server 发对话**（[`ws.rs:319, 411`](crates/zeroclaw-gateway/src/ws.rs#L319)）：
```json
{ "type": "message", "content": "Summarize today's standup" }
```

**Server → Client 流式响应**（[`ws.rs:5-13`](crates/zeroclaw-gateway/src/ws.rs#L5)）：
```json
{"type":"chunk",        "content":"partial text..."}
{"type":"thinking",     "content":"reasoning trace..."}
{"type":"tool_call",    "id":"<call_id>", "name":"shell", "args":{...}}
{"type":"tool_result",  "id":"<call_id>", "name":"shell", "output":"..."}
{"type":"done",         "full_response":"..."}
```

**取消进行中**：WS 协议**无**带内 abort 帧。从另一个 HTTP 客户端调 `POST /api/sessions/{id}/abort`（§3.4.7）或 `DELETE /api/sessions/{id}`（§3.4.4）。

#### 3.5.4 wscat 示例

```bash
# 安装：npm i -g wscat

# 恢复已有 session（浏览器风格 query token）
wscat -c "ws://127.0.0.1:42617/ws/chat?session_id=abc-123&token=$TOKEN"

# header 鉴权（wscat ≥ 5）
wscat -H "Authorization: Bearer $TOKEN" \
      -c "ws://127.0.0.1:42617/ws/chat?session_id=abc-123&name=Sprint+planning"

# 连上后粘贴这一行发对话：
{"type":"message","content":"Summarize today's standup"}
```

或用 websocat：
```bash
websocat "ws://127.0.0.1:42617/ws/chat?session_id=abc-123&token=$TOKEN"
```

### 3.6 SSE 全局事件流 `/api/events`

> 来源：[`sse.rs:51-90`](crates/zeroclaw-gateway/src/sse.rs#L51)，路由 [`lib.rs:1189`](crates/zeroclaw-gateway/src/lib.rs#L1189)

```bash
curl -N http://127.0.0.1:42617/api/events \
  -H "Authorization: Bearer $TOKEN" \
  -H "Accept: text/event-stream"
```

**特点**：
- **全局**事件流（不是 per-session），客户端按 `type` / `session_id` 自己过滤
- 鉴权**仅支持 header**（无 `?token=` 后备）
- 事件 schema（[`sse.rs:131-189`](crates/zeroclaw-gateway/src/sse.rs#L131)）：

```json
{"type":"llm_request",     "provider":"anthropic","model":"...","timestamp":"..."}
{"type":"agent_start",     "provider":"...","model":"...","timestamp":"..."}
{"type":"agent_end",       "provider":"...","model":"...","duration_ms":1234,"tokens_used":...,"cost_usd":...,"timestamp":"..."}
{"type":"tool_call_start", "tool":"shell","timestamp":"..."}
{"type":"tool_call",       "tool":"shell","duration_ms":120,"success":true,"timestamp":"..."}
{"type":"error",           "component":"...","message":"...","timestamp":"..."}
```

### 3.7 SSE 历史回放 `/api/events/history`

```bash
curl -s http://127.0.0.1:42617/api/events/history \
  -H "Authorization: Bearer $TOKEN"
```

**响应 200**：`{"events": [<event-object>, ...]}`，环形缓冲快照，最旧在前（[`sse.rs:43-47`](crates/zeroclaw-gateway/src/sse.rs#L43)）。

### 3.8 完整生命周期示例

```
CREATE → CHAT → RESUME → RENAME → ABORT → DELETE
```

```bash
# 1. CREATE：不传 session_id，服务端自动生成
wscat -H "Authorization: Bearer $TOKEN" -c "ws://127.0.0.1:42617/ws/chat"
# > {"type":"session_start","session_id":"f3c1-...","resumed":false,"message_count":0}
# 复制 session_id，记为 SID

# 2. CHAT：发对话
{"type":"message","content":"What's the weather?"}
# → 流式收到 chunk/tool_call/done

# 3. RESUME：从其他客户端重连同一 SID
wscat -H "Authorization: Bearer $TOKEN" \
      -c "ws://127.0.0.1:42617/ws/chat?session_id=$SID"
# > {"type":"session_start","session_id":"f3c1-...","resumed":true,"message_count":4}

# 4. RENAME
curl -s -X PUT http://127.0.0.1:42617/api/sessions/$SID \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"name":"Weather chat"}'

# 5. ABORT 进行中的 turn
curl -s -X POST http://127.0.0.1:42617/api/sessions/$SID/abort \
  -H "Authorization: Bearer $TOKEN"

# 6. DELETE
curl -s -X DELETE http://127.0.0.1:42617/api/sessions/$SID \
  -H "Authorization: Bearer $TOKEN"
```

### 3.9 自动清理

`gateway.session_ttl_hours`（默认 `0` = 关闭，[`schema.rs:2284-2286`](crates/zeroclaw-config/src/schema.rs#L2284)）开启后自动归档过期 session。

### 3.10 演进史：kanmars 二创 → upstream 追平

| 阶段 | 时间 | 事件 |
|---|---|---|
| **T0** | 2026-02 | kanmars fork 上游某 commit。当时 upstream gateway **没有** session 持久化能力，连接 `/ws/chat` 会丢历史 |
| **T1** | 2026-02 | kanmars 在自己 fork 上手写了 session 补丁（在 ws handler 注入 session_id query 参数 + 简单的 in-memory 历史记录） |
| **T2** | 2026-02-28 | upstream 提交 `bd0a12ad fix(gateway): persist ws chat history by session`（作者 @theonlyhennygod，zeroclaw 创建者）—— **官方版的 session 持久化诞生** |
| **T3** | 2026-04-25 | upstream PR #5705 `feat(gateway): session abort endpoint + incremental streaming persistence`（作者 @dangilles）—— **完整套件**：`/api/sessions/{id}/abort` + 每 500ms 增量保存 + `update_last` trait + `cancel_tokens` map |
| **T4** | 2026-05 | kanmars rebase 到新 upstream master。原二创补丁因为 upstream 已经覆盖同一功能且实现更完整，**自然被丢弃**（rebase 冲突时优先选 upstream 版本） |
| **T5（现在）** | 2026-05-06 | 现状：master 上 session 全套来自 upstream，kanmars fork 在 session 这块 **0 自有补丁** |

**结论**：这是 fork 与 upstream 协作的**理想结局**——临时补丁被官方功能取代，kanmars 不再背维护负担。**好事，不用做任何修复**。

**git 验证命令**（重做随时可查）：

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
# 找 ws.rs 历史
git log --all --oneline -- crates/zeroclaw-gateway/src/ws.rs | head -5
# 找 session_id 字段引入
git log --all --oneline -S "session_id" -- crates/zeroclaw-gateway/ | head -5
# 看核心两个 commit 详情
git show --stat bd0a12ad
git show --stat 99084185
# 确认 kanmars 没在 ws.rs 上做 commit
git log --all --author=kanmars -- crates/zeroclaw-gateway/src/ws.rs
# (空输出 = 确认无 kanmars 自有补丁)
```

---

## 四、Web Tool API 详解

### 4.1 GET /api/tools — 列出所有可用 tool

> 状态：✅ 已实现
> 来源：[`api.rs:134-156`](crates/zeroclaw-gateway/src/api.rs#L134)，路由 [`lib.rs:1090`](crates/zeroclaw-gateway/src/lib.rs#L1090)

```bash
curl -s http://127.0.0.1:42617/api/tools \
  -H "Authorization: Bearer $TOKEN" | jq
```

**响应 200**：
```json
{
  "tools": [
    {
      "name": "shell",
      "description": "Run a shell command",
      "parameters": { /* JSON Schema 透传 */ }
    },
    {
      "name": "node:host-01:gpio",
      "description": "GPIO control on host-01",
      "parameters": { /* ... */ }
    }
  ]
}
```

**字段说明**（[`web/src/types/api.ts:29`](web/src/types/api.ts#L29)）：
- `name`：tool 唯一名（前缀 `node:<node_id>:<cap>` 表示远程节点 tool）
- `description`：人类可读描述
- `parameters`：JSON Schema 对象（透传 `ToolSpec.parameters`）

**包含的 tool 来源**（[`lib.rs:632-633`](crates/zeroclaw-gateway/src/lib.rs#L632)）：
- 内置 tool（shell / file / browser / http / hardware / RAG-PDF 等）
- 经 MCP 导入的 tool（**注意**：MCP tool 必须在 gateway 启动前注册，否则不在 snapshot 中）
- `node_tool.rs` 远程节点 tool

### 4.2 WS / SSE 的 `tool_call` 事件

⚠️ **关键提示：WS 和 SSE 的 `tool_call` 是不同 schema，字段名不一样**。

**Chat WebSocket**（[`ws.rs:573-578`](crates/zeroclaw-gateway/src/ws.rs#L573)）—— LLM 决策触发，agent 流式输出时发：
```json
{ "type": "tool_call",   "id": "<call_id>", "name": "<tool_name>", "args": { /* JSON */ } }
{ "type": "tool_result", "id": "<call_id>", "name": "<tool_name>", "output": <JSON> }
```
字段用 **`name`**。

**Observability SSE `/api/events`**（[`sse.rs:140-156`](crates/zeroclaw-gateway/src/sse.rs#L140)）—— runtime observer 发的执行后摘要：
```json
{ "type": "tool_call_start", "tool": "<name>", "timestamp": "<RFC3339>" }
{ "type": "tool_call",       "tool": "<name>", "duration_ms": <int>, "success": <bool>, "timestamp": "<RFC3339>" }
```
字段用 **`tool`**（不是 `name`）。

⚠️ 这是 upstream 的字段命名不一致（已知问题），写消费方代码时注意区分。

### 4.3 建议方案：REST 直接触发 tool

> 状态：❌ 未实现，**建议方案如下**

**当前事实**：grep 整个 `crates/zeroclaw-gateway/src` 确认**无任何 `POST /api/tools/...` / `/invoke` / `/run` / `/execute` 端点**。tool 只能通过"发对话给 agent → LLM 决定调用"间接触发。

**为什么 upstream 不做**：哲学上 ZeroClaw 的所有 tool 调用都要经过 agent loop 的安全策略（autonomy = supervised → 中风险 op 需 approval、高风险 blocked、tool receipts 加密签名），直接 invoke 绕过 agent 等于绕过 security policy。

**如果要做（kanmars 二创建议）**：

```rust
// crates/zeroclaw-gateway/src/api.rs 新增 handler
pub async fn handle_api_tool_invoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(args): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    require_auth(&state, &headers)?;

    // 1. 找到 tool
    let tool = state.tools_registry.iter()
        .find(|t| t.name == name)
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"error":"tool not found"}))))?;

    // 2. 必须走 SecurityPolicy 检查（不要绕过！）
    state.security.check_tool_call(&name, &args)
        .map_err(|e| (StatusCode::FORBIDDEN, Json(json!({"error":e.to_string()}))))?;

    // 3. 调 Tool::execute
    let result = tool.execute(args).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error":e.to_string()}))))?;

    Ok(Json(result))
}

// crates/zeroclaw-gateway/src/lib.rs 注册路由（约 1090 行附近）
.route("/api/tools/{name}/invoke", post(handle_api_tool_invoke))
```

**关键约束**：
1. **必须复用 `SecurityPolicy::check_tool_call`**——绕过则 autonomy/sandbox 全废
2. 建议 default off，加 `gateway.allow_direct_tool_invoke: bool` 配置项，避免误开
3. Audit log 必须落地（否则 tool receipts 缺失这一类操作，违反 upstream 的 traceability 设计）
4. 给 high-risk tool（`shell` / 写 fs / 网络）加二次确认机制

**预估工作量**：3-5 天（含写测试 + 改 schema + 加 doc）。**风险等级 High**（按 [AGENTS.md](AGENTS.md#risk-tiers) 分类，触碰 gateway + tools 边界）。

### 4.4 演进史：kanmars 二创 → upstream 追平

| 阶段 | 时间 | 事件 |
|---|---|---|
| **T0** | 2026-02 | kanmars fork 上游某 commit。当时 upstream gateway **没有** `/api/tools` 列表端点，也没有 WS/SSE 流式 `tool_call` 事件 broadcast，web 端看不到 agent 在调什么 tool |
| **T1** | 2026-02 | kanmars 在自己 fork 上手写了 web tool 暴露补丁（`/api/tools` GET + WS `tool_call` 事件） |
| **T2** | 2026-04-10 | upstream 提交 `80f5c184 feat(workspace): extract zeroclaw-gateway crate`（作者 @shane.gg）—— **抽 zeroclaw-gateway 独立 crate 时一并把 `/api/tools` + `handle_api_tools` handler + WS/SSE `tool_call` 事件全套搬入 gateway**，同一 commit 落地 |
| **T3** | 2026-05 | kanmars rebase 到新 upstream master。原二创补丁因为 upstream 已经覆盖同一功能，**自然被丢弃**（rebase 冲突时优先选 upstream 版本） |
| **T4（现在）** | 2026-05-06 | 现状：master 上 `/api/tools` + WS/SSE `tool_call` 事件全套来自 upstream，kanmars fork 在 web tool 暴露这块 **0 自有补丁** |

**结论**：与 §3.10 的 session 故事完全同构——临时补丁被官方功能取代。**好事，不用做任何修复**。

**特别注意**：upstream 的实现仅覆盖"列出 + 流式观察"，**没有**做"REST 直接 invoke tool"。如果你 2 月份的二创补丁里**只**做了"列出 + 观察"，那 upstream 已 100% 追平；如果当年还做了"REST 直接触发"，那部分能力**目前不在 upstream**——需要按 [§4.3 建议方案](#43-建议方案rest-直接触发-tool) 重做。请回忆一下当年的二创范围。

**git 验证命令**：

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
# /api/tools 路由首次出现
git log --all --oneline -S '"/api/tools"' -- crates/zeroclaw-gateway/ | head -5
# handle_api_tools handler 首次出现
git log --all --oneline -S "handle_api_tools" -- crates/zeroclaw-gateway/ | head -5
# WS / SSE tool_call 事件首次出现
git log --all --oneline -S '"tool_call"' -- crates/zeroclaw-gateway/src/ws.rs | head -5
git log --all --oneline -S '"tool_call"' -- crates/zeroclaw-gateway/src/sse.rs | head -5
# 看 80f5c184 详情
git show --stat 80f5c184
# 确认 kanmars 没在 gateway 任何文件做 commit
git log --all --author=kanmars -- crates/zeroclaw-gateway/
# (空输出 = 确认无 kanmars 自有补丁)
```

### 4.5 节外补充：无 `/api/agents` 端点

无 `/api/agents` 列表端点，无 `/api/delegates`。delegate runtime 在 `lib.rs` 内部 `&config.agents` 读取，要看 agent 列表只能 `GET /api/config` 然后客户端解析。如有需要可加（同 §4.3 思路）。

---

## 五、记忆实时读取 AGENTS.md

> 状态：✅ **已实现（channel 路径）** —— 飞书 / Slack / 钉钉 / Telegram 等所有 channel 路径下，AGENTS.md / SOUL.md / TOOLS.md / IDENTITY.md / USER.md / MEMORY.md / BOOTSTRAP.md / skills 修改在**下条消息**生效，无需重启
>
> 来源：🟢 **kanmars 二创**（req [`kanmars.req.20260506.001`](kanmars.req.20260506.001.md)，2026-05-07 合入 master `47ad7766`）

### 5.1 当前事实（2026-05-07 起）

| 维度 | 现状 | 文件位置 |
|---|---|---|
| ✅ 文件读取**无缓存** | 每次直接 `std::fs::read_to_string` | [`system_prompt.rs:347`](crates/zeroclaw-runtime/src/agent/system_prompt.rs#L347) `inject_workspace_file()` |
| ✅ personality 也无缓存 | 同上每次新读 | [`agent/personality.rs`](crates/zeroclaw-runtime/src/agent/personality.rs) `load_personality_files()` |
| ✅ 注入 7 个文件 | `AGENTS.md / SOUL.md / TOOLS.md / IDENTITY.md / USER.md` + 条件 `BOOTSTRAP.md` + `MEMORY.md` | [`system_prompt.rs:22, 29, 35`](crates/zeroclaw-runtime/src/agent/system_prompt.rs#L22) |
| ✅ **channel 路径每条消息重读** | `rebuild_system_prompt_from_disk(&ctx)` 在每次 `process_channel_message` 调用时执行；产物含最新 7 个 bootstrap 文件 + 最新 skills + AIEOS（如配置） | [`orchestrator/mod.rs:1256`](crates/zeroclaw-channels/src/orchestrator/mod.rs#L1256) `rebuild_system_prompt_from_disk()` + [`:639`](crates/zeroclaw-channels/src/orchestrator/mod.rs#L639) `build_channel_tool_descs()` + [`:719`](crates/zeroclaw-channels/src/orchestrator/mod.rs#L719) `build_channel_runtime_system_prompt()` |
| ✅ webhook 路径同样行为 | `process_message` 路径一直是每次新建 prompt（gateway webhook 路径） | [`zeroclaw-runtime/src/agent/loop_.rs:3304`](crates/zeroclaw-runtime/src/agent/loop_.rs#L3304) |
| ⚠️ **CLI / `Agent` struct 路径仍只读 1 次** | `agent.rs` 的 `if self.history.is_empty() { build_system_prompt() }` —— 之后每轮 turn 复用 history 中的 system message。**与 channel 路径无关**，CLI 长会话用户才会踩到 | [`agent.rs:434, 1064, 1241`](crates/zeroclaw-runtime/src/agent/agent.rs#L1064) |

**结论**：
- ✅ **channel 用户**（飞书/Slack/钉钉/Telegram 等）：发消息编辑 AGENTS.md → 下条消息即生效。**关键目标已达成**。
- ✅ **webhook 用户**：与上同，且本 req 之前就已生效。
- ⚠️ **CLI 长会话用户**：单 session 内修改 AGENTS.md 不会被感知，需要 `/new` 重置或重启。**不在 req kanmars.req.20260506.001 范围**（req 明确"channel 路径"）。

### 5.2 实现方式（方案 A 全量重建）

每条 channel 消息进来时调用 `rebuild_system_prompt_from_disk(&ctx)`：

```
┌──────────────────────────────────────────────────────┐
│  process_channel_message(msg)                        │
│       ↓                                              │
│  rebuild_system_prompt_from_disk(&ctx)               │
│       ↓                                              │
│  build_channel_runtime_system_prompt(...)            │
│       ↓ ① re-read 7 bootstrap files                  │
│       ↓ ② re-scan skills/                            │
│       ↓ ③ AIEOS 分支自动重跑（同函数内）              │
│       ↓ ④ 追加 deferred MCP section（启动时算的）     │
│       ↓                                              │
│  base_system_prompt → build_channel_system_prompt    │
│       ↓ ⑤ 拼 datetime + channel 指令                 │
│       ↓                                              │
│  最终 system prompt 喂给 LLM                         │
└──────────────────────────────────────────────────────┘
```

**为什么不选 cache-friendly 方案**（mtime + content-hash 比对，opencode-memory v0.2.2 风格）：
- `## Current Date & Time` 段已每分钟变化 → cache 命中率本来就每分钟掉 1 次
- channel QPS << 1（人发消息），全量重建成本是 7 文件 `read_to_string` ≈ ms 级
- 实现复杂度（递归 stat skills 目录 + 签名结构 + 并发锁）远高于全量重建
- AC-9 在 PR 描述显式声明"接受 cache miss 上升换实时性"

### 5.3 演进史：kanmars 二创 → upstream 追平 → kanmars 二创补全

| 阶段 | 时间 | 事件 |
|---|---|---|
| **T0** | 2026-02 | kanmars fork 上游某 commit。当时 upstream 没有结构化身份/记忆文件加载 |
| **T1** | 2026-02 | kanmars 在自己 fork 上手写补丁，让 agent 启动时实时从磁盘读 AGENTS.md/SOUL.md 等（无缓存） |
| **T2** | 2026-03-24 | upstream `9c4ecfd1 feat(agent): add personality module ...`（@theonlyhennygod）—— `personality.rs` 诞生，无缓存读 |
| **T3** | 2026-04-10 | upstream `40980e4c refactor(agent): move system prompt builders from channels to agent`（@shane.gg）—— `inject_workspace_file` 搬入 `system_prompt.rs` 当前位置 |
| **T4** | 2026-05 | kanmars rebase 到新 upstream master。原"启动时读一次"二创补丁被 upstream 覆盖、自然丢弃 |
| **T5** | 2026-05-06 | kanmars 起 [`req kanmars.req.20260506.001`](kanmars.req.20260506.001.md)：发现 channel 路径"每条消息重读"是真缺口（upstream 也没做）—— 飞书改 AGENTS.md 必须重启进程才能感知 |
| **T6（现在）** | 2026-05-07 | kanmars 二创补丁合入 master `47ad7766`：抽 `build_channel_runtime_system_prompt` helper（DRY，启动 + 重建共用同一份代码）+ 加 `rebuild_system_prompt_from_disk` 包装 + 替换消息分支 + 删两个失去调用方的旧函数。**channel 路径热加载落地**。 |

**结论**：从 T0 → T4 是"临时补丁被官方功能取代"的好故事；T5 → T6 发现 upstream 也没解决的真缺口，kanmars 重新出手补齐了 channel 路径。**未来可以考虑把这个补丁回提到 upstream**——它是行为一致性修复（`process_message` webhook 路径已经是这个语义），不是 kanmars 私有需求。

⚠️ **CLI / Agent struct 路径仍是缺口**：`agent.rs:1064 / :1241` 的 `if self.history.is_empty()` 模式仍存在，CLI 长会话内修改 AGENTS.md 不会被感知。**channel 用户不受影响**（走的是 `orchestrator::process_channel_message` 路径，不经过 `Agent` struct）。如果 CLI 用户也需要"长会话内热加载"，需要另起 req（cache-friendly 方案适用此处，因为 CLI 长会话才有 cache 命中价值）。

**git 验证命令**：

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
# 看 kanmars 二创补丁
git show --stat 47ad7766
git log --all --oneline --grep "kanmars.req.20260506.001" | head -5

# 看 channel 路径的新 helper
grep -n "rebuild_system_prompt_from_disk\|build_channel_runtime_system_prompt" crates/zeroclaw-channels/src/orchestrator/mod.rs

# 看 Agent struct 路径仍是 cache-on-first-turn 模式
grep -n "self.history.is_empty" crates/zeroclaw-runtime/src/agent/agent.rs

# 看 webhook 路径每次重读（早期就是的）
grep -n "build_system_prompt_with_mode_and_autonomy" crates/zeroclaw-runtime/src/agent/loop_.rs
```

### 5.4 用户验证步骤（飞书示例）

```bash
# Terminal 1：启动 zeroclaw 服务
zeroclaw service start

# Terminal 2：编辑 AGENTS.md，加一条新规则
echo "" >> ~/.zeroclaw/workspace/AGENTS.md
echo "## 新规则：MAGIC-TOKEN-FOO" >> ~/.zeroclaw/workspace/AGENTS.md

# 飞书：发一条消息
"你的 system prompt 里有 MAGIC-TOKEN-FOO 吗？请逐字回答 yes 或 no"

# 预期：bot 回复 yes
# 失败排查：检查日志 grep "channel.system_prompt.rebuild_empty"
#          —— 如果出现说明 rebuild 失败回落到了启动 cache
```

skills 同理：在 `~/.zeroclaw/workspace/skills/` 下新建 `unicorn-skill/SKILL.md`（含 frontmatter `name`/`description`），下条飞书消息发"你有 unicorn-skill 这个 skill 吗"即可验证。

---

## 六、规划中的新功能（可行性 + 方案）

### 6.1 后台反思机制（对话 N 句后自动总结 + 生成 skill）

> 状态：🔵 **规划中** ｜ 参照 hermes Agent 风格 ｜ 可行性：**高**（基础设施 90% 已就位）

#### 6.1.1 现成可复用的拼图

| 组件 | 位置 | 作用 |
|---|---|---|
| **LLM 总结引擎** | [`crates/zeroclaw-memory/src/consolidation.rs:55`](crates/zeroclaw-memory/src/consolidation.rs#L55) `consolidate_turn()` | 已在做"两阶段固化"：phase 1 → `Daily` 类，phase 2 → `Core` 类，带冲突检测（cosine 0.85）+ importance 打分 |
| **程序化建 skill** | [`crates/zeroclaw-runtime/src/skills/creator.rs:37`](crates/zeroclaw-runtime/src/skills/creator.rs#L37) `SkillCreator::create_from_execution()` | 已能从执行轨迹自动落 SKILL.toml，带 embedding 去重 + LRU 上限。已在 [`agent/loop_.rs:2681-2685`](crates/zeroclaw-runtime/src/agent/loop_.rs#L2681) 接到了"CLI 执行后自动建 skill" |
| **Skill 改写** | [`crates/zeroclaw-runtime/src/skills/improver.rs`](crates/zeroclaw-runtime/src/skills/improver.rs) `SkillImprover::improve_skill()` | 原子改写已有 SKILL.toml（temp→validate→rename），带 cooldown + audit trail。**写入半边完备，等着被反思 pass 接** |
| **Heartbeat 引擎** | [`crates/zeroclaw-runtime/src/heartbeat/engine.rs`](crates/zeroclaw-runtime/src/heartbeat/engine.rs) | 已是"主动后台思考"骨架：定时 tick + Phase1 LLM 决策选任务 + Phase2 执行 |
| **Hook 系统** | [`crates/zeroclaw-runtime/src/hooks/{traits,runner}.rs`](crates/zeroclaw-runtime/src/hooks/traits.rs) | 已有 `on_session_start/end`、`on_llm_output`、`on_after_tool_call`、`on_message_sent`、`on_heartbeat_tick` |

#### 6.1.2 缺什么

1. ❌ 没有 `on_turn_complete` hook（注意 `loop_.rs:913` 的 `iteration` 计数器是**单条用户消息内的 tool-call 轮数**，不是对话轮数）
2. ❌ 没有"每 N 轮触发反思"的逻辑
3. ❌ 没有"反思 pass"——即从历史 N 轮 + 当前 skill 列表 → 输出"该改的 skill / 该建的 skill / 该写的 memory"的 LLM 函数
4. ❌ SOP 引擎（[`sop/types.rs:64-100`](crates/zeroclaw-runtime/src/sop/types.rs#L64)）的 `SopTrigger` 枚举只有 `Mqtt | Webhook | Cron | Peripheral | Manual`，没有 `PostTurn` 触发器

#### 6.1.3 推荐实现方案（最小爆炸半径）

```
┌──────────────────────────────────────────────────────────────┐
│ Step 1: zeroclaw-memory 新建 reflection.rs                   │
│         (仿 consolidation.rs 骨架)                            │
│   输入：最近 N 轮 history + 当前 skill list + AGENTS.md      │
│   输出：JSON {                                                │
│     skill_to_improve?: { slug, reason, new_content },        │
│     new_skill_spec?: { slug, description, content },         │
│     memory_update?: { category, content, importance }        │
│   }                                                          │
├──────────────────────────────────────────────────────────────┤
│ Step 2: zeroclaw-config 加 ReflectionConfig {                │
│   enabled: bool,                                             │
│   every_n_turns: u32,                                        │
│   cooldown_secs: u64,                                        │
│   history_window: u32,                                       │
│   model: Option<String>,                                     │
│ }                                                            │
├──────────────────────────────────────────────────────────────┤
│ Step 3: zeroclaw-channels/orchestrator/mod.rs:3463 旁边     │
│         加 turn_counter，每 N 轮 tokio::spawn reflect_turn   │
│         (与 consolidate_turn 并列 fire-and-forget)           │
├──────────────────────────────────────────────────────────────┤
│ Step 4: reflect_turn 把信号喂给现成的：                       │
│         - SkillImprover::improve_skill (已有 cooldown)       │
│         - SkillCreator::create_from_reflection (新增 sibling)│
│         - Memory::update (已有)                              │
├──────────────────────────────────────────────────────────────┤
│ Step 5（可选兜底）: daemon/mod.rs:542 那条                   │
│         "per tick recall + consolidation" 注释下面真把       │
│         consolidation/reflection 跑起来作为离线兜底           │
└──────────────────────────────────────────────────────────────┘
```

**第一版 PoC 建议**：只接 `SkillImprover`（已有写入半边），不动 `SkillCreator`，把爆炸半径压到最小。

**关键提醒**：[`crates/zeroclaw-runtime/AGENTS.md`](AGENTS.md) 明确说该 crate 是"过渡持有区，不要在这里加新功能"。新模块（reflection pass）应该放 `zeroclaw-memory`，trigger hook 放 channels orchestrator。

**预估工作量**：5-7 天（含 prompt 调优 + 测试 + 配置）。**风险等级 Medium**。

### 6.2 Skills 自动加载

> 状态：🟡 **基本免费已有**（每次调用都重扫，下条消息生效） ｜ 取决于"自动加载"语义

#### 6.2.1 当前模型：scan-on-demand（不是 startup-cached）

每个 agent loop / channel orchestrator / delegate tool 都重新跑 `load_skills_with_config(workspace_dir, &config)`：

| 调用点 | 文件 |
|---|---|
| Agent loop（2 处） | [`crates/zeroclaw-runtime/src/agent/loop_.rs:2326, 3304`](crates/zeroclaw-runtime/src/agent/loop_.rs#L2326) |
| Agent struct | [`crates/zeroclaw-runtime/src/agent/agent.rs:676`](crates/zeroclaw-runtime/src/agent/agent.rs#L676) |
| Channel orchestrator（2 处） | [`crates/zeroclaw-channels/src/orchestrator/mod.rs:1257, 5522`](crates/zeroclaw-channels/src/orchestrator/mod.rs#L1257)（前者由 req kanmars.req.20260506.001 引入的 `rebuild_system_prompt_from_disk` 调用，后者是启动路径） |
| Delegate tool | [`crates/zeroclaw-runtime/src/tools/delegate.rs:1037`](crates/zeroclaw-runtime/src/tools/delegate.rs#L1037) |
| Telegram channel | [`crates/zeroclaw-channels/src/telegram.rs:701`](crates/zeroclaw-channels/src/telegram.rs#L701) |

实现：[`crates/zeroclaw-runtime/src/skills/mod.rs:165`](crates/zeroclaw-runtime/src/skills/mod.rs#L165) `load_skills_with_config()` → 同步 `std::fs::read_dir` 扫三个源：
1. `workspace_dir/skills/`（用户 workspace）
2. `open_skills_dir`（外部 git clone `besoeasy/open-skills`，7d 同步标记）
3. `skills-registry/`（24h 标记）

每个 skill 文件夹格式优先级（[`mod.rs:264-282`](crates/zeroclaw-runtime/src/skills/mod.rs#L264)）：`SKILL.toml` → `manifest.toml` → `SKILL.md`。

#### 6.2.2 这意味着

✅ **"新 skill 不用重启就能用"** —— 当前已经免费有了。在 `workspace/skills/` 扔个新 SKILL.md，下条用户消息处理时就会被发现。
❌ **"正在跑的 agent loop 中途感知到新 skill"** —— 当前不行（loop 内单次执行用同一份扫描快照）。

#### 6.2.3 实现路径分两档

| 档次 | 方案 | 成本 |
|---|---|---|
| **轻**（推荐先做） | 写文档说明"下条消息自动加载"已有；在 `skill_http.rs` 加 `POST /api/skills/reload` 端点强制刷新；缩短 open-skills/registry 同步间隔 | 几十行代码 |
| **重** | 加 `notify` crate dep（**Cargo.toml/Cargo.lock 当前无任何 file-watcher 依赖**：no `notify`/`hotwatch`/`watchexec`/`inotify`），建 `SkillWatcher` 后台 task，引入 `Arc<RwLock<Vec<Skill>>>` 共享 cache，重构 7 个调用点 | 几百行 + 多文件改动 |

**注意**：watcher 应放 `zeroclaw-infra` crate（已是 cross-cutting infra 家），不要再往 runtime 堆。

**预估工作量**：轻档 1-2 天 / 重档 5-8 天。**风险等级**：轻档 Low / 重档 Medium。

#### 6.2.4 演进史：kanmars 从未在此处补丁，upstream 持续完善

| 阶段 | 时间 | 事件 |
|---|---|---|
| **T0** | 2026-03-18 | upstream 提交 `02688eb1 feat(skills): autonomous skill creation from multi-step tasks (#3916)`（作者 @theonlyhennygod）—— **`SkillCreator` 诞生**，第一次具备"程序化建 skill"能力 |
| **T1** | 2026-03-19 | upstream `b1d20d38 feat(skills): add read_skill for compact mode`（作者 @Alix-007）—— `load_skills` 路径首次出现 |
| **T2** | 2026-04 起 | upstream 持续打磨：`1e4a8092` universal registry support (agentskills.io / skills.sh)、`165cb335` registry-based bare-name install (#6045)、`2b5daff9` plugin skill capability for markdown-only bundles (#6141)、`2a37f389` allow_scripts policy、`fec71ab7` 修 hyphen 名解析、`2c0515b2` local paths win over registry…… 7+ commit |
| **现状** | 2026-05-06 | scan-on-demand 模型成熟，**"下条消息生效"型自动加载免费可用**。但 file watcher / "loop 中途感知"功能 **upstream 也没做** |

**与 §3.10 / §4.4 / §5.3 的不同**：这次**没有 kanmars 二创补丁的故事**——

- grep 整个 git 史：**kanmars 在 `crates/zeroclaw-runtime/src/skills/` 下提交数 = 0**
- 整个 git 史里**没有任何人**（upstream + kanmars）做过 `notify` / `SkillWatcher` 补丁
- 你 fork 时（2026-02）upstream skills 系统已具备基础形态，**或者**你当年没动过这块

**结论**：skills 自动加载（轻语义）是 upstream 一路完善的成果，kanmars 在此**从未需要二创**。如果要做"loop 中途感知"，那是真新功能，按 [§6.2.3 重档方案](#623-实现路径分两档) 自己起。

**git 验证命令**：

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw
# load_skills 历史
git log --all --oneline -S "load_skills" | head -10
# SkillCreator 历史
git log --all --oneline -S "SkillCreator" | head -5
# skills/mod.rs 历史
git log --all --oneline -- crates/zeroclaw-runtime/src/skills/mod.rs | head -10
# 关键 commit 详情
git show --stat 02688eb1
# 确认 kanmars 没在 skills/ 做 commit
git log --all --author=kanmars -- crates/zeroclaw-runtime/src/skills/
# (空输出 = 确认无 kanmars 自有补丁)
# 确认 upstream + kanmars 都没做 watcher
git log --all --oneline -S "SkillWatcher"
git log --all --oneline -S "notify::"
# (两条空输出 = 确认 watcher 是真新功能)
```

### 6.3 Subagent

> 状态：✅ **upstream 已完整实现**（叫 `delegate`） ｜ 几乎零代码改动即可用

#### 6.3.1 现成基础设施

ZeroClaw 已经把 subagent 做得相当成熟：

| 组件 | 位置 | 能力 |
|---|---|---|
| **配置** | [`crates/zeroclaw-config/src/schema.rs:651`](crates/zeroclaw-config/src/schema.rs#L651) `DelegateAgentConfig` | provider/model/system_prompt/api_key/temperature/**`max_depth`**（递归限制）/**`agentic`**（多轮 loop 模式）/`allowed_tools`/`max_iterations`/timeout/**`skills_directory`**（每 sub-agent 独立 skill 目录）/**`memory_namespace`**（记忆隔离） |
| **实现** | [`crates/zeroclaw-runtime/src/tools/delegate.rs:60`](crates/zeroclaw-runtime/src/tools/delegate.rs#L60) `DelegateTool` `impl Tool` | **就是"agent-as-tool"模式**。4 种执行模式：sync（[384](crates/zeroclaw-runtime/src/tools/delegate.rs#L384)）/ background 持久化（[554](crates/zeroclaw-runtime/src/tools/delegate.rs#L554)，写 `workspace/delegate_results/{task_id}.json`）/ 并行多 agent（[738](crates/zeroclaw-runtime/src/tools/delegate.rs#L738)）/ agentic 多轮 loop（[1095](crates/zeroclaw-runtime/src/tools/delegate.rs#L1095)，递归调 `run_tool_call_loop`）。`depth` 字段做 cascade 追踪，`with_cancellation_token` 级联取消，`NamespacedMemory` 记忆隔离 |
| **更高层编排** | [`crates/zeroclaw-tools/src/swarm.rs:19`](crates/zeroclaw-tools/src/swarm.rs#L19) `Swarm` + `SwarmStrategy` | 在 `DelegateTool` 之上做 parallel/sequential/voting 多 agent 策略 |
| **注册到工具表** | [`crates/zeroclaw-runtime/src/tools/mod.rs:889-911`](crates/zeroclaw-runtime/src/tools/mod.rs#L889) | 任何 agent 拿到 `delegate` 工具就能 spawn subagent |

#### 6.3.2 用法示例（config.toml）

```toml
# ~/.zeroclaw/config.toml 或 A/malorian-3516/config.toml

[agents.code-reviewer]
provider = "anthropic"
model = "claude-sonnet-4-6"
system_prompt = """
You are a senior code reviewer. Focus on:
1. Security issues
2. Performance bottlenecks
3. Code style
Return findings in JSON: { issues: [{severity, line, message}] }
"""
allowed_tools = ["read_file", "grep"]
max_iterations = 5
max_depth = 2
agentic = true
skills_directory = "skills/code-review"
memory_namespace = "code-review"

[agents.researcher]
provider = "openai"
model = "gpt-4o"
system_prompt = "Deep web research specialist..."
allowed_tools = ["web_search", "fetch_url"]
max_iterations = 10
agentic_timeout_secs = 300
```

父 agent 启用 `delegate` 工具后，调用方式（伪代码）：
```
parent agent → delegate({"agent": "code-reviewer", "task": "Review crates/zeroclaw-runtime/src/agent/loop_.rs"})
            → 子 agent 独立 namespace + 独立 skills 跑完
            → 返结果给父 agent
```

#### 6.3.3 缺什么（可选高级功能）

如果需要可以考虑做：

- ❌ **trait 级 `Agent` 抽象**——zeroclaw-api 里没有 `Agent` trait，agent loop 是具体实现在 runtime 里。让非 `DelegateTool` 调用方（gateway / ACP / plugins）spawn agent 需要这个抽象
- ❌ **跨进程 subagent 传输**——全是 in-process tokio task。如果要分进程/容器/WASM 隔离，需要 wire protocol（ACP 是 IDE↔agent 单向，不能直接复用）
- ❌ **subagent 流式回传父 agent**——只能等子 agent 返一个完整 ToolResult string，无 token 级流式
- ❌ **嵌套 subagent 内事件 observability**——`Observer` trait 是 per LLM call，无"depth-N subagent 内"框架

**建议**：80% 场景用现成 `DelegateTool` 即可，无需开发。**预估工作量：1-2 小时（写 config + 文档）**。

### 6.4 ROI 排序

| 优先级 | 功能 | 工作量 | 价值 |
|---|---|---|---|
| 🥇 P0 | **Subagent**（写配置） | **1-2 小时** | 高，立即可用 |
| 🥈 P1 | **后台反思**（reflection.rs + on_turn_complete hook） | **5-7 天** | 高，agent 自我进化核心 |
| 🥉 P2 | **REST 直接触发 tool**（如确实需要） | **3-5 天** | 中，需评估是否真要绕开 agent loop |
| ✅ ~~P3~~ | ~~AGENTS.md cache-friendly 实时注入（channel 路径）~~ | **已完成** 2026-05-07 | req `kanmars.req.20260506.001` 落地，channel 路径每条消息重读 |
| 🔧 P3' | **AGENTS.md 实时注入（CLI / Agent struct 路径）** | **2-3 天** | 低，CLI 长会话用户才有需求；可用 cache-friendly 方案 |
| 📦 P4 | **Skills 强制刷新端点**（轻档） | **1-2 天** | 低，下条消息已自动加载（channel + webhook 路径） |

---

## 附录 A：文件位置速查

| 主题 | 关键文件 |
|---|---|
| Session REST handler | [`crates/zeroclaw-gateway/src/api.rs:858-1149`](crates/zeroclaw-gateway/src/api.rs#L858) |
| Session 路由注册 | [`crates/zeroclaw-gateway/src/lib.rs:1090-1199`](crates/zeroclaw-gateway/src/lib.rs#L1090) |
| WebSocket 协议 | [`crates/zeroclaw-gateway/src/ws.rs`](crates/zeroclaw-gateway/src/ws.rs) |
| SSE 事件流 | [`crates/zeroclaw-gateway/src/sse.rs`](crates/zeroclaw-gateway/src/sse.rs) |
| Pairing 鉴权 | [`crates/zeroclaw-gateway/src/api_pairing.rs`](crates/zeroclaw-gateway/src/api_pairing.rs) |
| Tool API | [`crates/zeroclaw-gateway/src/api.rs:134-156`](crates/zeroclaw-gateway/src/api.rs#L134) |
| Gateway 默认配置 | [`crates/zeroclaw-config/src/schema.rs:2230-2313`](crates/zeroclaw-config/src/schema.rs#L2230) |
| 飞书 channel 实现 | [`crates/zeroclaw-channels/src/lark.rs`](crates/zeroclaw-channels/src/lark.rs) |
| 飞书 feature 定义 | [`Cargo.toml:286, 312`](Cargo.toml#L286) + [`crates/zeroclaw-channels/Cargo.toml:91`](crates/zeroclaw-channels/Cargo.toml#L91) |
| 记忆注入 | [`crates/zeroclaw-runtime/src/agent/system_prompt.rs:347-388`](crates/zeroclaw-runtime/src/agent/system_prompt.rs#L347) |
| Personality 注入 | [`crates/zeroclaw-runtime/src/agent/personality.rs:100-132`](crates/zeroclaw-runtime/src/agent/personality.rs#L100) |
| Consolidation 引擎 | [`crates/zeroclaw-memory/src/consolidation.rs`](crates/zeroclaw-memory/src/consolidation.rs) |
| Skill creator | [`crates/zeroclaw-runtime/src/skills/creator.rs`](crates/zeroclaw-runtime/src/skills/creator.rs) |
| Skill improver | [`crates/zeroclaw-runtime/src/skills/improver.rs`](crates/zeroclaw-runtime/src/skills/improver.rs) |
| Hook 系统 | [`crates/zeroclaw-runtime/src/hooks/`](crates/zeroclaw-runtime/src/hooks/) |
| Subagent 配置 | [`crates/zeroclaw-config/src/schema.rs:651`](crates/zeroclaw-config/src/schema.rs#L651) `DelegateAgentConfig` |
| Subagent 实现 | [`crates/zeroclaw-runtime/src/tools/delegate.rs`](crates/zeroclaw-runtime/src/tools/delegate.rs) |

## 附录 B：本文档维护

- **作者**：组件管理员（kanmars 沙箱实例）
- **生成时间**：2026-05-06，更新 2026-05-07（§5 状态翻转：req kanmars.req.20260506.001 落地）
- **本文档与 upstream README.md 关系**：本文是 kanmars 二创补充说明，不复制 upstream 通用内容。upstream 信息以 [README.md](README.md) 为准。
- **更新原则**：每次合并 upstream 后需重验本文档中"已实现"的状态是否仍成立（特别是 §3、§4、§5 三个二创点）。
- **更新日志**：
  - 2026-05-07：§5 状态从"⚠️ 部分实现"翻转为"✅ 已实现（channel 路径）"；§5.3 演进史增加 T6 里程碑（kanmars 二创补丁合入 master `47ad7766`）；§6.4 ROI 表 P3 标记完成；详见 [`kanmars.req.20260506.001.md`](kanmars.req.20260506.001.md) 实施记录。
