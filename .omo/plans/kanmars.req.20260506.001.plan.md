# Plan — kanmars.req.20260506.001 (Bootstrap files & skills hot-reload for channel runtime)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260506.001.plan |
| 关联需求 | `kanmars.req.20260506.001.md` |
| 起草日期 | 2026-05-06 |
| 修订日期 | 2026-05-06 (rev3：极简化，聚焦关键目标 + 保留 DRY) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `feat/channel-bootstrap-hot-reload` |
| 目标 PR 标题 | `feat(channels): hot-reload bootstrap files and skills on every channel message` |
| 风险等级 | Medium (改动 `zeroclaw-channels`，Experimental tier；零侵入 `zeroclaw-runtime`) |
| 选型方案 | **方案 A（每条消息全量重建）** —— 与 `process_message` 行为对齐 |

---

## 0. 关键目标（唯一的真理来源）

> 用户在飞书 / Slack / 钉钉等任何 channel 上发消息时，**对工作目录下 `AGENTS.md` / `SOUL.md` / `TOOLS.md` / `IDENTITY.md` / `USER.md` / `MEMORY.md` / `BOOTSTRAP.md` 与 `skills/` 目录的修改，下一条消息就生效**，不需要重启 zeroclaw 进程。

完成此目标即"功能完成"。任何不直接服务此目标的改动 = 越权。

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **不在 `zeroclaw-runtime` 增新功能**（其 `AGENTS.md` 明确）。只复用 `build_system_prompt_with_mode_and_autonomy` / `load_skills_with_config` / `skills_to_prompt_with_mode` 既有 public API。
2. **不引入 fs watcher / inotify / 后台轮询**（需求 N5）。
3. **不动 webhook (`process_message`) 与 CLI 路径**（需求 N2/N3）。
4. **不新增 `unwrap()` / `expect()`**（项目 Anti-Pattern #9）。
5. **改动后变成死代码的旧函数必须删**（项目 Anti-Pattern #8）。本计划范围内只涉及 2 个：`refreshed_new_session_system_prompt` 和 `replace_available_skills_section`。**不主动清理** `pending_new_sessions` 系列 —— 它若还服务于 `/new` 等其它路径就保留，不为简洁主动删（"非必要修改"红线）。
6. **DRY**：启动路径与消息重建路径都需要"构造完整 channel system prompt"，**必须抽成同一个函数**，不允许两边各写一遍（这是技术债，不是过度工程）。

---

## 1. 现状事实复核（已实测核对，行号与 HEAD 一致）

| 事实 | 文件:行 |
|---|---|
| `ChannelRuntimeContext` 持 `system_prompt: Arc<String>` + 重建所需的全部依赖（`workspace_dir`、`prompt_config`、`tools_registry`、`provider`、`autonomy_level` 等） | `crates/zeroclaw-channels/src/orchestrator/mod.rs:343-392` |
| 启动构建：`build_system_prompt_with_mode_and_autonomy(...)` + `build_tool_instructions(...)` + 追加 `deferred_section` | 同文件 `:5530-5557` |
| 启动写入 cache | `:5712` |
| **要改的核心点**：消息分支选 cache 还是 refreshed | `:2877-2891` |
| 要删的死代码：`refreshed_new_session_system_prompt`（只刷 skills） | `:1168-1178` |
| 要删的死代码：`replace_available_skills_section`（字符串替换 skills 段） | `:1125-1166` |
| `build_channel_system_prompt`（刷 datetime + 拼 channel 指令）**保留不动** | `:636-686` |
| 真正读 7 个 markdown 的位置 | `crates/zeroclaw-runtime/src/agent/system_prompt.rs:13-36` |
| AIEOS 分支在 `build_system_prompt_with_mode_and_autonomy` 内部，每次调用即重跑 → 方案 A 自然覆盖 | `system_prompt.rs:245-280` |

---

## 2. 设计：方案 A 全量重建

每条消息进来时，**重新调一次启动时同款的 prompt 构造代码**，得到一份新的 base system prompt。再交给现有 `build_channel_system_prompt` 去拼 datetime / channel 指令（保持现有行为）。

为什么够用：
- 7 个 bootstrap 文件总量 ≤ 140 KB，本地 SSD `read_to_string` ms 级
- channel QPS 远低于成本敏感线
- 与 webhook `process_message` 行为对齐，心智模型统一

为什么不选方案 B（mtime 缓存）：实现复杂、`## Current Date & Time` 段已每分钟变 → cache 命中收益边际为 0。

---

## 3. 实施分解

### Commit 1 — `refactor(channels): extract build_channel_runtime_system_prompt helper (DRY for hot-reload)`

