# Plan — kanmars.req.20260625.001 (Merge master into kanmars_main)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260625.001.plan |
| 关联需求 | 用户对话需求（2026-06-25）：将 master 分支最新更新合并到 kanmars_main 分支，消除自上次 merge base（`5a0d4f24`）以来积累的 370 个上游 commit 差异。master 包含约 200 个功能/修复 commit（635 文件，+125,689/-222,754 行），kanmars_main 有 14 个独有 commit（48 文件，+13,003/-6 行）。实测 `git merge --no-commit --no-ff master` 产生 3 个冲突文件（CHANGELOG-next.md、schema.rs、loop_.rs），均为中等复杂度，无不可调和的架构冲突。kanmars_main 的两个核心功能（`classifier_provider`、`max_context_window` 继承链）已被上游吸收，merge 后这些代码变为冗余，保留 master 版本即可。config.toml 零修改（schema_version 3 匹配，所有字段兼容）。 |
| 起草日期 | 2026-06-25 |
| 修订日期 | 2026-06-25 (rev0 — 初稿) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `kanmars_main`（直接在当前分支上 merge master） |
| 风险等级 | **Medium**（跨 Beta tier `zeroclaw-config` schema.rs 冲突 + Experimental tier `zeroclaw-runtime` loop_.rs 冲突；上游已吸收 kanmars_main 核心功能；config.toml 无需修改；3 个冲突文件均为文本级合并，无语义不可调和） |
| 基线 commit | `2838c4a2`（kanmars_main HEAD @ 2026-06-25）/ `c8c2921d`（master HEAD @ 2026-06-25） |
| Merge base | `5a0d4f24` |
| 预计工作量 | **AI agent 执行约 60-90 min**（含冲突解决 + cargo build/clippy/test wall-clock 15-20 min） |

---

## 0. 关键目标（唯一真理来源）

> **将 master 的 370 个 commit 合并到 kanmars_main，解决 3 个冲突文件，去除 kanmars_main 中已被上游吸收的冗余代码，确保编译通过、测试通过、config.toml 零修改即可运行。**

**完成此目标即"功能完成"**：

- `git merge master` 成功完成，3 个冲突文件全部解决
- kanmars_main 中被上游吸收的冗余代码（`classifier_provider`、`max_context_window`、`resolved_max_context_tokens`、`DEFAULT_MAX_CONTEXT_TOKENS`）保留 master 版本
- kanmars_main 独有功能（`build.rs` 编译时注入、`time_display` 模块、`post_compaction_context` 模块、CLI version 格式增强、`.sisyphus/` 元数据、`README.kanmars.md`）全部保留
- `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test --workspace` 全绿
- `/home/admin/workspace-public/kanmars/gloria/A/malorian-3516/config.toml` 零修改即可被 merge 后的二进制正确加载

**显式不在范围内**：

- ❌ 不修改 config.toml（schema_version 3 匹配，所有字段兼容，新增字段全部有 `#[serde(default)]`）
- ❌ 不向 master 提 PR（本次是 fork 内部 merge，吸收上游更新）
- ❌ 不做 squash merge（保留 master 的完整 commit history）
- ❌ 不删除 `.sisyphus/` 元数据（boulder/plans 是元数据，不是代码）
- ❌ 不删除 `README.kanmars.md`、`kanmars.req.*.md` 需求文档
- ❌ 不动 `kanmars_main.md`
- ❌ 不实施 master 新增功能的配置启用（A2A、eval harness、SOP 持久化等 — 这些是后续独立决策）
- ❌ 不修改 kanmars_main 独有的 `build.rs`（编译时注入 build-time/git-commit/rustc-version）
- ❌ 不修改 kanmars_main 独有的 `src/time_display.rs`、`src/agent/post_compaction_context.rs`

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **SINGLE SOURCE OF TRUTH 铁律**（AGENTS.md ABSOLUTE RULE）：
   - kanmars_main 的 `classifier_provider` 和 `max_context_window` 在上游已有等价实现 → 保留 master 版本，删除 kanmars_main 的重复定义 ✅ 合规
   - merge 后不新增任何 duplicate state ✅ 合规
