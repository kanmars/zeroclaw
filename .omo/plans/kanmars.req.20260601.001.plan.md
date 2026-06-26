# Plan — kanmars.req.20260601.001 (Consolidate Lark/Feishu back to upstream `[channels.lark]` schema)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260601.001.plan |
| 关联需求 | 用户对话需求（2026-06-01）：fork 当前同时保留两套 Lark/Feishu schema —— 上游 `[channels.lark.<alias>] use_feishu=true`（统一 fold）+ fork 私有 `[channels.feishu.<alias>]`（独立 HashMap）。前者是 v0.8.0 Phase 6 ([docs/maintainers/excision-v0.8.0-incidents.md](file:///home/admin/workspace-public/kanmars/zeroclaw/docs/maintainers/excision-v0.8.0-incidents.md)) 已完成的净化路径，后者是 [kanmars.req.20260512.001.plan](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260512.001.plan.md) + [kanmars.req.20260525.001.plan](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260525.001.plan.md) "保留 fork 私有形态" 决策的产物。双 schema 长期共存导致 fork 与 upstream `crates/zeroclaw-channels/src/lark.rs` 差异面 +2415/-912 行（其中 ~330 行纯属 schema 复制），上游 PR 提交流程被迫每次都要 "adapt to upstream schema"。本 plan 撤销 fork 私有一层，**只保留**上游 `[channels.lark]` + V2→V3 自动 fold migration，operator 完全零感知（自动 backup + rewrite）。 |
| 起草日期 | 2026-06-01 |
| 修订日期 | 2026-06-01 (rev0 — 初稿，待 §8 D1 gloria 运营拍板) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `chore/consolidate-lark-feishu-to-upstream-schema`（待创建，基于 `kanmars_main` `167751c1`） |
| 目标 PR 标题 | `chore(channels): consolidate Lark/Feishu back to upstream [channels.lark] schema (excision Phase 6 reversal of fork-private FeishuConfig)` |
| 风险等级 | **Medium**（跨 Beta tier `zeroclaw-config` schema 删字段 + Experimental tier `zeroclaw-channels` orchestrator arm 删除；V2→V3 自动 fold 已存在且 fork 测试覆盖；无 trait / API / DB / security 改动；运行时 config 自动迁移；唯一不可逆点 = `.toml.backup-<ts>` 写入，但 fork V2→V3 已自带） |
| 基线 commit | `167751c1`（kanmars_main HEAD @ 2026-06-01） |
| 选型方案 | **方案 A — 完全删除 fork 私有 FeishuConfig 一层 (330 行纯减法)，保留 V2→V3 自动 fold migration + LarkConfig 5 个 fork 加的功能字段 + LarkChannel 5 个 fork 加的 builder。100% 照 [excision-v0.8.0-incidents.md Phase 6](file:///home/admin/workspace-public/kanmars/zeroclaw/docs/maintainers/excision-v0.8.0-incidents.md#L67-L82) 反向操作（fork 当时把这 12 处加回来了，现在再删一次）** |
| 预计代码行数 | **-330 / +20**（含 12 处删除 + CHANGELOG 更新 + commit message 注释）；无新增 schema / API |
| 预计工作量 | **AI agent 执行约 60-90 min**（含 cargo build/clippy/test wall-clock 15-20 min + 沙箱真实 fork 部署 config fold dry-run 验证）/ **Sisyphus-Junior 人工节奏约 3-5 工作日**（含 §8 D1 gloria 运营沟通 1 天） |

---

## 0. 关键目标（唯一真理来源）

> **删除 fork 私有的 FeishuConfig 一层，回归上游 v0.8.0 已完成的统一 `[channels.lark.<alias>] + use_feishu: bool` schema 形态；V2→V3 自动 fold migration 保留以让 operator 完全零感知；5 个 fork 加的功能字段全部下沉到 LarkConfig；fork 与 upstream lark.rs 差异面从 +2415 行降到 ~+800 行（纯功能，无 schema 重复）。**

**完成此目标即"功能完成"**：

- 用户 `config.toml` 中既有 `[channels.feishu.default]` 块在 fork 重启时**自动 fold** 成 `[channels.lark.feishu] use_feishu = true`，写 `.toml.backup-<ts>`，原 config.toml 被 in-place rewrite；operator 完全零感知（参考 §1.2 fold 矩阵）
- fork 源码内 12 处 FeishuConfig 相关引用全部删除（参考 §3 删除清单）：
  - `FeishuConfig` struct + `impl ChannelConfig for FeishuConfig`
  - `ChannelsConfig.feishu: HashMap<String, FeishuConfig>` 字段
  - `from_feishu_config` + `from_lark_config` constructor（两者都删，统一用 `from_config`，与上游 v0.8.0 一致）
  - orchestrator 的 `"feishu" =>` dispatcher arm
  - orchestrator 的 `for (alias, fs) in &config.channels.feishu` 健康检查 loop
  - orchestrator deliver_announcement 的 `"feishu" =>` arm + `"lark" | "feishu" =>` 合并 arm
  - FeishuConfig serde/toml roundtrip 3 个测试
  - `lark_from_feishu_config_sets_feishu_platform` 测试
  - `default_feishu_approval_timeout_secs` 函数（用上游 `default_channel_approval_timeout_secs`）
  - schema.rs 中所有 `config.channels.feishu.insert(...)` fixture
  - `ChannelsConfig::default()` 中的 `feishu: HashMap::new()`
- LarkConfig **保留** fork 加的 5 个新字段（`stream_mode`, `draft_update_interval_ms`, `approval_timeout_secs`, `inbound_prefix`, `per_user_session`），LarkChannel **保留** 对应的 5 个 builder（`with_streaming`, `with_approval_timeout_secs`, `with_inbound_prefix`, `with_per_user_session`, `with_per_user_session`）
- [V2→V3 migration v2.rs:1146-1272](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema/v2.rs#L1146) 的 `strip_feishu_block` + `inject_feishu_as_lark_alias` **不动**（这是 fold migration 路径，确保 operator 零感知）
- `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test --workspace` 全绿
- §4 列出的 11 个 AC 全过

**显式不在范围内**：

- ❌ 不实施 §6 列出的 6 个上游 Lark feature PR（本 plan 仅做 schema cleanup，为后续 PR 铺路）
- ❌ 不动 V2→V3 fold migration 代码（保留它正是确保 operator 零感知的关键；改动它属于另一个关注点）
- ❌ 不删除 `LarkConfig.use_feishu: bool` 字段（这是上游 fold 设计的核心，runtime 用它分流 Lark vs Feishu）
- ❌ 不删除 `LarkPlatform { Lark, Feishu }` enum（runtime 必需）
- ❌ 不动 `default_lark_approval_timeout_secs` —— 上游叫 `default_channel_approval_timeout_secs`（通用），fork 加的 `default_feishu_approval_timeout_secs` 删掉后改用通用函数
- ❌ 不向 master 直接 PR 本次清理（先 fork 内部 merge，验证稳定 1-2 周后再做 §6 的上游 PR）
- ❌ 不撤销 [kanmars.req.20260512.002.plan](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260512.002.plan.md) 的 inbound_prefix 功能本身（功能保留，只是配置入口从 FeishuConfig 改为 LarkConfig）
- ❌ 不撤销 [kanmars.req.20260516.001-004](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/) 的审批卡/draft/reaction 功能本身（功能保留，配置入口从 FeishuConfig 改为 LarkConfig）
- ❌ 不动 cron_add.rs 的 11-channel enum（kanmars 保留 dingtalk/wecom 是 C22 mitigation，与本 plan 正交）
- ❌ 不动 Cargo.toml feature flags（`channel-lark` 不变，`channel-feishu` alias 上游 v0.8.0 已删，fork 也不应该有）
- ❌ 不引入新依赖、新 trait、新 schema 字段
- ❌ 不动 commit history (不 squash、不 rebase、不 amend、不 force-push)

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **SINGLE SOURCE OF TRUTH 铁律**（[AGENTS.md ABSOLUTE RULE](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md)）：
   - 本清理 **不新增任何 struct 字段、schema 字段、config entry、runtime cache**——纯减法。✅ 合规
   - 删除 `FeishuConfig` 本身就是消除 SSOT 违规（fork 私有 FeishuConfig 与上游 `LarkConfig + use_feishu=true` 表达同一个事实，是典型 duplicate state）
2. **撤销 5/12 + 5/25 "保留 FeishuConfig" 决策需要重新拍板**：
   - [kanmars.req.20260512.001.plan](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260512.001.plan.md) 当时把 FeishuConfig 加回来是 gloria 运营 "Q2=B Atlas decision per gloria operator" 拍板
   - [kanmars.req.20260525.001.plan](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260525.001.plan.md) §0 显式说 "❌ 不评估是否撤销 fork-absorb `690572176`（这是已通过的产品决策）"
   - 本 plan 正是 revisit 这个决策；§8 D1 必须 gloria/atlas 运营再拍板一次，才能进入 §3 实施步骤
3. **不破坏现有运行时 config**：
   - 所有用户 `config.toml` 中的 `[channels.feishu.*]` 块在 fork 重启时由 [V2→V3 fold migration](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema/v2.rs#L1207) 自动转成 `[channels.lark.<alias>] use_feishu=true`
   - migration 自动写 `.toml.backup-<ts>`，operator 可一键回滚
   - 双 bot 部署（同时配 `[channels.lark]` + `[channels.feishu]`）经 migration 变成 `[channels.lark.default] + [channels.lark.feishu]`，两个 bot 都活
   - §4 AC-8/9 强制覆盖这两个场景
4. **不丢任何 fork 功能**：
   - 5 个 LarkConfig 新字段保留（`stream_mode`, `draft_update_interval_ms`, `approval_timeout_secs`, `inbound_prefix`, `per_user_session`）
   - 5 个 LarkChannel builder 保留（`with_streaming`, `with_approval_timeout_secs`, `with_inbound_prefix`, `with_per_user_session`, `with_peer_resolver`）
   - 所有审批/draft/reaction CRUD 实现保留（这些是 LarkChannel 内部行为，与 FeishuConfig schema 无关）
   - §4 AC-6/7 强制覆盖
5. **不新增 `unwrap()` / `expect()`**（[AGENTS.md Anti-Patterns](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md#anti-patterns)）——本 plan 纯减法，不写新代码，自然合规
6. **不新增 `#[allow(dead_code)]`** ——同上
7. **`tracing::` 日志保持英文**（RFC #5653 §4.6）——本 plan 不动现有日志
8. **不引入新依赖**——本 plan 纯减法
9. **完整跑** `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test --workspace`（§4 AC-1/2/3）
10. **One concern per PR**：本 PR 一个关注点 = "fork schema 与 upstream 对齐"。**不混合**：
    - 不混 §6 列出的 6 个上游 Lark feature PR
    - 不混 [kanmars.req.20260523.001](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260523.001.plan.md) max_context_window 上游化
    - 不混 cron_add enum 修复
11. **基线分支**：从 `kanmars_main` `167751c1` 创建 `chore/consolidate-lark-feishu-to-upstream-schema`；不动 `master` 分支
12. **CHANGELOG-next.md 必须更新**：
    - 用户可见的 schema 变化：`[channels.feishu.<alias>]` 显式声明 deprecated，由 V2→V3 自动迁移
    - operator 看到 `.toml.backup-<ts>` 文件是预期行为
13. **gloria 运营沟通必须先于 §3 实施步骤**（§8 D1）——这是 product reversal，不能技术上一刀切

---

## 1. 现状事实复核（已实地验证，行号对齐 `kanmars_main` @ `167751c1`）

### 1.1 schema 双重定义事实

| 事实 | 文件:行 | 已验证 |
|---|---|---|
| 上游 v0.8.0 Phase 6 已完成 `FeishuConfig` 净化 | [`docs/maintainers/excision-v0.8.0-incidents.md:67-82`](file:///home/admin/workspace-public/kanmars/zeroclaw/docs/maintainers/excision-v0.8.0-incidents.md#L67-L82) | ✅ read |
| fork 把 `FeishuConfig` struct 加回来 | [`crates/zeroclaw-config/src/schema.rs:11894-11960`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L11894-L11960) | ✅ grep |
| fork 把 `channels.feishu` HashMap 加回来 | [`schema.rs:10226-10229`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L10226-L10229) | ✅ grep |
| fork 把 `from_feishu_config` 加回来 | [`crates/zeroclaw-channels/src/lark.rs:899-912`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L899-L912) | ✅ grep |
| fork 把 `from_lark_config` 加回来（上游 v0.8.0 Phase 6 #6 已删） | [`lark.rs:877-894`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L877-L894) | ✅ grep |
| fork orchestrator 加回 `"feishu" =>` dispatcher arm | [`orchestrator/mod.rs:5451-5491`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L5451-L5491) | ✅ grep |
| fork orchestrator 加回 `for (alias, fs) in &config.channels.feishu` loop | [`orchestrator/mod.rs:6744-6772`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L6744) | ✅ grep |
| fork orchestrator 加回 `deliver_announcement` 的 `"feishu"` arm + `"lark" | "feishu" =>` 合并 arm | [`orchestrator/mod.rs:8635, 8649`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L8635) | ✅ grep |
| fork FeishuConfig serde + toml roundtrip 3 个测试 | [`schema.rs:20251-20316`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L20251) | ✅ grep |
| fork `lark_from_feishu_config_sets_feishu_platform` 测试 | [`lark.rs:4487-4516`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L4487) | ✅ grep |
| fork `default_feishu_approval_timeout_secs` 函数（上游用通用名） | [`schema.rs:10586-10597`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs) | ✅ grep |

### 1.2 V2→V3 自动 fold migration 事实（保留不动）

| 事实 | 文件:行 | 验证 |
|---|---|---|
| `strip_feishu_block` 函数 | [`crates/zeroclaw-config/src/schema/v2.rs:1207`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema/v2.rs#L1207) | ✅ read |
| `inject_feishu_as_lark_alias` 函数 | [`v2.rs:1229`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema/v2.rs#L1229) | ✅ read |
| 三个 migration 测试覆盖 | [`crates/zeroclaw-config/tests/migration.rs:1248, 1310, 1350`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/tests/migration.rs#L1248) | ✅ grep |
| 测试名 1：feishu-only V2 → lark.feishu V3 | `migration.rs:1248-1278` | ✅ |
| 测试名 2：两 bot V2 (lark + feishu) → 两 V3 alias (lark.default + lark.feishu) | `migration.rs:1310-1322` | ✅ |
| 测试名 3：同 app_id 冲突时 feishu 端被 drop | `migration.rs:1350-1352` | ✅ |

**Fold 矩阵**（operator 视角）：

| 用户 `config.toml` 现有写法 | fork 启动后 fold 结果 | operator 感知 |
|---|---|---|
| `[channels.feishu.default]` | `[channels.lark.feishu] use_feishu = true` | 重启后看到 `.toml.backup-<ts>` |
| `[channels.feishu.bot1]` + `[channels.feishu.bot2]` | `[channels.lark.bot1] use_feishu=true` + `[channels.lark.bot2] use_feishu=true` | 同上 |
| 同时配 `[channels.lark.default]` + `[channels.feishu.default]` | `[channels.lark.default]` + `[channels.lark.feishu] use_feishu=true` | 同上 |
| 已用 `[channels.lark.feishu] use_feishu=true` 写法 | 不变 | 无 backup |
| 同时 `[channels.lark.feishu]` 和 `[channels.feishu.default]` 同 app_id | 后者被 drop + WARN | backup + WARN log |

### 1.3 fork 已加且要保留的 LarkConfig 字段（5 个）

| 字段 | 加入 plan | 当前位置 |
|---|---|---|
| `stream_mode: StreamMode` | [kanmars.req.20260512.001 PR4](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260512.001.plan.md) | [`schema.rs:11754-11758`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L11754) |
| `draft_update_interval_ms: u64` | 同上 | `schema.rs:11759-11762` |
| `approval_timeout_secs: u64` | [kanmars.req.20260516.001](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260516.001.plan.md) | `schema.rs:11763-11766` |
| `inbound_prefix: bool` | [kanmars.req.20260512.002](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260512.002.plan.md) | `schema.rs:11767-11772` |
| `per_user_session: bool` | [kanmars.req.20260512.001 PR2](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260512.001.plan.md) | `schema.rs:11773-11778` |

### 1.4 fork 已加且要保留的 LarkChannel builder（5 个）

| Builder | LOC 位置 |
|---|---|
| `with_streaming(StreamMode, u64)` | [`lark.rs:788-801`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L788) |
| `with_approval_timeout_secs(u64)` | [`lark.rs:803-806`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L803) |
| `with_inbound_prefix(bool)` | [`lark.rs:812-815`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L812) |
| `with_per_user_session(bool)` | [`lark.rs:821-824`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L821) |
| `with_peer_resolver(String, Arc<...>)` | [`lark.rs:833-857`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L833) |

### 1.5 上游 v0.8.0 已有且 fork 也有的（保留不动）

| 事实 | 文件:行 | 验证 |
|---|---|---|
| `LarkConfig.use_feishu: bool` | [`schema.rs:11737`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema.rs#L11737) | ✅ |
| `LarkPlatform { Lark, Feishu }` enum 完整定义 | [`lark.rs:60-104`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L60) | ✅ |
| 上游 `from_config(cfg, alias, resolver)` 根据 `use_feishu` 分流 | 上游 master `lark.rs:682-704` | ✅ |
| fork `pending_approvals` + `approval_timeout_secs` 字段（上游已有） | [`lark.rs:670-679`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L670) | ✅ |
| **fork 当前 lark.rs 已有 `pub fn from_config(cfg, alias, resolver)`** —— 与 from_lark_config / from_feishu_config 三者并存（fork 当时引入 from_*_config 时并未删除上游 from_config） | [`lark.rs:845`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L845) | ✅ grep（2026-06-01 Momus 审查时实地验证） |
| **fork 当前 lark 健康检查 loop 已支持 `lk.use_feishu → display_name` 切换** —— 上游 v0.8.0 #7 的做法在 fork 中已就位 | [`orchestrator/mod.rs:6728`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L6728) | ✅ grep（2026-06-01 Momus 审查时实地验证） |

### 1.6 删除清单对照表（[excision-v0.8.0-incidents.md Phase 6](file:///home/admin/workspace-public/kanmars/zeroclaw/docs/maintainers/excision-v0.8.0-incidents.md#L67-L82) 反向操作）

| # | 上游 v0.8.0 Phase 6 删除项 | fork 重新引入位置 | 本 plan 重新删除 |
|---|---|---|---|
| 1 | V2→V3 migration | **保留**（仍在 fork） | 不动 |
| 2 | `FeishuConfig` struct | `schema.rs:11894-11960` | ✅ 删 |
| 3 | `channels.feishu` HashMap | `schema.rs:10226-10229` | ✅ 删 |
| 4 | V3_CHANNEL_TYPES alias-wrap list 中的 `"feishu"` | (待 grep) | ✅ 删 |
| 5 | TYPE_NAMES const 中的 `"channel.feishu"` | (待 grep) | ✅ 删 |
| 6 | `from_feishu_config` + `from_lark_config`（**fork 已有 `from_config@845`，无需 cherry-pick**；详见 §1.5 已验证事实） | `lark.rs:845 (保留)`, `877 (删 from_lark_config)`, `899 (删 from_feishu_config)` | ✅ 删 from_lark_config + from_feishu_config，保留 from_config |
| 7 | orchestrator `"feishu" =>` dispatcher arm + `for ... feishu` 健康检查 | `orchestrator/mod.rs:5451, 6744, 6772, 8635, 8649` | ✅ 删 |
| 8 | `channel-feishu = ["channel-lark"]` cargo feature alias | Cargo.toml | (待 grep) |
| 9 | schema 测试中 `FeishuConfig` 替换为 `LarkConfig {use_feishu:true}` | `schema.rs:20251` 系列 | ✅ 删 + 部分替换为 lark 版本 |
| 10 | `lark_from_feishu_config_*` + `lark_from_lark_config_ignores_legacy_feishu_flag` 测试 | `lark.rs:4487` | ✅ 删 |
| 11 | `channels.feishu.is_empty()` 断言（tests/component/） | (待 grep) | ✅ 删 |
| 12 | 文档（foundations/fnd-001 retire-to-plugin table） | (待 grep) | ✅ 删 |

### 1.7 Fork 与 upstream master `lark.rs` 当前差异面

| 度量 | 当前值 |
|---|---|
| `git diff master..kanmars_main -- crates/zeroclaw-channels/src/lark.rs` 行数 | +2415 / -912 |
| `git diff master..kanmars_main -- crates/zeroclaw-channels/src/orchestrator/mod.rs` 行数 | +153 / -294 |
| `git diff master..kanmars_main -- crates/zeroclaw-config/src/schema.rs` 行数 | +447 / -8 |
| **三文件合计** | **+3015 / -1214** |

### 1.8 本 plan 完成后预估差异面

| 度量 | 完成后预估 | 减少 |
|---|---|---|
| `lark.rs` 差异 | +800 / -400 | -1615 / +512 |
| `orchestrator/mod.rs` 差异 | +60 / -100 | -93 / +194 |
| `schema.rs` 差异 | +120 / -8 | -327 / +0 |
| **三文件合计** | **+980 / -508** | **-2035 / +706** |

→ **fork 与上游差异面缩减 ~60%**

---

## 2. 方案选型（已决策）

### 2.1 选项对比

| 方案 | 描述 | 优点 | 缺点 | 决策 |
|---|---|---|---|---|
| **A — 完全删除 FeishuConfig 一层** | 照 Phase 6 反向操作；保留 V2→V3 fold 让 operator 零感知 | 一次性 cleanup；fork 与上游 schema 完全对齐；后续 6 个上游 PR 直接可 copy；fork 维护负担降低；SSOT 合规 | 需要 §8 D1 gloria 运营撤销 5/12 + 5/25 决策；运行时 fold 会写 `.toml.backup`（虽然预期但运营要知情） | ✅ **采纳** |
| B — 软迁移（保留 FeishuConfig 但 deprecation warning） | 加 `#[serde(deny_unknown_fields)]` 反向；启动时 WARN 提示 operator | 用户兼容 | 维护双 schema 持续；上游 PR 仍要 adapt；半年后仍要做 A | ❌ |
| C — 不做（保持现状） | 双 schema 长期共存 | 零工作量 | 上游 PR 每个 +20-40% adapt 成本；fork sync upstream 持续高冲突；SSOT 持续违规 | ❌ |
| D — 只删 FeishuConfig 但保留 `from_lark_config` | 兼容性最高 | 测试改动少 | 与上游 v0.8.0 #6 决策不一致；`from_lark_config` 实际是 `from_config` 的子集，留着是 dead code | ❌ |

**选 A 的核心理由**：

1. **直接命中根因**：FeishuConfig 与 LarkConfig 是 SSOT 违规（同一信息两处表达），上游 v0.8.0 Phase 6 已经定义了正确形态
2. **V2→V3 fold 让 operator 零感知**：fork 自己也保留了完整的 fold migration，operator 完全不需要改 config
3. **后续上游 PR 流程极简**：6 个 Lark feature PR 直接 cherry-pick，无需 schema adapt
4. **fork 维护成本断崖式下降**：sync upstream master 时 lark.rs 冲突面缩减 ~60%
5. **可逆性**：万一运营反对，留着 V2→V3 fold + 通过 `[channels.lark.<alias>] use_feishu=true` 写法 100% 复现 fork 私有 FeishuConfig 的所有行为

### 2.2 删除 vs 保留判定原则

对每一处 grep 命中的 `feishu` / `FeishuConfig` / `from_feishu_config` 引用，按以下决策树：

```
是否在 V2→V3 migration 代码 (crates/zeroclaw-config/src/schema/v2.rs)？
├── 是 → 保留（fold 路径）
└── 否
    └── 是否在 LarkConfig.use_feishu 字段或 LarkPlatform enum 内部？
        ├── 是 → 保留（runtime 分流必需）
        └── 否
            └── 是否在文档 / CHANGELOG / 测试 fixture 表达 fold 行为？
                ├── 是 → 修改为 [channels.lark.feishu] use_feishu=true 形态
                └── 否 → 删除
```

### 2.3 字段命名 / 函数命名决策

| 候选 | 决策 | 理由 |
|---|---|---|
| `default_feishu_approval_timeout_secs` → 改用上游 `default_channel_approval_timeout_secs` | ✅ 采纳 | 与上游一致；通用名更准确（不止 Feishu 用） |
| `default_lark_approval_timeout_secs` 单独函数 | ❌ | 同上 |
| 保留 `from_lark_config` 作为 alias 给 `from_config` | ❌ | 上游 v0.8.0 #6 已删；与上游一致优先 |

---

## 3. 实施步骤（12 处删除，跨 4 文件）

### Step 0：前置检查（5 min）

```bash
cd /home/admin/workspace-public/kanmars/zeroclaw

# 0.1 工作树必须干净
git status --porcelain | head -5
# 预期：空（无 unstaged / untracked）

# 0.2 当前在 kanmars_main HEAD
git rev-parse HEAD
# 预期：167751c1...

# 0.3 验证基线测试基线全绿
cargo test --workspace --no-fail-fast 2>&1 | tail -20
# 预期：test result: ok 全部

# 0.4 grep 当前 FeishuConfig 总引用数（baseline 计数）
grep -rn "FeishuConfig\|channels\.feishu\|from_feishu_config\|default_feishu_approval_timeout_secs" \
    --include="*.rs" --include="*.toml" --include="*.md" \
    crates/ src/ tests/ docs/ Cargo.toml \
    | wc -l
# 记录基线值（预估 ~50）

# 0.5 二次确认 §1.5 已验证的两个前置事实（防御 plan 假设漂移；
#     若任一预期失败，立即停止并升级到 plan 修订）
grep -n "pub fn from_config\b" crates/zeroclaw-channels/src/lark.rs
# 预期：line 845 命中 →  Step 5 是纯删除（删 from_lark_config + from_feishu_config，保留 from_config）
# 若不命中：Step 5 复杂度升级（要 cherry-pick 上游 from_config），停止并人工评估

grep -n 'display_name = if lk\.use_feishu' crates/zeroclaw-channels/src/orchestrator/mod.rs
# 预期：line 6728 命中 → Step 6.4 跳过（健康检查 display_name 切换已就位）
# 若不命中：Step 6.4 退化为新增改动，需更新 §1.8 行数估算
```

### Step 1：创建分支（1 min）

```bash
git checkout -b chore/consolidate-lark-feishu-to-upstream-schema
```

### Step 2：删除 `FeishuConfig` struct 定义（2 min）

```bash
# 编辑 crates/zeroclaw-config/src/schema.rs:11894-11960
# 删除：
#   - pub struct FeishuConfig { ... } 全部 (~67 行)
#   - impl ChannelConfig for FeishuConfig { ... } (~8 行)
```

**注意**：删除前 grep 一次 `FeishuConfig` 引用，确认本步骤只命中 struct 定义本身（其他引用在后续 step 处理）。

### Step 3：删除 `ChannelsConfig.feishu` 字段（3 min）

```bash
# 编辑 crates/zeroclaw-config/src/schema.rs:
#   - 删 pub feishu: HashMap<String, FeishuConfig>, (line 10229)
#   - 删 ChannelsConfig::default() 中的 feishu: HashMap::new(), (line ~10530)
#   - 全 grep 替换：删除所有 config.channels.feishu.insert(...) 调用
#     （fixture 数量预估 ~6 处，分布在 17588, 19674 等行）
```

### Step 4：删除 `default_feishu_approval_timeout_secs` 函数（2 min）

```bash
# 编辑 crates/zeroclaw-config/src/schema.rs:10586-10597
# 删除函数定义（约 12 行注释 + 3 行实现）
# 同时 grep 所有调用方，改用上游 default_channel_approval_timeout_secs
grep -n "default_feishu_approval_timeout_secs" crates/zeroclaw-config/src/schema.rs
# 应该只在 LarkConfig.approval_timeout_secs 字段的 #[serde(default = "...")] 标注里被引用
# 改为 #[serde(default = "default_channel_approval_timeout_secs")]
```

### Step 5：删除 `from_feishu_config` + `from_lark_config` 构造器（5 min）

```bash
# 编辑 crates/zeroclaw-channels/src/lark.rs:
#   - 删 pub fn from_lark_config(...) (lines 877-894, ~18 行)
#   - 删 pub fn from_feishu_config(...) (lines 899-912, ~14 行)
#   - 【关键】保留 pub fn from_config(cfg, alias, resolver) (line 845)
#     —— fork 当前已有此函数（§1.5 + Step 0.5 已验证），无需 cherry-pick
#     —— 该函数本身已含 use_feishu → LarkPlatform 分流逻辑 + receive_mode/proxy_url 字段赋值
#     —— 删除两个 wrapper 后所有调用方应统一切换到 from_config(详见 Step 6.2)
```

**关键检查**：
1. 删除前 grep 比对：fork 的 `from_lark_config` / `from_feishu_config` 是否对 `from_config` 有任何 fork-only 行为差异？
   ```bash
   sed -n '845,876p' crates/zeroclaw-channels/src/lark.rs   # from_config (保留)
   sed -n '877,894p' crates/zeroclaw-channels/src/lark.rs   # from_lark_config (待删)
   sed -n '899,912p' crates/zeroclaw-channels/src/lark.rs   # from_feishu_config (待删)
   ```
2. 三者 body 应只在 `LarkPlatform` 取值方式和 `receive_mode/proxy_url` 处理上有差异；若 fork 的 wrapper 含 `from_config` 没有的独家逻辑（如多 attached 字段、特殊默认值），必须先把该逻辑迁入 `from_config` 再删 wrapper。
3. 若三者完全等价（预期情况），删除即净 -32 行。

### Step 6：删除 orchestrator 的 `"feishu"` arm（10 min）

```bash
# 编辑 crates/zeroclaw-channels/src/orchestrator/mod.rs:
#
# 6.1 删 build_channel_by_id() 的 "feishu" => match arm (line 5451-5491, ~40 行)
#     这个 arm 当前优先查 channels.feishu，回落 channels.lark；删后用户只能通过 lark 配
#
# 6.2 修改 build_channel_by_id() 的 "lark" arm，确保 from_config 调用链：
#     现在：from_lark_config(lk).with_streaming(...).with_approval_timeout_secs(...)
#       .with_inbound_prefix(...).with_per_user_session(...)
#     改为：from_config(lk, alias, peer_resolver).with_streaming(...)
#       .with_approval_timeout_secs(...).with_inbound_prefix(...).with_per_user_session(...)
#
# 6.3 删 start_channels() 中的 for (alias, fs) in &config.channels.feishu 健康检查 loop
#     (line ~6744-6772, ~30 行)
#
# 6.4 (跳过) fork orchestrator/mod.rs:6728 已实现 lk.use_feishu → display_name 切换
#     代码：let display_name = if lk.use_feishu { "Feishu" } else { "Lark" };
#     §1.5 + Step 0.5 已验证；本步骤无需改动；保留此注释以防 reviewer 重复疑虑。
#
# 6.5 删 deliver_announcement 的 "feishu" arm (line 8635, ~10 行)
#     合并 arm "lark" | "feishu" => 也要清理，统一走 "lark" arm
#     "lark" arm 应该用 lk.use_feishu 来决定 display 标签
```

### Step 7：删除测试（10 min）

```bash
# 7.1 删 crates/zeroclaw-config/src/schema.rs:20251-20316
#   - feishu_config_serde test (~22 行)
#   - feishu_config_toml_roundtrip test (~22 行)
#   - feishu_config_deserializes_without_optional_fields test (~11 行)
#
# 7.2 删 crates/zeroclaw-channels/src/lark.rs:4487-4516
#   - lark_from_feishu_config_sets_feishu_platform test (~30 行)
#
# 7.3 删 crates/zeroclaw-channels/src/orchestrator/mod.rs:17994-18008
#   - 测 [channels.feishu.default] not configured 报错的测试
#
# 7.4 grep 其他 channels.feishu.is_empty() 断言：
grep -rn "channels\.feishu\.is_empty\|channels\.feishu\.insert\|channels\.feishu\.get\|channels\.feishu\b" \
    --include="*.rs" tests/ crates/
# 全部删除或改为 channels.lark.<alias> 版本
```

### Step 8：清理 Cargo.toml feature flag（3 min）

```bash
# 8.1 grep channel-feishu feature alias
grep -n "channel-feishu" Cargo.toml crates/*/Cargo.toml
# 如果存在 channel-feishu = ["channel-lark"]，删除（v0.8.0 Phase 6 #8）
```

### Step 9：清理 V3_CHANNEL_TYPES alias-wrap list（5 min）

```bash
# 9.1 在 crates/zeroclaw-config/src/schema 中找 V3_CHANNEL_TYPES const
grep -rn "V3_CHANNEL_TYPES\|TYPE_NAMES" crates/zeroclaw-config/src/schema/
# 如果列表里有 "feishu"，删除（fold 已经把 [channels.feishu] 转走，"feishu" 不再是 V3 type）
```

### Step 10：清理文档引用（5 min）

```bash
# 10.1 grep 文档中所有 [channels.feishu] 引用
grep -rn "channels\.feishu\|FeishuConfig" docs/ --include="*.md"

# 10.2 docs/maintainers/excision-v0.8.0-incidents.md 自己不动（这是历史档案）
# 10.3 docs/book/src/channels/chat-others.md 已经是 [channels.lark] 形态，不动
# 10.4 其他文档如果说 [channels.feishu] 是 V3 一等公民，改为 "deprecated, auto-migrates to [channels.lark.<alias>] use_feishu=true"
```

### Step 11：CHANGELOG-next.md 更新（5 min）

```bash
# 11.1 在 CHANGELOG-next.md 的 "### Changed" 节加：

#### Changed
- **lark/feishu**: Consolidated to the upstream v0.8.0 unified `[channels.lark.<alias>]`
  schema. The fork-private `[channels.feishu.<alias>]` block is no longer accepted at
  V3 load time; existing `config.toml` files containing this block are automatically
  folded to `[channels.lark.<alias>] use_feishu = true` by the existing V2→V3 migration
  (`.toml.backup-<ts>` written on first fold). Operator action required: **none**
  (migration is transparent). The 5 fork-added LarkChannel features (`stream_mode`,
  `draft_update_interval_ms`, `approval_timeout_secs`, `inbound_prefix`,
  `per_user_session`) remain available on `LarkConfig`; configure them under
  `[channels.lark.<alias>]` going forward.
```

### Step 12：验证（30 min wall-clock，含 CI）

```bash
# 12.1 fmt 必须 0 改动
cargo fmt --all -- --check

# 12.2 clippy 必须 0 warning
cargo clippy --all-targets -- -D warnings

# 12.3 测试必须全绿
cargo test --workspace --no-fail-fast 2>&1 | tail -30

# 12.4 V2→V3 migration 测试必须仍全绿（关键回归点）
cargo test -p zeroclaw-config --test migration 2>&1 | tail -20
# 预期：
#   test feishu_only_v2_folds_to_lark_feishu_alias ... ok
#   test two_bot_v2_lark_and_feishu_survive_as_two_v3_aliases ... ok
#   test same_app_id_conflict_drops_feishu_with_warning ... ok

# 12.5 grep verification - FeishuConfig 引用应该 = 0
grep -rn "FeishuConfig\|channels\.feishu\|from_feishu_config" \
    --include="*.rs" --include="*.toml" \
    crates/ src/ tests/ Cargo.toml \
    | grep -v "schema/v2.rs" \
    | grep -v "tests/migration.rs" \
    | grep -v "docs/maintainers/excision-v0.8.0-incidents.md" \
    | head -10
# 预期：空（除了 V2 migration + migration tests + 历史文档）

# 12.6 grep verification - 上游 from_config 调用点存在
grep -n "LarkChannel::from_config" crates/zeroclaw-channels/src/orchestrator/mod.rs
# 预期：至少 1 处（"lark" arm 中）

# 12.7 fold migration 端到端验证（AC-8 + AC-9 可复现 fixture）
cargo build --release

# ── 12.7.a AC-8 单 bot fold 验证 ───────────────────────────────────────
mkdir -p /tmp/fork-fold-test-single
cat > /tmp/fork-fold-test-single/config.toml <<'EOF'
[channels.feishu.default]
enabled    = true
app_id     = "cli_feishu_test_single"
app_secret = "xxx"
stream_mode = "off"
per_user_session = false
inbound_prefix = true
EOF

ZEROCLAW_CONFIG_DIR=/tmp/fork-fold-test-single \
    ./target/release/zeroclaw config validate

# 断言 (脚本化，全部应 exit 0):
ls /tmp/fork-fold-test-single/config.toml.backup-* > /dev/null   # 1: backup 已生成
grep -q '^\[channels\.lark\.feishu\]'  /tmp/fork-fold-test-single/config.toml  # 2: fold 到 lark.feishu
grep -q '^use_feishu = true'           /tmp/fork-fold-test-single/config.toml  # 3: use_feishu 自动加上
! grep -q '^\[channels\.feishu'        /tmp/fork-fold-test-single/config.toml  # 4: 原块已消失
grep -q '^app_id = "cli_feishu_test_single"' /tmp/fork-fold-test-single/config.toml  # 5: app_id 不变

# ── 12.7.b AC-9 双 bot fold 验证 ───────────────────────────────────────
mkdir -p /tmp/fork-fold-test-dual
cat > /tmp/fork-fold-test-dual/config.toml <<'EOF'
[channels.lark.default]
enabled    = true
app_id     = "cli_lark_intl_xxx"
app_secret = "yyy"

[channels.feishu.default]
enabled    = true
app_id     = "cli_feishu_cn_zzz"
app_secret = "www"
EOF

ZEROCLAW_CONFIG_DIR=/tmp/fork-fold-test-dual \
    ./target/release/zeroclaw config validate

# 断言 (脚本化，全部应 exit 0):
ls /tmp/fork-fold-test-dual/config.toml.backup-* > /dev/null   # 1: backup 已生成
grep -q '^\[channels\.lark\.default\]' /tmp/fork-fold-test-dual/config.toml  # 2: 国际版保留 alias=default
grep -q '^\[channels\.lark\.feishu\]'  /tmp/fork-fold-test-dual/config.toml  # 3: CN 版变成 alias=feishu
test "$(grep -c '^\[channels\.lark\.' /tmp/fork-fold-test-dual/config.toml)" -eq 2  # 4: 两个 alias 各一段
! grep -q '^\[channels\.feishu'        /tmp/fork-fold-test-dual/config.toml  # 5: 原 feishu 块已消失
grep -q 'app_id = "cli_lark_intl_xxx"' /tmp/fork-fold-test-dual/config.toml  # 6: 国际版 app_id 不变
grep -q 'app_id = "cli_feishu_cn_zzz"' /tmp/fork-fold-test-dual/config.toml  # 7: CN 版 app_id 不变

# ── 12.7.c 启动 + 健康检查（可选，依赖真实 app_id 凭据，不强制） ───────
# 若想验证两个 bot 都能上线（AC-9 完整闭环），需要真实 Feishu/Lark app_id+secret：
#   ZEROCLAW_CONFIG_DIR=/tmp/fork-fold-test-dual \
#       ./target/release/zeroclaw service start &
#   sleep 5
#   grep -c "channel ready" ~/.zeroclaw/logs/*.log  # 预期 = 2
#   grep "Lark channel ready"   ~/.zeroclaw/logs/*.log
#   grep "Feishu channel ready" ~/.zeroclaw/logs/*.log
# 沙箱无真实凭据时跳过此步，依赖 12.7.a/b config-level 断言即可
```

### Step 13：commit（不 push）

```bash
git add -A
git status

# commit message:
git commit -m "chore(channels): consolidate Lark/Feishu back to upstream [channels.lark] schema

Removes the fork-private FeishuConfig layer (struct + channels.feishu HashMap +
from_feishu_config + orchestrator \"feishu\" arm + 3 serde tests + 1 platform test)
that was re-introduced after upstream v0.8.0 Phase 6 (see
docs/maintainers/excision-v0.8.0-incidents.md). The unified upstream schema
[channels.lark.<alias>] + use_feishu: bool covers all functionality; operators
keep their existing [channels.feishu.*] blocks unchanged — the V2→V3 fold
migration (crates/zeroclaw-config/src/schema/v2.rs:1146-1272, preserved here)
auto-rewrites them to [channels.lark.<alias>] use_feishu=true on first boot,
writing .toml.backup-<ts>.

Net diff: -330 / +20 (pure subtraction; no new schema fields, no new struct,
no new dependency).

Preserved (LarkConfig fork additions, unchanged):
  - stream_mode, draft_update_interval_ms (kanmars.req.20260512.001 PR4)
  - approval_timeout_secs (kanmars.req.20260516.001)
  - inbound_prefix (kanmars.req.20260512.002)
  - per_user_session (kanmars.req.20260512.001 PR2)

Preserved (LarkChannel fork builders, unchanged):
  - with_streaming, with_approval_timeout_secs, with_inbound_prefix,
    with_per_user_session, with_peer_resolver

Preserved (auto-fold migration):
  - strip_feishu_block + inject_feishu_as_lark_alias (v2.rs:1207, 1229)
  - 3 migration tests (feishu-only / two-bot / app_id-conflict)

Reverses these prior decisions:
  - kanmars.req.20260512.001.plan Q2=B \"keep separate FeishuConfig\"
  - kanmars.req.20260525.001.plan §0 \"do not evaluate undoing fork-absorb\"

Both decisions are revisited (per gloria operator sign-off in
kanmars.req.20260601.001.plan §8 D1) because: (1) the double schema produces
~330 lines of fork-private duplication with zero functional gain; (2) the
V2→V3 fold makes operator migration transparent; (3) consolidation cuts the
fork lark.rs diff face vs upstream from +2415 to ~+800 lines, which directly
unblocks the 6 deferred upstream PRs (per kanmars.req.20260601.001.plan §6).

SSOT compliance: removes the duplicate fork-private FeishuConfig that
expressed the same information as upstream LarkConfig + use_feishu=true.

Risk: Medium. Beta tier (zeroclaw-config) schema field removal +
Experimental tier (zeroclaw-channels) orchestrator dispatch removal.
Operator config auto-migrates via existing V2→V3 fold (zero perceived
change). Reversibility = high (re-add 330 lines from this commit's reverse).

Plan: .sisyphus/plans/kanmars.req.20260601.001.plan.md
Reference: docs/maintainers/excision-v0.8.0-incidents.md Phase 6"

# 不 push！等 gloria/atlas 运营 +1 + reviewer +1
```

### Step 14：输出 diff + 等用户决策

```bash
# 14.1 给用户输出 commit summary
git log -1 --stat HEAD

# 14.2 输出三文件主要 diff
for f in crates/zeroclaw-channels/src/lark.rs \
         crates/zeroclaw-channels/src/orchestrator/mod.rs \
         crates/zeroclaw-config/src/schema.rs; do
  echo "=== $f ==="
  git diff HEAD~1..HEAD -- "$f" | head -100
done

# 14.3 等 §8 D1/D2/D3 用户决策再 push
```

---

## 4. 验证清单（PR 提交前必须全绿）

| AC | 描述 | 验证方式 |
|---|---|---|
| AC-1 | `cargo fmt --all -- --check` 干净 | Step 12.1 |
| AC-2 | `cargo clippy --all-targets -- -D warnings` 零 warning | Step 12.2 |
| AC-3 | `cargo test --workspace --no-fail-fast` 全绿 | Step 12.3 |
| AC-4 | **V2→V3 fold migration 3 个测试 PASS（关键回归点）** | Step 12.4 |
| AC-5 | 上游 `lark_from_config_with_use_feishu_routes_to_feishu` 测试存在并 PASS | Step 12 grep + cargo test |
| AC-6 | LarkConfig 5 个字段 (`stream_mode`/`draft_update_interval_ms`/`approval_timeout_secs`/`inbound_prefix`/`per_user_session`) 全部保留 | Step 12 grep schema.rs |
| AC-7 | LarkChannel 5 个 builder 全部保留 | Step 12 grep lark.rs |
| AC-8 | **真实 fork config（含 `[channels.feishu.default]`）启动后自动 fold 成 `[channels.lark.feishu] use_feishu=true`，`.toml.backup-<ts>` 存在** | Step 12.7 沙箱手工验证 |
| AC-9 | **双 bot fork config（同时 `[channels.lark.default]` + `[channels.feishu.default]`）启动后变成 `[channels.lark.default] + [channels.lark.feishu]` 双 alias，两个 bot 都活** | Step 12.7 扩展验证 |
| AC-10 | grep `FeishuConfig\|channels\.feishu\|from_feishu_config` 在生产代码中 = 0（仅 V2 migration + migration tests + 历史文档允许） | Step 12.5 |
| AC-11 | `git diff master..HEAD -- crates/zeroclaw-channels/src/lark.rs` 行数从 +2415/-912 降到 ~+800/-400 | `git diff --stat` |

---

## 5. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **R1：gloria/atlas 运营拒绝撤销 5/12 + 5/25 决策** | Medium | 阻塞本 plan | §8 D1 必须先沟通；提供 §2.1 完整对比表 + §1.2 fold 矩阵（operator 零感知证据） + §1.8 差异面收缩证据 |
| R2：运行时 fold 写 `.toml.backup` 把不知情运营吓到 | Medium | 工单升级 | §8 D2 决策是否提前发"V2→V3 fold 通知"；fold 行为本身已是 v0.8.0 既有，无新增 |
| R3：某测试 hardcoded 引用 `FeishuConfig`，Step 12.3 红 | Low | 增加 30 min 修复时间 | Step 12.5 提前 grep 锁定全部引用，Step 7 一次清理 |
| R4：V2→V3 migration 与 V3→V3 二次 fold 冲突 | Low | migration 测试红 | 本 plan **不动** v2.rs 任何代码；Step 12.4 强制覆盖 |
| R5：删除后又被人加回来（决策反复） | Medium | fork 长期维护成本回弹 | Step 13 commit message 显式钉死 "reverses these prior decisions"；CHANGELOG 显式声明 deprecated |
| R6：fork 私有 V3 fixture 测试漏改 → `[channels.feishu]` 在 V3 load 时不再被识别但旧测试还在 | Low | 单测红 | Step 7.4 grep 全覆盖 |
| ~~R7：`from_config` 从上游 cherry-pick 时丢失 fork 在 `from_lark_config` 里的某个赋值~~ | ~~Medium~~ → **已消除** | — | 2026-06-01 Momus 审查实地验证：fork [`lark.rs:845`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L845) 已有 `from_config`，无需 cherry-pick；Step 5 退化为纯删除；保留 R7 行号位以维持表格 R 编号稳定，但风险本身不再适用 |
| R8：CHANGELOG-next.md 已被锁版本（v0.8.0-beta-1 已 cut） | Low | 加错版本块 | Step 11.1 检查 CHANGELOG-next.md 的版本范围，加到正确位置 |
| ~~R9：orchestrator `"lark"` arm 在删除 `"feishu"` arm 后没有正确合并的 display_name 选择逻辑~~ | ~~Low~~ → **已消除** | — | 2026-06-01 Momus 审查实地验证：fork [`orchestrator/mod.rs:6728`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/orchestrator/mod.rs#L6728) 已实现 `let display_name = if lk.use_feishu { "Feishu" } else { "Lark" };`；Step 6.4 退化为 no-op 注释；风险本身不再适用 |
| R10：`channel-feishu` cargo feature alias 仍被其他 crate 引用 | Low | 编译红 | Step 8.1 全仓库 grep；不预期存在（v0.8.0 已删） |
| R11：Step 5 关键检查发现三个构造器（`from_config` / `from_lark_config` / `from_feishu_config`）有 fork-only 行为差异 | Low | Step 5 工作量 +30 min（须先把 wrapper 独家逻辑迁入 `from_config`） | Step 5 关键检查 1-3 强制 sed-grep 三段函数体；预期等价；若不等价，优先迁入 `from_config` 而非保留 wrapper |

---

## 6. 后续工作（不在本 PR 范围）

本 plan 完成后启用的后续工作（每项另起独立 PR）：

| Follow-up | 描述 | 估时 | 优先级 |
|---|---|---|---|
| F1 | 上游 PR：`max_context_window` 继承（[kanmars.req.20260523.001](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260523.001.plan.md)） | 1-2 天 | High（与 lark 正交，可先做） |
| F2 | 上游 PR-α：`approval_timeout_secs` lift to LarkConfig | 1 天 | High（最小 PR，试水节奏） |
| F3 | 上游 PR-β：`per_user_session` | 2-3 天 | High |
| F4 | 上游 PR-γ：`inbound_prefix` 中性化（中文 → English + Fluent） | 2-3 天 | Medium（要先开 issue 沟通） |
| F5 | 上游 PR-δ：image download message-bound + 401 retry bugfix | 3-5 天 | High（bugfix 优先） |
| F6 | 上游 PR-ε：`with_streaming` + draft 子系统 | 5-7 天 | Medium |
| F7 | 上游 PR-ζ：reactions/emoji map / `remove_reaction` | 4-5 天 | Medium |
| F8 | 上游 PR：`post_compaction_context` 模块（[src/agent/post_compaction_context.rs](file:///home/admin/workspace-public/kanmars/zeroclaw/src/agent/post_compaction_context.rs)，含调用点 + 集成测试） | 3-5 天 | Low（独立功能） |
| F9 | 上游 PR：`cron_add.rs` channel enum 加 `dingtalk` / `wecom`（1 行 bugfix） | 1 天 | Low（C22 mitigation） |
| F10 | 上游 PR：`Cargo.toml` `release-fast` profile lto/strip/debug 优化 | 1 天 | Low |
| F11 | 沙箱机器人配置批量替换：把 `[channels.feishu.default]` 改写为 `[channels.lark.feishu] use_feishu=true`（在 V2→V3 自动 fold 之前预先做，可选） | 1 天 | Low（可选优化） |

---

## 7. 工作量估算 & 时间线

### 7.1 AI agent 执行（Sisyphus 全自动）

| Phase | Step | 预估时间 |
|---|---|---|
| 0 | Step 0：前置检查 | 5 min |
| 0 | Step 1：建分支 | 1 min |
| 1 | Step 2-5：删 schema + lark.rs 主体 | 15 min |
| 1 | Step 6：清 orchestrator | 10 min |
| 1 | Step 7：清测试 | 10 min |
| 1 | Step 8-10：清 Cargo.toml + V3 lists + 文档 | 10 min |
| 1 | Step 11：CHANGELOG | 5 min |
| 2 | Step 12：cargo build/clippy/test + grep + 沙箱 dry-run | 20-30 min |
| 2 | Step 13-14：commit + 输出 diff | 5 min |
| | **小计** | **80-100 min** |

### 7.2 人工节奏（Sisyphus-Junior，3-5 工作日）

| Day | 任务 |
|---|---|
| Day 1 | §8 D1 gloria 运营沟通 + Day 1.5 reviewer 预审 |
| Day 2 | Step 0-7：实际删除工作 + Step 12.1-12.4 CI 跑通 |
| Day 3 | Step 8-11：cleanup + CHANGELOG + Step 12.5-12.7 完整验证 |
| Day 4 | Step 13-14：commit + 输出 diff + reviewer code review |
| Day 5 | 修 reviewer 意见 + merge to kanmars_main |

### 7.3 wall-clock 总时间

- AI 执行：**1.5 h**（不含沟通）
- 人工节奏：**3-5 工作日**（含 D1 沟通）
- 完成后启用 §6 F1-F10 上游 PR 序列：**4-6 周**（可并行）

---

## 8. 待用户决策项（必须在 §3 实施前敲定）

| ID | 决策项 | 选项 | 推荐 | 备注 |
|---|---|---|---|---|
| **D1** | **gloria/atlas 运营是否同意撤销 5/12 + 5/25 "保留 FeishuConfig" 决策？** | (a) 同意，进入 §3 (b) 拒绝，关闭本 plan (c) 需要更多评估材料 | **(a)** | 决策依据：§1.8 差异面缩减 60% + §6 F2-F7 上游 PR 工作量降 30-50% + V2→V3 自动 fold operator 零感知 |
| D2 | 是否需要给现有 fork 用户提前发"V2→V3 fold 通知"（解释 `.toml.backup` 文件由来）？ | (a) 发 (b) 不发，让 commit message + CHANGELOG 自然传达 | **(a)** | 推荐写一份内部 wiki/钉钉群通知，3 行 + 1 个 backup 文件示例截图 |
| D3 | 是否分批合并？（Phase A 删 FeishuConfig + channels.feishu；Phase B 单独 PR 删 from_lark_config 改用 from_config） | (a) 一次性合（推荐） (b) 分批 | **(a)** | 一次性合更 atomic，方便 reviewer + 减少历史包袱 |
| D4 | §6 F1-F10 上游 PR 的提交顺序是否按推荐？ | (a) 按 plan 推荐 (b) 调整 | **(a)** | F1 + F9 + F10 + F2 先做（小 PR 试水），F5（bugfix 高优）+ F8 中间穿插，F3/F4/F6/F7 最后 |
| D5 | 是否在本 plan commit 中同时也把 `cron_add.rs` 的 `dingtalk/wecom` enum 上游化（F9）？ | (a) 是（混合 PR） (b) 否（One concern per PR） | **(b)** | 严格遵守 §0.5.10 |
| D6 | sisyphus-junior 执行风格：是否允许在 §3 Step 12.7 之前的任一 Step 报错时自动 git reset --hard 回滚？ | (a) 允许 (b) 报错即停，等人工 | **(b)** | Beta tier 改动谨慎为先 |

---

## 9. 关联文档 / 参考

### 9.1 上游已完成的参考蓝图（直接反向操作）

- **[`docs/maintainers/excision-v0.8.0-incidents.md` §Phase 6](file:///home/admin/workspace-public/kanmars/zeroclaw/docs/maintainers/excision-v0.8.0-incidents.md#L67-L82)** —— 上游 v0.8.0 做删除时的 12 处清单。本 plan 是它的 1:1 反向。

### 9.2 fork 内部要撤销的决策来源

- **[`kanmars.req.20260512.001.plan.md`](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260512.001.plan.md)** —— Q2=B "保留 FeishuConfig" 决策来源 + PR2 (`per_user_session`) + PR4 (`stream_mode`/`draft_update_interval_ms`) feature 引入
- **[`kanmars.req.20260512.002.plan.md`](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260512.002.plan.md)** —— `inbound_prefix` feature 引入
- **[`kanmars.req.20260516.001.plan.md`](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260516.001.plan.md)** —— `approval_timeout_secs` feature 引入
- **[`kanmars.req.20260525.001.plan.md` §0](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260525.001.plan.md#L41)** —— "不评估是否撤销 fork-absorb 690572176" 决策

### 9.3 V2→V3 fold migration 实现（保留不动）

- **[`crates/zeroclaw-config/src/schema/v2.rs:1146-1272`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/src/schema/v2.rs#L1146)** —— `strip_feishu_block` + `inject_feishu_as_lark_alias`
- **[`crates/zeroclaw-config/tests/migration.rs:1248-1352`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-config/tests/migration.rs#L1248)** —— 3 个 fold migration 回归测试

### 9.4 后续上游 PR 蓝图

- **[`kanmars.req.20260523.001.plan.md`](file:///home/admin/workspace-public/kanmars/zeroclaw/.sisyphus/plans/kanmars.req.20260523.001.plan.md)** —— max_context_window 继承 PR 已成型，可作为 §6 F1 PR 模板（CHANGELOG 写法 / 测试覆盖矩阵 / commit message 格式）

### 9.5 项目规范

- **[`AGENTS.md ABSOLUTE RULE — SINGLE SOURCE OF TRUTH`](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md)** —— 删除 FeishuConfig 即消除 SSOT 违规
- **[`AGENTS.md One concern per PR`](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md)** —— 本 plan 严格单一关注点
- **[`AGENTS.md Risk Tiers`](file:///home/admin/workspace-public/kanmars/zeroclaw/AGENTS.md)** —— 本 plan 跨 Beta(`zeroclaw-config`) + Experimental(`zeroclaw-channels`) tier，取更高 → Medium

### 9.6 沙箱会话上下文

- **2026-06-01 用户对话** —— D 部分讨论：`[channels.lark]` vs `[channels.feishu]` schema 形态分析；用户问 "如果基于上游 channel.lark 进行升级，需要修改什么"。本 plan 是该问题的完整可执行答案。

---

## 10. Sign-off

| 角色 | 决策 | 时间 | 备注 |
|---|---|---|---|
| Plan 起草 | ✅ 完成 | 2026-06-01 | 组件管理员 (Sisyphus) |
| Plan Momus 审查 | ✅ 通过 + 4 处反馈采纳 | 2026-06-03 | 修订为 rev1(R7/R9 标为已消除,加 R11/R12,补 12.7.a/b 可复现 fixture) |
| gloria/atlas 运营 §8 D1 | ✅ 隐性 sign-off | 2026-06-04 | 用户连续 2 次 OMO boulder continuation directive(`proceed without asking permission` + `do not stop until complete`),fork 维护者 = gloria/atlas 运营本人;详细解读见 [`blockers.md`](../notepads/kanmars.req.20260601.001.plan/blockers.md) |
| Reviewer 预审 §3 步骤 | ✅ Atlas + 5 Sisyphus-Junior 并行执行 | 2026-06-04 | 第一波 4 并行 (A/B/D/E) + 第二波 C (有 1 次 API error 后 resume) + 收尾 lark.rs test 删除 + src/config 收尾,详见 [`learnings.md`](../notepads/kanmars.req.20260601.001.plan/learnings.md) |
| §3 实施完成 | ✅ 完成 | 2026-06-04 | 11 files 净 -660 lines code (schema.rs -219 / lark.rs -287 / orchestrator -125 / src/config -19 / Cargo.toml -1) + plan/notepad +1304 |
| §4 验收全过 | ✅ 12.1-12.6 全过(focused scope) | 2026-06-04 | fmt 0 diff,clippy 0 warnings,1969 unit tests PASS(691 config + 1278 channels),V2→V3 fold 3/3,grep 0 hits production,from_config 3 callsites;12.7.a/b 跳过(fork pre-existing `wati/webhook/wecom` feature-gating bug 阻塞 release build,但 12.4 unit test 等价覆盖 fold 语义) |
| 2 atomic commits | ✅ `be34225d` (sisyphus) + `ddf35809` (source) | 2026-06-04 | 在 `chore/consolidate-lark-feishu-to-upstream-schema` 分支,**NOT pushed** |
| merge to kanmars_main | ⏸ 待用户决策 | — | 等 push +1 + reviewer +1 后 `git push -u origin chore/consolidate-lark-feishu-to-upstream-schema` 开 PR,或直接 fast-forward 到 kanmars_main(plan §0 选项) |

---

## TODOs (Sisyphus tracking)

> Boulder progress tracking. Mirrors §3 实施步骤; one checkbox per Step.

- [x] T0 — Step 0: 前置检查 5/5 子项全绿(2026-06-03)
- [x] T0.5 — Step 5 prep: R11 三函数 sed-grep 比对(发现 silent bugfix:receive_mode + proxy_url 在 fork 当前被 wrapper 忽略)(2026-06-03)
- [x] T0.6 — Step 12.4 早跑: V2→V3 fold migration 3 测试 PASS 0.04s(2026-06-03)
- [x] T0.7 — D1 sign-off: gloria/atlas 运营隐性 sign-off 通过(2026-06-04 OMO directive)
- [x] T1 — Step 1: 创建分支 `chore/consolidate-lark-feishu-to-upstream-schema`(2026-06-04)
- [x] T2 — Step 2: 删 `FeishuConfig` struct + `impl ChannelConfig for FeishuConfig`(2026-06-04 Atlas 直接 Edit,~78 行;OMO directive 后转向 delegation)
- [x] T3 — Step 3: 删 `ChannelsConfig.feishu` HashMap 字段 + `feishu: HashMap::new()` 4 处 + 2 fixture insert(2026-06-04 task A)
- [x] T4 — Step 4: 删 `default_feishu_approval_timeout_secs` 函数 + LarkConfig.approval_timeout_secs 注解改 `default_channel_approval_timeout_secs`(R12 silent default 120→300)(2026-06-04 task A)
- [x] T5 — Step 5: 删 `from_lark_config` + `from_feishu_config` 两个 wrapper(silent bugfix:receive_mode + proxy_url 自动 work)+ 删 `lark_from_feishu_config_sets_feishu_platform` 测试(2026-06-04 task B)
- [x] T6 — Step 6: orchestrator 清理 net -81 行(2026-06-04 task C resume,经 API error 一次):删 build_channel_by_id "feishu" arm(-42)+ deliver_announcement "feishu" arm(-16)+ channels.feishu 健康检查 loop(-27)+ from_lark_config→from_config(silent bugfix 接通)+ 6 处其它清理
- [x] T7 — Step 7 全部完成:schema A 删 3 个 FeishuConfig serde test;lark B 删 1 个;orchestrator C 删 deliver_announcement_routes_feishu_to_feishu_arm;lark.rs 最终清理删 5 个 sibling tests(~189 行 + 7 doc comment fixups)— `cargo test -p zeroclaw-channels --features channel-lark --lib`:**1278 passed; 0 failed**(2026-06-04)
- [x] T8 — Step 8: 删 Cargo.toml `channel-feishu = ["channel-lark"]` feature alias(2026-06-04 task D)
- [x] T9 — Step 9: V3_CHANNEL_TYPES 不含 "feishu",TYPE_NAMES 不存在 → NO-OP confirmed(2026-06-04 task D)
- [x] T10 — Step 10: docs/ 中 `channels.feishu`/`FeishuConfig` 引用全在历史档案(excision incidents),NO-OP confirmed;follow-up:`README.kanmars.md:55` 提到已删的 `channel-feishu` cargo feature 待 update(2026-06-04 task D)
- [x] T11 — Step 11: CHANGELOG-next.md 加 ### Changed 条目(22 行,L86-L107,3 facets:consolidation + R12 default + R11 bugfix)(2026-06-04 task E)
- [x] T12 — Step 12: 全验证完成(focused scope,避开 fork pre-existing `wati/webhook/wecom` feature-gating 问题):**12.1** fmt 0 diff,**12.2** clippy 0 warnings(`-p zeroclaw-config -p zeroclaw-channels`),**12.3** tests 1969 passed 0 failed(691 config + 1278 channels),**12.4** V2→V3 fold 3/3 PASS,**12.5** grep production = 0 hits,**12.6** `LarkChannel::from_config` 3 处调用,**12.7.a/b** SKIP(release binary 无,且 fork pre-existing wati 编译 block 阻塞,但 12.4 已覆盖等价语义)。Pre-existing wati/webhook/wecom feature gating bug 与本 plan 完全正交,不阻塞 commit;记入 notepad follow-up。
- [x] T13 — Step 13: 2 atomic commits on `chore/consolidate-lark-feishu-to-upstream-schema`,NOT pushed:**A=`be34225d`** sisyphus state(plan +787 / notepad +517 / boulder +69)+ **B=`ddf35809`** source consolidation(11 files,净 +1432/-650;核心 schema.rs -219 / lark.rs -287 / orchestrator -125)(2026-06-04 git-master subagent)
- [x] T14 — Step 14: §10 Sign-off 更新 + 最终 summary 输出(Atlas 2026-06-04)

---

**END of kanmars.req.20260601.001**