**唯一目的**：把 `start_channels:5530-5557` 那一段（计算 `bootstrap_max_chars` + 调 `build_system_prompt_with_mode_and_autonomy` + 追加 `build_tool_instructions` 与 `deferred_section`）抽成一个 crate-private 纯函数，让启动路径和后续 Commit 2 的消息重建路径共用同一份代码。

**函数签名**（一个函数，一个名字，避免命名混乱）：

```rust
fn build_channel_runtime_system_prompt(
    workspace_dir: &std::path::Path,
    config: &zeroclaw_config::schema::Config,
    model: &str,
    tool_descs: &[(&str, &str)],
    skills: &[zeroclaw_runtime::skills::Skill],
    tools_registry: &[Box<dyn Tool>],
    provider: &dyn Provider,
    deferred_section: &str,
) -> String
```

> `compact_context` 与 `max_system_prompt_chars` 直接从入参 `config.agent` 内联读取（与 `bootstrap_max_chars = if compact_context { Some(6000) } else { None }` 取值逻辑一并搬进 helper）—— 减少签名参数数量，反正都从同一个 `Config` 读。

**改动范围**：
- 新增 1 个私有 `fn`（约 20 行，从 `:5530-5557` 平移）
- 启动调用点替换为 `let mut system_prompt = build_channel_runtime_system_prompt(...);` —— 行为字节级等价

**校验**：
1. `cargo build -p zeroclaw-channels` 绿
2. `cargo clippy -p zeroclaw-channels --all-targets -- -D warnings` 绿
3. **行为等价证据**：在 `#[cfg(test)] mod tests` 块新增 1 个测试 `helper_byte_equivalent_to_inline`：
   - 用 tempdir 构造最小 workspace + 默认 `Config`，分别调"抽取前的 inline 代码块（暂时复制到测试体里作为 oracle）"和新 helper，断言 `assert_eq!(left, right)`（**字节级**，不是 `len ==`）
   - 此测试 Commit 3 完成后由 AC 测试覆盖，**可保留也可删，二选一在 PR 描述中说明**

### Commit 2 — `feat(channels): rebuild system prompt from disk on every channel message`

**唯一目的**：让消息分支每次都调 `build_channel_runtime_system_prompt`，从而拿到最新的 bootstrap 文件 + skills + AIEOS。

**改动 1**：`ChannelRuntimeContext` 新增 1 个字段（**仅 1 个**）：

```rust
deferred_mcp_section: Arc<String>,  // 启动时算一次，重建时原样追加
```

启动初始化时把 `start_channels:5554-5557` 已计算的 `deferred_section` 字符串塞进去。

**改动 2**：在 mod.rs 顶层新增一个 ctx-aware 的薄包装（**约 8 行**）：

```rust
fn rebuild_system_prompt_from_disk(ctx: &ChannelRuntimeContext) -> String {
    let skills = zeroclaw_runtime::skills::load_skills_with_config(
        ctx.workspace_dir.as_ref(),
        ctx.prompt_config.as_ref(),
    );
    let tool_descs = build_channel_tool_descs(ctx.prompt_config.as_ref());
    build_channel_runtime_system_prompt(
        ctx.workspace_dir.as_ref(),
        ctx.prompt_config.as_ref(),
        ctx.model.as_str(),
        &tool_descs,
        &skills,
        ctx.tools_registry.as_ref(),
        ctx.provider.as_ref(),
        ctx.deferred_mcp_section.as_str(),
    )
}
```

> `build_channel_tool_descs(config) -> Vec<(&'static str, &'static str)>` —— 把 `start_channels:~5400-5528` 的 tool_descs 构造段也抽成一个函数（DRY，启动路径同时切换到调用它）。值都是 `&'static str` 字面量，零分配。

**改动 3**：`:2877-2881` 替换为：

```rust
// Always rebuild from disk so AGENTS.md / SOUL.md / TOOLS.md / IDENTITY.md /
// USER.md / MEMORY.md / BOOTSTRAP.md edits AND skills/ changes take effect
// on the very next message — matches process_message (webhook) semantics.
//
// IO failure inside rebuild already degrades to "[File not found]" markers
// via inject_workspace_file; if the whole rebuild somehow returns empty,
// fall back to the startup cache.
let rebuilt = rebuild_system_prompt_from_disk(ctx.as_ref());
let base_system_prompt = if rebuilt.is_empty() {
    tracing::warn!(
        error_key = "channel.system_prompt.rebuild_empty",
        "System prompt rebuild produced empty string; falling back to startup cache"
    );
    ctx.system_prompt.as_str().to_string()
} else {
    rebuilt
};
```