2. **不破坏现有运行时 config**：
   - config.toml 的 `schema_version = 3` 与 master `CURRENT_SCHEMA_VERSION = 3` 匹配
   - 所有现有 config 字段在 master schema 中有 `#[serde(default)]` 兜底
   - 新增字段（`summary_provider`、`delegates`、`a2a` 等）不填不影响运行
3. **不丢任何 kanmars_main 独有功能**：
   - `build.rs` 编译时注入 → 保留
   - `time_display` 模块 → 保留
   - `post_compaction_context` 模块 → 保留
   - CLI version 格式增强（build-time + git-commit + rustc-version）→ 保留
   - `.sisyphus/` 元数据 → 保留
4. **不新增 `unwrap()` / `expect()`**（AGENTS.md Anti-Patterns）
5. **`tracing::` 日志保持英文**（RFC #5653 §4.6）

---

## 1. 前置知识

### 1.1 分支状态快照

| 指标 | master | kanmars_main |
|---|---|---|
| HEAD commit | `c8c2921d` | `2838c4a2` |
| 自 merge base 的 commit 数 | 370 | 14 |
| 自 merge base 的文件变更数 | 635 | 48 |
| 自 merge base 的行变更 | +125,689 / -222,754 | +13,003 / -6 |
| Merge base | `5a0d4f24` | `5a0d4f24` |

### 1.2 冲突文件清单（实测）

| 文件 | 冲突原因 | 解决策略 |
|---|---|---|
| `CHANGELOG-next.md` | 两边都有 changelog 追加 | 保留 master 版本 + 追加 kanmars_main 条目 |
| `crates/zeroclaw-config/src/schema.rs` | kanmars_main 加了 `max_context_window` + `classifier_provider` + `max_context_tokens` + `resolved_max_context_tokens`，master 也加了同名字段 | 保留 master 版本（上游实现更完整，有 `summary_provider`、`delegates` 等配套字段） |
| `crates/zeroclaw-runtime/src/agent/loop_.rs` | kanmars_main 改了 4 处调用，master 对同一文件做了 10,771 行级重构 | 保留 master 版本（重构后代码结构完全不同，kanmars_main 的 4 处改动在 master 上已被等价实现覆盖） |

### 1.3 kanmars_main 独有改动清单（merge 后必须保留）

| 文件 | 改动内容 | 保留方式 |
|---|---|---|
| `build.rs` | 编译时注入 `ZEROCLAW_BUILD_TIME` / `ZEROCLAW_GIT_COMMIT` / `ZEROCLAW_RUSTC_VERSION` 环境变量 | git 自动保留（master 无此文件） |
| `src/time_display.rs` | 时间显示模块（8 行） | git 自动保留（master 无此文件） |
| `src/agent/post_compaction_context.rs` | compaction 后上下文恢复模块（176 行） | git 自动保留（master 无此文件） |
| `src/main.rs` | CLI version 格式增强（`concat!` 注入 build-time 等）+ `mod time_display` + `mod agent` | 手工合并（保留 kanmars_main 的增强 + master 的其他 main.rs 改动） |
| `src/agent/mod.rs` | `pub mod post_compaction_context;`（2 行） | git 自动保留（master 无此文件） |
| `.sisyphus/` | 14 个 plan/notepad/boulder 元数据文件 | git 自动保留（master 无此目录） |
| `README.kanmars.md` | kanmars fork 说明文档（1003 行） | git 自动保留（master 无此文件） |
| `kanmars.req.*.md` | 需求文档（1 个文件） | git 自动保留（master 无此文件） |
| `kanmars_main.md` | 分支说明文档（20 行） | git 自动保留（master 无此文件） |
| `CHANGELOG-next.md` | kanmars_main 追加的 2 行 changelog | 手工合并（冲突文件） |
| `crates/zeroclaw-config/src/schema.rs` | kanmars_main 加的 `max_context_window` + `classifier_provider` + `max_context_tokens` + `resolved_max_context_tokens` + 6 个测试 | **丢弃**（master 已有等价实现） |
| `crates/zeroclaw-runtime/src/agent/loop_.rs` | kanmars_main 改的 4 处 `effective_max_context_tokens` → `resolved_max_context_tokens_for_agent` | **丢弃**（master 重构后已使用等价 API） |

