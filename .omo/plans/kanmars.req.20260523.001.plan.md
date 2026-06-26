# Plan — kanmars.req.20260523.001 (Inherit `agent.max_context_tokens` from `model.max_context_window`)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260523.001.plan |
| 关联需求 | 用户对话需求（2026-05-23）：gloria 实测发现 deepseek-v4-pro 支持 1M context，但 ZeroClaw `agent.max_context_tokens` 默认 32K + `threshold_ratio=0.5` → 19K 历史触发 26.7s 压缩。根因是 `agent.max_context_tokens` **不跟随 LLM 真实能力**，且 `runtime_profiles.<alias>.max_context_tokens` 字段是 dead config（grep 0 处使用）。要求：给 `ModelProviderConfig` 加 `max_context_window: Option<usize>` 字段，让 `agent.max_context_tokens` 默认继承之，否则 fallback 32K。 |
| 起草日期 | 2026-05-23 |
| 修订日期 | 2026-05-23 (rev0 — 初稿) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `feat/model-context-window-inheritance` ✅ **已创建**（基于 `kanmars_main`，与 0522.001 PR 同基线，功能正交无冲突） |
| 风险等级 | **Low-Medium**（`zeroclaw-config` Beta tier 加新 optional 字段 + 改一个现有字段类型为 Option；`zeroclaw-runtime`/`zeroclaw-channels` Experimental tier 调用点替换；新字段全部 `Default::default()` 向后 100% 兼容；不动 trait / security / gateway 边界） |
| 基线 commit | `bf5049e2b0b21ca336415fa2afc9b55763725d78`（`kanmars_main` HEAD，与 0522.001 PR 同基线）|
| 选型方案 | **方案 A — `ModelProviderConfig` 加 `max_context_window: Option<usize>` + `AliasedAgentConfig.max_context_tokens` 改类型为 `Option<usize>` + 引入 `DEFAULT_MAX_CONTEXT_TOKENS` 常量 + 2 个 helper method（Config + AliasedAgentConfig），优先级 agent.explicit > model.window > 32K** |
| 预计代码行数 | **+95 / -8**（含 schema 字段 + 类型改动 + 常量 + 2 helpers + Default impl + 5 调用点替换 + 6 新单测 + CHANGELOG，明细见 §7）|
| 预计工作量 | **AI agent 执行约 22-28 min**（含 CI wall-clock 10-15 min）/ **Sisyphus-Junior 人工节奏约 60 min**（不含已完成的 Step 0） |

---

## 0. 关键目标（唯一真理来源）

> **修正 ZeroClaw 配置陷阱：让 `agent.max_context_tokens` 默认跟随 LLM 真实能力（model.max_context_window），消除"DeepSeek 1M 但 ZeroClaw 默认 32K → 短消息就触发压缩"的根因。**

**完成此目标即"功能完成"**：

- 用户在 `[providers.models.deepseek.default]` 加一行 `max_context_window = 1000000`，**不**写 `[agents.default].max_context_tokens` → ZeroClaw runtime 用 1M 作 budget → 19K 不再触发压缩 → 省 26.7s
- 用户**显式**写 `[agents.default].max_context_tokens = 50000` → ZeroClaw 用 50K，**忽略** model.max_context_window（operator override 优先）
- 用户既不配 model.max_context_window 也不配 agent.max_context_tokens → fallback 32K（与 PR 前**完全一致**）
- 用户旧配置 `[agents.default].max_context_tokens = 32000`（显式）→ 反序列化为 `Some(32000)` → 用 32K（与 PR 前完全一致）
- 5 个 `agent.max_context_tokens` 使用点（orchestrator:7467 + loop_:3520/3904/3972/4042）全部走 helper resolver
- 不破坏现有 1191 + 88 个单元测试
- 引入 `pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 32_000;` 消除 3+ 处魔法数字

**显式不在范围内**：

- ❌ 不内置 per-model context window hardcoded 表（如 "deepseek-v4-pro 自动 = 1M"）—— 太脆弱，operator 必须读 model 文档显式配
- ❌ 不修 `runtime_profiles.<alias>.max_context_tokens` 的 dead inheritance —— 它是 ZeroClaw 历史包袱，本 PR 仅解决 model → agent 一条继承链；如要修 runtime_profile → agent，另起 PR
- ❌ 不动 `context_compression.threshold_ratio`（继续 user 配置的 0.5）
- ❌ 不重命名 `agent.max_context_tokens`（虽然命名误导，但改名是 breaking change，需 deprecation 周期）
- ❌ 不动 `classify_channel_reply_intent`（那是 0522.001 PR 的范围；本 PR 与之正交）
- ❌ 不动 `ModelProviderConfig.max_tokens`（output 上限，与本 PR 的 input context 上限是两个东西）
- ❌ 不为 cron / sop / heartbeat / delegate 加 max_context_window 字段 —— 它们走相同的 agent 配置，自动受益
- ❌ 不动 trait（`zeroclaw-api`）
- ❌ 不引入新依赖

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **SINGLE SOURCE OF TRUTH 铁律**（[zeroclaw AGENTS.md ABSOLUTE RULE](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md)）：
   - **`max_context_window` 是 model 物理属性的 source of truth** —— 新字段直接定义在 `ModelProviderConfig`，不引用其他地方。✅ 合规
   - **`max_context_tokens` 改 Option 是 schema 演化** —— 原 `usize + default 32K` 字段含义不变（compression trigger 阈值），仅添加"未配置"语义；同款字段不会出现在第二个地方。✅ 合规
   - ❌ 禁止把 `max_context_window` 字段复制到 `AliasedAgentConfig`（应通过 helper 解析继承）
   - ❌ 禁止在 `ChannelRuntimeContext` 加 `effective_max_context_tokens: usize` 缓存字段（必须 helper 每次调用即时解析，支持 hot-reload）
   - ✅ 允许：`context_token_budget` 字段保留（它是已计算的结果，每次构建 `ChannelRuntimeContext` 时通过 helper 重算）
