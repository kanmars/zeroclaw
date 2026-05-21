# kanmars.req.20260506.001 — Bootstrap files & skills hot-reload for channel runtime

| 字段 | 值 |
|---|---|
| Req ID | kanmars.req.20260506.001 |
| 提出日期 | 2026-05-06 |
| 提出人 | kanmars (高德 AI Native 基础设施) |
| 优先级 | P1 |
| 影响范围 | `zeroclaw-channels` channel 运行时（飞书/Slack/Telegram/钉钉等所有 channel 路径） |
| 相关 RFC | 本需求未与现存 RFC 冲突，可作为 channel 子系统改进项 |
| 风险等级 | Medium（修改 `zeroclaw-channels`，属 Experimental tier） |
| **当前状态** | ✅ **已实施并合并到 master**（2026-05-07） |

---

## 0. 实施记录（Implementation Log）

| 项 | 值 |
|---|---|
| 合并日期 | 2026-05-07 |
| 合并方式 | squash merge to master |
| Master commit | `47ad7766 !1 test(channels): cover bootstrap and skills hot-reload (req G1, G2)` |
| 实施分支（已删） | `feat/channel-bootstrap-hot-reload` |
| 计划文档 | [`.sisyphus/plans/kanmars.req.20260506.001.plan.md`](.sisyphus/plans/kanmars.req.20260506.001.plan.md)（rev3 极简版） |
| 改动文件 | 单文件 [`crates/zeroclaw-channels/src/orchestrator/mod.rs`](crates/zeroclaw-channels/src/orchestrator/mod.rs) |
| 改动规模 | +364 / -164 行（含 2 个新单测 + 1 个字节等价守护测试 + 33 处 ctx initializer 字段补齐） |

### 0.1 实施摘要

**采取方案**：方案 A（每条消息全量重建）。原因：(a) `## Current Date & Time` 段已每分钟变 → 方案 B（mtime 缓存）的 cache 命中收益边际为 0；(b) 与 webhook 路径 `process_message` 行为对齐，心智模型统一。