### 1.4 已被上游吸收的 kanmars_main 功能

| kanmars_main 功能 | master 等价实现 | master 位置 |
|---|---|---|
| `ModelProviderConfig.max_context_window: Option<usize>` | 同名字段 | `schema.rs` 行 10587 |
| `AliasedAgentConfig.classifier_provider: ModelProviderRef` | 同名字段 | `schema.rs` 行 3420 |
| `AliasedAgentConfig.max_context_tokens: Option<usize>` | 同名字段 | `schema.rs` 行 3264 |
| `AliasedAgentConfig::resolved_max_context_tokens()` | 同名方法 | `schema.rs` 行 3728 |
| `Config::resolved_max_context_tokens_for_agent()` | 同名方法 | `schema.rs` 行 3404 |
| `DEFAULT_MAX_CONTEXT_TOKENS: usize = 32_000` | 同名常量 | `schema.rs` |
| `loop_.rs` 4 处调用替换 | master 重构后已使用等价 API | `loop_.rs` 各处 |

### 1.5 Master 关键新功能（影响 merge 后行为的变更）

| 功能 | 对 kanmars_main 的影响 |
|---|---|
| Agent turn 引擎统一（`#7540`） | `loop_.rs` 结构大改，kanmars_main 的 4 处改动位置已不存在 |
| History pruning 重写（`#8196`） | `context_compressor.rs` 已删除，config.toml 中 `context_compression` section 字段仍有 `#[serde(default)]` 但运行时语义已变 |
| Provider dispatch 归因（`#7748`） | 新增 `dispatch.rs`，不影响 config |
| Control Plane 模块 | 新增 `control_plane/` 目录，不影响 config |
| A2A Agent Discovery（`#7763`） | 新增 `[agents.<alias>.a2a]` config section（`#[serde(default)]`，不填不生效） |
| SOP SQLite 持久化（`#8206`） | `[sop]` section 扩展字段（`#[serde(default)]`，不填不生效） |
| Per-agent delegate roster（`#7590`） | 新增 `delegates` + `delegate_same_risk_profile` 字段（`#[serde(default)]`） |
| Summary provider | 新增 `summary_provider` 字段（`#[serde(default)]`） |
| Skill manage tool | 新增 `skill_manage.rs`，不影响 config |
| Observability trace_id（`#8065`） | 日志按 trace_id 关联 + cost_usd 记录，不影响 config |
| Cost budget reloadable（`#8004`） | `[cost]` section 运行时行为变化，config 格式不变 |
| Cron in-flight lock（`#8107`） | scheduler 防重复启动，不影响 config |

---

## 2. 实施步骤

### Wave 1：Pre-merge 准备（预计 5 min）

#### Task 1.1：创建 merge 分支 + 备份

```bash
# 确保 kanmars_main 是最新的
git checkout kanmars_main
git status  # 确认 working tree clean

# 创建 merge commit 分支（直接在 kanmars_main 上操作）
# 不需要新建分支 — 用户要求在 kanmars_main 上直接 merge
```

**验证**：`git status` 显示 `nothing to commit, working tree clean`

#### Task 1.2：备份 config.toml

```bash
cp /home/admin/workspace-public/kanmars/gloria/A/malorian-3516/config.toml \
   /home/admin/workspace-public/kanmars/gloria/A/malorian-3516/config.toml.backup-pre-merge-$(date +%Y%m%d-%H%M%S)
```

**验证**：备份文件存在且内容与源文件一致（`diff` 无输出）

### Wave 2：执行 Merge + 解决冲突（预计 30-40 min）

#### Task 2.1：执行 git merge

```bash
git merge master
```

**预期结果**：3 个冲突文件，merge 暂停等待解决。

#### Task 2.2：解决冲突 — CHANGELOG-next.md

**策略**：保留 master 版本为主体，在适当位置追加 kanmars_main 的 2 行 changelog 条目。