**改动 4**：删除 `refreshed_new_session_system_prompt`（`:1168-1178`）和 `replace_available_skills_section`（`:1125-1166`）—— 这两个函数在 Commit 2 后**确定**变成死代码（grep 后无其它调用）。

**不做的事**（聚焦边界）：
- ❌ **不主动清理** `pending_new_sessions` / `mark_sender_for_new_session` / `take_pending_new_session`。先 grep 确认 `:1895` 与 `:2743` 的调用上下文。如果它们只服务于已删的"刷 skills"分支 → 保留 callsite 但 `take_pending_new_session` 的返回值改为 `let _ = ...`（仍清 set），让数据结构维持原状不破坏其它潜在语义；如果还服务 `/new` 命令清 history → 保留全部。**Commit 2 范围内不删**，把"是否清理"留作后续独立 PR 讨论。
- ❌ **不抽** `rebuild_system_prompt_from_disk` 的 ctx 解包成更深层结构。它就是个 8 行的薄包装。

**校验**：
1. `cargo build -p zeroclaw-channels` 绿
2. `cargo clippy -p zeroclaw-channels --all-targets -- -D warnings` 绿
3. **手工 grep 验证**：
   ```bash
   rg 'refreshed_new_session_system_prompt|replace_available_skills_section' crates/
   ```
   只剩 0 个 hit（确认死代码已删干净）

### Commit 3 — `test(channels): cover bootstrap & skills hot-reload (AC-1, AC-3)`

只加 **2 个**单元测试（聚焦关键目标的最小验证集）：

| Test | 覆盖 | 步骤 |
|---|---|---|
| `agents_md_change_visible_on_next_rebuild` | AC-1 / AC-2 代表 | `tempdir + AGENTS.md (无 token) → build_channel_runtime_system_prompt → assert !contains("MAGIC-TOKEN") → append 一行 → rebuild → assert contains` |
| `skill_added_visible_on_next_rebuild` | AC-3 / AC-4 代表 | `tempdir + skills/foo/SKILL.md → rebuild → 不含 unicorn → 写 skills/unicorn/SKILL.md → rebuild → 含 unicorn` |

**为什么只 2 个**：
- 7 个 bootstrap 文件走的是同一个 `inject_workspace_file` 代码路径（`system_prompt.rs:347-388`）→ 测 1 个 = 测 7 个，剩余 6 个由人工 PR 描述附 1 条飞书复现流水兜底
- skills 增删走同一个 `load_skills_with_config` → 测 add 顺带覆盖 remove 的反向
- AC-5 (BOOTSTRAP.md 删除) / AC-6 (USER.md 首次出现) / AC-7 (多 channel 一致) / AIEOS / 字节级守护：**全部不加自动化测试**，由 PR 描述的"reviewer 走读 checklist + 手工 lark 复现"兜底。这是聚焦目标的代价，写明在 §5。

测试位置：`mod.rs` 末尾既有 `#[cfg(test)] mod tests` 块（与现有 `build_system_prompt` 测试同款风格），直接调 helper（无需 mock provider，用最小 stub provider）。

**校验**：
- `cargo test -p zeroclaw-channels --lib` 绿
- 全量 `cargo test` 0 退化

---

## 4. 验证矩阵（极简版）

| AC | 验证方式 | 谁执行 |
|---|---|---|
| AC-1 / AC-2 | 单元测试 `agents_md_change_visible_on_next_rebuild`（1 文件代表 7 文件） | CI |
| AC-3 / AC-4 | 单元测试 `skill_added_visible_on_next_rebuild` | CI |
| AC-5 / AC-6 | **不加自动化测试**。PR 描述附手工复现：删除 BOOTSTRAP.md / 新建 USER.md 各 1 次飞书消息，证明降级与首次加载正常 | 起草人 |
| AC-7 | **代码 review 兜底**：reviewer 确认 `rebuild_system_prompt_from_disk` 与 `build_channel_runtime_system_prompt` 函数体内无 `match msg.channel` / `channel_name == "lark"` 等分叉 | reviewer |
| AC-8 | PR 描述附实测 latency：`time` 跑一次 `build_channel_runtime_system_prompt` 单测，期望 < 50 ms | 起草人 |
| AC-9 | PR 描述 1 段："接受 cache miss 上升换实时性。理由：(a) `## Current Date & Time` 段每分钟已 miss；(b) channel QPS << cache 经济线" | 起草人 |
| AIEOS（req §4 第 5 条） | **不加自动化测试**。在 PR 描述声明："AIEOS 分支位于 `system_prompt.rs:245-280`，每次调 `build_system_prompt_with_mode_and_autonomy` 即重跑 `load_aieos_identity()`，方案 A 自然覆盖" | 起草人 |

