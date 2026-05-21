# Plan — kanmars.req.20260508.001 (Beijing time across user-visible surfaces)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260508.001.plan |
| 关联需求 | 无独立 req 文档（用户口头需求 + 4 路并行 explore 审计报告） |
| 起草日期 | 2026-05-08 |
| 修订日期 | 2026-05-08 (rev3.1：Momus 第 3 轮 ACCEPT WITH MINOR PATCHES —— 修 cron/mod.rs 11 处计数错误 + Commit 6 反向 grep 覆盖 bare `Local::now()` 形式) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `feat/beijing-time-display` |
| 目标 PR 标题 | `feat: render user-visible time in Beijing time (Asia/Shanghai)` |
| 风险等级 | Low（仅改用户/LLM 直接可见的时间显示，零 schema/API/DB/storage/test/metadata 改动） |
| 选型方案 | **存储层保持 UTC 不动；显示/LLM/日志层硬编码转 `Asia/Shanghai`**（用户已确认） |

---

## 0. 关键目标（唯一的真理来源）

> ZeroClaw 在**用户/LLM 直接看见时间的地方**，全部显示北京时间（Asia/Shanghai）。**用户没有设置 OS `TZ` 环境变量**，所有依赖 `chrono::Local::now()` 的位置必须改为 `chrono::Utc::now().with_timezone(&chrono_tz::Asia::Shanghai)` 显式硬编码。

**用户/LLM 直接可见的时间面 = 4 类**：
1. **CLI 终端输出**（`zeroclaw cron list` / `zeroclaw auth list`）
2. **日志文件**（`zeroclaw.log` 行首时间戳）
3. **LLM prompt**（system prompt / per-turn 注入 / heartbeat 决策 prompt / MCSS 报告）
4. **Agent tool 返回值**（schedule / cron_runs / cron_add / delegate 工具的 JSON 响应）

完成此 4 类即"功能完成"。**任何其它"顺手修的 BUG / 一致性问题 / metadata 改进"都是 scope creep，不在范围内**。

**显式不在范围内**（rev3 强化）：
- ❌ 11 个 SQLite 库存储层 —— 用户已认可保持 UTC
- ❌ Gateway REST/SSE/WS API wire format —— 用户已认可保持 UTC
- ❌ OAuth / MS365 / Slack / Bluesky / WhatsApp 等协议字段 —— 协议要求 UTC
- ❌ `cost/tracker.rs` 日切桶 —— 风险大，独立 PR
- ❌ **测试代码内的 `chrono::Local::now()` / `chrono::Utc::now()`** —— 测试代码 0 用户可见
- ❌ **存储/计算/metadata 路径的 UTC 用法** —— 包括 `runtime_trace.rs` 测试代码、`hygiene.rs` state 文件读写与 cutoff 计算、`skillforge/integrate.rs` TOML manifest metadata 字段，都属于"非显示路径"
- ❌ **`Local::now() / Utc::now()` 同文件混用的"潜在歧义"修复** —— 是真 BUG 但不是用户当前要解决的问题，独立 PR

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不在 `zeroclaw-runtime` 增新功能**（其 [AGENTS.md](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/AGENTS.md) 明确"transitional holding crate"）。本计划在 runtime 内**仅替换 API 调用**（`Utc::now()` / `Local::now()` → `Utc::now().with_timezone(&Asia::Shanghai)`），不新增模块/函数/trait。
2. **不新增 `unwrap()` / `expect()`**（项目 Anti-Pattern #9）。`chrono_tz::Asia::Shanghai` 是编译期常量。
3. **不新增任何 `#[allow(dead_code)]`** —— 直接违反项目 Anti-Pattern #8（"Do not suppress unused production code … delete it"）。每个新加的 helper 函数必须立即被≥1 处生产代码调用。
4. **不动 struct 字段类型** —— 如 [tools/cron_runs.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/cron_runs.rs) `RunView { started_at: DateTime<Utc>, ... }` 必须保留类型，转时区在 serialize 前用 `serde_json::json!()` 就地构造，避免影响 SQLite 反序列化路径与潜在外部 consumer。
5. **DRY**：`src/main.rs:format_expiry`（2 处）+ `src/cron/mod.rs`（12 处）= 14 处调用同一表达式 → **抽 1 个 helper 函数**（`fmt_beijing_rfc3339`，**只此 1 个**），放入新文件 `src/time_display.rs`。runtime crate 内则就地展开（受 §0.5 #1 约束）。

---

## 1. 现状事实复核（rev3 重新实测，行号与 HEAD 一致）

### 1.1 依赖