**步骤**：
1. 打开冲突文件，定位 `<<<<<<<` / `=======` / `>>>>>>>` 标记
2. 保留 master 侧的完整 changelog 内容
3. 在 master 内容的适当位置（按时间顺序）插入 kanmars_main 的条目：
   - `feat(config): inherit agent.max_context_tokens from model.max_context_window`
   - `feat(agents): add per-agent classifier_provider to route reply-intent precheck to a cheaper model`
4. 删除所有冲突标记

**验证**：`grep -c '<<<<<<' CHANGELOG-next.md` 输出 `0`

#### Task 2.3：解决冲突 — crates/zeroclaw-config/src/schema.rs

**策略**：**保留 master 版本**。kanmars_main 的所有改动（`max_context_window`、`classifier_provider`、`max_context_tokens`、`resolved_max_context_tokens`、`DEFAULT_MAX_CONTEXT_TOKENS`、6 个测试）在 master 上已有等价且更完整的实现。

**步骤**：
1. 对于每个冲突 hunk，选择 master 侧（`>>>>>>> master`）的内容
2. 删除所有冲突标记
3. 确认 kanmars_main 独有的 6 个测试函数不在文件中（master 有自己的等价测试）

**验证**：
- `grep -c '<<<<<<' crates/zeroclaw-config/src/schema.rs` 输出 `0`
- `grep 'classifier_provider' crates/zeroclaw-config/src/schema.rs` 有输出（master 版本存在）
- `grep 'max_context_window' crates/zeroclaw-config/src/schema.rs` 有输出（master 版本存在）
- `grep 'resolved_max_context_tokens' crates/zeroclaw-config/src/schema.rs` 有输出（master 版本存在）

#### Task 2.4：解决冲突 — crates/zeroclaw-runtime/src/agent/loop_.rs

**策略**：**保留 master 版本**。master 对此文件做了 10,771 行级重构（turn/ 子模块拆分、ResolvedAgentExecution 等），kanmars_main 的 4 处 `effective_max_context_tokens` → `resolved_max_context_tokens_for_agent` 替换在 master 重构后的代码中已被等价实现覆盖。

**步骤**：
1. 对于每个冲突 hunk，选择 master 侧的内容
2. 删除所有冲突标记

**验证**：
- `grep -c '<<<<<<' crates/zeroclaw-runtime/src/agent/loop_.rs` 输出 `0`
- `grep 'resolved_max_context_tokens_for_agent\|effective_max_context_tokens' crates/zeroclaw-runtime/src/agent/loop_.rs` — 确认 master 版本的 API 调用存在

#### Task 2.5：检查 src/main.rs 是否自动合并成功

`src/main.rs` 不在冲突列表中，但 kanmars_main 对此文件有改动（CLI version 格式增强 + `mod time_display`）。需要确认 git 自动合并结果正确。

**验证**：
- `grep 'ZEROCLAW_BUILD_TIME' src/main.rs` 有输出（kanmars_main 的增强保留）
- `grep 'mod time_display' src/main.rs` 有输出（kanmars_main 的模块声明保留）
- `grep -c '<<<<<<' src/main.rs` 输出 `0`（无残留冲突标记）

#### Task 2.6：Stage 所有解决后的文件

```bash
git add CHANGELOG-next.md crates/zeroclaw-config/src/schema.rs crates/zeroclaw-runtime/src/agent/loop_.rs
```

### Wave 3：验证编译 + 测试（预计 20-30 min）

#### Task 3.1：cargo fmt

```bash
cargo fmt --all -- --check
```

**预期**：退出码 0。如果有格式化问题，执行 `cargo fmt --all` 修复后重新检查。

#### Task 3.2：cargo clippy

```bash
cargo clippy --all-targets -- -D warnings
```

**预期**：退出码 0，无 warning。

**如果 clippy 报错**：
- 如果报错来自 master 代码（非 kanmars_main 独有文件）→ 记录但不修复（上游问题，不属于本次 merge 范围）
- 如果报错来自 kanmars_main 独有文件（`build.rs`、`time_display.rs`、`post_compaction_context.rs`）→ 修复

#### Task 3.3：cargo test

```bash
cargo test --workspace
```

**预期**：全部通过。