预 PR 三连：

```bash
./dev/ci.sh all
# 若不可用，按项目根 AGENTS.md "Commands" 节：
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## 5. 已知舍弃（聚焦目标的代价，PR 描述需明示）

为遵守"非必要无用修改尽量避免"原则，本次**不做**以下事，承担由此带来的边界覆盖度损失：

| 舍弃项 | 理由 | 风险兜底 |
|---|---|---|
| AIEOS 自动化测试 | 自然覆盖 + 用户目标未提及 | 文档化 + 同函数内分支结构稳定 |
| AC-5 / AC-6 自动化测试 | 走的是同一份 `inject_workspace_file` 代码路径 | PR 描述附 2 条手工飞书复现 |
| AC-7 多 channel 静态守护测试 | 守护的是未来不变量 | reviewer code review 兜底 |
| Commit 1 字节级等价测试常驻 | Commit 3 的 AC 测试已能反向验证 | 测试可在 Commit 3 后删（PR 描述说明） |
| 全量 7 文件参数化遍历测试 | 1 文件代表 7 文件（同一代码路径） | 单元测试 1 + 手工 1 = 2 次抽样 |
| `pending_new_sessions` 系列主动清理 | 与关键目标无关，且可能服务 `/new` 命令 | 留作独立 PR，本次不动 |

如果未来某个舍弃项被证明造成问题，按需补一个测试即可（每个测试都是独立可加项，不影响本期合并）。

---

## 6. 失败应对

- **PR rollback**：单分支单线性，`git revert <merge-sha>` 即可。
- **运行时 fallback**：rebuild 返回空串时退化到 `ctx.system_prompt` 启动 cache + `tracing::warn!` 带 `error_key="channel.system_prompt.rebuild_empty"`。
- **不引入** 配置开关（需求 §7 标"不强制"）。

---

## 7. 跨工程影响

| Crate | 改动 |
|---|---|
| `zeroclaw-channels` | `orchestrator/mod.rs`：抽 `build_channel_runtime_system_prompt` + `build_channel_tool_descs` + 加 `rebuild_system_prompt_from_disk` 8 行包装 + 改消息分支 4 行 + 删 `refreshed_new_session_system_prompt` & `replace_available_skills_section` + 加 1 个 ctx 字段 + 新增 2 个单元测试。**总 diff 约 +50 / -80 行**。 |
| 所有其它 crate | **0 改动** |

---

## 8. Definition of Done

- [x] `feat/channel-bootstrap-hot-reload` 分支单 PR  ← 用户 2026-05-09 确认完成（分支已存在 + 已与 master 同步合并）
- [x] 2 个新增单元测试通过（`agents_md_change_visible_on_next_rebuild` + `skill_added_visible_on_next_rebuild`）  ← 用户 2026-05-09 确认完成
- [x] `./dev/ci.sh all`（或等价三连）全绿  ← 用户 2026-05-09 确认完成
- [x] PR 描述含：  ← 用户 2026-05-09 确认完成
  - 飞书手工复现 AC-1（`echo MAGIC-TOKEN-FOO >> AGENTS.md` → 发消息 → 截图证明 prompt 含新 token）—— **这是用户唯一直接验证的证据**
  - 飞书手工复现 AC-3（新增 `skills/unicorn/SKILL.md` → 发消息 → 截图证明 prompt 含 unicorn）
  - 飞书手工复现 AC-5 / AC-6（删 BOOTSTRAP.md / 新建 USER.md，各 1 次）
  - AC-8 latency 数字（`time` 单测耗时）
  - AC-9 cache 影响 1 段声明
  - AIEOS 自然覆盖说明 1 段
  - rollback 命令
- [x] 0 新增 `unwrap` / `expect`  ← 用户 2026-05-09 确认完成
- [x] `refreshed_new_session_system_prompt` & `replace_available_skills_section` 已删（`rg` 0 hit）  ← 实测 grep `crates/zeroclaw-channels/src/` 0 hit ✅
- [x] PR 至少 1 个 approving review  ← 用户 2026-05-09 确认完成（PR 已合并，review 隐含通过）

---

## 9. 重新执行说明（架构变化时）

本计划目标 = "channel 路径下 bootstrap 文件 + skills 修改无需重启即生效"。架构变化时，找到新架构下"channel 消息处理时构造 system prompt"的位置，应用同款"消息分支调启动同款 helper"模式即可。

---

**END of plan (rev3 — 极简化)**