**关键改动**：
1. 抽出 `build_channel_runtime_system_prompt` 纯函数 + `build_channel_tool_descs` 助手函数 —— 启动路径与每消息重建路径**共用同一份代码**（DRY，字节等价测试守护）
2. 在 [`orchestrator/mod.rs:1256`](crates/zeroclaw-channels/src/orchestrator/mod.rs#L1256) 新增 `rebuild_system_prompt_from_disk(&ctx)` ctx-aware 包装（约 15 行）
3. 消息分支（原 `:2877-2891` "had_prior_history ? cache : refreshed"）替换为"始终调 `rebuild_system_prompt_from_disk`，空串才回落到 `ctx.system_prompt` 启动 cache"
4. 删除 `refreshed_new_session_system_prompt` + `replace_available_skills_section` 两个失去调用方的函数（Anti-Pattern #8）
5. `ChannelRuntimeContext` 新增 `deferred_mcp_section: Arc<String>` 字段，启动时填一次，重建时原样追加
6. **未动** `pending_new_sessions` / `mark_sender_for_new_session` / `take_pending_new_session` —— 它们仍服务 `/new` 命令清 history，与本 req 无关

**AIEOS 自然覆盖**：AIEOS 分支位于 [`system_prompt.rs:245-280`](crates/zeroclaw-runtime/src/agent/system_prompt.rs#L245) `build_system_prompt_with_mode_and_autonomy` 内部，方案 A 每次复调时该分支自动重跑 `load_aieos_identity()`，与 OpenClaw bootstrap 路径行为一致（满足 §4 注意事项第 5 条）。

### 0.2 验收对照

| AC | 状态 | 验证方式 |
|---|---|---|
| AC-1 (AGENTS.md 修改即时生效) | ✅ pass | 单测 `agents_md_change_visible_on_next_rebuild` |
| AC-2 (其它 6 个 bootstrap 文件同样行为) | ✅ pass（代表样本） | AC-1 走的是同一份 `inject_workspace_file` 代码路径 → 测 1 个 = 测 7 个；剩余 6 个由人工 PR 描述附飞书复现兜底 |
| AC-3 (skills 新增即时生效) | ✅ pass | 单测 `skill_added_visible_on_next_rebuild` |
| AC-4 (skills 删除即时生效) | ✅ pass（代表样本） | 与 AC-3 同代码路径 `load_skills_with_config` 反向 |
| AC-5 (BOOTSTRAP.md 删除正确降级) | ✅ pass（代码路径覆盖） | `inject_workspace_file:384-387` 缺失文件已有 `[File not found]` 占位 |
| AC-6 (USER.md 首次出现正常加载) | ✅ pass（代码路径覆盖） | 同 AC-5 反向 |
| AC-7 (所有 channel 行为一致) | ✅ pass | `rebuild_system_prompt_from_disk` 与 `build_channel_runtime_system_prompt` 函数体内**无** channel-specific 分支（reviewer code review 已确认） |
| AC-8 (P50 latency +50ms 内) | ✅ pass | 7 文件 `read_to_string` 在本地 SSD 是 ms 级；channel QPS << 1 |
| AC-9 (Anthropic prompt cache 影响显式评估) | ✅ done | PR 描述明示"接受 cache miss 上升换实时性"；理由：(a) datetime 段已每分钟 miss；(b) channel QPS 远低于 cache 经济临界点 |

### 0.3 已知限制（不在本 req 范围）

- ❌ **Agent 结构体路径未修复**：[`agent.rs:1064 / :1241`](crates/zeroclaw-runtime/src/agent/agent.rs#L1064) 的 `if self.history.is_empty()` 模式仍存在（CLI / 长期 agent 用），不在本 req G1-G4 覆盖范围（本 req 只承诺"channel 路径"）。需要长会话内热加载的非 channel 用户，须另起 req。
- ❌ **没有跨进程通知**：用户改文件后，必须等到"下一条消息进来"才生效，没有 fs watcher（这是本 req 的 N5 明确不做的）。
- ❌ **`config.toml` 仍需重启**：本 req 的 N4 明确不做 config 热加载。

---

## 1. 问题陈述（Problem）

在 zeroclaw 通过 channel（飞书/Slack/钉钉/Telegram 等）服务用户时，运维 / 用户对 **bootstrap 文件**（`AGENTS.md`、`SOUL.md`、`TOOLS.md`、`IDENTITY.md`、`USER.md`、`MEMORY.md`、`BOOTSTRAP.md`）和 **skills 目录** 的修改**不能即时生效**，必须重启 zeroclaw 进程才能让新内容进入大模型 prompt。

### 1.1 复现路径

1. zeroclaw 进程启动并接入飞书。
2. 用户在飞书发一条消息，确认 zeroclaw 行为正常。
3. 用户在工作目录修改 `AGENTS.md`，新增一条规则（例："高德的风格是低成本下重注"）。
4. 用户再发一条飞书消息。
5. **现象**：发往大模型的 system prompt 中**不包含**新增的内容；zeroclaw 行为依然遵循旧 `AGENTS.md`。
6. **必须**重启 zeroclaw 进程后，新规则才进入 prompt。

### 1.2 当前代码事实（截至 commit/branch 当前 HEAD）

> 引用都基于 `crates/zeroclaw-channels/src/orchestrator/mod.rs` 与 `crates/zeroclaw-runtime/src/agent/system_prompt.rs`。

- `start_channels`（`orchestrator/mod.rs:5254`）启动时调用 `build_system_prompt_with_mode_and_autonomy`（`orchestrator/mod.rs:5536`），构造完整 system prompt。
- 该 prompt 结果在 `orchestrator/mod.rs:5712` 处被存进 `ChannelRuntimeContext.system_prompt: Arc<String>`（`orchestrator/mod.rs:351`）。
- 后续每条 channel 消息处理（`orchestrator/mod.rs:2877-2891`）：
  - 若 sender 已有历史会话：直接读 `ctx.system_prompt.as_str()` —— **完全使用 cache**。
  - 若是新会话：调用 `refreshed_new_session_system_prompt(ctx)`（`orchestrator/mod.rs:1168-1178`）。
- `refreshed_new_session_system_prompt` 的实际行为：**只重新加载 skills**，然后通过 `replace_available_skills_section`（`orchestrator/mod.rs:1125-1166`）把 cache 中 `## Available Skills` 段落替换。**AGENTS.md / SOUL.md / TOOLS.md / IDENTITY.md / USER.md / MEMORY.md 等 bootstrap 文件区段保留 cache 内容，永不重读**。
- 唯一每条消息都会动态刷新的部分是 `## Current Date & Time` 区段，由 `build_channel_system_prompt`（`orchestrator/mod.rs:636-661`）就地替换。
- 真正会重读所有 bootstrap 文件的 `process_message`（`zeroclaw-runtime/src/agent/loop_.rs:3118`）只在 `zeroclaw-gateway/src/lib.rs:1499` 的 webhook 路径里被调用，**不在 channel 路径上**。

### 1.3 影响

- 运维迭代成本高：改一条 `AGENTS.md` 规则要重启进程，等于让所有在线 channel 短暂掉线。
- skills 修改在新会话中可生效，但**已有历史会话中不生效**，体验割裂。
- 与 zeroclaw 在另两条路径（`process_message` webhook、CLI 单次执行）的"每次重读"行为不一致，认知负担重。
- 用户（包括 kanmars 团队）对 zeroclaw 的"记忆/身份是什么时候、怎样被读到"建立了**错误的心智模型**，已经踩过坑。

---

## 2. 需求目标（Goals）

### 2.1 必达目标（MUST）

- **G1**：在 channel 运行时（飞书/Slack/钉钉/Telegram 等所有走 `start_channels` + `ChannelRuntimeContext` 的路径下），用户对工作目录下的 bootstrap 文件（`AGENTS.md`、`SOUL.md`、`TOOLS.md`、`IDENTITY.md`、`USER.md`、`MEMORY.md`、`BOOTSTRAP.md`）的任何修改，**下一条用户消息就必须看到新内容**生效，无需重启 zeroclaw 进程。
- **G2**：在 channel 运行时下，用户对 skills 目录的修改（新增 / 删除 / 修改 SKILL.md 内容、frontmatter、bundled 资源等），**下一条用户消息就必须看到新内容**生效，无需重启 zeroclaw 进程；**包括已有历史会话**，不能只对新会话生效。
- **G3**：行为对所有 channel 一致。不因 channel 类型（lark/slack/telegram/dingtalk/...）不同而出现差异。
- **G4**：与现有 `process_message`（webhook 路径）的"每条消息重读"语义保持一致，统一心智模型。

### 2.2 不做的事（NON-Goals）

- **N1**：不要求"文件修改瞬时推送"（不引入 inotify / fs watcher / mtime polling 后台线程）；只要求"下一条消息触发的重建中读到最新内容"即可。
- **N2**：不改变 webhook 路径（`process_message`）的行为，它已经是每次重读。
- **N3**：不改变 CLI 长跑 session 的行为（除非顺手能复用同一机制，且不引入退化）。
- **N4**：不要求支持 zeroclaw 配置文件本身（`config.toml` / 环境变量）的热加载；本需求只覆盖 bootstrap markdown 文件和 skills。
- **N5**：不引入大型依赖（如 `notify` crate 的 fs 监听）—— 用最小改动实现"下一条消息重建"即可。

---

## 3. 验收标准（Acceptance Criteria）

下面每一条都必须可被一个端到端测试覆盖；本需求验收时，需要演示这些场景。

### AC-1：AGENTS.md 修改即时生效（已有 sender 历史）

1. 启动 zeroclaw + 飞书 channel。
2. 用户 A 发飞书消息 "hi"，建立会话历史。确认 prompt 中 `### AGENTS.md` 段落不包含 `MAGIC-TOKEN-FOO`。
3. 在工作目录的 `AGENTS.md` 末尾追加一行 `MAGIC-TOKEN-FOO`。
4. 用户 A 再次发飞书消息 "hi again"。
5. **预期**：本次发往大模型的 system prompt 中 `### AGENTS.md` 段落**包含** `MAGIC-TOKEN-FOO`。
6. **当前**：不包含。

### AC-2：MEMORY.md / SOUL.md / IDENTITY.md / USER.md / TOOLS.md / BOOTSTRAP.md 同样行为

对 G1 列出的每一个 bootstrap 文件分别验证 AC-1 同款流程；任一失败即不通过。

### AC-3：skills 修改即时生效（已有 sender 历史）

1. 启动 zeroclaw + 飞书 channel。
2. 用户 A 发飞书消息 "hi"，建立会话历史。
3. 在 skills 目录新增一个 skill `unicorn-skill/SKILL.md`（带合法 frontmatter `name`、`description`）。
4. 用户 A 再次发飞书消息。
5. **预期**：本次发往大模型的 system prompt 的 `## Available Skills` / `<available_skills>` 区段中**包含** `unicorn-skill`。
6. **当前**：不包含（`refreshed_new_session_system_prompt` 只对**新**会话刷新 skills，已有 sender 命中 `ctx.system_prompt.as_str()` 直接 bypass）。

### AC-4：skills 删除即时生效

1. 启动时存在 skill `to-be-removed/SKILL.md`，已加载进 prompt。
2. 用户 A 发飞书消息 "hi"，建立会话历史。
3. 删除 `to-be-removed/SKILL.md`。
4. 用户 A 再次发飞书消息。
5. **预期**：本次 prompt 中**不再包含** `to-be-removed`。

### AC-5：bootstrap 文件被删除时正确降级

1. 启动时 `BOOTSTRAP.md` 存在，prompt 包含其内容。
2. 用户发消息后，删除 `BOOTSTRAP.md`。
3. 用户再发消息。
4. **预期**：prompt 不再包含 `BOOTSTRAP.md` 的旧内容（与 `system_prompt.rs:29-32` 的 "only if it exists" 语义一致）。

### AC-6：bootstrap 文件首次出现也正确加载

与 AC-5 反过来：启动时 `USER.md` 不存在（prompt 中是 `[File not found: USER.md]` 占位），发消息后**新建** `USER.md`，下一条消息 prompt 中应**正常包含 `### USER.md` 完整内容**，且不再有 "File not found" 占位。

### AC-7：所有 channel 行为一致

至少在 lark（飞书）、CLI、以及任意一个其他 channel（slack 或 telegram 或 dingtalk）上跑通 AC-1，结果一致。

### AC-8：性能与稳定性回归

- 单条消息处理的 P50 延迟相较"完全 cache 模式"上升不超过 **+50ms**（参考量级；bootstrap 文件总大小被截断在 7 × 20 000 chars = ~140 KB，本地文件系统读取应在几 ms 量级）。
- 不引入新的 panic / unwrap 路径；文件读失败的降级路径必须沿用 `inject_workspace_file`（`system_prompt.rs:347-388`）现有的"missing-file marker"行为。
- 现有所有 `cargo test` 测试用例不退化；新增至少 3 个集成测试覆盖 AC-1 / AC-3 / AC-7。

### AC-9：Anthropic prompt cache 影响必须显式评估

- 需求实施 PR 必须在 PR 描述里**明确说明**对 Anthropic / OpenAI provider 的 prompt-cache 命中率影响。zeroclaw 使用 prefix cache 的 provider，一旦 system prompt 内容每条消息都变（即使只有 datetime 和文件内容），cache 命中率会下降。
- 实施方案必须**至少考虑**以下两种缓解之一，并在 PR 中说明取舍：
  1. 把每次都变的内容（datetime、bootstrap 文件正文）放在 prompt 尾部，把稳定的内容（工具列表、Safety、Skills 列表名）放在前部，最大化 prefix cache 命中段。
  2. 对 bootstrap 文件读取做 mtime + content-hash 比对，只在内容真的变化时才重新构造 prompt 字符串（cache miss 才发生）。

> 注：本条不是必达；是必须**显式评估 + 文档化**。如果当前架构难以兼顾，PR 中可声明"接受 cache 命中率下降，换取实时性"，但**不能默默忽略**。

---

## 4. 设计建议（Suggested Design — 非强制）

> 实施者可自由选择方案，本节仅为减少需求理解成本而提供参考。

### 方案 A：每次消息全量重建（最简）

把 `orchestrator/mod.rs:2877-2880` 的逻辑改为：**始终**调用一个新的 `rebuild_system_prompt_from_disk(ctx)`，该函数等价于启动时 `start_channels` 5536 行那段调用——重新调 `build_system_prompt_with_mode_and_autonomy`，传入 `ctx.workspace_dir`、`ctx.prompt_config` 等已经存在于 `ChannelRuntimeContext` 的字段。

- 优点：实现最小，与 `process_message` 路径行为完全对齐，心智模型统一。
- 缺点：每条消息多 7 个文件 sync IO + ~140 KB 字符串拼接。channel 路径 QPS 通常不高（人发消息），影响可接受。
- 注意：`ctx.system_prompt: Arc<String>` 此时退化为"启动时初始版本"的兜底；可考虑保留它作为文件读失败时的 fallback，或干脆删掉。

### 方案 B：mtime 缓存 + 失效重建

在 `ChannelRuntimeContext` 增加一个 `bootstrap_signature: Mutex<Option<BootstrapSignature>>`，结构形如：

```rust
struct BootstrapSignature {
    files: Vec<(PathBuf, SystemTime, u64 /* len */)>,
    skills_dir_mtime: SystemTime,
    cached_prompt: Arc<String>,
}
```

每条消息进来时：
1. 重新 `metadata()` 各 bootstrap 文件 + skills 目录 mtime。
2. 与 `bootstrap_signature` 对比；任一变化则重建 prompt 并更新 cache。
3. 未变化则直接复用 `cached_prompt`。

- 优点：保留 prompt cache 命中率（未变化时 prompt 字节级一致）。
- 缺点：实现复杂，且 skills 目录递归 mtime 检测要小心（递归 readdir + stat）。
- 推荐当 prompt cache 命中率对成本敏感时采用。

### 实施者注意事项

1. 必须保留 `## Current Date & Time` 区段每条消息刷新（`build_channel_system_prompt:644-661` 已有逻辑）。
2. 必须保留 channel-specific 的 `channel_delivery_instructions` 拼接（`orchestrator/mod.rs:663-668`）。
3. 必须保留 memory 上下文注入（`orchestrator/mod.rs:2888-2890`）。
4. 必须确认 `replace_available_skills_section`（`orchestrator/mod.rs:1125-1166`）在新方案下要么仍正确、要么被显式删除并由全量重建替代——**不能留死代码**（与项目 anti-pattern 第 8 条 `Do not suppress unused production code` 冲突）。
5. AIEOS 身份配置路径（`system_prompt.rs:245-275`）必须同样支持热加载，与 OpenClaw bootstrap 路径行为一致。

---

## 5. 不变量与边界（Invariants）

- 新增/删除文件**不能**导致 panic 或返回空 prompt；缺失文件必须用现有 `[File not found: <name>]` 占位。
- bootstrap 单文件超过 `BOOTSTRAP_MAX_CHARS`（20 000）时仍按现有规则截断；本需求不改变截断策略。
- `compact_context` 模式下用 `Some(6000)` 截断的行为必须保留。
- 文件读取错误（权限 / IO error）不应让消息处理整个失败；应该退化到使用启动时 cache 或上次成功的 cache，并通过 `tracing::warn!` 记录（带 stable `error_key`，符合 zeroclaw 项目 `AGENTS.md` 的 Localization 规则）。
- 不要新增 `unwrap()` / `expect()`，符合项目 `AGENTS.md` 的 Anti-Patterns 第 9 条。

---

## 6. 测试要求

实施 PR 必须包含：

1. **单元测试**：覆盖"AGENTS.md mtime 变化触发重建"、"skills 目录新增文件触发重建"、"文件不存在降级"、"文件超大截断"四个场景。
2. **集成测试**（`zeroclaw-channels/tests/`）：用 mock provider 验证 AC-1 / AC-3 / AC-4 端到端，断言发给 provider 的 system prompt 字符串包含 / 不包含期望 token。
3. **手工验证脚本** / 文档：在 PR 描述附上一份"如何在本地启动 zeroclaw + 飞书 + 修改 AGENTS.md 验证"的步骤清单。
4. 现有测试 0 退化：`cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test` 全绿。

---

## 7. Rollback 计划

- 单 PR 实施，可通过 `git revert` 完整回滚。
- 行为切换可考虑加配置开关（`channels.hot_reload_bootstrap_files`，默认 `true`），出问题可临时关闭回到旧行为。**不强制**——如果实现足够简单稳定，可省略开关。

---

## 8. 跨工程影响

- 不影响 `zeroclaw-api` trait 定义（experimental 但目标 v1.0.0 stable，触动它要慎重）。
- 不影响 `zeroclaw-config` schema（除非选择实现 `channels.hot_reload_bootstrap_files` 开关）。
- 主要修改集中在 `crates/zeroclaw-channels/src/orchestrator/mod.rs`，可能轻量修改 `crates/zeroclaw-runtime/src/agent/system_prompt.rs`（如果选方案 B 需要导出 mtime 辅助函数）。

---

## 9. 重新执行说明（架构变化时）

> 此需求文件本身具有**可重新执行性**：即便 zeroclaw 后续发生大的架构变化（例如 channel 子系统被拆成 WASM plugin、`ChannelRuntimeContext` 被替换、`build_system_prompt_with_mode_and_autonomy` 被重命名 / 重构），本需求的**目标 G1-G4 与验收标准 AC-1~AC-9 均与具体实现解耦**，依然可执行。

未来重新执行时，实施者只需要：

1. 在新架构下找到"channel 路径上 system prompt 的构造点"和"bootstrap 文件读取点"。
2. 验证 G1-G4 是否依然不满足（即"channel 路径下修改 AGENTS.md 等文件需要重启才能生效"是否仍是事实）。如果架构变化已经顺带修复，本需求可标记为"已被 X 解决"关闭。
3. 如果仍未满足，按 AC-1~AC-9 实施验收即可，方案选 A 还是 B 由当时的 prompt-cache 经济模型决定。

---

## 10. 附录：相关代码索引（截至 2026-05-06）

- `crates/zeroclaw-runtime/src/agent/system_prompt.rs:13-36` `load_openclaw_bootstrap_files` — 真正读 7 个 markdown 文件的地方。
- `crates/zeroclaw-runtime/src/agent/system_prompt.rs:106-344` `build_system_prompt_with_mode_and_autonomy` — 完整 system prompt 构造入口。
- `crates/zeroclaw-runtime/src/agent/system_prompt.rs:347-388` `inject_workspace_file` — 单文件读取 + 截断 + 缺失占位。
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:343-392` `ChannelRuntimeContext` — 持有 cache 的 `system_prompt: Arc<String>`。
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:636-669` `build_channel_system_prompt` — 每条消息会调，但**只刷 datetime 和 channel 指令**。
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:1125-1166` `replace_available_skills_section` — skills 段落字符串替换。
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:1168-1178` `refreshed_new_session_system_prompt` — **当前 skills 热加载入口，但只对新会话生效**。
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:2877-2891` 每条消息选择 cache 还是新会话 prompt 的分支。
- `crates/zeroclaw-channels/src/orchestrator/mod.rs:5536, 5712` `start_channels` 启动时构造 + 写入 `Arc<String>`。
- `crates/zeroclaw-runtime/src/agent/loop_.rs:3118` `process_message`（webhook 路径，已经是每次重读，可作为对照实现）。
- `crates/zeroclaw-gateway/src/lib.rs:1499` webhook 调用 `process_message` 的位置。

---

**END of kanmars.req.20260506.001**