**如果测试失败**：
- 如果失败来自 master 代码 → 记录但不修复（上游问题）
- 如果失败来自 kanmars_main 独有模块 → 修复
- 如果失败来自 schema.rs 的 kanmars_main 测试（已被丢弃的 6 个测试）→ 确认 master 版本的等价测试通过即可

#### Task 3.4：config.toml 兼容性验证

```bash
# 确认 config.toml 未被修改
diff /home/admin/workspace-public/kanmars/gloria/A/malorian-3516/config.toml \
     /home/admin/workspace-public/kanmars/gloria/A/malorian-3516/config.toml.backup-pre-merge-*
```

**预期**：无差异（config.toml 零修改）。

### Wave 4：提交 Merge Commit（预计 5 min）

#### Task 4.1：提交

```bash
git commit -m "chore: merge upstream master into kanmars_main (2026-06-25)

Merge 370 upstream commits from master (5a0d4f24..c8c2921d) into kanmars_main.

Conflict resolution:
- CHANGELOG-next.md: kept master content, appended kanmars_main entries
- schema.rs: kept master version (kanmars_main features absorbed upstream)
- loop_.rs: kept master version (10K-line refactor covers kanmars_main changes)

kanmars_main unique features preserved:
- build.rs compile-time injection (build-time/git-commit/rustc-version)
- src/time_display.rs module
- src/agent/post_compaction_context.rs module
- CLI version format enhancement
- .sisyphus/ metadata
- README.kanmars.md

Config compatibility: schema_version 3 matches, all fields compatible,
zero changes required to existing config.toml."
```

#### Task 4.2：最终验证

```bash
git log --oneline -5
git status  # 确认 clean working tree
```

---

## 3. 冲突解决决策矩阵

| 冲突文件 | kanmars_main 侧内容 | master 侧内容 | 决策 | 理由 |
|---|---|---|---|---|
| `CHANGELOG-next.md` | 2 行 kanmars 条目 | 277 行上游 changelog | **合并**（保留 master + 追加 kanmars） | 两边的 changelog 都是事实记录，不互斥 |
| `schema.rs` | +194 行（`max_context_window` + `classifier_provider` + `max_context_tokens` + `resolved_max_context_tokens` + 6 测试） | +5916 行（全面重构，含同名功能 + `summary_provider` + `delegates` + `a2a` 等） | **保留 master** | master 实现更完整，kanmars_main 的功能是 master 的子集 |
| `loop_.rs` | 4 处 `effective_max_context_tokens` → `resolved_max_context_tokens_for_agent` 替换（+8/-8 行） | +10,771/-行级重构（turn/ 子模块拆分、ResolvedAgentExecution） | **保留 master** | master 重构后代码结构完全不同，kanmars_main 的改动目标行已不存在，master 版本已使用等价 API |

---

## 4. Acceptance Criteria（AC）

| AC | 描述 | 验证方式 |
|---|---|---|
| AC-1 | `git merge master` 成功完成 | `git log --oneline -1` 显示 merge commit |
| AC-2 | 无残留冲突标记 | `grep -r '<<<<<<' --include='*.rs' --include='*.md' .` 无输出 |
| AC-3 | `cargo fmt --all -- --check` 通过 | 退出码 0 |
| AC-4 | `cargo clippy --all-targets -- -D warnings` 通过 | 退出码 0（或仅 master 已知问题） |
| AC-5 | `cargo test --workspace` 通过 | 全部 pass（或仅 master 已知失败） |
| AC-6 | config.toml 零修改 | `diff` 与备份无差异 |
| AC-7 | kanmars_main 独有功能保留 | `grep 'ZEROCLAW_BUILD_TIME' src/main.rs` 有输出 + `ls src/time_display.rs src/agent/post_compaction_context.rs` 存在 |
| AC-8 | master 新功能可用 | `grep 'a2a' crates/zeroclaw-config/src/schema.rs` 有输出 |
| AC-9 | 被吸收的功能由 master 版本提供 | `grep 'classifier_provider' crates/zeroclaw-config/src/schema.rs` 有输出 |
| AC-10 | Working tree clean | `git status` 显示 `nothing to commit, working tree clean` |
| AC-11 | schema_version 匹配 | config.toml `schema_version = 3` == master `CURRENT_SCHEMA_VERSION = 3` |