| 事实 | 文件:行 |
|---|---|
| 根 `Cargo.toml`: `chrono-tz = { version = "0.10", optional = true }` 已被 agent-runtime feature 激活 | [Cargo.toml:182,257](file:///home/admin/workspace-public/kanmars/zeroclaw/Cargo.toml#L182) |
| `zeroclaw-runtime/Cargo.toml`: `chrono-tz = "0.10"` always-on | [crates/zeroclaw-runtime/Cargo.toml:24](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/Cargo.toml#L24) |
| `tracing-subscriber` features：`["fmt", "ansi", "env-filter"]` —— `BeijingTimer impl FormatTime` 只依赖 `fmt`，**无需新增 feature** | [Cargo.toml:116](file:///home/admin/workspace-public/kanmars/zeroclaw/Cargo.toml#L116) |
| `src/util.rs` 当前内容 = 1 行 `pub use zeroclaw_runtime::util::*;` (re-export) | [src/util.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/src/util.rs) |
| `src/main.rs:204` 已 `mod util;` 注册 | [src/main.rs:204](file:///home/admin/workspace-public/kanmars/zeroclaw/src/main.rs#L204) |

### 1.2 待改位置（rev3：4 类用户可见面，共 32 处生产代码）

| 阶段 | 位置 | 处数 | 当前 | 类别 |
|---|---|---|---|---|
| **P1.1** | [src/main.rs:1260-1267](file:///home/admin/workspace-public/kanmars/zeroclaw/src/main.rs#L1260) | 1 | 无 `.with_timer()` → 默认 UTC | 日志 |
| **P1.2** | [crates/zeroclaw-runtime/src/heartbeat/engine.rs:328-340](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/heartbeat/engine.rs#L328) | 1 | `Utc::now()` + 字面 `"Current time: {} UTC"` | LLM prompt |
| **P1.3** | [crates/zeroclaw-runtime/src/tools/security_ops.rs:217](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/security_ops.rs#L217) | 1 | `Utc::now().format("%Y-%m-%d %H:%M UTC")` | 用户可见报告 |
| **P2.1** | [src/cron/mod.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/src/cron/mod.rs) 行 21,27,69,78,110,118,148,157,188,196,272 | 11 | `next_run.to_rfc3339()` | CLI |
| **P2.2** | [src/main.rs:3354-3370](file:///home/admin/workspace-public/kanmars/zeroclaw/src/main.rs#L3354) `format_expiry()` | 2 | `ts.to_rfc3339()` | CLI |
| **P3.1** | [crates/zeroclaw-runtime/src/tools/schedule.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/schedule.rs) 行 193,199,221,222,336,359,387 | 7 | `to_rfc3339()` | Tool 返回 |
| **P3.2** | [crates/zeroclaw-runtime/src/tools/cron_runs.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/cron_runs.rs) `RunView` 序列化点 | 2 字段 | serde 默认 → UTC | Tool 返回 |
| **P3.3** | [crates/zeroclaw-runtime/src/tools/cron_add.rs:362](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/cron_add.rs#L362) | 1+ | serde 默认 → UTC | Tool 返回 |
| **P3.4** | [crates/zeroclaw-runtime/src/tools/delegate.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/delegate.rs) 行 619,689,1022 | 3 | `Utc::now().to_rfc3339()` | Tool 返回 |
| **P5** | [agent/system_prompt.rs:283-289](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/agent/system_prompt.rs#L283) (1) + [agent/prompt.rs:259-274](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/agent/prompt.rs#L259) (1) + [agent/agent.rs:1094-1099,1271](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/agent/agent.rs#L1094) (2) + [agent/loop_.rs:2571,2861,3449](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/agent/loop_.rs#L2571) (3) + [orchestrator/mod.rs:771](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L771) (1) | **8** | `chrono::Local::now()` | LLM prompt |
| | **合计** | **~38 处** | | |

### 1.3 rev3 显式不动清单（已实测属"非显示路径"，本计划不动）

| 位置 | rev3 实测验证 |
|---|---|
| [observability/runtime_trace.rs:216](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/observability/runtime_trace.rs#L216) | 同文件 line 371/399 实测在 `#[test] fn rolling_mode_keeps_latest_entries` (line 363) / `find_event_by_id_returns_match` (line 390) 内，是**测试代码**。Line 216 本身写的是 `state/runtime-trace.jsonl` 的 `timestamp` 字段，无 user-visible / LLM-visible 路径。**不动** |
| [memory/hygiene.rs:108-113,122-123](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-memory/src/hygiene.rs#L108) | 实测：line 108 是 `parse_from_rfc3339` 解析 state 文件；line 113 是 `signed_duration_since` 纯算术；line 123 是 `Utc::now().to_rfc3339()` 写 JSON state 文件。**全部存储/计算路径，0 用户可见输出**。**不动** |
| [skillforge/integrate.rs:84](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/skillforge/integrate.rs#L84) | 实测：写入生成的 TOML skill manifest `[skill.metadata].forge_timestamp` 字段，是 metadata，非用户/LLM 可见输出。**不动** |
| [orchestrator/mod.rs:9291](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L9291) | 实测在 `#[test] fn prompt_no_daily_memory_injection` 测试函数内，构造 fixture 文件名。**测试代码不动** |
| `crates/zeroclaw-memory/src/{sqlite,audit,response_cache}.rs` 已有 `Local::now()` 写库 | 存储层，不动 |
| Gateway/SSE 130 处 `to_rfc3339()` | API wire format，不动 |

---

## 2. 设计：硬编码 `Asia::Shanghai`

### 2.1 为什么不依赖 `chrono::Local`

用户**没有设置 `TZ` 环境变量**。`chrono::Local::now()` 读 `/etc/localtime`，部署环境可能未配置或被覆盖。**唯一可靠 = 代码写死**。

### 2.2 转换原语

```rust
// 在 binary helper 内：
chrono::Utc::now().with_timezone(&chrono_tz::Asia::Shanghai)
// 对已有的 DateTime<Utc> 字段：
ts.with_timezone(&chrono_tz::Asia::Shanghai)
```

`chrono_tz::Asia::Shanghai` 是编译期生成的零分配常量。无需 unwrap。

### 2.3 显示格式约定

| 场景 | 格式 | 示例 |
|---|---|---|
| CLI / Tool JSON / 日志 timer | RFC3339 with offset | `2026-05-08T18:30:00+08:00` |
| LLM prompt（与现有 `Local::now()` 输出对齐） | `%Y-%m-%d %H:%M:%S %Z` | `2026-05-08 18:30:00 CST` |

### 2.4 关于 `%Z` 输出稳定性

`chrono_tz::Asia::Shanghai` `%Z` 输出 `CST`，但 chrono `%Z` 历来在不同版本/不同 tz 数据可能返回 abbreviation 或空串。**所有断言统一**：
- 正向断言含 `+0800` 或 `+08:00` 或 `CST` 三选一
- 反向断言**不**含字面 `" UTC"`

---

## 3. 实施分解（rev3：6 个 commit，每个直接服务 4 类用户可见面）

### Commit 1 — `feat(cli): introduce time display helper for Asia/Shanghai`

**唯一目的**：为 Commit 2（CLI 14 处调用）提供 helper。**只此 1 个函数，0 死代码**。

**改动**：新建 `src/time_display.rs`（11 行）：

```rust
//! Time formatting helpers for user-visible binary surfaces.
//! Always renders in Asia/Shanghai (Beijing time) regardless of OS TZ
//! (we do not depend on /etc/localtime).

use chrono::DateTime;
use chrono_tz::Asia::Shanghai;

/// Format a UTC timestamp as RFC3339 in Beijing time, e.g.
/// `2026-05-08T18:30:00+08:00`.
pub fn fmt_beijing_rfc3339(ts: DateTime<chrono::Utc>) -> String {
    ts.with_timezone(&Shanghai).to_rfc3339()
}
```

**注册**：`src/main.rs` 紧邻 `mod util;` (line 204) 后加 `mod time_display;`。

**校验**：
- `cargo build` 绿
- `cargo clippy -- -D warnings` 绿
- `src/util.rs` 未污染：
  ```bash
  diff <(cat src/util.rs) <(echo 'pub use zeroclaw_runtime::util::*;')
  # 期望：无输出
  ```

> **注**：本 commit 单独存在时 helper 0 调用，但 Commit 2 在同 PR 内立即引入 14 处调用，**满足 §0.5 #3** "立即被≥1 处生产代码调用"。git bisect 中这 1 个 commit 短暂 0 调用，cargo clippy 默认对 binary crate `pub fn` 不 warn，无需 `#[allow(dead_code)]`。

---

### Commit 2 — `feat(cli): render cron list and auth expiry in Beijing time`

**唯一目的**：P2.1 + P2.2 —— `zeroclaw cron list` 与 `zeroclaw auth list` CLI 输出变北京时间。

**改动 1（P2.1，11 处）**：[src/cron/mod.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/src/cron/mod.rs) 行 21,27,69,78,110,118,148,157,188,196,272（rev3.1 实测：`grep -c 'to_rfc3339' src/cron/mod.rs` = 11，rev3 误写为 12）：

```rust
// 替换模式：
dt.to_rfc3339()              → crate::time_display::fmt_beijing_rfc3339(dt)
|d| d.to_rfc3339()           → |d| crate::time_display::fmt_beijing_rfc3339(d)
```

> 起草人执行前重 `grep -n 'to_rfc3339' src/cron/mod.rs` 核对实际行号。

**改动 2（P2.2，2 处）**：[src/main.rs:3354-3370](file:///home/admin/workspace-public/kanmars/zeroclaw/src/main.rs#L3354) `format_expiry()` 两处 `ts.to_rfc3339()` → `crate::time_display::fmt_beijing_rfc3339(ts)`

**校验**：
- `cargo build` 绿；`cargo clippy -- -D warnings` 绿
- 正向 grep：
  ```bash
  grep -c 'fmt_beijing_rfc3339' src/cron/mod.rs src/main.rs
  # 期望：cron/mod.rs ≥ 11，main.rs ≥ 2
  ```
- 反向 grep：
  ```bash
  grep -n '\.to_rfc3339()' src/cron/mod.rs
  # 期望：0 hit
  grep -n '\.to_rfc3339()' src/main.rs | sed -n '/^33[5-7][0-9]:/p'
  # 期望（format_expiry 函数体范围 3354-3370）：0 hit
  ```
- 手工 smoke：先 `cargo run -- cron add '0 9 * * *' --command 'echo hi'` 创建临时 job（如命令格式不符按 `cron --help` 调整），再 `cargo run -- cron list`，期望输出含 `+08:00`

---

### Commit 3 — `feat(log): emit tracing logs in Beijing time`

**唯一目的**：P1.1 —— 整个 `zeroclaw.log` 时间戳变北京时间。

**改动**：[src/main.rs:1260-1267](file:///home/admin/workspace-public/kanmars/zeroclaw/src/main.rs#L1260) 周边。

**Step 1**：在 `src/main.rs` 顶层加 12 行 struct + impl：
```rust
struct BeijingTimer;
impl tracing_subscriber::fmt::time::FormatTime for BeijingTimer {
    fn format_time(
        &self,
        w: &mut tracing_subscriber::fmt::format::Writer<'_>,
    ) -> std::fmt::Result {
        let now = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Shanghai);
        write!(w, "{}", now.format("%Y-%m-%dT%H:%M:%S%.3f%:z"))
    }
}
```

**Step 2**：subscriber builder 加 1 行：
```rust
let subscriber = fmt::Subscriber::builder()
    .with_writer(std::io::stderr)
    .with_timer(BeijingTimer)            // ← 新增
    .with_env_filter(...)
    .finish();
```

**校验**：
- `cargo build` 绿；`cargo clippy -- -D warnings` 绿
- 正向 grep：
  ```bash
  grep -nc 'BeijingTimer' src/main.rs
  # 期望：≥ 3（struct 定义、impl 头、.with_timer 调用）
  ```
- 手工 smoke：`RUST_LOG=info cargo run -- agent --help 2>&1 | head -3` 期望首字段含 `+08:00`

---

### Commit 4 — `fix(runtime): hardcode Asia/Shanghai in heartbeat decision prompt and MCSS report`

**唯一目的**：P1.2 + P1.3 —— 修两处 LLM/用户看到 UTC 字面量的 BUG。runtime 内就地修改，不抽 helper。

**改动 1（P1.2）**：[heartbeat/engine.rs:328-340](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/heartbeat/engine.rs#L328)：
```rust
let now = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Shanghai);
let mut prompt = format!(
    "...\n\n\
     Current time: {} ({})\n\n\          // ← 去掉字面 "UTC"
     ...",
    now.format("%Y-%m-%d %H:%M:%S %Z"),  // ← 加 %Z
    now.format("%A"),
);
```

**改动 2（P1.3）**：[tools/security_ops.rs:217](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/security_ops.rs#L217)：
```rust
let now = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Shanghai);
format!("...Generated: {}", now.format("%Y-%m-%d %H:%M %Z"))
```

> 起草人执行时同时检查 line 165 `scan_date` 是否参与人类可读输出格式化；如是，一并改；如仅写存储，**不动**。

**校验**：
- `cargo build -p zeroclaw-runtime` 绿；`cargo clippy -p zeroclaw-runtime --all-targets -- -D warnings` 绿
- 反向 grep（最关键）：
  ```bash
  grep -n ' UTC' crates/zeroclaw-runtime/src/heartbeat/engine.rs crates/zeroclaw-runtime/src/tools/security_ops.rs
  # 期望：0 hit
  ```
- 单测：`cargo test -p zeroclaw-runtime heartbeat::engine`。如 `build_decision_prompt` 现有测试断言 prompt 文本含 `"UTC"`，按 §2.4 更新断言：
  ```rust
  assert!(!prompt.contains(" UTC"));
  assert!(prompt.contains("+0800") || prompt.contains("+08:00") || prompt.contains("CST"));
  ```

---

### Commit 5 — `fix(runtime): render schedule/cron/delegate tool outputs in Beijing time`

**唯一目的**：P3.1 + P3.2 + P3.3 + P3.4 —— Agent 通过工具看到的时间不再是 UTC。

**改动 1（P3.1，7 处）**：[tools/schedule.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/schedule.rs) 行 193, 199, 221, 222, 336, 359, 387：
```rust
dt.to_rfc3339()  →  dt.with_timezone(&chrono_tz::Asia::Shanghai).to_rfc3339()
```

**改动 2（P3.4，3 处）**：[tools/delegate.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/delegate.rs) 行 619, 689, 1022 同款替换。

**改动 3（P3.2 + P3.3，方案 6A：不改 struct）**：

[tools/cron_runs.rs:21-30](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/cron_runs.rs#L21) `RunView` struct **保留**：
```rust
#[derive(Serialize)]
struct RunView {
    started_at: chrono::DateTime<chrono::Utc>,    // ← 保留
    finished_at: chrono::DateTime<chrono::Utc>,   // ← 保留
    ...
}
```

**禁止改 struct 字段类型**（§0.5 #4）。改造方式：在 serialize 前用 `serde_json::json!()` 就地构造转时区后的 JSON：

```rust
let runs_json: Vec<serde_json::Value> = runs.iter().map(|r| {
    serde_json::json!({
        "id": r.id,
        "job_id": r.job_id,
        "started_at": r.started_at.with_timezone(&chrono_tz::Asia::Shanghai).to_rfc3339(),
        "finished_at": r.finished_at.with_timezone(&chrono_tz::Asia::Shanghai).to_rfc3339(),
        "status": r.status,
        "output": r.output,
        "duration_ms": r.duration_ms,
    })
}).collect();
```

[tools/cron_add.rs:362](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-runtime/src/tools/cron_add.rs#L362) 同款处理：起草人执行时 read 上下文，对所有 `DateTime<Utc>` 字段在序列化时调 `.with_timezone(&chrono_tz::Asia::Shanghai).to_rfc3339()`。

**校验**：
- `cargo build -p zeroclaw-runtime` 绿；`cargo clippy -p zeroclaw-runtime --all-targets -- -D warnings` 绿
- 正向 grep：
  ```bash
  grep -nc 'with_timezone(&chrono_tz::Asia::Shanghai)' \
    crates/zeroclaw-runtime/src/tools/schedule.rs \
    crates/zeroclaw-runtime/src/tools/delegate.rs \
    crates/zeroclaw-runtime/src/tools/cron_runs.rs \
    crates/zeroclaw-runtime/src/tools/cron_add.rs
  # 期望：schedule.rs ≥ 7、delegate.rs ≥ 3、cron_runs.rs ≥ 2、cron_add.rs ≥ 1
  ```
- 反向 grep：
  ```bash
  grep -n '\.to_rfc3339()' crates/zeroclaw-runtime/src/tools/schedule.rs | grep -v 'with_timezone'
  # 期望：0 hit
  ```
- struct 未变更校验：
  ```bash
  grep -A 9 'struct RunView' crates/zeroclaw-runtime/src/tools/cron_runs.rs | grep 'DateTime<chrono::Utc>'
  # 期望：≥ 2 hit（started_at + finished_at 类型未变）
  ```
- `cargo test -p zeroclaw-runtime` 绿（如有断言 UTC RFC3339 输出的测试需同步更新）

---

### Commit 6 — `fix(runtime): hardcode Asia/Shanghai in LLM prompt time injection`

**唯一目的**：P5 —— **8 处生产代码** `chrono::Local::now()` → `Utc::now().with_timezone(&chrono_tz::Asia::Shanghai)`，摆脱 OS TZ 依赖。

**rev3 确认**：测试代码（`orchestrator/mod.rs:9291`）已在 §0 / §1.3 显式排除，**不动**。生产代码 P5 总数 = 8 处。

| 文件 | 行 | 处数 |
|---|---|---|
| `crates/zeroclaw-runtime/src/agent/system_prompt.rs` | 283-289 | 1 |
| `crates/zeroclaw-runtime/src/agent/prompt.rs` | 259-274 | 1 |
| `crates/zeroclaw-runtime/src/agent/agent.rs` | 1094-1099, 1271 | 2 |
| `crates/zeroclaw-runtime/src/agent/loop_.rs` | 2571, 2861, 3449 | 3 |
| `crates/zeroclaw-channels/src/orchestrator/mod.rs` | 771 | 1 |
| **合计** | | **8** |

**替换模式**（每处一致）：
```rust
chrono::Local::now()  →  chrono::Utc::now().with_timezone(&chrono_tz::Asia::Shanghai)
```

**import 同步**：每个文件 read use 列表后决定。
- 4 个 `agent/*.rs` 文件：如全文唯一 chrono::Local 用法是 `Local::now()`，删除 use 列表中的 `Local`
- **`prompt.rs` 特殊（rev3.1 实测）**：当前用 bare `Local::now()`（line 259），顶部 `use chrono::{Datelike, Local, Timelike};` 三个一起 import；改完后该文件不再用 `Local`，**删 `Local` 保留 `Datelike, Timelike`**。其它 7 处用 fully-qualified `chrono::Local::now()` 不依赖 use 列表
- `orchestrator/mod.rs` **特殊**：测试代码 line 9291 仍用 `chrono::Local::now()`，如该文件原有 `use chrono::Local;`，**保留**不删（测试代码需要）

**校验**：
- `cargo build --workspace` 绿；`cargo clippy --workspace --all-targets -- -D warnings` 绿
- `cargo test --workspace` 绿
- 正向 grep：
  ```bash
  grep -rnc 'with_timezone(&chrono_tz::Asia::Shanghai)' \
    crates/zeroclaw-runtime/src/agent/ \
    crates/zeroclaw-channels/src/orchestrator/mod.rs
  # 期望：≥ 8 hit
  ```
- **反向 grep（rev3.1 修正：扩展正则覆盖 bare 与 fully-qualified 两种形式）**：
  ```bash
  # 4 个 agent 文件 0 残留（注意 prompt.rs:259 当前用 bare `Local::now()`，不能只匹配 chrono::Local::now）：
  grep -nE '(chrono::)?Local::now\(\)' \
    crates/zeroclaw-runtime/src/agent/system_prompt.rs \
    crates/zeroclaw-runtime/src/agent/prompt.rs \
    crates/zeroclaw-runtime/src/agent/agent.rs \
    crates/zeroclaw-runtime/src/agent/loop_.rs
  # 期望：0 hit

  # orchestrator/mod.rs 仅剩测试代码 1 hit：
  grep -nE '(chrono::)?Local::now\(\)' crates/zeroclaw-channels/src/orchestrator/mod.rs
  # 期望：仅 1 hit，行号 9291
  ```
- 手工 smoke：`cargo run -- agent` 交互问"现在几点"，确认回复含北京时间

---

## 4. 验证矩阵

| 改动点 | 验证方式 | 谁执行 |
|---|---|---|
| Commit 1 helper | `cargo build` 绿 + `src/util.rs` diff 无输出 | CI |
| P2.1 cron list | grep：cron/mod.rs `fmt_beijing_rfc3339` ≥ 12 hit、裸 `to_rfc3339` 0 hit；手工 `cron list` 含 `+08:00` | CI + 起草人 |
| P2.2 auth list | grep：format_expiry 范围裸 `to_rfc3339` 0 hit；手工 `auth list` 含 `+08:00` | CI + 起草人 |
| P1.1 日志 | grep：`BeijingTimer` ≥ 3 hit；手工 daemon stderr 首字段含 `+08:00` | CI + 起草人 |
| P1.2 heartbeat | 单测：断言不含 ` UTC`、含 `+0800`/`+08:00`/`CST` 任一 | CI |
| P1.3 MCSS | grep：security_ops.rs ` UTC` 0 hit | CI |
| P3.1-P3.4 | grep：4 文件 `with_timezone(&chrono_tz::Asia::Shanghai)` 计数符合预期；schedule.rs 内裸 `to_rfc3339` 0 hit | CI |
| P3.2 struct 未变 | grep：`RunView` `started_at` 仍为 `chrono::DateTime<chrono::Utc>` | CI |
| P5 LLM prompt | grep（扩展正则覆盖 bare/fully-qualified 两种形式）：`grep -nE '(chrono::)?Local::now\(\)'` 4 个 agent 文件 0 hit；orchestrator/mod.rs 仅剩 line 9291 | CI |
| P5 实运行 | 手工：`cargo run -- agent` 问 "现在几点" 截图 | 起草人 |

预 PR 三连：
```bash
./dev/ci.sh all
# 或：
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## 5. 已知舍弃（rev3 强化 "反 scope creep"）

| 舍弃项 | 理由 | 兜底 |
|---|---|---|
| 11 个 SQLite 库存储层 | 用户已认可 UTC | 显示层覆盖用户感知面 |
| Gateway REST/SSE 130 处 wire format | breaking change 风险；前端转 | 独立 PR |
| `cost/tracker.rs` 日切桶 | 迁移当天双计/漏计 | 独立 PR + 数据迁移 |
| 文件名时区不一致 | 影响外部脚本 | 独立 PR |
| memory 存储层 `Local::now()` 写库 | 存储层范畴 | 独立 PR |
| 全局 `[runtime].display_timezone` 配置 | 用户需求是"全部北京"，硬编码即可 | 未来需多时区时再加 |
| **runtime_trace.rs 测试代码 `Utc::now()`**（line 371/399）+ 生产代码 `Local::now()`（line 216） | rev3 实测 line 371/399 在 `#[test] fn` 内、line 216 写 jsonl 但**0 用户/LLM 可见路径** | 独立 PR（chore: unify residual UTC/Local in non-display paths） |
| **memory/hygiene.rs 同文件 Local + Utc 混用** | rev3 实测 line 108-113/122-123 全部 state 文件读写 + cutoff 计算，**0 用户可见** | 独立 PR |
| **skillforge/integrate.rs:84 字面 Z 后缀** | rev3 实测写 TOML manifest metadata 字段，**非显示路径** | 独立 PR |
| 测试代码 `chrono::Local::now()`（orchestrator/mod.rs:9291） | 测试代码 0 OS TZ 风险 | 不动 |
| `fmt_beijing_human` helper（rev2 曾保留） | 0 调用点 + `#[allow(dead_code)]` 直接违反 Anti-Pattern #8 | rev3 删除 |
| `tracing-subscriber` 加 chrono feature（rev1 曾有 Commit） | `BeijingTimer` 自定义 timer 不需要；不必要 Cargo 改动 | rev3 删除 |

**核心原则**：每一项被舍弃的改动都问一句"这能让用户/LLM 看到北京时间吗？" —— 答案是"否" → 不在本计划范围。

---

## 6. 失败应对

- **PR rollback**：单分支线性，`git revert <merge-sha>` 即可。
- **运行时 fallback**：无 —— `chrono_tz::Asia::Shanghai` 是编译期常量。
- **测试 fallback**：现有断言 prompt/output 文本含 `"UTC"` 的测试需同步更新断言（按 §2.4），不算 regression。

---

## 7. 跨工程影响

| Crate | 改动 |
|---|---|
| 根 binary (`src/`) | 新增 `src/time_display.rs`（11 行）；`src/main.rs` 加 1 行 `mod time_display;` + P1.1 (~13 行 BeijingTimer + 1 行 `.with_timer()`) + P2.2 `format_expiry` 2 处；改 `src/cron/mod.rs` 12 处。**`src/util.rs` 不动**。 |
| `zeroclaw-runtime` | **0 新功能**：heartbeat/engine.rs (P1.2)、tools/security_ops.rs (P1.3)、tools/schedule.rs (P3.1, 7 处)、tools/cron_runs.rs (P3.2)、tools/cron_add.rs (P3.3)、tools/delegate.rs (P3.4, 3 处)、agent/{system_prompt,prompt,agent,loop_}.rs (P5 共 7 处) |
| `zeroclaw-channels` | `orchestrator/mod.rs` 1 处生产代码（line 771） |
| 其它 crate | **0 改动** |
| Cargo.toml | **0 改动** |

**总 diff 估计**：+50 / -35 行，跨 ~11 个文件。

---

## 8. Definition of Done

- [x] `feat/beijing-time-display` 分支单 PR  ← 分支 ✅；PR 已合并（用户 2026-05-09 确认；master HEAD = `f85ebf32` = 分支顶 commit，已 fast-forward）
- [x] **6 个 commit** 按上述顺序提交，每个独立通过 `cargo build`  ← e2c5c2ee / 282010e0 / a9344d66 / eeec384a / ed4e292e / f85ebf32
- [x] `./dev/ci.sh all`（或等价三连）全绿  ← scoped 验证通过：runtime + channels + main bin 全绿；workspace build 受 GTK 依赖（screenshot 图形库）阻塞，与时区改动无关
- [x] PR 描述含（rev3.1 实测：3/5 实证 done by orchestrator；2 项交互式留给用户）  ← 用户 2026-05-09 确认完成：
  - **手工实证 1**：`zeroclaw cron list` 含 `+08:00`  ← ✅ 实测 `next=2026-05-08T20:29:30.255629112+08:00`
  - **手工实证 2**：`zeroclaw auth list` 含 `+08:00`  ← ⚠️ N/A（测试机无 OAuth provider；代码路径已 clippy+grep 验证）
  - **手工实证 3**：daemon stderr 首条日志含 `+08:00`  ← ✅ 实测 `2026-05-08T19:28:41.881+08:00 INFO ...`
  - **手工实证 4**：`cargo run -- agent` 问 "现在几点" 截图  ← ⏸ 需要交互式 REPL，留给用户
  - **手工实证 5**：触发 1 次 heartbeat 决策（runtime_trace.jsonl 或 debug log）含 `+0800`/`+08:00`/`CST`，不含字面 ` UTC`  ← ⏸ 需触发 heartbeat，留给用户
  - 6 个 commit 的 grep verify 命令与输出  ← ✅ 已写入 `.sisyphus/notepads/kanmars.req.20260508.001.plan/pr-body.md`
  - rollback 命令  ← ✅ `git revert <merge-sha>` 单线性
  - §5 已知舍弃 1 段  ← ✅ 已写入 PR body
- [x] 0 新增 `unwrap()` / `expect()`  ← `git diff master..HEAD | grep '^+' | grep -E '\.unwrap\(\)|\.expect\(\(' ` 0 hits
- [x] 0 新增 `#[allow(dead_code)]`  ← 0 hits
- [x] runtime crate 内 0 新增 public 函数 / 新模块  ← 仅 API 替换，无新模块/函数
- [x] 生产代码 0 残留 `Local::now()`（bare 与 fully-qualified 两种形式都要查）：
  ```bash
  grep -rnE '(chrono::)?Local::now\(\)' crates/zeroclaw-runtime/src/agent/ crates/zeroclaw-channels/src/orchestrator/mod.rs
  # 实测：仅剩 orchestrator/mod.rs:9291（测试代码）✅
  ```
- [x] `src/util.rs` 未变；**`Cargo.toml` 必要修订**（rev3.1 起草时未预见）：
  ```bash
  diff <(cat src/util.rs) <(echo 'pub use zeroclaw_runtime::util::*;')   # 无输出 ✅
  git diff master -- Cargo.toml crates/*/Cargo.toml
  # 实测：crates/zeroclaw-channels/Cargo.toml +1 行 `chrono-tz = "0.10"`
  # 原因：C6 让 orchestrator/mod.rs 用 chrono_tz::Asia::Shanghai，channels crate 此前无 chrono-tz 依赖路径，必须加；不加则编译失败。
  # 这是合理偏离，subagent 在 C6 主动发现并在 commit message 解释。
  ```
- [x] PR 至少 1 个 approving review  ← 用户 2026-05-09 确认完成（PR 已合并，review 隐含通过）

---

## 9. 重新执行说明（架构变化时）

本计划目标 = "ZeroClaw 在 4 类用户/LLM 直接可见的时间面显示北京时间"。

未来如果：
- 引入 `[runtime].display_timezone` 全局配置 → 把硬编码 `chrono_tz::Asia::Shanghai` 替换为读配置即可
- runtime crate 解耦完成 → 改动随子 crate 迁移，每处都是局部 API 替换，迁移成本接近零

---

## 10. 附录：哲学小结

> **存储用 UTC（科学坐标系），显示用 Asia/Shanghai（用户母语）**。本计划严格在两层之间画边界——任何"顺手优化"、"潜在 BUG 修复"、"一致性 cleanup"都不在本期范围内。

---

## 11. 修订记录

### rev1 → rev2（Momus 第 1 轮审查响应）
修 3 BLOCKER（util.rs 文件冲突 / scout.rs 引用错误 / orchestrator 漏 9291）+ 2 MAJOR（ChronoLocal 矛盾 / struct 类型变更）+ 5 MINOR。删除 rev1 Commit 1（不必要 Cargo feature 改动），commit 总数 8→7。

### rev2 → rev3（Momus "反 scope creep" 复审响应）
**核心修订**：删除 Commit 7（一致性 BUG 修复），commit 总数 7→6。删除 `fmt_beijing_human` 死代码 helper。

| Momus 反馈 | 严重度 | rev3 修订 |
|---|---|---|
| Commit 7 P4.1 (`runtime_trace.rs:371,399` 在测试代码) | 🔴 BLOCKER | 完全删除 P4.1，§5 显式声明独立 PR |
| Commit 7 P4.2 (`hygiene.rs:108-123` 全是 state 文件读写 + 计算) | 🔴 BLOCKER | 完全删除 P4.2 |
| Commit 7 P4.3 (`skillforge/integrate.rs:84` 是 metadata 非显示) | 🔴 BLOCKER | 完全删除 P4.3 |
| Commit 1 `fmt_beijing_human` 0 调用 + `#[allow(dead_code)]` | 🔴 BLOCKER | 删除 `fmt_beijing_human`，仅保留 `fmt_beijing_rfc3339` |
| Commit 1 是否合并到就地展开 | 🟡 可选 | 保留 helper（14 处调用同一表达式确实值得 DRY） |

**rev3 净效果**：
- 6 个 commit（rev2 是 7 个）
- ~11 个文件改动（rev2 是 13 个）
- +50 / -35 行（rev2 是 +85 / -45）
- **每个改动都能直接回答"这让用户/LLM 看到北京时间了吗？" → 答案都是 YES**

### rev3 → rev3.1（Momus 第 3 轮 ACCEPT WITH MINOR PATCHES 响应）

| Momus 反馈 | 严重度 | rev3.1 修订 |
|---|---|---|
| `src/cron/mod.rs` 实测 11 处 `to_rfc3339`，rev3 误写 12 处 | 🟢 MINOR | §1.2 P2.1 / §Commit 2 改动 1 / §Commit 2 校验 grep 三处 `12` → `11`（实测 `grep -c 'to_rfc3339' src/cron/mod.rs` = 11） |
| Commit 6 反向 grep 用 `chrono::Local::now()` 漏 prompt.rs:259 的 bare `Local::now()` 形式 | 🟢 MINOR | §Commit 6 反向 grep / §4 验证矩阵 / §8 DoD 三处改用扩展正则 `grep -nE '(chrono::)?Local::now\(\)'` 覆盖两种形式；§Commit 6 import 同步段补充 prompt.rs 特殊处理（`use chrono::{Datelike, Local, Timelike}` 改完需删 `Local`） |

**Momus 第 3 轮判定**：[ACCEPT WITH MINOR PATCHES] —— rev3.1 应用补丁后**可以执行**，无需再次审查。
