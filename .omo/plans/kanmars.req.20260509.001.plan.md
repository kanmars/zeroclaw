# Plan — kanmars.req.20260509.001 (Replace DEBUG_CHAT eprintln! with symmetric structured tracing)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260509.001.plan |
| 关联需求 | 无独立 req 文档（用户直接对话需求 + 现场日志诊断） |
| 起草日期 | 2026-05-09 |
| 修订日期 | 2026-05-09 (rev2.2：用户实测部署后 PR1 follow-up 3 个 commit 覆盖 chat_with_system success body + chat() 路径 + doc deprecation；§5 PR2 验收勾 4 项副作用真达成项（#3/#4/#5/#12），其余 8 项保留 [ ] + 注释说明属 PR2 helper 化未涉及；rev2.1：Momus 第 2 轮 ACCEPT 算术修正；rev2：4 MINOR + 2 NIT) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | PR1: `fix/debug-chat-tracing-replace`；PR2: `feat/llm-http-symmetric-tracing` |
| 目标 PR 标题 | PR1: `fix(providers): replace DEBUG_CHAT eprintln! with tracing in chat_with_system`；PR2: `feat(providers): symmetric structured tracing for all LLM HTTP calls` |
| 风险等级 | PR1 = Low（纯日志通道替换，无业务逻辑改动）；PR2 = Low-Medium（覆盖面扩大，新增脱敏 helper） |
| 选型方案 | **两步走：PR1 止血先合，PR2 完整结构化重构后合**（用户已确认） |

---

## 0. 关键目标（唯一的真理来源）

> 把 `crates/zeroclaw-providers/src/compatible.rs` 内所有 `DEBUG_CHAT*` 系列 `eprintln!` 替换为 `tracing` 调用，**消除"日志在 stdout/stderr 双流之间错乱"的现象**；并把同文件内 4 条 LLM HTTP 调用路径的日志统一成**对称的 request/response 事件对**（同字段、同顺序、同 `req_id` 缝合）。

**完成此目标即"功能完成"**。任何"顺手优化"、"额外 channel/runtime crate 改动"、"通用 logging 框架抽取"都是 scope creep，不在范围内。

**显式不在范围内**：
- ❌ 改 tracing-subscriber 初始化（`src/main.rs` 那块归 `kanmars.req.20260508.001` 已合并的 BeijingTimer 管，本计划不动）
- ❌ 引入 `tracing-appender` non-blocking writer（架构升级，独立 PR）
- ❌ 抽通用 HTTP logging crate / middleware
- ❌ 改 `chat_via_responses` / `list_models` 内的 HTTP 日志（它们当前**没有** `DEBUG_CHAT*`，本计划仅"替换+对称已存在的"，不"扩大覆盖到新路径"）—— 但 `chat_with_history` / `chat_with_tools` 在 PR2 内补齐，因为这俩与 `chat_with_system`/`chat` 是同一族 `chat_completions_url` 调用，不补齐就破坏 PR2 的"对称"承诺
- ❌ 改 `compatible.rs` 之外的任何文件（PR1 完全不改；PR2 仅在 `compatible.rs` 内增 helper）
- ❌ 删除 `llm_http_debug_info` 字段或改它的 schema（向后兼容用户 config）
- ❌ 删 `eprintln!` 之外的 `println!`（行 12/14 那种 streaming 输出在另一处，非本范围）
- ❌ 跨 crate 的 logging convention 文档化（独立 docs PR）

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不新增 `unwrap()` / `expect()`**（项目 Anti-Pattern #9）。`Uuid::new_v4()` / `Instant::now()` / `truncate_for_log` 全为 infallible。**特别注意**：reqwest `RequestBuilder` 不能直读 headers，必须 `try_clone().and_then(|b| b.build().ok())` —— 见 §2.2.1 helper 设计与 §2.2.6 request headers 策略。
2. **不新增 `#[allow(dead_code)]`**（Anti-Pattern #8）。每个新加的 helper（`truncate_for_log` / `sanitize_headers` / `log_llm_http_request` / `log_llm_http_response` / `log_llm_http_error`）必须立即被 ≥1 处生产代码调用 —— 这直接驱动 §3.2.1 把 helper 引入 commit 与首次调用合并。
3. **不动 `compatible.rs` 之外的文件**。helper 函数全部放本文件内（`fn` 私有 + `mod` inline test），不外露 API、不破坏 crate 边界。
4. **不动 `Cargo.toml`**。`uuid 1.22 features=["v4","std"]` 已在 [crates/zeroclaw-providers/Cargo.toml:34](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/Cargo.toml#L34) 存在，`tracing` 已在 deps，无需新增。
5. **不动业务逻辑**。仅日志通道与字段格式变化，HTTP send / fallback / parse / error propagation 全部保留原行为字节级一致。
6. **PR1 必须独立可合**。即使 PR2 永不上线，PR1 也是一个完整闭环（修复"stderr/stdout 错乱"），不引入未完成依赖。
7. **PR2 不能删 `llm_http_debug_info` 字段**。现网 config 仍可能写 `llm_http_debug = true`，schema 兼容性优先于代码简洁。

---

## 1. 现状事实复核（rev2 实测 + Momus 复核，行号与 HEAD 一致）

### 1.1 依赖

| 事实 | 文件:行 |
|---|---|
| `uuid 1.22` features=`["v4","std"]` 已在 providers crate deps | [crates/zeroclaw-providers/Cargo.toml:34](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/Cargo.toml#L34) |
| `tracing` 已在 providers crate deps（同文件已用 `tracing::debug!`） | [compatible.rs:1047](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L1047) |
| `llm_http_debug_info: pub(crate) bool` 字段 | [compatible.rs:50](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L50) |
| `AuthStyle` enum（`Bearer`/`XApiKey`/`Custom(String)`/`ZhipuJwt`） | [compatible.rs:60-71](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L60) |

### 1.2 待改位置（PR1 + PR2 总览）

⚠️ **rev2 修正**：Momus 实测 `chat_with_system` 内 `eprintln!` macro 调用是 **8 个**（不是 rev1 写的 9）。rev1 把跨多行的 macro 调用（如 1909-1912）误重复计数。当前数字均为 macro 调用数。

| 函数 | 起始行 | DEBUG_CHAT 类 eprintln! 行号 | 数量 | 当前开关 | PR1 范围 | PR2 范围 |
|---|---|---|---|---|---|---|
| `chat_with_system` | 1857 | 1908, 1909, 1913, 1914, 1918, 1934, 1939, 1961 | **8** | ❌ 永远开 | ✅ 全替 | 重构为 helper |
| `chat_with_history` | 2005 | 无 | 0 | n/a | ❌ 不改 | ✅ 新增 helper 调用 |
| `chat_with_tools` | 2106 | 无 | 0 | n/a | ❌ 不改 | ✅ 新增 helper 调用 |
| `chat`（native tools 主路径） | 2217 | 2258, 2259, 2263, 2268, 2269, 2270, 2271, 2272, 2273, 2275, 2276, 2277, 2284(`eprint!`), 2286, 2289, 2291, 2292, 2293, 2294, 2301, 2307 | **21** | ✅ `llm_http_debug_info` | ❌ 不改 | ✅ 全替 + 删 curl 块 |
| **合计** | | | **29** | | **8** | **29** |

**curl-format 块边界澄清**（Momus P2 修正）：
- header 块 = [compatible.rs:2258-2273](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L2258)（含尾部 `===` 闭合分隔符，原 rev1 写 2258-2272 偏 1）
- curl-format 块 = [compatible.rs:2274-2295](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L2274)（从 `if let Ok(req) = req_builder.try_clone()...` 开始，原 rev1 写 2273-2295 偏 1）
- **PR2 实施时按代码块语义定位**（"`if self.llm_http_debug_info {` 整块替换"）而非行号定位，避免 PR1 合并后行号漂移影响 PR2

### 1.3 现状日志样本（来自 [/home/admin/workspace-public/kanmars/gloria/A/logs/zeroclaw.log](file:///home/admin/workspace-public/kanmars/gloria/A/logs/zeroclaw.log) 第 5-11 行）

```
===========================================
DEBUG_CHAT: provider=Custom, url=https://coding.dashscope.aliyuncs.com/v1/chat/completions, model=qwen3.6-plus
DEBUG_CHAT auth_header: Bearer
DEBUG_CHAT request: {"model":"qwen3.6-plus","messages":[...]}    ← 114175 chars 单行
===========================================
DEBUG_CHAT response headers: {"content-type": "application/json", ...}  ← 比 ⏱ Starting LLM call 早出现
[2m2026-05-09T18:36:56.109+08:00[0m [32m INFO[0m ... ⏱ Starting LLM call elapsed_before_llm_ms=2657
[2m2026-05-09T18:37:30.698+08:00[0m [32m INFO[0m ... ⏱ LLM call completed llm_call_ms=34588
```

**3 类问题已实测确认**：
1. **跨流错乱**：`response headers` 行（stderr）出现在 `Starting LLM call` 行（stdout）之前 —— 落盘顺序 ≠ 事件时间顺序
2. **巨型行**：行 1 = 144,779 字符 / 行 7 = 114,175 字符，是并发 `eprintln!` 没有行级锁导致的拼接污染
3. **不对称**：request 不打 HTTP headers，response 打了；request 打 `auth_header` enum（配置层），与 wire 上的真实 `Authorization` header 完全不是一回事

---

## 2. 设计

### 2.1 PR1 设计：纯通道替换（约 ±20 行 net diff）

**原则**：1:1 替换，零行为变化，不引入新概念。

| 旧 | 新 |
|---|---|
| `eprintln!("==========…")` (1908, 1918) | **删** |
| `eprintln!("DEBUG_CHAT: provider={}, url={}, model={}", ...)` (1909) | `tracing::debug!(target: "zeroclaw_providers::http", provider = %self.name, url = %url, model = %model, "DEBUG_CHAT request begin")` |
| `eprintln!("DEBUG_CHAT auth_header: {:?}", self.auth_header)` (1913) | `tracing::debug!(target: "zeroclaw_providers::http", auth_header = ?self.auth_header, "DEBUG_CHAT auth_header")` |
| `eprintln!("DEBUG_CHAT request: {}", serde_json::to_string(&request).unwrap_or_default())` (1914) | `let request_json = serde_json::to_string(&request).unwrap_or_default();`<br>`tracing::debug!(target: "zeroclaw_providers::http", body_bytes = request_json.len(), body = %request_json, "DEBUG_CHAT request body")` |
| `eprintln!("DEBUG_CHAT response headers: {:?}", response.headers())` (1934) | `tracing::debug!(target: "zeroclaw_providers::http", headers = ?response.headers(), "DEBUG_CHAT response headers")` |
| `eprintln!("DEBUG_CHAT error: {}", chat_error)` (1939) | `tracing::warn!(target: "zeroclaw_providers::http", error = %chat_error, "DEBUG_CHAT transport error")` |
| `eprintln!("DEBUG_CHAT response error: status={}, error={}", status, error)` (1961) | `tracing::warn!(target: "zeroclaw_providers::http", status = status.as_u16(), error_body = %error, "DEBUG_CHAT response error")` ⚠️ **rev2 NIT P-N5**：用 `error_body` 字段名而非 `body`，避免与 transport `error` 字段混淆 |

**字段名保留 `DEBUG_CHAT` 前缀的事件 message** —— 让习惯 grep `DEBUG_CHAT` 的运维仍然能定位（向后兼容审计），但**字段已结构化**，PR2 时再统一清理 message 名。

**开关**：PR1 不引入新开关。`tracing::debug!` 默认被 `RUST_LOG=info` 过滤掉。开 debug 用 `RUST_LOG=zeroclaw_providers::http=debug zeroclaw service start`。这与"现网默认 INFO 级别看不到 DEBUG_CHAT 海量日志"是**改善**——之前是无条件喷，现在是按需开。

**用户行为变化告示**（必须写进 PR1 description）：
> ⚠️ **Behavior change**: `chat_with_system` 路径下的 DEBUG_CHAT 日志现在受 `RUST_LOG` 控制，默认 INFO 级别下**不再输出**。如需保留旧行为，启动时设 `RUST_LOG=zeroclaw_providers::http=debug`。

### 2.2 PR2 设计：对称化 + 结构化 + 覆盖 4 条路径

#### 2.2.1 引入 5 个私有 helper（全在 `compatible.rs` 文件内）

```rust
// =============================================================================
// HTTP debug logging — symmetric request/response events sharing a `req_id`.
//
// Toggle:
//   - `RUST_LOG=zeroclaw_providers::http=debug` for skeleton (provider/url/model/status/elapsed/body_bytes)
//   - `RUST_LOG=zeroclaw_providers::http=trace` for body content + sanitized headers
//
// Two events per call:
//   * "llm.http.request"   – before .send()
//   * "llm.http.response"  – after headers received  (or "llm.http.error")
// =============================================================================

const HTTP_LOG_BODY_MAX: usize = 4096;

/// Truncate to HTTP_LOG_BODY_MAX bytes on a UTF-8 char boundary.
fn truncate_for_log(s: &str) -> String {
    if s.len() <= HTTP_LOG_BODY_MAX {
        return s.to_string();
    }
    let cut = s
        .char_indices()
        .take_while(|(i, _)| *i < HTTP_LOG_BODY_MAX)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!("{}…[truncated, total={}B]", &s[..cut], s.len())
}

/// Mask Authorization / Cookie / api-key style headers; keep others verbatim.
fn sanitize_headers(headers: &reqwest::header::HeaderMap) -> Vec<(String, String)> {
    const SENSITIVE: &[&str] = &[
        "authorization", "cookie", "set-cookie",
        "x-api-key", "api-key", "proxy-authorization",
    ];
    headers.iter().map(|(name, value)| {
        let n = name.as_str();
        let v = if SENSITIVE.contains(&n.to_ascii_lowercase().as_str()) {
            value.to_str().ok()
                .map(|s| {
                    if s.len() > 12 {
                        let prefix_end = s.find(' ').map(|i| i + 1).unwrap_or(0);
                        let (prefix, secret) = s.split_at(prefix_end);
                        let tail_start = secret.len().saturating_sub(4);
                        format!("{prefix}***{}", &secret[tail_start..])
                    } else {
                        "***".to_string()
                    }
                })
                .unwrap_or_else(|| "***".to_string())
        } else {
            value.to_str().unwrap_or("[binary]").to_string()
        };
        (n.to_string(), v)
    }).collect()
}

fn log_llm_http_request(
    req_id: uuid::Uuid, provider: &str, url: &str, model: &str,
    method: &str, body_bytes: usize, body_json: &str,
) { /* tracing::debug! skeleton + tracing::trace! body */ }

fn log_llm_http_response(
    req_id: uuid::Uuid, provider: &str, url: &str, model: &str,
    status: u16, elapsed_ms: u64,
    headers: &reqwest::header::HeaderMap,
    body_text: Option<&str>,  // Some(s) iff non-2xx
) { /* tracing::debug! skeleton + tracing::trace! sanitized_headers + body */ }

fn log_llm_http_error(
    req_id: uuid::Uuid, provider: &str, url: &str, model: &str,
    elapsed_ms: u64, error: &dyn std::fmt::Display,
) { /* tracing::warn! */ }
```

**为什么这样切**：
- `truncate_for_log` 复用率 ≥4 处（每条路径 request body + 每条路径 error body）
- `sanitize_headers` 复用率 ≥4 处（4 路径 × response 的 headers；request headers 不打，见 §2.2.6）
- `log_llm_http_*` 三函数封装"先 debug! 列骨架、再 trace! 列细节"两档分级，避免每个调用点重复 5-8 行 tracing 模板

#### 2.2.2 对称字段表（PR2 后 grep `req_id=…` 出来的样子）

| 字段 | request | response | error |
|---|---|---|---|
| `req_id` | UUIDv4 | 同 | 同 |
| `direction` | `"request"` | `"response"` | `"error"` |
| `provider` / `url` / `model` | ✓ | ✓ | ✓ |
| `method` | ✓ | — | — |
| `body_bytes` | ✓ | ✓（仅 error 体） | — |
| `body` (debug) | 截断 | 截断（仅 non-2xx） | — |
| `body` (trace) | 全文 | 全文 | — |
| `headers` (trace, 脱敏) | **— 不打**（rev2 §2.2.6） | ✓ | — |
| `status` | — | ✓ | — |
| `elapsed_ms` | — | ✓ | ✓ |
| `error` | — | — | `%display` |

#### 2.2.3 关键不对称的合理性说明

- **request 没有 `status`/`elapsed_ms`**：物理上不存在（还没发出去）
- **response 没有 `method`**：跟 request 同 `req_id` 一查就有
- **error 没有 `body`**：transport error 没有响应体
- **request 的 `body` 总打**（截断），response 的 `body` **只在 non-2xx 时打**：success path 不需要日志展开 100KB 响应（runtime 会自己 parse 进结构化结果），non-2xx 必须打用于排查
- **request 的 `headers` 不打**：见 §2.2.6 — reqwest `RequestBuilder` 不能直读 headers，要 build 一次会触发 `unwrap` 风险

#### 2.2.4 `chat`（native tools）路径的特殊处理

[compatible.rs:2274-2295](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L2274) 那段手工拼 HTTP/1.1 报文 + curl 格式的代码（约 22 行）**直接删除**，理由：
- reqwest `Request` 不能 100% 还原可执行 curl（auth 已脱敏成 `***`）
- `body=` 字段在 `RUST_LOG=…=trace` 下已能看到 request JSON
- response 的 `headers=` 字段在 trace 级别已能看到完整 wire response headers（脱敏后）
- 删它换来 ~22 行净减少 + 与其他 3 条路径同构

**实施时按代码块语义定位**：找 `if self.llm_http_debug_info {` 块整体替换，不依赖具体行号。

`llm_http_debug_info` 字段**保留**（schema 向后兼容）。运行时行为改为：
- 当字段为 `false` 时 → 当前路径**不**调用 `log_llm_http_*`（保持"完全静默"语义，避免噪音）
- 当字段为 `true` 时 → 调用 `log_llm_http_*`，由 `RUST_LOG` 决定 debug/trace 出多少

→ 字段语义升级为"是否参与 HTTP 日志体系"，而不是"是否往 stderr 喷"。等价或更强。

#### 2.2.5 4 条路径覆盖完整性

| 路径 | request 调用点 | response success | response non-2xx | transport error |
|---|---|---|---|---|
| `chat_with_system` (1857) | 在 .send() 前 | 在 Ok arm | 在 `!is_success()` arm | 在 Err arm |
| `chat_with_history` (2005) | 同 | 同 | 在 `!is_success()` arm（**新增**，当前无日志） | 在 Err arm（**新增**） |
| `chat_with_tools` (2106) | 同 | 同 | 在 `!is_success()` arm（**新增**） | 在 Err arm（**新增**） |
| `chat` (2217, native) | 同（受 `llm_http_debug_info` gate） | 同 | 同 | 同 |

→ PR2 后 grep `llm.http.request` 与 grep `llm.http.(response|error)` 行数应**完全相等**（同一进程同一时间窗内）。

#### 2.2.6 ⚠️ rev2 新增：request headers 显式不打的决策

reqwest `RequestBuilder::headers()` 不存在，要拿到 headers 必须 `try_clone().and_then(|b| b.build().ok())` 后访问 `Request::headers()`。**任意一处 `.unwrap()` 都违反 §0.5 #1**。

3 个备选方案：

| 选项 | 优点 | 缺点 | 决策 |
|---|---|---|---|
| A. 不打 request headers | 零 unwrap 风险，helper 签名简单 | 字段表 trace 级少一项 | **✅ 选用** |
| B. 用 `try_clone().and_then(\|b\| b.build().ok()).map(\|r\| sanitize_headers(r.headers()))` | 完整对称 | helper 签名引入 `Option`、增加 build 一次的 CPU 开销 | 否 |
| C. 在 helper 外手工 build 后传入 | 同 B | 调用点更繁琐，4 路径 × 2 行模板 = 8 行重复 | 否 |

**核心论据**：request headers 在 LLM HTTP 调用场景几乎只有 `Content-Type: application/json` + `Authorization` + `User-Agent` + `extra_headers`（用户配置）四类。前三类**完全可预测**，第四类**已有** `auth_header = ?self.auth_header` debug 字段（在 PR1 时保留迁移）和 `extra_headers` 字段（可在 PR2 helper 内额外打 `extra_headers = ?self.extra_headers`，零 unwrap 风险）。**信息无丢失**。

→ 字段表 §2.2.2 已修正：request 的 `headers (trace)` 列从"✓"改"— 不打"。

---

## 3. 实施步骤

### PR1: `fix/debug-chat-tracing-replace`（预计 5 分钟开发 + 5 分钟评审）

#### 3.1.1 PR1 唯一一个 commit

**Commit 1/1**: `fix(providers): replace DEBUG_CHAT eprintln! with tracing in chat_with_system`

**改动文件**：
- `crates/zeroclaw-providers/src/compatible.rs`（约 ±20 行 net）

**精确编辑清单**（用 Edit 工具逐条执行，按当前 HEAD 行号）：

| # | 操作 | 位置 | 具体 |
|---|---|---|---|
| 1 | 删 | 1908 | `eprintln!("==========…");` |
| 2 | 替 | 1909-1912 | `eprintln!("DEBUG_CHAT: provider={}, url={}, model={}", self.name, url, model);` → `tracing::debug!(target: "zeroclaw_providers::http", provider = %self.name, url = %url, model = %model, "DEBUG_CHAT request begin");` |
| 3 | 替 | 1913 | `eprintln!("DEBUG_CHAT auth_header: {:?}", self.auth_header);` → `tracing::debug!(target: "zeroclaw_providers::http", auth_header = ?self.auth_header, "DEBUG_CHAT auth_header");` |
| 4 | 替 | 1914-1917 | `eprintln!("DEBUG_CHAT request: {}", serde_json::to_string(&request).unwrap_or_default());` → 拆为 2 行：<br>`let request_json = serde_json::to_string(&request).unwrap_or_default();`<br>`tracing::debug!(target: "zeroclaw_providers::http", body_bytes = request_json.len(), body = %request_json, "DEBUG_CHAT request body");` |
| 5 | 删 | 1918 | `eprintln!("==========…");` |
| 6 | 替 | 1934 | `eprintln!("DEBUG_CHAT response headers: {:?}", response.headers());` → `tracing::debug!(target: "zeroclaw_providers::http", headers = ?response.headers(), "DEBUG_CHAT response headers");` |
| 7 | 替 | 1939 | `eprintln!("DEBUG_CHAT error: {}", chat_error);` → `tracing::warn!(target: "zeroclaw_providers::http", error = %chat_error, "DEBUG_CHAT transport error");` |
| 8 | 替 | 1961-1964 | `eprintln!("DEBUG_CHAT response error: status={}, error={}", status, error);` → `tracing::warn!(target: "zeroclaw_providers::http", status = status.as_u16(), error_body = %error, "DEBUG_CHAT response error");` ⚠️ rev2: 用 `error_body` 字段而非 `body`（区分于 transport `error`） |

**操作合计**：8 个 eprintln! macro 调用 → 删 2 + 替 6 → 净产生 **6 条 tracing 调用**（4 debug + 2 warn；操作 4 额外产生 1 个 `let request_json` 变量绑定，**不是** tracing 调用，不计入此计数）。

**验证**：
1. `cargo check -p zeroclaw-providers` → exit 0
2. `cargo clippy -p zeroclaw-providers --all-targets -- -D warnings` → exit 0
3. `cargo test -p zeroclaw-providers` → all pass（无新增测试，验证零回归）
4. **正向 grep**：`grep -c '"DEBUG_CHAT' crates/zeroclaw-providers/src/compatible.rs` 在 `chat_with_system` 函数范围（1857-2003）内应 ≥6（事件 message 字符串保留）
5. **反向 grep**（关键断言）：
   ```bash
   awk 'NR==1857,/^    async fn chat_with_history/' crates/zeroclaw-providers/src/compatible.rs \
     | grep -c 'eprintln!'
   ```
   应输出 `0`
6. 手动 smoke：本地启动并触发一次 LLM 调用，确认日志：
   ```bash
   RUST_LOG=zeroclaw_providers::http=debug cargo run --bin zeroclaw -- agent <<< 'hi' \
     > /tmp/zeroclaw-pr1-smoke.log 2>&1
   grep -c 'DEBUG_CHAT request begin' /tmp/zeroclaw-pr1-smoke.log     # 期望 ≥1
   grep -c 'DEBUG_CHAT request body' /tmp/zeroclaw-pr1-smoke.log      # 期望 ≥1
   grep -c 'DEBUG_CHAT response headers' /tmp/zeroclaw-pr1-smoke.log  # 期望 ≥1
   grep -E '\+08:00.*DEBUG_CHAT' /tmp/zeroclaw-pr1-smoke.log | head -3 # 期望行首带 BeijingTimer 时间戳
   ```

**回滚**：单 commit revert，影响零业务逻辑。

### PR2: `feat/llm-http-symmetric-tracing`（预计 1 小时开发 + 30 分钟评审）

PR1 合并并观察 ≥1 晚日志稳定后开始。

#### 3.2.1 PR2 commit 拆分（4 个 atomic commits，rev2 已合并初稿的 commit 1+2 避免 dead code）

**Commit 1/4**: `refactor(providers): introduce symmetric HTTP log helpers and migrate chat_with_system`

- 新增 `truncate_for_log` / `sanitize_headers` / `log_llm_http_request` / `log_llm_http_response` / `log_llm_http_error`
- `chat_with_system` 8 处替换为 helper 调用（让 helper 立即被 ≥1 处生产代码调用，满足 §0.5 #2）
- inline test 模块覆盖 helper 边界（multi-byte UTF-8 截断 / Authorization 脱敏 / Cookie 脱敏 / 普通 header 不脱敏）

**Commit 2/4**: `feat(providers): add HTTP request/response logging to chat_with_history`

- `chat_with_history` (2005) 在 `.send()` 前 + Ok/Err arm + non-2xx arm 加 helper 调用
- 不改业务逻辑

**Commit 3/4**: `feat(providers): add HTTP request/response logging to chat_with_tools`

- `chat_with_tools` (2106) 同上

**Commit 4/4**: `refactor(providers): migrate chat() native path to unified HTTP log helpers`

- `chat` (2217) 删除 [2274-2295](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L2274) curl-format 块（按 `if let Ok(req) = req_builder.try_clone()...` 块整体定位，避免 PR1 合并后行号漂移）
- 替换 [2258-2273](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L2258) header 块为 helper 调用（按 `if self.llm_http_debug_info {` 块整体定位）
- 替换 [2301](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L2301) / [2307](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L2307) 为 helper 调用
- `llm_http_debug_info` 语义改为"是否参与 helper 调用"（`if self.llm_http_debug_info { log_llm_http_* }`）
- 字段名 / schema 不变

#### 3.2.2 PR2 验证

每个 commit 后单独跑：
1. `cargo check -p zeroclaw-providers` → exit 0
2. `cargo clippy -p zeroclaw-providers --all-targets -- -D warnings` → exit 0
3. `cargo test -p zeroclaw-providers` → all pass
4. **inline test 触发**（rev2 P3 修正：用 fn 名过滤而非 cargo `--test '*'` glob，cargo 不接受 glob）：
   ```bash
   cargo test -p zeroclaw-providers truncate_for_log
   cargo test -p zeroclaw-providers sanitize_headers
   ```
   或更宽：`cargo test -p zeroclaw-providers compatible::tests::`

整个 PR2 合并前：
5. `./dev/ci.sh all` → exit 0（项目 Pre-PR 标准门禁）
6. **对称性验证脚本**（rev2 P-N6 修正：cargo run 输出重定向到日志文件，让 grep 有源数据）：
   ```bash
   RUST_LOG=zeroclaw_providers::http=trace cargo run --bin zeroclaw -- agent <<< 'hi' \
     > /tmp/zeroclaw-pr2-smoke.log 2>&1
   REQUESTS=$(grep -c '"llm.http.request"' /tmp/zeroclaw-pr2-smoke.log)
   RESPONSES=$(grep -cE '"llm.http.(response|error)"' /tmp/zeroclaw-pr2-smoke.log)
   test "$REQUESTS" = "$RESPONSES" || { echo "asymmetric: req=$REQUESTS resp=$RESPONSES"; exit 1; }
   # 提取一个 req_id 验证 grep 能成对匹配
   REQ_ID=$(grep '"llm.http.request"' /tmp/zeroclaw-pr2-smoke.log | head -1 \
     | grep -oP '(req_id=|"req_id":")\K[a-f0-9-]+')
   PAIR_COUNT=$(grep -c "req_id[=:]\"\?$REQ_ID" /tmp/zeroclaw-pr2-smoke.log)
   test "$PAIR_COUNT" = "2" || { echo "req_id $REQ_ID found $PAIR_COUNT lines, expected 2"; exit 1; }
   ```
7. **脱敏验证**：
   ```bash
   ! grep -E 'Bearer (sk|xa)-[a-zA-Z0-9]{10,}' /tmp/zeroclaw-pr2-smoke.log
   # 反向断言：trace 级日志里不应出现完整 API key
   ! grep -iE '(authorization|x-api-key|cookie):\s+[a-zA-Z0-9_.-]{20,}' /tmp/zeroclaw-pr2-smoke.log
   # 反向断言：sensitive header 不应整段明文落盘
   ```
8. **对称表字段断言**（rev2 新增）：
   ```bash
   # request 行必须含 method 字段
   grep '"llm.http.request"' /tmp/zeroclaw-pr2-smoke.log | grep -q 'method='
   # response 行必须含 status + elapsed_ms 字段
   grep '"llm.http.response"' /tmp/zeroclaw-pr2-smoke.log | grep -q 'status=' \
     && grep '"llm.http.response"' /tmp/zeroclaw-pr2-smoke.log | grep -q 'elapsed_ms='
   # provider/url/model 三字段在 request 与 response 行均出现
   for fld in provider url model; do
     grep '"llm.http.request"' /tmp/zeroclaw-pr2-smoke.log | grep -q "$fld="
     grep '"llm.http.response"' /tmp/zeroclaw-pr2-smoke.log | grep -q "$fld="
   done
   ```

---

## 4. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| PR1 后用户依赖 `RUST_LOG=info` 默认看到 DEBUG_CHAT，升级后看不到 | 中 | 低（运维抱怨） | PR1 description 明确 behavior change + 升级命令 |
| `tracing::debug!` 在高 QPS 下宏展开开销 vs `eprintln!` | 极低 | 极低 | tracing 在被 EnvFilter 关闭时是 zero-cost；当前 `eprintln!` 永远写更贵 |
| `sanitize_headers` 漏过非常见 secret header（e.g. `X-Custom-Auth`） | 中 | 中（日志泄密） | 黑名单 + 白名单兜底：未识别 header **默认不脱敏**保留可调试性，但在 Risk doc 中声明"非标准 secret header 自行配 RUST_LOG 过滤" |
| `truncate_for_log` 截断点切到多字节 UTF-8 中间 → tracing fmt panic | 低 | 中（日志线程崩） | rev2 §2.2.1 给出完整代码：`s.char_indices().take_while(\|(i,_)\| *i < HTTP_LOG_BODY_MAX).last().map(\|(i,c)\| i + c.len_utf8())` —— 在 char 边界后切，inline test 覆盖 |
| PR2 合并后 grep `DEBUG_CHAT` 突然找不到（运维 muscle memory） | 中 | 低 | PR2 description 提供迁移命令：`grep 'llm.http' instead of 'DEBUG_CHAT'` |
| `chat_via_responses` / `list_models` 内仍有零散日志，破坏"全文件 100% 覆盖"承诺 | 低 | 低 | 范围已显式排除（§0），grep 验证只针对 4 条 chat 路径 |
| **新增 rev2**：reqwest `RequestBuilder` 不能直读 headers，强行读会引入 unwrap 违反 §0.5 #1 | 中 | 高（项目红线） | §2.2.6 决策选项 A：request headers 不打；改用 `auth_header = ?self.auth_header` + `extra_headers = ?self.extra_headers` 字段替代，零 build 调用、零 unwrap |

---

## 5. 验收清单（PR1 + PR2 各一份）

### PR1 验收

- [x] `crates/zeroclaw-providers/src/compatible.rs` 的 `chat_with_system` 函数（行 1857-2003）内 `eprintln!` 计数 = 0（用 §3.1.1 验证步骤 5 反向 grep 脚本机器化验证）  ← atlas 2026-05-09 亲跑：`grep -c eprintln! = 0` ✅
- [x] 同函数内 `tracing::(debug|warn)!` 计数 = **6**（明细：4 debug = `request begin` + `auth_header` + `request body` + `response headers`；2 warn = `transport error` + `response error`。操作 4 额外产生 1 个 `let request_json = ...` 变量绑定，不是 tracing 调用，**不计入此计数**）  ← atlas 亲跑：`grep -cE 'tracing::(debug|warn)!' = 6`，明细 6 行完全对齐 ✅
- [x] `cargo clippy -p zeroclaw-providers --all-targets -- -D warnings` exit 0  ← atlas 亲跑 0.39s exit 0 ✅
- [x] `cargo test -p zeroclaw-providers` all pass  ← atlas 亲跑：807 passed; 0 failed ✅
- [ ] §3.1.1 验证步骤 6 smoke 4 条 grep 全部 ≥1  ← ⏸ 留给用户：本沙箱无 LLM API key，需 PR 合并后用户在 gloria/A 实例跑 `RUST_LOG=zeroclaw_providers::http=debug zeroclaw service start` 触发一次对话验证
- [x] PR description 含 behavior change 声明 + `RUST_LOG=zeroclaw_providers::http=debug` 升级命令  ← 写入 commit body + 输出给用户的 PR template
- [x] 单 commit，commit message 符合 conventional commits（`fix(providers): ...`）  ← commit `f8a01a29` 已落地（1 file +7 -17）+ 追加 commit `f49d325b`（用户实测发现 success response body 未打印 → atlas 加 1 行对称补丁，1 file +1 -0，与 request body 字段对称）

### PR2 验收

> ⚠️ **rev2.2 状态说明**（2026-05-09）：本计划事实上**没有进入 PR2 阶段**。用户在 PR1 部署后实测发现 `chat()`（主对话路径）日志缺失，atlas 在 PR1 同分支 `fix/debug-chat-tracing-replace` 上追加了 3 个 PR1 风格的 follow-up commit（`f49d325b` chat_with_system success body / `0b56f359` chat() 路径 7 个 tracing + 删 22 行死代码 / `a24488ec` doc 修订），覆盖了 PR2 验收清单中**4 项作为副作用真实达成**的项。其余 **8 项是 PR2 helper 化设计的核心**（5 helpers / req_id / 脱敏 / 截断 / 对称 smoke / dev/ci.sh / atomic commits 内容匹配 / PR description），**本次未涉及**。如要做完整 PR2，需独立 PR3。

- [ ] 5 个 helper 函数均存在且**至少被 1 处生产代码调用**（grep 验证，无 dead_code）  ← ❌ **未做**：本次走 PR1 风格简单 tracing，未引入 helper 函数
- [ ] 4 条 chat 路径（`chat_with_system` / `chat_with_history` / `chat_with_tools` / `chat`）每条至少 1 处 `log_llm_http_request` + 1 处 `log_llm_http_response` + 1 处 `log_llm_http_error`  ← ❌ **未做**：helper 不存在；且 `chat_with_history` / `chat_with_tools` 完全未碰
- [x] `compatible.rs` 内 `eprintln!` 计数 = 0（全文件强断言）  ← atlas 2026-05-09 亲跑 `grep -c 'eprintln!' = 0` ✅（chat_with_system + chat() 已清；其他函数本来无 eprintln）
- [x] `compatible.rs` 内 `DEBUG_CHAT_FUNC` 字符串引用 = 0（旧标签全部清除）  ← atlas 2026-05-09 亲跑 `grep -c 'DEBUG_CHAT_FUNC' = 0` ✅（commit `0b56f359` 删 12 处运行时引用 + commit `a24488ec` 修 2 处 doc comment）
- [x] `compatible.rs` 内不引入新的 `.unwrap()` / `.expect()`（grep diff 反向断言）  ← atlas 亲跑 `git diff master..HEAD | grep -E '\.unwrap\(\)|\.expect\(' = 0` ✅
- [ ] 对称性 smoke 脚本（§3.2.2 第 6 条）pass  ← ❌ **未做**：脚本 grep `"llm.http.request"` / `"llm.http.response"` 模式不匹配实际事件名 `"DEBUG_CHAT request begin"` 等；req_id 未实现
- [ ] 脱敏 smoke 脚本（§3.2.2 第 7 条）pass  ← ❌ **未做**：`sanitize_headers` 未实现；实测 response headers 中 `set-cookie: acw_tc=180dd...` 真实裸打
- [ ] 对称表字段断言（§3.2.2 第 8 条）pass  ← ❌ **未做**：同上 grep 模式不匹配
- [ ] `./dev/ci.sh all` exit 0  ← ❌ **未跑**：本次仅跑 `cargo check/clippy/test -p zeroclaw-providers`
- [ ] PR description 含字段表（§2.2.2） + 迁移命令（grep `llm.http` 替 `DEBUG_CHAT`）+ §2.2.6 request headers 不打的决策说明  ← ❌ **未做**：本次 PR1 follow-up 不开新 PR description
- [ ] 4 个 atomic commits，每个独立可 revert  ← ⚠️ **形式达成内容不符**：分支有 4 个 atlas commit（f8a01a29 / f49d325b / 0b56f359 / a24488ec）+ 1 个用户 init（2e746c54）= 5 个，每个独立可 revert；但内容不是 PR2 plan 设计的 "1 helper-introducing + 1 chat_with_history + 1 chat_with_tools + 1 chat" 拆分
- [x] `llm_http_debug_info` 字段保留 + schema 兼容性测试 pass  ← ✅ 字段 [compatible.rs:50](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L50) + builder method [compatible.rs:323](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-providers/src/compatible.rs#L323) 均保留（commit `a24488ec` 标 deprecated no-op）；schema 从未暴露此字段，无 schema 兼容性可破

---

## 6. Out of Scope（明确不做，避免 scope creep）

| 项 | 不做的理由 |
|---|---|
| 改 tracing-subscriber 初始化 | 已被 `kanmars.req.20260508.001` BeijingTimer 改完，无叠加需求 |
| 引入 `tracing-appender` non-blocking writer | 架构升级，独立 RFC |
| 通用 HTTP logging crate 抽取 | 仅 1 处使用，YAGNI |
| `chat_via_responses` 加日志 | 该路径无现存 `DEBUG_CHAT*`，本计划范围"替换+对称已存在"，不"扩大覆盖到新路径" |
| 删除 `llm_http_debug_info` 字段 | 现网 config 兼容优先 |
| 改 `println!` 行为（streaming chunk 回显） | 不在 compatible.rs，独立问题 |
| 跨 crate logging convention 文档化 | 独立 docs PR |
| 给所有 tracing 事件加 `req_id`（非 HTTP 类） | 范围爆炸，独立 RFC |
| 打 request headers（trace 级） | §2.2.6 已论证：reqwest API 限制 + unwrap 红线 + 信息无丢失 |

---

## 7. 时间预估

| 阶段 | 预估 |
|---|---|
| PR1 实现 | 5 min |
| PR1 验证 + smoke | 10 min |
| PR1 评审 + 合并 | 用户决定 |
| PR1 观察期 | ≥1 晚日志稳定 |
| PR2 实现 | 60 min |
| PR2 验证 + smoke | 30 min |
| PR2 评审 + 合并 | 用户决定 |

总开发时间 ≈ 105 分钟，分两次提交。

---

## 8. 决策记录

| 决策点 | 选择 | 替代方案 | 选择理由 |
|---|---|---|---|
| 单 PR vs 两 PR | 两 PR | 单 PR | 用户已确认 PR1 先止血、PR2 完整重构 |
| target name | `zeroclaw_providers::http` | `zeroclaw_providers::compatible::http` | 更短、更易记，未来如新增其他 provider 文件可复用 |
| 字段名 `req_id` vs `request_id` | `req_id` | `request_id` | 与 reqwest 社区习惯一致，更短 |
| body 截断长度 | 4096 | 1024 / 8192 | 4KB 既能容纳大多数小 prompt 全文，又不会单行过长 |
| 脱敏策略 | 黑名单（已知 secret header） | 白名单（仅显示已知安全 header） | 调试场景需要看到非常规 header，黑名单更友好；secret header 加入黑名单即可 |
| 是否删 `llm_http_debug_info` 字段 | 保留 | 删除 | schema 向后兼容优先 |
| 新增 helper 放哪 | `compatible.rs` 文件内私有 fn | 新文件 `http_log.rs` | 仅 1 处用，单文件足够；新文件徒增导入复杂度 |
| `Uuid` v4 vs v7 | v4 | v7 | v4 features 已存在；v7 需新增 features，违反 §0.5 #4 |
| **rev2 新增**：response error 字段名 `body` vs `error_body` | `error_body` | `body` | Momus N5：与 transport `error` 字段名形成视觉对照，结构化 grep 时不混淆 |
| **rev2 新增**：request headers 是否打 | 不打 | 打（需 try_clone+build） | §2.2.6 详细论证：unwrap 红线 + 信息无丢失（已有 `auth_header` + `extra_headers` 替代字段） |
| **rev2 新增**：commit 拆分 5 vs 4 | 4（合并 helper 引入与首次调用） | 5（helper 单独 commit） | §0.5 #2 反 dead_code 红线 |

---

## 9. 附录：用户原始需求（保留原话避免漂移）

> 那我感觉可以把 `compatible.rs:1908-1939` 的 7 处 eprintln! 全替成 tracing::debug!，
>
> 另外request,response能不能对称一点，我看着这个用====分割也很错乱
>
> 请给我个建议

→ **本计划严格对应这两点**：
1. 用户口述"7 处 eprintln!"，rev1 实测 9（误算），rev2 Momus 复核 = **8 处 macro 调用**（行 1908/1909/1913/1914/1918/1934/1939/1961）。最终编辑清单 §3.1.1 = 8 操作（删 2 + 替 6），产生 **6 条 tracing 调用**（4 debug + 2 warn；操作 4 额外产生 1 `let request_json` 绑定，不计入 tracing 调用数）。PR1 完成。
2. 对称化 + 删 `====` 分隔符；PR2 完成。

任何超出这两点的改动 = scope creep，必须独立 PR。

---

## 10. 修订日志

### rev2.2（2026-05-09，PR1 follow-up 3 commit + PR2 验收 4/12 真达成）

用户在 PR1 部署后通过实测发现两类问题，atlas 在同分支追加 3 个 follow-up commit：

1. **commit `f49d325b`**: 用户报告"看不到 success response body"。atlas 在 `chat_with_system` 末尾加 1 行 `tracing::debug!` 打 success body（与 request body 字段对称）。
2. **commit `0b56f359`**: 用户报告"主对话日志没打"——查证主对话走 `chat()`（native tools 路径）而 PR1 只覆盖 `chat_with_system`。atlas 在 `chat()` 内：
   - 加 7 个同款 tracing 调用（与 chat_with_system 字段对称）
   - 把 `response.json()` 拆成 `response.text()` + `tracing::debug!` + `serde_json::from_str()` 以便打 success body
   - 删除 22 行从未启用的 `DEBUG_CHAT_FUNC` 死代码（`with_llm_http_debug_info` builder 全仓库 0 调用 + schema 不暴露字段，事实上死代码）
3. **commit `a24488ec`**: 修 2 处 doc comment 引用 `DEBUG_CHAT_FUNC` 旧行为，标记 `llm_http_debug_info` 字段 + setter 为 deprecated no-op（保留 API back-compat）。

PR2 验收清单 12 项中 **4 项作为副作用真达成**（#3 全文件 eprintln=0 / #4 全文件 DEBUG_CHAT_FUNC=0 / #5 无新 unwrap / #12 字段保留）；**8 项是 PR2 helper 化核心未涉及**（5 helpers / req_id / 脱敏 / 截断 / 对称 smoke / dev/ci.sh / atomic commits 内容匹配 / PR description），保留 [ ] + 注释。

如需做完整 PR2 helper-based 重构，需独立 PR3。

### rev2.1（2026-05-09，Momus 第 2 轮 ACCEPT）

应用 1 NIT：

- **算术修正**：§3.1.1 操作合计行 + §5 PR1 验收第 2 条："tracing 调用数 7" → "6"。`let request_json` 是变量绑定不是 tracing 调用，rev2 误算。

### rev2（2026-05-09，Momus 第 1 轮 ACCEPT WITH PATCHES）

应用 4 MINOR + 2 NIT：

- **P1/P1b**：§1.2 表 `chat_with_system` 数量 9 → 8；§9 附录 7→9 → 7→8
- **P2**：§2.2.4 / §3.2.1 Commit 4/4 行号 2258-2272 / 2273-2295 → 2258-2273 / 2274-2295；改用代码块语义定位（"`if self.llm_http_debug_info {` 整块"）避免 PR1 合并后行号漂移
- **P3**：§3.2.2 第 4 条 cargo test 命令 `--test '*'` glob → 按 fn 名过滤
- **P4**：§5 PR1 验收第 2 条加详细分项注释（4 debug + 2 warn + 1 拆出 = 7）
- **P5（NIT 升 MINOR 处理）**：§2.2.1/§2.2.2/§2.2.6 新增 request headers 不打的完整论证 + 替代字段方案；§2.2.1 helper 签名去掉 `headers: Option<&HeaderMap>` 参数
- **P-N5（NIT）**：§2.1 PR1 表 + §3.1.1 操作 8：response error 字段名 `body` → `error_body`
- **P-N6（NIT）**：§3.2.2 第 6 条 smoke 脚本加 `> /tmp/zeroclaw-pr2-smoke.log 2>&1`；§3.1.1 验证步骤 6 同步加重定向；req_id grep 兼容 fmt 与 JSON 两种 formatter
- **新增 §2.2.1 truncate_for_log 完整代码**：char_indices + len_utf8 安全切点
- **新增 §3.2.2 第 8 条字段断言**：自动验证字段表 §2.2.2 在实际日志中兑现
- **新增 §5 PR2 验收"不引入新 unwrap"项**：grep diff 反向断言