2. **不新增 `unwrap()` / `expect()`**（[zeroclaw AGENTS.md Anti-Patterns](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md#anti-patterns)）
3. **不新增 `#[allow(dead_code)]`**
4. **`tracing::` 日志保持英文**（RFC #5653 §4.6）
5. **不引入新依赖**
6. **完整跑** `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test`
7. **One concern per PR**：本 PR 一个关注点 = "model.max_context_window → agent.max_context_tokens 继承机制"。不与 classifier_provider（0522.001 PR）、runtime_profile dead config 修复、agent 字段重命名混合
8. **CHANGELOG-next.md 必须更新**（用户可见的新字段 + 行为变化）
9. **配置向后 100% 兼容**：
   - 所有现有 `[agents.*]` 不带 `max_context_tokens` 字段 → 加载后**行为不变**（除非 model 也配了 `max_context_window`，这正是本 PR 目的）
   - 所有现有 `[agents.*]` 显式 `max_context_tokens = N` → 反序列化为 `Some(N)`，行为与原 `usize` 一致
   - 所有现有 `[providers.models.*.*]` 不带 `max_context_window` → 字段是 None，继承链 fallback 32K（与原行为一致）
10. **Risk Tier 自评**：跨 Beta(`zeroclaw-config`) + Experimental(`zeroclaw-channels` + `zeroclaw-runtime`) crate；按 [AGENTS.md Risk Tiers](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md#risk-tiers) 取**更高级别 Low-Medium**
11. **基线分支**：从 `kanmars_main`（不是 `master`，也不是 0522.001 的 `feat/classifier-provider-per-agent-override` 分支）拉新分支 `feat/model-context-window-inheritance`，基线 commit `bf5049e2`。**本 PR 与 0522.001 PR 完全正交**，没有 import/struct/字段冲突，两者可独立 merge。

---

## 1. 现状事实复核（Step 0 已实地验证，行号对齐 `kanmars_main` @ `bf5049e2`）

### 1.1 关键代码位置

| 事实 | 文件:行 | Step 0 验证 |
|---|---|---|
| **`AliasedAgentConfig.max_context_tokens` 当前定义**（待改类型 `usize` → `Option<usize>`）| [crates/zeroclaw-config/src/schema.rs:2821-2825](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2821-L2825) | ✅ grep |
| 字段语义注释（"Maximum estimated tokens for conversation history before compaction triggers" — 实际是 compression trigger 阈值，不是 LLM 上限） | schema.rs:2822-2824 | ✅ |
| `default_agent_max_context_tokens()` 函数返回 32_000 | [schema.rs:4407-4409](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L4407-L4409) | ✅ grep |
| **`AliasedAgentConfig::default()` 内的 `max_context_tokens` 初始化**（待改 `None`）| [schema.rs:2938](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2938) | ✅ grep |
| **`ModelProviderConfig` 定义**（待加 `max_context_window` 字段） | [schema.rs:635-711](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L635-L711) | ✅ grep |
| ModelProviderConfig 内**没有** `context_window` / `max_input_tokens` / `context_length` 字段（grep 0 hits）| 同上 | ✅ grep |
| `ModelProviderConfig.max_tokens` (output 上限，与 input context 无关) | schema.rs:~684 | ✅ |
| **`Config::resolved_model_provider_for_agent` 现成 helper**（新 helper 照抄风格） | [schema.rs:3116-3098](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L3088-L3098) | ✅ grep |
| `Config::model_provider_for_agent` helper（接受 agent_alias，返回 `&ModelProviderConfig`） | [schema.rs:3074-3078](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L3074-L3078) | ✅ grep |
| **`context_token_budget` 在 ChannelRuntimeContext 的赋值点**（5 处调用之一，最易改） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:7467](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L7467) | ✅ grep |
| **`agent.max_context_tokens` 在 loop_.rs 4 处使用**（待改为 helper 调用） | [crates/zeroclaw-runtime/src/agent/loop_.rs:3520](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-runtime/src/agent/loop_.rs#L3520), [:3904](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-runtime/src/agent/loop_.rs#L3904), [:3972](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-runtime/src/agent/loop_.rs#L3972), [:4042](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-runtime/src/agent/loop_.rs#L4042) | ✅ grep |
| loop_.rs 4 处使用点 scope 中**有 `config` 变量**（可拿到 `&Config`）| 见 §1.4 上下文分析 | ✅ grep |
| `ContextCompressor::new(config, context_window: usize)` 接口（消费方，第 2 参数即 budget） | [crates/zeroclaw-runtime/src/agent/context_compressor.rs:~190](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-runtime/src/agent/context_compressor.rs) | ✅ grep |
| `RuntimeProfileConfigOverride.max_context_tokens: Option<usize>`（runtime_profile 字段，是 dead config，grep 0 处读取） | [schema.rs:8910](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L8910) | ✅ grep |
| `crates/zeroclaw-config/fixtures/v1.toml:149` 含 `max_context_tokens = 32000`（migration 测试 fixture，反序列化为 Some(32000) 行为不变） | fixtures/v1.toml:149 | ✅ grep |

### 1.2 用户实证证据（gloria 部署 16:49:57 日志）

[`/Users/kanmars/workspace/kanmars_gloria/A/logs/zeroclaw.log`](file:///Users/kanmars/workspace/kanmars_gloria/A/logs/zeroclaw.log) 中"现在白银价格是多少"消息触发链：

| 时刻 | 事件 | 数据 |
|---|---|---|
| 16:49:58.063 | inbound → 处理开始 | "现在白银价格是多少" |
| 16:49:58.099 | memory recall | 34ms |
| 16:49:58.099 → 16:50:24.829 | **context compression** | tokens_before=19035 → tokens_after=9654, passes=1, **耗时 26.7 秒** |
| 16:50:24.829 → 16:50:27.093 | classifier (kimi-k2.5, 0522.001 PR 生效) | ~2.3s |
| 16:50:27.093 | starting LLM call | elapsed_before_llm_ms=29029 |

**根因计算**：
- 用户 [`runtime_profiles.default.max_context_tokens = 1_000_000`](file:///Users/kanmars/workspace/kanmars_gloria/A/malorian-3516/config.toml) **是 dead config**（grep 0 处使用）
- 用户 `[agents.default]` 不显式配 `max_context_tokens` → 走默认 `default_agent_max_context_tokens() = 32_000`
- `context_compression.threshold_ratio = 0.5`
- threshold = 32,000 × 0.5 = **16,000 tokens**
- tokens_before = 19,035 > 16,000 → **触发压缩** ✅

**本 PR 修复后**：
- 用户在 `[providers.models.deepseek.default]` 加 `max_context_window = 1_000_000`
- `[agents.default].max_context_tokens` 仍不配（None）→ helper 继承 model.max_context_window = 1,000,000
- threshold = 1,000,000 × 0.5 = **500,000 tokens**
- 19,035 << 500,000 → **不触发** ✅ → 省 26.7s

### 1.3 当前 schema 字段定义（待改）

```rust
// crates/zeroclaw-config/src/schema.rs:2821-2825（待改）
/// Maximum estimated tokens for conversation history before compaction triggers.
/// Uses ~4 chars/token heuristic. When this threshold is exceeded, older messages
/// are summarized to preserve context while staying within budget. Default: `32000`.
#[serde(default = "default_agent_max_context_tokens")]
pub max_context_tokens: usize,
```

### 1.4 loop_.rs 4 处使用点 scope 分析（Step 0 已 grep）

| line | 上下文 | 是否有 `&Config` ref | 是否有 `agent_alias` |
|---|---|---|---|
| 3520 | CLI mode 调用，紧邻 `&config.pacing` | ✅ 有 `config` 变量 | ⚠️ 需 grep 函数签名确认 |
| 3904 | interactive CLI 调用，紧邻 `&config.pacing` | ✅ 有 `config` 变量 | ⚠️ 待确认 |
| 3972 | `ContextCompressor::new(agent.context_compression.clone(), agent.max_context_tokens)` | ⚠️ 有 `agent` 变量；config 不确定 | ⚠️ 待确认 |
| 4042 | 同 3972 | ⚠️ 同 3972 | ⚠️ 待确认 |

**结论**：3520 / 3904 几乎肯定能拿到 `(&Config, &AliasedAgentConfig)`，可直接调 `AliasedAgentConfig::resolved_max_context_tokens(model_cfg)` —— 实施时 sub-agent 先用 `config.model_provider_for_agent(&agent_alias)` 拿到 model_cfg，传给 helper。

3972 / 4042 在 `ContextCompressor::new` 调用旁边，可能没 config ref：**fallback 策略**：sub-agent 实施时如果发现 scope 不够：
- **选项 A**：从外层注入 `config` 引用（小幅 refactor 上层函数签名）
- **选项 B**：暂时传 `agent.max_context_tokens.unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS)`（不享受继承，但向后兼容）+ 加 TODO 注释

Plan **建议优先 A**，B 作为兜底（不阻塞 PR）。

---

## 2. 方案选型（已决策）

### 2.1 选项对比

| 方案 | 描述 | 优点 | 缺点 | 决策 |
|---|---|---|---|---|
| **A — `Option<usize>` + helper inheritance** | model 加 `max_context_window: Option<usize>`；agent.max_context_tokens 改 `Option<usize>`；2 个 helper 封装继承链 | 完全向后兼容；语义明确；2 个 helper DRY；旧配置零感知 | agent 字段类型变化（usize → Option<usize>），4 处 loop_.rs 调用必须改 | ✅ **采纳** |
| B — `usize` + sentinel 0 表示"继承" | agent.max_context_tokens = 0 表示 inherit，>0 表示 explicit | 不变字段类型 | sentinel 模式反直觉；旧配置 `= 0` 行为变化（虽几乎无人这么写） | ❌ |
| C — 内置 hardcoded per-model context window 表 | `match model { "deepseek-v4-pro" => 1_000_000, ... }` | 用户零配置 | 表维护成本高；同模型不同 endpoint 限制不同（dashscope vs 直连）；overshoot 风险 | ❌ |
| D — 让 ZeroClaw 启动期向 provider 查询 context window | 调 OpenAI `/models` API 自动发现 | 自动 | 启动延迟；并非所有 provider 都暴露此 API；网络依赖 | ❌ |
| E — 改 `default_agent_max_context_tokens()` 默认值从 32K 调大到 200K | 一行改动 | 极简 | 不解决"DeepSeek 1M / Claude 200K / GPT 128K 各异"的根本问题；可能 overshoot 8K 模型 | ❌ |

**选 A 的核心理由**：

1. **直接命中根因**：model 物理属性放 model 配置（SSOT 合规），agent 用户偏好放 agent 配置（operator override）
2. **完全向后兼容**：旧配置零感知；新功能 opt-in
3. **与现有 ZeroClaw 配置风格一致**：参照 `tts_provider` / `transcription_provider` 等 `Option<T>` 配置模式
4. **2 个 helper DRY**：5 个使用点一处定义，避免逻辑分散

### 2.2 字段命名决策

| 候选 | 决策 | 理由 |
|---|---|---|
| **`max_context_window: Option<usize>`** | ✅ **采纳**（用户已确认） | 与 `max_tokens`（output 上限）形成对照；语义明确"上下文窗口上限"；与社区惯用名（如 OpenAI docs）一致 |
| `context_window` | ❌ | 无 `max_` 前缀，含义模糊（是上限还是当前值？） |
| `context_length` | ❌ | "length" 含义重叠 prompt length，不如 window 清晰 |
| `input_token_limit` | ❌ | 与 `max_tokens`（output）平行命名，但 ZeroClaw 已用 max_ 前缀，统一更好 |

### 2.3 类型设计（D2 解决方案 — 用户已 confirm）

**核心问题**：现有 `pub max_context_tokens: usize` + `default = "default_agent_max_context_tokens"(返回 32000)` 无法区分"未配置"和"显式配 = 32000"。

**解决**：改为 `pub max_context_tokens: Option<usize>` + `#[serde(default)]`（默认 None）。

**反序列化矩阵**：

| TOML 形式 | 反序列化后 | 语义 |
|---|---|---|
| 不写字段 | `None` | "我没意见，继承 model.max_context_window，否则 fallback 32K" |
| `max_context_tokens = 32000` | `Some(32000)` | "我就要 32K，不管 model" |
| `max_context_tokens = 100000` | `Some(100000)` | "我就要 100K" |

**向后兼容验证**（用户已 confirm）：

| 旧配置 | 旧行为 | 新行为 | 是否破坏 |
|---|---|---|---|
| 不写 + model 不配 max_context_window | 32K | 32K | ✅ 一致 |
| 显式 `= 32000` | 32K | 32K | ✅ 一致 |
| 显式 `= N` | N | N | ✅ 一致 |
| **不写 + model 新配 `max_context_window = 1M`** | **32K** | **1M** | ⚠️ **意图变化，符合本 PR 目的** |

### 2.4 helper 设计

**两层 helper**（参考 §1.1 现有 `model_provider_for_agent` / `resolved_model_provider_for_agent` 风格）：

```rust
// 1. AliasedAgentConfig 上的 method — 给已经拿到 model_cfg ref 的调用方用（loop_.rs）
impl AliasedAgentConfig {
    /// Returns the effective `max_context_tokens` for this agent.
    /// Priority: explicit `self.max_context_tokens` > `model_cfg.max_context_window` > `DEFAULT_MAX_CONTEXT_TOKENS` (32K).
    #[must_use]
    pub fn resolved_max_context_tokens(
        &self,
        model_cfg: Option<&ModelProviderConfig>,
    ) -> usize {
        if let Some(explicit) = self.max_context_tokens {
            return explicit;
        }
        if let Some(mc) = model_cfg
            && let Some(window) = mc.max_context_window
        {
            return window;
        }
        DEFAULT_MAX_CONTEXT_TOKENS
    }
}

// 2. Config 上的 method — 给 orchestrator 用，封装 model lookup
impl Config {
    /// Returns the effective `max_context_tokens` for the given agent alias.
    /// Looks up the agent's `model_provider`, then delegates to
    /// `AliasedAgentConfig::resolved_max_context_tokens`. Returns
    /// `DEFAULT_MAX_CONTEXT_TOKENS` (32K) when the agent does not exist.
    #[must_use]
    pub fn resolved_max_context_tokens_for_agent(
        &self,
        agent_alias: &str,
    ) -> usize {
        let Some(agent) = self.agents.get(agent_alias) else {
            return DEFAULT_MAX_CONTEXT_TOKENS;
        };
        agent.resolved_max_context_tokens(self.model_provider_for_agent(agent_alias))
    }
}

// 3. 常量 — 消除 3+ 处魔法数字
pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 32_000;
```

**5 个使用点替换**：

| 调用点 | 替换为 | 备注 |
|---|---|---|
| orchestrator/mod.rs:7467 `agent.max_context_tokens` | `config.resolved_max_context_tokens_for_agent(&agent_alias)` | 有 `&Config + agent_alias`，最干净 |
| loop_.rs:3520 / 3904 `agent.max_context_tokens` | `agent.resolved_max_context_tokens(config.model_provider_for_agent(&agent_alias))` | scope 有 config 引用 |
| loop_.rs:3972 / 4042 `agent.max_context_tokens` | 同上，或 fallback `agent.max_context_tokens.unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS)` 如 scope 不够 | 实施时按 §1.4 选项 A/B 决策 |

---

## 3. 实施步骤（5 处编辑，跨 3 文件）

### Step 0 — 分支准备 + 前置 grep ✅ **已完成**

```bash
cd /Users/kanmars/workspace/kanmars_zeroclaw_github
git checkout kanmars_main                                         # bf5049e2
git checkout -b feat/model-context-window-inheritance
# ✅ Switched to a new branch
```

**Step 0 grep 结果**（已嵌入 §1.1 表格）：

| grep | 关键发现 |
|---|---|
| `AliasedAgentConfig.max_context_tokens` | 类型 `usize` @ schema.rs:2825，默认 32_000 @ schema.rs:4407，Default impl @ schema.rs:2938 |
| `ModelProviderConfig` 现有字段 | 无 `context_window` / `max_input_tokens` / `context_length`，可安全新加 |
| 5 个 `agent.max_context_tokens` 使用点 | orchestrator:7467 + loop_:3520/3904/3972/4042 全确认 |
| loop_.rs scope | 3520 / 3904 有 `&config` ref；3972 / 4042 待 sub-agent 实施时确认 |
| `runtime_profiles.<alias>.max_context_tokens` | dead config（grep 整代码库 0 处读取），不修复（超出本 PR 范围） |

### Step 1 — Schema 改动 + helpers（zeroclaw-config crate）

#### 1a. `ModelProviderConfig` 加 `max_context_window` 字段

**位置**：[schema.rs:~684 ModelProviderConfig 内](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L684)，紧邻现有 `max_tokens` 字段（output 上限）后。

```rust
/// Maximum context window of this model in tokens, as documented by the provider.
/// Used as the default for `agent.max_context_tokens` when the agent does not
/// override. `None` (default) means ZeroClaw has no model-side hint → falls back
/// to `DEFAULT_MAX_CONTEXT_TOKENS` (32K) unless the agent overrides.
///
/// This is the model's INPUT capacity (history + system prompt + user message),
/// distinct from `max_tokens` above which is the OUTPUT generation cap.
///
/// Common values (operator must check provider docs):
///   - DeepSeek-V4-Pro / V4-Flash: 1_000_000
///   - Claude 3.5/4 Sonnet / Opus: 200_000
///   - GPT-4o / GPT-4-Turbo: 128_000
///   - Qwen3.6 series (dashscope coding plan): 1_000_000
///   - Kimi-K2.5 / K2.6: 262_144
///   - Moonshot-Kimi-K2-Instruct: 131_072
#[serde(default, skip_serializing_if = "Option::is_none")]
pub max_context_window: Option<usize>,
```

#### 1b. `AliasedAgentConfig.max_context_tokens` 改类型

**位置**：[schema.rs:2821-2825](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2821-L2825)

```rust
// Before:
#[serde(default = "default_agent_max_context_tokens")]
pub max_context_tokens: usize,

// After:
/// Maximum estimated tokens for conversation history before compaction triggers.
/// Uses ~4 chars/token heuristic. When this threshold is exceeded, older messages
/// are summarized to preserve context while staying within budget.
///
/// `None` (default) inherits from the resolved model's `max_context_window`;
/// if neither this field nor `max_context_window` is set, falls back to
/// `DEFAULT_MAX_CONTEXT_TOKENS` (32K). Set explicitly to override the model hint
/// (e.g. for cost capping: even if model supports 1M, you may only want to pay
/// for 100K of input per request).
///
/// Use `Config::resolved_max_context_tokens_for_agent` or
/// `AliasedAgentConfig::resolved_max_context_tokens` to consume this field —
/// never read it directly (you'd miss the inheritance chain).
#[serde(default)]
pub max_context_tokens: Option<usize>,
```

#### 1c. `default_agent_max_context_tokens()` 函数 → 删除 + 引入 `DEFAULT_MAX_CONTEXT_TOKENS` 常量

**删除**：[schema.rs:4407-4409](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L4407-L4409) 整段（不再被引用）

**新增**：常量定义（紧邻 ModelProviderConfig 或文件顶部 const 区域）

```rust
/// Fallback `max_context_tokens` budget when neither the agent nor its resolved
/// model provides an explicit value. Conservative 32K matches the historical
/// default of `AliasedAgentConfig::max_context_tokens` before this field became
/// optional. Operators wanting to take advantage of long-context models should
/// set `max_context_window` on the model provider (preferred) or
/// `max_context_tokens` on the agent.
pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 32_000;
```

#### 1d. `AliasedAgentConfig::default()` 改 `None`

**位置**：[schema.rs:2938](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2938)

```rust
// Before:
max_context_tokens: default_agent_max_context_tokens(),

// After:
max_context_tokens: None,
```

#### 1e. 2 个 helper method

**位置**：紧挨 [`Config::model_provider_for_agent` @ schema.rs:3074](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L3074) 之后

```rust
// 见 §2.4 helper 设计，照搬即可。
```

注：`AliasedAgentConfig::resolved_max_context_tokens` 加在 `impl AliasedAgentConfig` 内（如果还没有，新建 `impl AliasedAgentConfig { ... }` block 紧邻 struct 定义）。

#### 1f. 单测（3 个）

加到 `#[cfg(test)] mod tests` 内（同 0522.001 PR 的位置）：

```rust
#[test]
async fn resolved_max_context_tokens_uses_explicit_agent_value() {
    let mut agent = AliasedAgentConfig::default();
    agent.max_context_tokens = Some(50_000);
    let model_cfg = ModelProviderConfig {
        max_context_window: Some(1_000_000),
        ..Default::default()
    };
    assert_eq!(agent.resolved_max_context_tokens(Some(&model_cfg)), 50_000,
        "explicit agent override must win over model hint");
}

#[test]
async fn resolved_max_context_tokens_inherits_from_model_when_agent_unset() {
    let agent = AliasedAgentConfig::default();  // max_context_tokens = None
    let model_cfg = ModelProviderConfig {
        max_context_window: Some(1_000_000),
        ..Default::default()
    };
    assert_eq!(agent.resolved_max_context_tokens(Some(&model_cfg)), 1_000_000,
        "unset agent must inherit model.max_context_window");
}

#[test]
async fn resolved_max_context_tokens_falls_back_to_default_when_both_unset() {
    let agent = AliasedAgentConfig::default();
    let model_cfg = ModelProviderConfig::default();  // max_context_window = None
    assert_eq!(agent.resolved_max_context_tokens(Some(&model_cfg)),
        DEFAULT_MAX_CONTEXT_TOKENS,
        "both unset must fall back to 32K");
    assert_eq!(agent.resolved_max_context_tokens(None),
        DEFAULT_MAX_CONTEXT_TOKENS,
        "None model_cfg must also fall back");
}
```

#### 1g. 验证

```bash
cargo check -p zeroclaw-config
cargo test -p zeroclaw-config resolved_max_context_tokens
```

预期：编译通过 + 3 测试 passed。

### Step 2 — `orchestrator/mod.rs:7467` 替换

**位置**：[crates/zeroclaw-channels/src/orchestrator/mod.rs:7467](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L7467)

```rust
// Before:
context_token_budget: agent.max_context_tokens,

// After:
context_token_budget: config.resolved_max_context_tokens_for_agent(&agent_alias),
```

**注意**：确认 scope 内有 `config: &Config` + `agent_alias: &String/&str`。如果只有 `&AliasedAgentConfig`，改用 `agent.resolved_max_context_tokens(config.model_provider_for_agent(&agent_alias))`。

### Step 3 — `loop_.rs` 4 处替换

**4 处**：3520 / 3904 / 3972 / 4042

按 §2.4 + §1.4 策略：

```rust
// 通用模式（推荐，scope 有 config + agent_alias）：
// Before:
agent.max_context_tokens,

// After:
agent.resolved_max_context_tokens(config.model_provider_for_agent(&agent_alias)),

// 退化模式（仅当 scope 不够，sub-agent 在实施时判断）：
agent.max_context_tokens.unwrap_or(zeroclaw_config::schema::DEFAULT_MAX_CONTEXT_TOKENS),
// + TODO 注释：「scope 不含 &Config，无法继承 model 提示；后续 PR 重构上层签名」
```

**实施时 sub-agent 需要**：
1. 先 read 4 处上下文确认 scope（有无 `config` 和 `agent_alias`）
2. 优先用通用模式
3. 不够时退化 + TODO

### Step 4 — Schema 单测扩展（3 个端到端集成）

加到 `#[cfg(test)] mod tests` 内（同 Step 1f 位置）：

```rust
#[test]
async fn config_resolved_max_context_tokens_for_agent_full_inheritance() {
    let toml = r#"
        [providers.models.deepseek.default]
        api_key = "k"
        model = "deepseek-v4-pro"
        max_context_window = 1000000

        [risk_profiles.default]
        level = "supervised"

        [agents.default]
        enabled = true
        model_provider = "deepseek.default"
        risk_profile = "default"
        # max_context_tokens not set → inherit
    "#;
    let cfg: Config = toml::from_str(toml).unwrap();
    cfg.validate().unwrap();
    assert_eq!(cfg.resolved_max_context_tokens_for_agent("default"), 1_000_000);
}

#[test]
async fn config_resolved_max_context_tokens_for_agent_explicit_override() {
    let toml = r#"
        [providers.models.deepseek.default]
        api_key = "k"
        model = "deepseek-v4-pro"
        max_context_window = 1000000

        [risk_profiles.default]
        level = "supervised"

        [agents.default]
        enabled = true
        model_provider = "deepseek.default"
        risk_profile = "default"
        max_context_tokens = 50000
    "#;
    let cfg: Config = toml::from_str(toml).unwrap();
    cfg.validate().unwrap();
    assert_eq!(cfg.resolved_max_context_tokens_for_agent("default"), 50_000,
        "explicit agent override must win over model.max_context_window");
}

#[test]
async fn config_resolved_max_context_tokens_for_agent_fallback_32k() {
    let toml = r#"
        [providers.models.deepseek.default]
        api_key = "k"
        model = "deepseek-v4-pro"
        # no max_context_window

        [risk_profiles.default]
        level = "supervised"

        [agents.default]
        enabled = true
        model_provider = "deepseek.default"
        risk_profile = "default"
        # no max_context_tokens
    "#;
    let cfg: Config = toml::from_str(toml).unwrap();
    cfg.validate().unwrap();
    assert_eq!(cfg.resolved_max_context_tokens_for_agent("default"),
        DEFAULT_MAX_CONTEXT_TOKENS);
}
```

### Step 5 — 静态检查 + 全测试

```bash
cargo fmt -- crates/zeroclaw-config/src/schema.rs \
             crates/zeroclaw-channels/src/orchestrator/mod.rs \
             crates/zeroclaw-runtime/src/agent/loop_.rs

cargo clippy -p zeroclaw-config --all-targets -- -D warnings
cargo clippy -p zeroclaw-channels --features channel-lark --all-targets -- -D warnings
cargo clippy -p zeroclaw-runtime --all-targets -- -D warnings

cargo test -p zeroclaw-config
cargo test -p zeroclaw-channels --features channel-lark --lib
cargo test -p zeroclaw-runtime --lib
```

**预期**：
- zeroclaw-config 单测：88 pre-existing + 6 新（3 helper + 3 integration） = **94 passed**
- zeroclaw-channels 单测：1191 passed
- zeroclaw-runtime 单测：维持现状
- pre-existing 5 个 lark.rs clippy errors **不变**（baseline 已确认）

### Step 6 — CHANGELOG-next.md 更新

```markdown
### Added

- **providers/models**: Added `max_context_window: Option<usize>` field to
  `ModelProviderConfig` documenting the model's input context window in tokens.
  When the agent does not override, `agent.max_context_tokens` now inherits this
  value. Common settings (operator must verify against provider docs):

      [providers.models.deepseek.default]
      max_context_window = 1000000   # DeepSeek V4 series

      [providers.models.anthropic.default]
      max_context_window = 200000    # Claude 3.5/4 family

      [providers.models.openai.default]
      max_context_window = 128000    # GPT-4o / GPT-4-Turbo

### Changed

- **agents**: `AliasedAgentConfig.max_context_tokens` is now
  `Option<usize>` (was `usize` with default 32000). When unset (the new default),
  the runtime inherits from the resolved model's `max_context_window`; if both
  are unset, falls back to `DEFAULT_MAX_CONTEXT_TOKENS` (32K). Explicit values
  (including `= 32000`) continue to take precedence over the model hint —
  operators wanting strict cost capping can keep their existing values
  unchanged.

  Previously this field always read 32K when unwritten, ignoring the model's
  real capacity (e.g. DeepSeek-V4 1M, Claude 200K). The result was unnecessary
  context-compression cycles on long histories (gloria observed 26.7s
  compression on a 19K-token history because 19K > 32K × 0.5 threshold).

  **Backward compatibility**: All existing configs continue to work unchanged.
  The new field's introduction is purely additive; the only behavior change is
  for configs that combine `agent.max_context_tokens` unset + a new
  `model.max_context_window` value (which is the intended use case of this
  feature).
```

### Step 7 — Atomic commit + push

```bash
git status --short
git add crates/zeroclaw-config/src/schema.rs \
        crates/zeroclaw-channels/src/orchestrator/mod.rs \
        crates/zeroclaw-runtime/src/agent/loop_.rs \
        CHANGELOG-next.md \
        .sisyphus/plans/kanmars.req.20260523.001.plan.md
git diff --stat HEAD                # 期望 5 文件
git commit -F - <<'EOF'
feat(config): inherit agent.max_context_tokens from model.max_context_window

ZeroClaw's `agent.max_context_tokens` previously defaulted to 32K via a
hardcoded default function, regardless of the underlying model's real
context capacity. This caused two visible problems:

  1. Operators using long-context models (DeepSeek V4 1M, Claude 200K,
     GPT-4o 128K) would silently get the 32K cap unless they knew to
     override `agent.max_context_tokens` explicitly. With
     `context_compression.threshold_ratio = 0.5` (default), even 19K
     conversations triggered the LLM-powered context compression — a
     26.7-second hit per message in gloria's deepseek-v4-pro setup.

  2. The closest-named config field (`runtime_profiles.<alias>
     .max_context_tokens`) looked like it should override the agent
     default, but grep across the codebase confirms zero call sites
     actually read it — it's a dead inheritance field. Operators (this
     one included) configured it confidently, only to discover it was
     ignored.

This patch fixes the root cause by introducing model-provider-level
context-window declaration and inheritance:

  * `ModelProviderConfig.max_context_window: Option<usize>` — new field
    where operators declare the model's documented input capacity. Common
    values are listed inline (`200_000` for Claude, `1_000_000` for
    DeepSeek-V4 / Qwen3.6, `262_144` for Kimi-K2.5).

  * `AliasedAgentConfig.max_context_tokens` changed from `usize`
    (default 32_000) to `Option<usize>` (default None). Existing configs
    with explicit values continue to work unchanged thanks to
    `Some(n)`/`None` distinguishing user intent.

  * New `DEFAULT_MAX_CONTEXT_TOKENS: usize = 32_000` constant replaces
    the previous `default_agent_max_context_tokens()` function, used as
    the final fallback in the resolution chain.

  * Two helpers in `zeroclaw-config/src/schema.rs`:
      - `AliasedAgentConfig::resolved_max_context_tokens(model_cfg)`
      - `Config::resolved_max_context_tokens_for_agent(alias)`
    Both apply the priority `agent.explicit > model.window > 32K`.

  * Five call sites updated to route through the helper:
      - orchestrator/mod.rs:7467 (ChannelRuntimeContext construction)
      - loop_.rs:3520, 3904, 3972, 4042 (CLI / interactive paths)

SSOT compliance: `max_context_window` is the source of truth for the
model's physical capacity, declared once on the provider. The agent
field is an operator override that takes priority when set. No field
is duplicated; the runtime cache (`ChannelRuntimeContext.context_token_budget`)
is recomputed from `Config` on every channel-context construction,
preserving hot-reload semantics.

Backward compatibility:
  - Configs not touching either field: still get 32K (identical
    pre-PR behavior).
  - Configs with explicit `agent.max_context_tokens = N`: still get
    N (Some(N) deserialization is equivalent).
  - Configs that add the new `model.max_context_window` but leave
    `agent.max_context_tokens` unset: get the model's value — the
    intended improvement, opt-in.

Validation: existing 88 zeroclaw-config tests pass; 6 new tests cover
the resolver matrix (explicit / inherit / fallback) at both the
agent-level and config-level helpers; 1191 zeroclaw-channels tests
pass unchanged.

Not in this PR (deferred follow-ups):
  - Internal `runtime_profiles.<alias>.max_context_tokens` dead-config
    fix (separate concern; would change another inheritance link).
  - Field rename `agent.max_context_tokens` →
    `agent.compression_trigger_tokens` for naming clarity (breaking
    change, needs deprecation cycle).
  - Auto-discovery of model context windows via OpenAI `/models`
    endpoint (provider-specific, network-dependent).

Risk: Low-Medium (Beta `zeroclaw-config` + Experimental
`zeroclaw-channels`/`zeroclaw-runtime`; new field is opt-in
default-off; agent field type change is fully backward-compatible per
the deserialization matrix in the plan). See plan
`.sisyphus/plans/kanmars.req.20260523.001.plan.md` for the full
rationale.

Co-authored-by: Sisyphus <sisyphus@ohmyopencode.local>
EOF
git push -u origin feat/model-context-window-inheritance
```

---

## 4. 验证清单（PR 提交前必须全绿）

| # | 项 | 命令 | 预期 |
|---|---|---|---|
| V1 | model 新字段 | `grep -nE "pub max_context_window: Option<usize>" crates/zeroclaw-config/src/schema.rs` | 1 行 |
| V2 | agent 字段已改 Option | `grep -nE "pub max_context_tokens: Option<usize>" crates/zeroclaw-config/src/schema.rs` | 1 行 |
| V3 | 常量定义 | `grep -nE "pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 32_000" crates/zeroclaw-config/src/schema.rs` | 1 行 |
| V4 | 旧 default fn 删除 | `grep -nE "fn default_agent_max_context_tokens" crates/zeroclaw-config/src/schema.rs` | 0 行 |
| V5 | 2 个 helper 存在 | `grep -nE "fn resolved_max_context_tokens" crates/zeroclaw-config/src/schema.rs` | ≥ 2 行 |
| V6 | orchestrator 调用点替换 | `grep -nE "resolved_max_context_tokens" crates/zeroclaw-channels/src/orchestrator/mod.rs` | ≥ 1 行 |
| V7 | loop_.rs 4 处替换 | `grep -nE "resolved_max_context_tokens\|DEFAULT_MAX_CONTEXT_TOKENS" crates/zeroclaw-runtime/src/agent/loop_.rs` | ≥ 4 行 |
| V8 | 旧字段直接访问已无 | `grep -nE "agent\.max_context_tokens[^.]" crates/zeroclaw-runtime/src/agent/loop_.rs crates/zeroclaw-channels/src/orchestrator/mod.rs` | 0 行（除 helper 内部）|
| V9 | Default impl 已改 None | `grep -nE "max_context_tokens: None" crates/zeroclaw-config/src/schema.rs` | ≥ 1 行 |
| V10 | 不引入新依赖 | `git diff kanmars_main -- Cargo.toml 'crates/*/Cargo.toml'` | 无变更 |
| V11 | 不动 zeroclaw-api | `git diff kanmars_main -- crates/zeroclaw-api/` | 无变更 |
| V12 | SSOT 检查：未在 ctx 缓存独立字段 | `grep -nE "effective_max_context_tokens\|cached_context_budget" crates/zeroclaw-channels/src/orchestrator/mod.rs` | 0 行 |
| V13 | Format | `cargo fmt --all -- --check` | exit 0 |
| V14 | Lint zeroclaw-config | `cargo clippy -p zeroclaw-config --all-targets -- -D warnings` | exit 0 |
| V15 | Lint zeroclaw-channels | `cargo clippy -p zeroclaw-channels --features channel-lark --all-targets -- -D warnings` | exit 0（5 个 pre-existing lark.rs errors 与本 PR 无关，git stash 验证过）|
| V16 | Lint zeroclaw-runtime | `cargo clippy -p zeroclaw-runtime --all-targets -- -D warnings` | exit 0 |
| V17 | 新单测 (Step 1f + 4) | `cargo test -p zeroclaw-config resolved_max_context_tokens` | 6 passed |
| V18 | zeroclaw-config 全套 | `cargo test -p zeroclaw-config` | 88 pre-existing + 6 新 = 94 passed |
| V19 | zeroclaw-channels 全套 | `cargo test -p zeroclaw-channels --features channel-lark --lib` | 1191 passed（与 0522.001 PR 时数据一致）|
| V20 | zeroclaw-runtime 全套 | `cargo test -p zeroclaw-runtime --lib` | 维持 baseline |
| V21 | CHANGELOG 已写 | `grep -nE "max_context_window" CHANGELOG-next.md` | ≥ 1 行 |
| V22 | 改动文件数 | `git diff --stat HEAD~1..HEAD` | 5 文件（schema.rs + mod.rs + loop_.rs + CHANGELOG + plan）|

### 4.1 线上回归验证（部署 gloria 后用户实测）

| 场景 | 配置 | 期望 |
|---|---|---|
| **R1 — 老用户零感知** | 不写任何 `max_context_window`、不写 `max_context_tokens` | 行为与 PR 前完全一致（32K 阈值）|
| **R2 — gloria 实际场景** | gloria 加 `[providers.models.deepseek.default] max_context_window = 1000000` | "现在白银价格是多少"那条 19K 消息 **不再触发** 26.7s 压缩；total elapsed_before_llm_ms 从 29s 降至 ~3-5s |
| **R3 — 显式 cost 控制** | 加 `[agents.default] max_context_tokens = 100000` | 即使 model 配 1M，agent 也只用 100K |
| **R4 — 类型变化向后兼容** | 旧 fixtures/v1.toml `max_context_tokens = 32000` | 反序列化成 `Some(32000)`，validate + migrate 通过 |

---

## 5. 风险与缓解

| # | 风险 | 严重性 | 缓解 |
|---|---|---|---|
| R1 | **类型变化破坏现有调用方**：所有 `agent.max_context_tokens` 直接读取（不带 `.unwrap_or`）现在是 `Option<usize>`，编译报错 | 中（确定会发生）| Step 3 显式替换 4 处 loop_.rs 调用，外加 Step 2 替换 orchestrator；V8 grep 校验无遗漏 |
| R2 | **loop_.rs scope 拿不到 `&Config` ref，需要重构上层签名** | 中（实施时知道）| §1.4 给出 fallback B 方案（`agent.max_context_tokens.unwrap_or(DEFAULT)`），不阻塞但失去新功能 |
| R3 | **opt-in 失效 — 操作员忘了在 model 配 max_context_window，行为仍是 32K** | 低 | CHANGELOG 突出说明 + 常用值表内嵌注释 |
| R4 | **fixtures/v1.toml 历史 fixture 反序列化变化**（`= 32000` → `Some(32000)`）| 极低 | 类型 `Option<usize>` 接受裸数字反序列化为 `Some`，行为完全等价；migration test 应自动通过；V19 集成测试覆盖 |
| R5 | **下游 ContextCompressor::new(config, context_window: usize) 签名假设 usize** | 极低 | helper 返回 `usize`，调用方传 helper 输出，类型仍是 usize → 下游零改动 |
| R6 | **2 个 helper 名称太相似（`resolved_max_context_tokens` vs `resolved_max_context_tokens_for_agent`）** | 极低 | 命名与现有 `model_provider_for_agent` / `resolved_model_provider_for_agent` 一致 |
| R7 | **DeepSeek V4 1M 文档外的边界 — 极长输入下推理时间随 input 增长** | 低 | operator 责任（PR 文档已警告）；如发现，operator 可降到 `max_context_tokens = 200000` |
| R8 | **0522.001 PR 的 classifier 已用 kimi 但仍走 32K 触发的 compression** | 已知问题，本 PR 解决 | 即本 PR 目的 |

### 5.1 回退方案

如 PR merge + 部署后发现 R2 失败（loop_.rs scope 不够）：

1. **配置层回退**：用户在 `[agents.<alias>]` 显式写 `max_context_tokens = <model 实际值>`（手动模拟继承）→ 立即恢复
2. **代码层回退**：`git revert <commit_sha>`（单 commit，5 文件）

### 5.2 升级路径

如本 PR 大规模采用后用户反馈：

1. follow-up：fix `runtime_profiles.<alias>.max_context_tokens` dead config（让它真的覆盖 agent）
2. follow-up：rename `agent.max_context_tokens` → `agent.compression_trigger_tokens` + deprecation alias
3. follow-up：内置常用 model 的 `max_context_window` 默认值（如 `provider = "deepseek"` 自动设 1M）

---

## 6. 后续工作（不在本 PR 范围）

| 编号 | 待解决问题 | 建议优先级 |
|---|---|---|
| F1 | **`runtime_profiles.<alias>.max_context_tokens` dead config 修复** —— 让它真的覆盖 agent 默认（grep 0 处使用，是历史包袱）| Medium |
| F2 | **`agent.max_context_tokens` 重命名**为 `agent.compression_trigger_tokens` 消除命名误导，需 deprecation alias 跨 2 个 release | Low |
| F3 | **内置常用 model 的 `max_context_window` 默认值表**（如 `provider = "deepseek"` 自动 1M, `provider = "anthropic"` 自动 200K） | Low |
| F4 | **observability 增强** —— 在 context_compression 触发日志中加 `effective_budget` 字段，便于运维诊断 | Low |
| F5 | **回提上游**：本 PR 是行为优化 + 向后兼容，可考虑回提 zeroclaw-labs upstream | Low |

---

## 7. 工作量估算 & 时间线

### 7.1 双轨估算

| 阶段 | 行数 | AI agent 估 | Sisyphus-Junior 人工估 |
|---|---|---|---|
| Step 0（分支 + 前置 grep） | — | ✅ 已完成 | ✅ 已完成 |
| Step 1（schema 改动 — 1a/b/c/d/e/f 全部） | +60 / -8 | **8 min** | 30 min |
| Step 2（orchestrator/mod.rs:7467 替换） | +1 / -1 | **1 min** | 3 min |
| Step 3（loop_.rs 4 处替换） | +8 / -4 | **5 min**（含 scope 探查）| 12 min |
| Step 4（schema 集成测试 3 个） | +50 / 0 | 同 Step 1 (合并) | 10 min |
| Step 5（fmt + clippy + test） | — | **10-15 min** wall-clock | 10 min |
| Step 6（CHANGELOG-next.md） | +15 / 0 | **1 min** | 5 min |
| Step 7（commit + push） | — | **1 min** | 5 min |
| **合计** | **≈ +135 / -8**（不含 plan）| **≈ 26-31 min**（含 CI 等待 10-15 min） | ≈ 75 min |

### 7.2 关键不确定项

| # | 不确定项 | 触发后增量 |
|---|---|---|
| U1 | loop_.rs 3972/4042 scope 不含 `config` ref → 需重构上层签名 | +5-10 min |
| U2 | cargo test 首跑发现 fixtures/v1.toml migration 失败 | +5 min（应该不会发生，Option 反序列化兼容）|
| U3 | clippy 抱怨 `if let Some(n) = ... { return n; }` 形式可改 `unwrap_or_else` | +1 min |
| U4 | 单测的 TOML preamble 同 0522.001 一样需补 `risk_profile` | +2 min |

**乐观**：22 min / **预期**：28 min / **悲观**：40 min

---

## 8. 待用户决策项（已全部敲定）

| # | 项 | 决策 |
|---|---|---|
| D1 | model 字段名 | **`max_context_window: Option<usize>`** ✅ |
| D2 | agent 字段类型 | **`Option<usize>`**（详解见 §2.3，用户已 confirm "D2 OK"）✅ |
| D3 | 实现方式 | **Config + AliasedAgentConfig 各加一个 helper method** ✅ |
| D4 | 优先级 | **agent.explicit > model.window > 32K** ✅ |
| D5 | 向后兼容审计 | **完全兼容**（见 §2.3 矩阵） ✅ |
| D6 | 32K 常量 | **`pub const DEFAULT_MAX_CONTEXT_TOKENS: usize = 32_000;`** ✅ |
| D7 | plan ID | **`kanmars.req.20260523.001.plan`** ✅ |
| D8 | 实施 | **fan-out 3 个并行 sub-agent**（Batch 1: schema + CHANGELOG 并行 → Batch 2: channels + runtime）✅ |
| D9 | loop_.rs scope 不够时兜底 | **优先重构传 `&Config`，不行时退化 `unwrap_or(DEFAULT)` + TODO**（实施时 sub-agent 自决）|

---

## 9. 关联文档 / 参考

- 需求源对话：本会话第 N-N+5 轮（gloria 实测 → 根因分析 → 用户拍板需求）
- 上一份 PR：[`.sisyphus/plans/kanmars.req.20260522.001.plan.md`](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/.sisyphus/plans/kanmars.req.20260522.001.plan.md)（classifier_provider PR，与本 PR 正交）
- gloria 实证日志：`/Users/kanmars/workspace/kanmars_gloria/A/logs/zeroclaw.log` L68（"Proactive context compression applied"，19035 → 9654 tokens / 26.7s）
- [zeroclaw AGENTS.md — ABSOLUTE RULE SINGLE SOURCE OF TRUTH](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md)
- 同款参考实现：
  - [`Config::model_provider_for_agent` @ schema.rs:3074](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L3074)（helper 风格参考）
  - [`Config::resolved_model_provider_for_agent` @ schema.rs:3088](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L3088)（命名风格参考）
- DeepSeek-V4 1M context 官方公告：[https://api-docs.deepseek.com/news/news260424](https://api-docs.deepseek.com/news/news260424)
- Kimi-K2.5 256K context（百炼）：本会话第 N-1 轮 web search

---

## 10. Sign-off

| 角色 | 状态 |
|---|---|
| 起草人（组件管理员）| ✅ 已完成（rev0） |
| 计划审阅人（用户）| ✅ 已确认（"我已经手工合并了，直接开始吧"） |
| 实施授权 | ✅ 已授权 |
| 实施状态 | ✅ Step 1-6 全部完成 + 验证通过；Step 7 (git commit + push) 进行中 |

**当前模式**：plan 实施完毕，等 commit + push。

## 10.1 实施记录（2026-05-23T01:13 起草，01:50 完成 ≈ 37 min wall-clock）

### 完成清单（实际执行）

- [x] Step 0 — 分支 `feat/model-context-window-inheritance` 已基于 `kanmars_main @ 045b60ac`（含 0522.001 PR 已 merge）创建；前置 grep 已跑完，rev0 修订完成
- [x] Step 1 — schema.rs 全部 6 sub-edits 完成 + 6 测试 passed（Task A `bg_fc65e777`，9m42s — session error 但工作已落盘）
  - 1a: `max_context_window: Option<usize>` 字段插入 schema.rs:691（ModelProviderConfig 内）
  - 1b: `max_context_tokens` 类型从 `usize` 改为 `Option<usize>` @ schema.rs:2860
  - 1c: `DEFAULT_MAX_CONTEXT_TOKENS: usize = 32_000` 常量添加 @ schema.rs:637；旧 `default_agent_max_context_tokens` 函数删除
  - 1d: Default impl 改为 `max_context_tokens: None` @ schema.rs:2973
  - 1e: 2 helpers 添加：`AliasedAgentConfig::resolved_max_context_tokens` @ schema.rs:3011 + `Config::resolved_max_context_tokens_for_agent` @ schema.rs:3166
  - 1f: 3 unit tests 添加 @ schema.rs:22213-22236
  - Step 4: 3 integration tests 添加 @ schema.rs:22248-22310
- [x] Step 2 — orchestrator/mod.rs:7467 调用点替换为 `config.resolved_max_context_tokens_for_agent(agent_alias)`（Task C `bg_082f58e7`，8m27s）
- [x] Step 3 — loop_.rs 4 处全部 preferred form 替换（无 fallback 无 TODO）
  - mod.rs:7467 + loop_.rs 5 处 sites scope 分析全部确认有 `&Config + agent_alias`
  - 全部用 `agent.resolved_max_context_tokens(config.model_provider_for_agent(agent_alias))` 形式
- [x] Step 5 — 静态检查 + 全套测试：
  - cargo check zeroclaw-config: OK
  - cargo check zeroclaw-channels: OK
  - cargo check zeroclaw-runtime: OK
  - cargo clippy zeroclaw-config -D warnings: ✅ exit 0（首次 1 个 field-reassign-with-default error，由 Task D `bg_c36a6ba4` 修复）
  - cargo clippy zeroclaw-channels: 7 errors（**全部 pre-existing in baseline**，与本 PR 无关 — git stash 验证过）
  - cargo clippy zeroclaw-runtime: ✅ exit 0
  - cargo test zeroclaw-config: ✅ 752 passed（pre-existing 658 + 6 new + 88 migration tests）
  - cargo test zeroclaw-channels --features channel-lark --lib: ✅ 1191 passed, 0 failed
  - cargo test zeroclaw-runtime --lib: ✅ 1830 passed, 0 failed, 1 ignored
- [x] Step 5 V1-V22 grep 验证清单全过：
  - V1 `max_context_window` 字段 ✅ 1 行
  - V2 `max_context_tokens: Option<usize>` ✅ 2 行（2860 新 + 8974 pre-existing dead config）
  - V3 `DEFAULT_MAX_CONTEXT_TOKENS` 常量 ✅ 1 行
  - V4 旧 default fn ✅ 0 行（已删）
  - V5 helpers ✅ ≥2
  - V6 orchestrator 调用 ✅ 1 行（mod.rs:7467）
  - V7 loop_.rs 调用 ✅ 4 行
  - V8 旧字段直接访问 ✅ 0 行
  - V10 Cargo.toml 0 diff ✅
  - V11 zeroclaw-api 0 diff ✅
  - V21 CHANGELOG max_context_window ✅ 6 hits
- [x] Step 6 — CHANGELOG-next.md 双段（Task B `bg_80c3ea23`，1m50s）：
  - `### Added` 追加 `max_context_window` 条目 @ lines 51-63（保留 0522.001 classifier_provider 条目不变）
  - `### Changed` 新建段落 @ lines 65-84（agent.max_context_tokens 类型变化 + 向后兼容声明 + gloria 26.7s 实证）

### 实际改动

- `crates/zeroclaw-config/src/schema.rs`: **+180 行 / -19 行**（plan 估算 +60，差是因为 6 测试 + 完整 doc comments）
- `crates/zeroclaw-channels/src/orchestrator/mod.rs`: **+1 / -1**
- `crates/zeroclaw-runtime/src/agent/loop_.rs`: **+8 / -4**
- `CHANGELOG-next.md`: **+35 行**（plan 估算 +30）
- `.sisyphus/plans/kanmars.req.20260523.001.plan.md`: 本节追加 + §10 状态更新
- `.sisyphus/notepads/kanmars.req.20260523.001/learnings.md`: sub-agent 写入了详细 deviation report + line-drift 分析 + 验证矩阵

### 实施 sub-agent 调度记录

| Task | Agent | Duration | Session ID | 状态 |
|---|---|---|---|---|
| `bg_fc65e777` zeroclaw-config 改动 + 6 测试 | Sisyphus-Junior (unspecified-high) | 9m 42s | `ses_1af49a6b6ffe89I0H8lKJQCv6A` | ✅ 工作完成（最终响应 API hiccup，但所有改动已落盘） |
| `bg_80c3ea23` CHANGELOG 双段 | Sisyphus-Junior (writing) | 1m 50s | `ses_1af4951d7ffeCeIMzdGG6GRL7g` | ✅ 完成 |
| `bg_082f58e7` channels + runtime 5 调用点 | Sisyphus-Junior (unspecified-high) | 8m 27s | `ses_1af3e50a3ffeoiT76i4zhcP2UK` | ✅ 完成 |
| `bg_c36a6ba4` clippy field-reassign 修复 | Sisyphus-Junior (quick) | 1m 15s | `ses_1af34872cffel1Oft5Ffl3E1xY` | ✅ 完成 |

**并行调度**：Batch 1 (Task A + Task B 同时 fire) wall-clock ≈ 9m42s（max of 9m42s, 1m50s）。Batch 2 (Task C, 依赖 A) 8m27s。Batch 3 (Task D, clippy hotfix) 1m15s。

总实施时间（Batch 1+2+3 + atlas verify）≈ 37 min wall-clock，**与 plan §7 估算的"AI agent 22-28 min"相符**（多出 ~10 min 主要在 Task A 的 session API hiccup + Task D 这个 plan 没预料的 clippy hotfix）。

### 关键洞察

1. **Plan 行号 drift 警告过于保守**：plan §3 警告"行号会偏移 +25"，实际 schema.rs 偏移 0~+28 不均匀，channels/runtime 目标 sites **0 drift**（5 个 sites 都离 0522.001 的 resolve_classifier_route helper 很远）。下次起草 plan 不必如此保守
2. **Task A 全部 5 sites 都用 preferred form**：scope 分析确认 `&Config + agent_alias` 全部可达，无需 fallback。Plan §1.4 "可能需要重构上层"的担忧没有发生 —— `pub async fn run(config: Config, agent_alias: &str, ...)` 是顶层签名，闭包 capture 能完整传透
3. **clippy field-reassign 是 plan 没预料的小坑**：`let mut x = Default::default(); x.field = ...` 模式触发警告。Task D 用 1m15s 修好。下次 plan 应该提前在测试代码里用 struct literal 形式
4. **API session hiccup 不影响实际工作**：sub-agent 的所有文件 edit 都是事务性的（Edit tool 的 atomic write），即使 session 最终响应失败，已 commit 到磁盘的改动是完整的。验证方式：grep + cargo check + notepad learnings.md
5. **Pre-existing 7 clippy errors（不是 5）**：0522.001 merge 时把基线带到了 7（之前 5 是 0522.001 之前的状态）。本 PR 不引入新 error。下次记录 baseline 时要随 master 更新

### Step 7 待执行

由 atlas 接下来 delegate 给 git-master skill：
- `git add` 5 个文件（schema.rs + mod.rs + loop_.rs + CHANGELOG + 本 plan）
- atomic commit（用 plan §3 Step 7 准备好的 80 行 commit message）
- `git push -u origin feat/model-context-window-inheritance`（沙箱实测 push 到 github 凭证有效，与 0522.001 同）
- 然后 user 在 github 上创建 PR / merge

**关键审阅点**：

1. **D9 兜底策略**：loop_.rs 3972/4042 scope 不够时，sub-agent 自决"重构传 config" vs "退化 unwrap_or"。可接受吗？还是要求一律重构（强制 A）？
2. **Step 1c 删除 `default_agent_max_context_tokens()` 函数**：函数本身没有外部消费方（仅 schema.rs 内 2 处使用），删除安全。是否同意？
3. **Step 6 CHANGELOG 文本**：已起草，可调整措辞
4. **测试覆盖度**：6 个新单测够吗？还需要加 e2e（用真实 LLM 验证 compression 不触发）？
5. **base branch**：本 PR 基于 `kanmars_main`（与 0522.001 同基线），merge 后两个 PR 各自独立 vs 串行？

实施授权后将严格按 §3 Step 1 → Step 7 顺序：
- **Batch 1（并行）**：Task A（zeroclaw-config 改动 + 6 测试）+ Task B（CHANGELOG）
- **等 Batch 1 完成**：Task C（zeroclaw-channels:7467 + zeroclaw-runtime loop_.rs 4 处）
- **atlas verify**：Phase 1+2+3+4 gate
- **commit + push** by git-master skill

预期总 wall-clock：**~28 min**。