---

## 5. 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|---|---|---|---|
| schema.rs 冲突解决遗漏 hunk | 低 | 编译失败 | Task 2.3 验证步骤 + Task 3.2 clippy |
| loop_.rs 冲突解决后 kanmars_main 独有逻辑丢失 | 低 | 功能缺失 | Task 2.5 检查 main.rs + Task 3.3 测试 |
| master 代码本身有编译问题 | 低 | clippy/test 失败 | 记录但不修复（不属于 merge 范围） |
| config.toml 字段在新 schema 下行为变化 | 低 | 运行时行为差异 | `context_compression` 字段仍有 `#[serde(default)]`，不影响加载；运行时语义变化是上游设计决策 |
| `post_compaction_context.rs` 依赖的 `context_compressor.rs` 已被 master 删除 | **中** | 编译失败 | 如果 `post_compaction_context.rs` 引用了已删除的 `context_compressor` 类型/函数，需要适配到新 API（`history_trim`） |

### 5.1 风险 #5 详细分析（`post_compaction_context` 依赖检查）

kanmars_main 的 `src/agent/post_compaction_context.rs`（176 行）是在旧 `context_compressor.rs` 存在时编写的。master 已删除 `context_compressor.rs`，改为 `history_trim.rs`。

**需要在 Task 3.2（clippy）阶段确认**：
- 如果 `post_compaction_context.rs` 引用了 `context_compressor` 模块的类型 → 需要改为引用 `history_trim` 的等价类型
- 如果 `post_compaction_context.rs` 只引用 `agent/loop_` 的公共接口 → 无影响

**缓解**：这是 kanmars_main 独有文件，如果编译失败，修复范围局限在这 176 行内。

---

## 6. Merge 后的可选后续工作（不在本 plan 范围内）

| 后续工作 | 依赖 | 优先级 |
|---|---|---|
| 启用 A2A agent discovery | 本 merge 完成 + `[agents.default.a2a]` 配置 | 低 |
| 启用 SOP SQLite 持久化 | 本 merge 完成 + `[sop]` 扩展配置 | 低 |
| 配置 `summary_provider` | 本 merge 完成 + 选择模型 | 低 |
| 配置 `delegates` 跨 agent 委托 | 本 merge 完成 + 多 agent 部署 | 低 |
| 向 master 提 PR（kanmars_main 独有功能） | 本 merge 完成 + 功能稳定 1-2 周 | 中 |
| 评估 `history_trim` 对 `context_compression` 配置的影响 | 本 merge 完成 | 中 |

---

## 7. 回滚方案

如果 merge 后出现不可修复的问题：

```bash
# 回滚到 merge 前的状态
git reset --hard 2838c4a2

# 恢复 config.toml（如果被修改过）
cp /home/admin/workspace-public/kanmars/gloria/A/malorian-3516/config.toml.backup-pre-merge-* \
   /home/admin/workspace-public/kanmars/gloria/A/malorian-3516/config.toml
```

---

## 8. 依赖与前置条件

| 编号 | 依赖项 | 状态 | 说明 |
|---|---|---|---|
| D1 | kanmars_main working tree clean | ✅ 已确认 | `git status` 无 pending changes |
| D2 | master 分支已 fetch 到最新 | ✅ 已确认 | `c8c2921d` 是 master HEAD |
| D3 | config.toml 备份 | 待执行 | Task 1.2 |
| D4 | 沙箱有足够磁盘空间 | ✅ 已确认 | merge + build 预计需要 ~2GB |

---

## 9. 执行时间线预估

| 阶段 | 预计耗时 | 累计 |
|---|---|---|
| Wave 1：Pre-merge 准备 | 5 min | 5 min |
| Wave 2：Merge + 冲突解决 | 30-40 min | 35-45 min |
| Wave 3：编译 + 测试验证 | 20-30 min | 55-75 min |
| Wave 4：提交 + 最终验证 | 5 min | 60-80 min |
| **总计** | **60-80 min** | — |
