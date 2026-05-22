# Plan — kanmars.req.20260522.001 (Per-agent classifier provider override for `classify_channel_reply_intent`)

| 字段 | 值 |
|---|---|
| Plan ID | kanmars.req.20260522.001.plan |
| 关联需求 | 用户对话需求（2026-05-22）：『飞书渠道分类功能（`classify_channel_reply_intent`）目前复用主 agent 的 provider+model，每条消息都用大模型（如 qwen3.6-plus）跑分类，又慢又费。要支持配置独立的小模型/免费模型（如 `kimi-k2.5`）—— 不配置则保持现状（复用主模型），配置了就走配置的 provider+model。』 |
| 起草日期 | 2026-05-22 |
| 修订日期 | 2026-05-22 (rev1 — Step 0 已完成，发现 `typed_provider_refs` 通用化校验数组可直接复用，Step 2 从 12 行降到 1 行；字段位置改到 `transcription_provider` 之后；`ModelProviders::find` 签名已确认；基线分支锁定 `kanmars_main`) |
| 起草人 | 组件管理员 (Sisyphus) |
| 目标分支 | `feat/classifier-provider-per-agent-override` ✅ **已创建**（基于 `kanmars_main`） |
| 风险等级 | **Low-Medium**（`zeroclaw-config` Beta tier 加新 optional 字段 + `zeroclaw-channels` Experimental tier 行为变更；新字段全部 `Default::default()` 向后兼容；不动 trait / security / gateway 边界） |
| 基线 commit | `bf5049e2b0b21ca336415fa2afc9b55763725d78`（`kanmars_main` HEAD，message: `Merge remote-tracking branch 'origin/master' into try-merge-master`） |
| 选型方案 | **方案 A — `AliasedAgentConfig` 加 `classifier_provider: ModelProviderRef` 单字段**，per-agent 粒度，与现有 `tts_provider` / `transcription_provider` 字段同构；空字符串（`Default`）= 继承主 agent；非空 = 解析 `<type>.<alias>` 到 `[providers.models.<type>.<alias>]` 并即时构造 provider |
| 预计代码行数 | **+175 / -7**（含 schema 字段 25 + `typed_provider_refs` 加 1 行 5 + resolver helper 40 + 调用点改写 10 + 5 新单测 75 + CHANGELOG 20，明细见 §7）|
| 预计工作量 | **AI agent 执行约 30-40 min** / **Sisyphus-Junior 人工节奏约 77 min**（不含已完成的 Step 0）；其中 CI wall-clock 占 10-20 min 不可压缩，详见 §7.1 |

---

## 0. 关键目标（唯一真理来源）

> **让 `classify_channel_reply_intent` 调用支持独立的 provider/model：未配置时完全保持当前"复用主 agent 的 `active_model_provider` + `route.model`"行为（向后 100% 兼容）；配置了 `[agents.<alias>].classifier_provider = "<type>.<alias>"` 时，所有 lark/feishu/slack/discord/telegram 等 channel 的回复意图分类调用都改走该 provider 对应的 `[providers.models.<type>.<alias>]` 配置的模型，从而把昂贵主模型的"分类成本"切换到便宜/免费小模型。**

**完成此目标即"功能完成"**：

- 用户在 `config.toml` 写：
  ```toml
  [providers.models.custom.default]
  api_key = "sk-sp-xxxxxx"
  model = "qwen3.6-plus"
  uri = "https://coding.dashscope.aliyuncs.com/v1"
  wire_api = "chat_completions"

  [providers.models.custom.kimi-k2-5]
  api_key = "sk-sp-xxxxxx"
  model = "kimi-k2.5"
  uri = "https://coding.dashscope.aliyuncs.com/v1"
  wire_api = "chat_completions"

  [agents.default]
  model_provider = "custom.default"
  classifier_provider = "custom.kimi-k2-5"   # ← 本 PR 新增字段
  ```
  → 飞书来一条消息 → orchestrator 调用 `classify_channel_reply_intent` 时使用 **`kimi-k2.5`** 跑 REPLY/NO_REPLY 分类；分类返回 `Reply` 后进入 agent loop 用 **`qwen3.6-plus`** 回答用户
- **不写 `classifier_provider` 时**：行为与本 PR 之前**完全一致**（继承 `active_model_provider` + `route.model`）—— 老用户零感知
- **`classifier_provider = ""`**（显式空字符串）：与不写等价（`is_empty()` 走继承分支；validate 跳过 `value.is_empty() { continue }`）
- 配置错误（`<type>.<alias>` 在 `[providers.models]` 下不存在）→ `Config::validate()` 启动期 fail-loud，与 `tts_provider` / `transcription_provider` 同款的 `DanglingReference` 错误码
- ACP 渠道（[orchestrator/mod.rs:3492-3522](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3492-L3522) 强制 REPLY）依然完全跳过分类器调用，**新字段对 ACP 无影响** —— 文档明示
- 新 provider 实例**复用 [`ProviderCacheMap` @ mod.rs:358](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L358) 现有缓存** —— 不重复实例化，不开第二个连接池
- 不写 `classifier_provider` 的 agent 行为完全等价于 PR 前 `kanmars_main` HEAD

**显式不在范围内**：

- ❌ 不加 `classifier_temperature: Option<f64>`（用户明确否决，留 follow-up F1）
- ❌ 不加 `classifier_history_window: Option<usize>`（小模型上下文不足问题，留 follow-up F2）
- ❌ 不动 `QueryClassificationConfig`（[schema.rs:9274](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L9274)）—— 它管的是 Layer 2.a 规则型路由覆盖，不是 LLM 分类器
- ❌ 不动规则型 [`zeroclaw_runtime::agent::classifier::classify`](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-runtime/src/agent/classifier.rs#L14)（不用 LLM）
- ❌ 不动 Layer 2.a 的 route 覆盖逻辑（[orchestrator/mod.rs:3235-3248](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3235-L3248)）—— 它继续按规则匹配换主 route
- ❌ 不改 `classify_channel_reply_intent` 函数本身的签名（仍是 `model_provider: &dyn ModelProvider, ..., model: &str`）—— 只在调用点换传参
- ❌ 不改其他用到 `AliasedAgentConfig` 的消费方（cron / sop / heartbeat / delegate）—— 新字段对它们透明（默认空，不读则无效）
- ❌ 不动 trait（`zeroclaw-api`）
- ❌ 不引入新依赖
- ❌ 不为 memory consolidation / reflection / 其他后续可能想用独立模型的场景预先抽象 trait（YAGNI，未来真要再加同款字段）

## 0.5 非协商前提（来自项目规范，不可绕过）

1. **SINGLE SOURCE OF TRUTH 铁律**（[zeroclaw AGENTS.md ABSOLUTE RULE](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md)）—— 新字段必须能用项目要求的 "源即此处 / 引用他处" 二选一公式声明：
   > **"新增 `classifier_provider: ModelProviderRef` 字段。Source of truth is `[providers.models.<type>.<alias>]` —— 此字段是 reference，不是 duplicate。`api_key` / `uri` / `temperature` / `model` 等所有具体字段全部留在 `[providers.models]` 表里唯一一份，本字段仅持有 `<type>.<alias>` 字符串引用，运行期通过 `get_or_create_provider` 即时解析。"**
   - ❌ 禁止新建 `ClassifierProviderConfig { provider, model, api_key, uri, temperature, ... }` 嵌套结构 —— 100% 自动 revert
   - ❌ 禁止在 `ChannelRuntimeContext` 缓存 `classifier_provider: Arc<dyn ModelProvider>` 字段 —— 会变成"快照式复制"，配置热更时不刷新（与历史 `Vec<allowed_users>` 同款 anti-pattern）
   - ✅ 允许：通过 `ctx.agent_cfg.classifier_provider`（reference，每次读 Config）+ `get_or_create_provider`（cache hit by key，不是字段缓存）即时解析
2. **不新增 `unwrap()` / `expect()`**（[zeroclaw AGENTS.md Anti-Patterns](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md#anti-patterns)）
3. **不新增 `#[allow(dead_code)]`**
4. **`tracing::` 日志保持英文**（RFC #5653 §4.6）
5. **不引入新依赖**
6. **完整跑** `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` + `./dev/ci.sh all`
7. **One concern per PR**：本 PR 一个关注点 = "给回复意图分类器开放 per-agent provider 覆盖入口"。**不**同时做 temperature / history-window / 其他可选项 —— 任何额外字段都作为 follow-up
8. **CHANGELOG-next.md 必须更新**（用户可见的新配置字段）
9. **配置向后 100% 兼容**：所有现有 `[agents.*]` block **不带** `classifier_provider` 字段，加载后必须与 PR 前等价。`#[serde(default)]` + `ModelProviderRef` 的 `Default = String::new()` 保证空字段不在 TOML 中出现
10. **Risk Tier 自评**：跨 Beta(`zeroclaw-config`) + Experimental(`zeroclaw-channels`) crate；按 [AGENTS.md Risk Tiers](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md#risk-tiers) 取**更高级别 Low-Medium**
11. **基线分支**（rev1 锁定）：从 **`kanmars_main`**（不是 `master`）拉新分支 `feat/classifier-provider-per-agent-override`，基线 commit `bf5049e2`

---

## 1. 现状事实复核（Step 0 已实地验证，行号对齐 `kanmars_main` @ `bf5049e2`）

### 1.1 关键代码位置

| 事实 | 文件:行 | Step 0 验证 |
|---|---|---|
| **`AliasedAgentConfig` 定义**（待加 `classifier_provider` 字段） | [crates/zeroclaw-config/src/schema.rs:2725](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2725) | ✅ grep #1 |
| `AliasedAgentConfig.model_provider`（mandatory，非新字段镜像目标） | [crates/zeroclaw-config/src/schema.rs:2738](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2738) | ✅ grep #1 |
| `AliasedAgentConfig.tts_provider`（optional ModelProviderRef，**新字段同款语义**）| [crates/zeroclaw-config/src/schema.rs:2769](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2769) | ✅ grep #1 |
| `AliasedAgentConfig.transcription_provider`（optional，**新字段紧挨它插入**）| [crates/zeroclaw-config/src/schema.rs:2778](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2778) | ✅ grep #1 |
| **`ModelProviderRef` newtype 定义**（透明 String，`Default=""`，已支持 `is_empty()` / `as_str()` / `trim()`（via Deref<str>） / `From<&str>`） | [crates/zeroclaw-config/src/providers.rs:151](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/providers.rs#L151) + 宏定义 [L58-L149](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/providers.rs#L58-L149) | ✅ grep #2 |
| **`ModelProviders::find(family, alias) -> Option<&ModelProviderConfig>`**（Step 3 resolver 直接调用，签名 100% 确认） | [crates/zeroclaw-config/src/providers.rs:332](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/providers.rs#L332) | ✅ grep #3 |
| `ModelProviders::ensure / contains_model_provider_type / aliases_of`（bonus，本 PR 不用，但备查） | [crates/zeroclaw-config/src/providers.rs:349 / 371 / 385](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/providers.rs#L349) | ✅ grep #3 |
| `Providers` 容器（`models: ModelProviders` 字段在此） | [crates/zeroclaw-config/src/providers.rs:615-619](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/providers.rs#L615-L619) | ✅ grep #3 |
| `ModelProviderConfig` 真值表（被引用方，含 `model` / `api_key` / `uri` / `temperature` 等字段） | [crates/zeroclaw-config/src/schema.rs:635-711](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L635-L711) | — |
| `Config::model_provider_for_agent` / `resolved_model_provider_for_agent`（现成 helper，pattern 参考） | [crates/zeroclaw-config/src/schema.rs:3074-3098](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L3074-L3098) | ✅ grep #5 上下文 |
| ★ **`Config::validate` 的 `typed_provider_refs` 通用化校验数组**（**本 PR 直接复用，加 1 行 tuple 即可**） | [crates/zeroclaw-config/src/schema.rs:14581-14612](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L14581-L14612) | ✅ grep #5 |
| 现有 `agent.model_provider` 单独 fail-loud（mandatory 字段，与 classifier_provider 路径不同） | [crates/zeroclaw-config/src/schema.rs:14509-14537](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L14509-L14537) | ✅ grep #5 |
| **`classify_channel_reply_intent` 函数定义**（签名不变，调用方换传参） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:2254-2303](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2254-L2303) | — |
| `parse_reply_intent`（兜底解析，不动） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:2308-2341](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2308-L2341) | — |
| **`classify_channel_reply_intent` 唯一调用点**（本 PR 改造焦点） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:3477-3485](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3477-L3485) | — |
| ACP 跳过分类器强制 REPLY 路径（保持不变，文档明示） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:3492-3522](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3492-L3522) | — |
| **`ChannelRuntimeContext.agent_cfg: Arc<AliasedAgentConfig>`**（调用点已能取到 agent 配置） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:345](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L345) | — |
| `ChannelRuntimeContext.prompt_config: Arc<Config>`（用于 resolver 查 ModelProviderConfig 取 `model` 字符串） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:346](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L346) | — |
| `ChannelRuntimeContext.provider_cache: ProviderCacheMap`（**复用此缓存，禁止开第二个**） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:358](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L358) | — |
| **`get_or_create_provider(ctx, provider_name: &str, route_api_key: Option<&str>) -> anyhow::Result<Arc<dyn ModelProvider>>`** | [crates/zeroclaw-channels/src/orchestrator/mod.rs:1543-1547](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L1543-L1547) | ✅ grep #4 |
| **`ChannelRouteSelection { model_provider: String, model: String, api_key: Option<String> }`**（`.model` 是普通 `String`，`route.model.clone()` 正确） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:251-259](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L251-L259) | ✅ grep bonus |
| `runtime_defaults.temperature` 现有取值（继续传给分类器） | [crates/zeroclaw-channels/src/orchestrator/mod.rs:1005](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L1005) `runtime_defaults_snapshot` | — |
| `route.model.as_str()` / `route.model.clone()` 当前 7 处调用（验证 V5 用） | mod.rs:1917 / 3242 / 3245 / 3455 / 3481 / 3810 / 3882 | ✅ grep bonus |
| 参考实现 — `MemoryConfig.embedding_model` 同款 `Option<String>` + `hint:` 模式 | [crates/zeroclaw-config/src/schema.rs:8269](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L8269) | — |
| 参考实现 — `resolve_embedding_config` 同款即时解析 resolver | [crates/zeroclaw-memory/src/lib.rs:219-285](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-memory/src/lib.rs#L219-L285) | — |
| Provider 工厂 — `create_routed_model_provider_with_options`（`get_or_create_provider` 内部最终走到这里） | [crates/zeroclaw-providers/src/lib.rs:1326-1424](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-providers/src/lib.rs#L1326-L1424) | — |

### 1.2 用户配置实证

用户给出的目标 TOML 配置（[需求原文](#关联需求)）：

```toml
[providers.models.custom.default]            # ← 主模型，贵
api_key = "sk-sp-xxxxxx"
max_tokens = 65536
model = "qwen3.6-plus"
name = "custom:https://coding.dashscope.aliyuncs.com/v1"
temperature = 0.7
timeout_secs = 120
uri = "https://coding.dashscope.aliyuncs.com/v1"
wire_api = "chat_completions"

[providers.models.custom.kimi-k2-5]          # ← 分类器模型，快+免费
api_key = "sk-sp-xxxxxx"
max_tokens = 65536
model = "kimi-k2.5"
name = "custom:https://coding.dashscope.aliyuncs.com/v1"
uri = "https://coding.dashscope.aliyuncs.com/v1"
wire_api = "chat_completions"

[agents.default]
enabled = true
channels = ["lark.feishu", "feishu.feishu"]
cron_jobs = ["白天反思_每_2h", "夜间反思_03_00_单次"]
model_provider = "custom.default"
classifier_provider = "custom.kimi-k2-5"     # ← 本 PR 新增字段
```

⚠️ **TOML 命名约束**：TOML 表名中的 `.` 是嵌套分隔符 —— `[providers.models.custom.kimi-k2.5]` 会被解析成四级嵌套 `kimi-k2.5`，不是预期的 alias `"kimi-k2.5"`。**alias 名只能用 ASCII 字母 / 数字 / `-` / `_`**，所以用户配置里 alias 必须写 `kimi-k2-5`（短横线），而内层 `model = "kimi-k2.5"` 字段值是字符串，带 `.` 没问题。**Step 7 的 CHANGELOG-next.md 必须明示此约束**。

### 1.3 当前调用点结构（待改写）

```rust
// crates/zeroclaw-channels/src/orchestrator/mod.rs:3476-3485（当前）
// ── Reply-intent precheck ────────────────────────────────────────
let classifier_intent = classify_channel_reply_intent(
    active_model_provider.as_ref(),     // ← 主 agent 的 provider（要可配置覆盖）
    history[0].content.as_str(),
    &history,
    route.model.as_str(),                // ← 主 agent 的 model（要可配置覆盖）
    runtime_defaults.temperature,        // ← 保持继承
)
.await
.unwrap_or(AssistantChannelOutcome::Reply(String::new()));
```

`active_model_provider` 是 `Arc<dyn ModelProvider>`，由上方 [L3251 `get_or_create_provider`](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3251) 解析自 `route.model_provider`（已被 Layer 2.a [L3235-3248](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3235-L3248) `query_classification` 规则覆盖过的最终路由）。`route.model` 同源。

### 1.4 ★ 关键发现 — `typed_provider_refs` 通用化校验数组（rev1 新增）

[schema.rs:14581-14612](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L14581-L14612) 已经存在一个 **通用化的 typed-provider-ref 校验循环**，目前覆盖 `tts_provider` + `transcription_provider`：

```rust
let typed_provider_refs: &[(&str, &str, &str)] = &[
    ("providers.tts", "tts_provider", agent.tts_provider.trim()),
    (
        "providers.transcription",
        "transcription_provider",
        agent.transcription_provider.trim(),
    ),
];
for (section_prefix, field, value) in typed_provider_refs {
    if value.is_empty() {
        continue;
    }
    match value.split_once('.') {
        Some((ty, inner)) if !ty.is_empty() && !inner.is_empty() => {
            let exists = self
                .get_map_keys(&format!("{section_prefix}.{ty}"))
                .is_some_and(|keys| keys.iter().any(|k| k == inner));
            if !exists {
                validation_bail!(
                    DanglingReference,
                    format!("agents.{alias}.{field}"),
                    "agents.{alias}.{field} = {value:?} but {section_prefix}.{ty}.{inner} is not configured",
                );
            }
        }
        _ => validation_bail!(
            InvalidFormat,
            format!("agents.{alias}.{field}"),
            "agents.{alias}.{field} must be dotted form `<type>.<alias>` (got {value:?})",
        ),
    }
}
```

**含义**：本 PR 的 validate 不用手写新 block，只需 **在数组里加一行 tuple**：

```rust
let typed_provider_refs: &[(&str, &str, &str)] = &[
    ("providers.tts", "tts_provider", agent.tts_provider.trim()),
    (
        "providers.transcription",
        "transcription_provider",
        agent.transcription_provider.trim(),
    ),
    // NEW in this PR:
    (
        "providers.models",                                       // section
        "classifier_provider",                                    // field name (用于错误信息)
        agent.classifier_provider.trim(),                         // 实际值
    ),
];
```

- **优点**：与 `tts_provider` / `transcription_provider` 共享完全相同的错误码（`DanglingReference` / `InvalidFormat`）、错误信息格式、空值跳过语义、`get_map_keys` 查找路径；零新代码路径
- **限制**：错误信息文本由通用模板生成（`agents.{alias}.classifier_provider = {value:?} but providers.models.{ty}.{inner} is not configured`），不能像 `agent.model_provider` 那样写自定义的"must reference a configured model model_provider (e.g. ...)"友好提示。**这个 tradeoff 可接受** —— 通用错误信息已经包含定位所需全部信息

---

## 2. 方案选型（已决策）

### 2.1 选项对比

| 方案 | 描述 | 优点 | 缺点 | 决策 |
|---|---|---|---|---|
| **A — `AliasedAgentConfig.classifier_provider: ModelProviderRef`** | 单字段，per-agent 粒度，直接复用 `ModelProviderRef` newtype + `get_or_create_provider` + `typed_provider_refs` validate | 与 `tts_provider` / `transcription_provider` 完全同构；validate 只加 1 行 tuple；调用点能直接 `ctx.agent_cfg.classifier_provider` 取值 | per-agent 而非全局；多 agent 用户要每个 agent 单独配 | ✅ **采纳** |
| B — `QueryClassificationConfig.classifier_provider` | 单字段，全局粒度 | 一处配置作用于所有 agent | 与 `QueryClassificationConfig` 现语义（规则匹配）混淆；强耦合 query_classification 模块；多 agent 用户失去灵活性；不能复用 `typed_provider_refs` | ❌ |
| C — `ClassifierConfig { provider, model, temperature, api_key, uri }` 嵌套块 | 完整独立配置块 | 完全自治 | **违反 SSOT 铁律**（duplicate `api_key`/`uri`/`temperature`）→ 自动 revert | ❌ |
| D — 在 `ChannelRuntimeContext` 加 `classifier_provider: Option<Arc<dyn ModelProvider>>` 字段 | 启动期就解析好缓存到 ctx | 调用点无需 await resolver | 字段缓存 = duplicate state；配置热更不刷新；与 `Vec<allowed_users>` 同类 anti-pattern | ❌ |
| E — `Channel::supports_classifier_override()` trait 方法 | trait 抽象 | 跨 channel 通用 | 动 `zeroclaw-api` Stable trait（违反 §0.5）；其他 channel 没这个概念 | ❌ |

**选 A 的核心理由**：

1. **直接命中根因**：调用点 [orchestrator/mod.rs:3477](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3477) 已经能拿到 `ctx.agent_cfg`，新字段就在那里读
2. **遵循 SSOT 铁律**：`ModelProviderRef` 是字符串引用，所有具体配置留在 `[providers.models.<type>.<alias>]` 唯一一份
3. **与现有 typed provider refs 完全同构**：用户一眼能懂，validate 复用 1 行 tuple，文档可以一句话说清
4. **零新工具函数**：`ModelProviders::find` + `get_or_create_provider` + `typed_provider_refs` 全部现成
5. **回滚干净**：单字段，删字段即回滚；新字段全默认空，老用户零感知

### 2.2 字段名最终决策

| 候选 | 决策 | 理由 |
|---|---|---|
| `classifier_model_provider` | ❌ | 与 `model_provider` 太像，扫一眼难区分 |
| **`classifier_provider`** | ✅ **采纳**（用户已确认） | 短、清晰；语义"分类器用的 provider"；与 `tts_provider` / `transcription_provider` 同款命名（动词+provider）|
| `intent_classifier_provider` | ❌ | 太长 |
| `reply_intent_provider` | ❌ | 实现细节泄露到字段名 |

### 2.3 是否同时加 `classifier_temperature`

**否**（用户已确认）。理由：

1. 分类器通常 `temperature=0` 才稳定，但当前共用 `runtime_defaults.temperature` 也能跑（首轮上线先解决"用什么 model"是主问题）
2. follow-up PR 可以单独加，符合 "one concern per PR"
3. 真要锁 0 的用户，可以单独建一个 `[providers.models.custom.kimi-k2-5-deterministic]` alias 把 `temperature = 0` 写死在 provider 配置里 —— 不阻塞

### 2.4 解析路径（关键技术细节，rev1 已实地确认）

调用点需要从 `<type>.<alias>` 字符串拿到两样东西：

1. **`Arc<dyn ModelProvider>`** —— 用现成 `get_or_create_provider(ctx, provider_str, model_cfg.api_key.as_deref()).await`
2. **`model: String`** —— 调 [`ctx.prompt_config.providers.models.find(type_key, alias_key)`](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/providers.rs#L332) 取 `&ModelProviderConfig`，读 `.model.clone().unwrap_or_default()`

★ **`find` 方法已确认存在**（rev0 caveat 删除）：

```rust
// crates/zeroclaw-config/src/providers.rs:332
pub fn find(&self, family: &str, alias: &str) -> Option<&ModelProviderConfig> {
    macro_rules! emit_get { ... }
    for_each_model_provider_slot!(emit_get)
}
```

resolver 函数原型（详细实现见 §3 Step 3）：

```rust
async fn resolve_classifier_route(
    ctx: &ChannelRuntimeContext,
    provider_ref: &ModelProviderRef,
) -> Option<(Arc<dyn ModelProvider>, String)> {
    if provider_ref.is_empty() { return None; }
    let (type_key, alias_key) = provider_ref.as_str().split_once('.')?;
    let model_cfg = ctx.prompt_config.providers.models.find(type_key, alias_key)?;
    let model = model_cfg.model.clone().unwrap_or_default();
    if model.is_empty() { return None; }
    let provider = get_or_create_provider(ctx, provider_ref.as_str(), model_cfg.api_key.as_deref()).await.ok()?;
    Some((provider, model))
}
```

### 2.5 Provider 缓存复用

[`ProviderCacheMap` @ orchestrator/mod.rs:358](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L358) 已是 key-by-`<type>.<alias>` 的 `LruCache`。`get_or_create_provider("custom.kimi-k2-5", ...)` 第一次解析、之后所有消息都命中缓存 → **不开第二个连接池**。

**禁止**在 `ChannelRuntimeContext` 加 `classifier_provider: Arc<dyn ModelProvider>` 字段缓存 —— 那是 duplicate state，配置 hot-reload 不刷新。即时解析 + cache hit 是正确架构。

---

## 3. 实施步骤（4 处编辑，跨 2 文件 `schema.rs` + `mod.rs`）

### Step 0 — 分支准备 + 前置 grep ✅ **已完成（rev1）**

执行记录（2026-05-22 起草日）：

```bash
cd /Users/kanmars/workspace/kanmars_zeroclaw_github

# 基线确认：当前已在 kanmars_main 分支
git status --short                              # 干净工作树（仅 plan 文件 untracked）
git rev-parse HEAD                              # bf5049e2b0b21ca336415fa2afc9b55763725d78
git log --oneline -1                            # bf5049e2 Merge remote-tracking branch 'origin/master' into try-merge-master

# 分支创建
git checkout -b feat/classifier-provider-per-agent-override
# ✅ Switched to a new branch
```

**Step 0 grep 结果**（已嵌入 §1.1 表格的 "Step 0 验证" 列）：

| grep | 命令 | 关键发现 |
|---|---|---|
| #1 | `AliasedAgentConfig` 结构 | 字段顺序：`model_provider` (2738) → `tts_provider` (2769) → `transcription_provider` (2778)；**新字段最佳插入位置：2778 之后**（与其他 optional typed provider refs 集中） |
| #2 | `ModelProviderRef` API | 通过 `define_provider_ref!` 宏生成；`Default = String::new()`；`Deref<str>` → `.trim()` / `.is_empty()` / `.as_str()` 全部可用 |
| #3 | `ModelProviders` find/get API | **`find(family, alias) -> Option<&ModelProviderConfig>` 存在 @ providers.rs:332**，签名 100% 匹配假设 |
| #4 | `get_or_create_provider` 签名 | `async fn get_or_create_provider(ctx, provider_name: &str, route_api_key: Option<&str>) -> anyhow::Result<Arc<dyn ModelProvider>>` @ mod.rs:1543 |
| #5 | `Config::validate` 对 model_provider 的 fail-loud | ★ **发现 `typed_provider_refs` 通用化数组 @ schema.rs:14581-14612**，`tts_provider` / `transcription_provider` 已在内 → **本 PR 加 1 行即可** |
| bonus | `ChannelRouteSelection.model` 类型 | `String`（非 Arc<String>），`route.model.clone()` 是直接 String clone，OK |

**Step 0 验收**：✅ 全部 grep 已跑完，所有悬而未决项落地，rev1 修订完成。

### Step 1 — Schema 加字段（5 min，rev1 微调位置）

**位置**：[`AliasedAgentConfig.transcription_provider` 字段之后 @ schema.rs:2779](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2779)，与其他 optional typed provider refs 集中。

```rust
// crates/zeroclaw-config/src/schema.rs
// 在 transcription_provider 字段（L2778）之后插入
pub struct AliasedAgentConfig {
    // ... 已有字段 ...
    #[serde(default)]
    pub transcription_provider: crate::providers::TranscriptionProviderRef,

    /// Optional override for the per-message LLM reply-intent classifier
    /// (`classify_channel_reply_intent` in zeroclaw-channels). When non-empty,
    /// the channel orchestrator routes the "should this message be replied to?"
    /// classification call to `[providers.models.<type>.<alias>]` referenced
    /// here, instead of reusing the main agent's `model_provider`.
    ///
    /// Source of truth for api_key / uri / model / temperature etc. is the
    /// referenced `[providers.models.<type>.<alias>]` entry. This field is
    /// a reference only (NEVER a copy) — per AGENTS.md SINGLE SOURCE OF TRUTH.
    ///
    /// Empty (`Default`) = inherit the main agent's resolved provider+model
    /// (preserves pre-PR behavior; backward compatible).
    ///
    /// Use case: classification is a cheap REPLY/NO_REPLY decision, doesn't
    /// need a high-end model. Point this at a fast/free small model
    /// (e.g. `kimi-k2.5`, `qwen-turbo`) while `model_provider` stays on the
    /// expensive answering model (e.g. `qwen3.6-plus`).
    ///
    /// Note: TOML table names cannot contain `.`, so alias `kimi-k2.5`
    /// must be written as `[providers.models.custom.kimi-k2-5]`. The
    /// underlying `model = "kimi-k2.5"` string can still contain dots.
    ///
    /// ACP channels (IDE-direct) always reply and skip the classifier
    /// entirely — this field has no effect on ACP traffic.
    #[serde(default)]
    pub classifier_provider: crate::providers::ModelProviderRef,

    // ── Agent loop / runtime tunables ... (其余不变) ──
}
```

**为什么用 `#[serde(default)]` + `ModelProviderRef`（不是 `Option<ModelProviderRef>`）**：

`ModelProviderRef::default()` = `Self(String::new())` 已经天然表达"未配置"，`is_empty()` 检测干净。`Option<ModelProviderRef>` 会让 TOML 出现 `classifier_provider = ""` 与不写字段两种"未配置"形式，反序列化模糊。`tts_provider` / `transcription_provider` 已经是同款 `ModelProviderRef`（非 Option）模式。

### Step 2 — Config::validate 加 1 行 tuple（2 min，rev1 大幅简化）

**位置**：[schema.rs:14581-14588 的 `typed_provider_refs` 数组](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L14581-L14588) 末尾追加 1 行 tuple。

```rust
// crates/zeroclaw-config/src/schema.rs（line 14581 起的数组扩展）
let typed_provider_refs: &[(&str, &str, &str)] = &[
    ("providers.tts", "tts_provider", agent.tts_provider.trim()),
    (
        "providers.transcription",
        "transcription_provider",
        agent.transcription_provider.trim(),
    ),
    // NEW in this PR (kanmars.req.20260522.001):
    (
        "providers.models",
        "classifier_provider",
        agent.classifier_provider.trim(),
    ),
];
// for-loop 之后的代码全部不动 —— 自动 fail-loud 配置错误的 ref
```

**自动获得**（来自 [L14589-L14612 现有循环](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L14589-L14612)）：

- 空字符串 → 跳过校验（保持向后兼容）
- 非空但格式错（如 `"customsubcust"`）→ `InvalidFormat` 错误，文本 `"agents.{alias}.classifier_provider must be dotted form '<type>.<alias>' (got ...)"` 
- 非空但 alias 不存在 → `DanglingReference` 错误，文本 `"agents.{alias}.classifier_provider = ... but providers.models.{ty}.{inner} is not configured"`
- 与 `tts_provider` / `transcription_provider` 共享同款错误码、同款 dashboard 表单 inline 错误绑定

**总改动**：单一 5 行 tuple 插入 + 周围逗号。**5 行 diff，2 分钟**。

### Step 3 — 新增 `resolve_classifier_route` resolver（15 min）

**位置**：[`crates/zeroclaw-channels/src/orchestrator/mod.rs`](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs)，紧挨 `parse_reply_intent` 函数之后（约 L2342 之后），新增一个 module-private async helper：

```rust
// 紧挨 parse_reply_intent 之后插入
//
// crates/zeroclaw-channels/src/orchestrator/mod.rs

/// Resolve a per-agent `classifier_provider` ref to a (provider, model)
/// pair for `classify_channel_reply_intent`. Returns `None` when the
/// ref is empty or unresolvable; the caller MUST then fall back to the
/// main agent's `active_model_provider` + `route.model`.
///
/// Per AGENTS.md SINGLE SOURCE OF TRUTH: this function reads the
/// referenced `[providers.models.<type>.<alias>]` entry on every call
/// (no field cache on `ChannelRuntimeContext`). The provider instance
/// itself is deduped through the existing `provider_cache` LRU.
///
/// See `kanmars.req.20260522.001.plan.md` for rationale.
async fn resolve_classifier_route(
    ctx: &ChannelRuntimeContext,
    provider_ref: &zeroclaw_config::providers::ModelProviderRef,
) -> Option<(Arc<dyn ModelProvider>, String)> {
    if provider_ref.is_empty() {
        return None;
    }

    let provider_str = provider_ref.as_str();
    let (type_key, alias_key) = provider_str.split_once('.')?;

    // Source of truth lookup — re-read every call so a config hot-reload
    // is reflected without a daemon restart.
    let model_cfg = ctx
        .prompt_config
        .providers
        .models
        .find(type_key, alias_key)?; // signature confirmed: providers.rs:332
    let model = model_cfg.model.clone().unwrap_or_default();
    if model.is_empty() {
        ::zeroclaw_log::record!(
            WARN,
            ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                .with_attrs(::serde_json::json!({"provider": provider_str})),
            "classifier_provider points to a [providers.models] entry without a `model` field; falling back to main agent"
        );
        return None;
    }

    // Reuse the shared provider cache. `None` api_key means "use whatever
    // is in the referenced ModelProviderConfig" — get_or_create_provider
    // already handles that path identically to the main route.
    let provider = match get_or_create_provider(ctx, provider_str, model_cfg.api_key.as_deref()).await {
        Ok(p) => p,
        Err(e) => {
            let safe_err = zeroclaw_providers::sanitize_api_error(&e.to_string());
            ::zeroclaw_log::record!(
                WARN,
                ::zeroclaw_log::Event::new(module_path!(), ::zeroclaw_log::Action::Note)
                    .with_outcome(::zeroclaw_log::EventOutcome::Unknown)
                    .with_attrs(::serde_json::json!({"provider": provider_str, "error": safe_err})),
                "Failed to initialize classifier_provider; falling back to main agent provider"
            );
            return None;
        }
    };

    Some((provider, model))
}
```

**关键决策**：解析失败时**软降级**到主 agent（warn log + 返回 None），不让用户的对话因为分类器配置错误而完全瘫痪。Validate 已经在启动期 fail-loud 过一次了，运行期 hot-reload 出问题就软降级。

### Step 4 — 改写调用点（5 min）

**位置**：[orchestrator/mod.rs:3476-3485](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3476-L3485)

```rust
// Before:
let classifier_intent = classify_channel_reply_intent(
    active_model_provider.as_ref(),
    history[0].content.as_str(),
    &history,
    route.model.as_str(),
    runtime_defaults.temperature,
)
.await
.unwrap_or(AssistantChannelOutcome::Reply(String::new()));

// After:
// Resolve the classifier route — per-agent override (Step 3 resolver)
// or fall back to the main agent's active route. The override lets
// operators point classification at a cheap/free small model while
// keeping the expensive answering model on the main route.
let (classifier_provider_arc, classifier_model_owned): (Arc<dyn ModelProvider>, String) =
    match resolve_classifier_route(ctx.as_ref(), &ctx.agent_cfg.classifier_provider).await {
        Some(pair) => pair,
        None => (Arc::clone(&active_model_provider), route.model.clone()),
    };

let classifier_intent = classify_channel_reply_intent(
    classifier_provider_arc.as_ref(),
    history[0].content.as_str(),
    &history,
    classifier_model_owned.as_str(),
    runtime_defaults.temperature,
)
.await
.unwrap_or(AssistantChannelOutcome::Reply(String::new()));
```

**注意**：

- `active_model_provider` 上方 [L3251](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3251) 拿到时是 `Arc<dyn ModelProvider>`；`Arc::clone` 是廉价引用计数 +1
- `route.model` 类型已 grep bonus 确认为 `String`（`ChannelRouteSelection.model: String` @ [L254](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L254)）；`route.model.clone()` 直接 String clone
- `temperature` 保持继承 `runtime_defaults.temperature`（用户决策不开放该字段）
- 老的 `unwrap_or(AssistantChannelOutcome::Reply(String::new()))` 兜底语义保留不变

### Step 5 — 单测（25 min）

#### 5a. Schema validate 单测（3 个，复用通用化校验路径）

**位置**：`crates/zeroclaw-config/src/schema.rs` 的 `#[cfg(test)] mod tests` 内（找现有 `tts_provider` 校验测试同款位置；如无则放在 `agents.{alias}` 校验测试附近）

```rust
#[test]
fn config_validate_rejects_classifier_provider_pointing_at_missing_alias() {
    // Use the SHARED `typed_provider_refs` validation loop — same error
    // surface as tts_provider / transcription_provider.
    let toml = r#"
        [providers.models.custom.default]
        api_key = "k"
        model = "qwen3.6-plus"
        uri = "https://example.com/v1"
        wire_api = "chat_completions"

        [agents.default]
        enabled = true
        model_provider = "custom.default"
        classifier_provider = "custom.does-not-exist"
    "#;
    let cfg: Config = toml::from_str(toml).unwrap();
    let err = cfg.validate().expect_err("missing alias must fail validate");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("classifier_provider")
            && msg.contains("does-not-exist")
            && msg.contains("providers.models.custom.does-not-exist is not configured"),
        "expected DanglingReference error mentioning field + alias + section, got: {msg}"
    );
}

#[test]
fn config_validate_accepts_classifier_provider_pointing_at_existing_alias() {
    let toml = r#"
        [providers.models.custom.default]
        api_key = "k1"
        model = "qwen3.6-plus"
        uri = "https://example.com/v1"
        wire_api = "chat_completions"

        [providers.models.custom.kimi-k2-5]
        api_key = "k2"
        model = "kimi-k2.5"
        uri = "https://example.com/v1"
        wire_api = "chat_completions"

        [agents.default]
        enabled = true
        model_provider = "custom.default"
        classifier_provider = "custom.kimi-k2-5"
    "#;
    let cfg: Config = toml::from_str(toml).unwrap();
    cfg.validate().expect("validate must succeed for resolvable ref");
    assert_eq!(
        cfg.agents.get("default").unwrap().classifier_provider.as_str(),
        "custom.kimi-k2-5"
    );
}

#[test]
fn config_validate_accepts_empty_classifier_provider_as_inheritance_signal() {
    // No classifier_provider field at all → must validate, must remain
    // the empty default. This pins backward compatibility.
    let toml = r#"
        [providers.models.custom.default]
        api_key = "k"
        model = "qwen3.6-plus"
        uri = "https://example.com/v1"
        wire_api = "chat_completions"

        [agents.default]
        enabled = true
        model_provider = "custom.default"
    "#;
    let cfg: Config = toml::from_str(toml).unwrap();
    cfg.validate().expect("missing classifier_provider must validate");
    assert!(cfg.agents.get("default").unwrap().classifier_provider.is_empty());
}
```

#### 5b. resolver 单测

**位置**：`crates/zeroclaw-channels/src/orchestrator/mod.rs` 的 `#[cfg(test)] mod tests` 内

```rust
#[tokio::test]
async fn resolve_classifier_route_returns_none_for_empty_ref() {
    let ctx = make_test_runtime_context();  // 沿用 file 内现有 test helper
    let empty = zeroclaw_config::providers::ModelProviderRef::default();
    let result = resolve_classifier_route(&ctx, &empty).await;
    assert!(result.is_none(), "empty ref must fall back to main agent");
}

#[tokio::test]
async fn resolve_classifier_route_returns_none_for_unresolvable_ref() {
    let ctx = make_test_runtime_context();
    let bogus = zeroclaw_config::providers::ModelProviderRef::from("custom.does-not-exist");
    let result = resolve_classifier_route(&ctx, &bogus).await;
    assert!(result.is_none(), "unresolvable ref must soft-fail to None");
}
```

**`make_test_runtime_context` 兜底**：如果 mod.rs 内现有 test 没有这个 helper，Step 5b 实施时改用 mod.rs 现有最近的 `ChannelRuntimeContext` 构造 helper 名字（grep `fn.*ChannelRuntimeContext|cfg!\(test\)` 定位）。如果完全没有，跳过 5b 单测（resolver 行为可由 §4 R4 / R5 线上验收覆盖；不阻塞 PR）。

#### 5c. 是否新增集成测试模拟 `classifier_provider = "..."` 真实路径？

**否**。理由：

- 现有 `classify_channel_reply_intent` 集成测试已经覆盖"主 agent 跑分类器"的完整流程
- 加 `classifier_provider` 后，**唯一**新行为 = "resolver 返回 `Some(...)` 而不是 `None`，然后传给同一个 `classify_channel_reply_intent`"
- resolver 自己有 5b 单测；`classify_channel_reply_intent` 自己已经被测
- 真要再加集成测试需要 mock 第二个 ModelProvider 实例，成本远高于价值
- 线上验收（§4 验收清单）会真实跑 kimi-k2.5

### Step 6 — 静态检查 + 全测试（10 min）

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /Users/kanmars/workspace/kanmars_zeroclaw_github

# Format only files this PR touches
rustfmt --edition 2024 \
  crates/zeroclaw-config/src/schema.rs \
  crates/zeroclaw-channels/src/orchestrator/mod.rs

# Manually inspect diff and revert any unrelated fmt-only hunks
git diff --stat
git diff crates/zeroclaw-config/src/schema.rs | head -80
git diff crates/zeroclaw-channels/src/orchestrator/mod.rs | head -120

# Static checks
cargo clippy -p zeroclaw-config --all-targets -- -D warnings
cargo clippy -p zeroclaw-channels --features channel-lark --all-targets -- -D warnings
cargo test -p zeroclaw-config
cargo test -p zeroclaw-channels --features channel-lark

# Full CI
./dev/ci.sh all
```

**预期**：

- clippy exit 0
- zeroclaw-config 单测：3 个新 validate 测试全绿
- zeroclaw-channels 单测：2 个新 resolver 测试全绿（或 5b 跳过，见 caveat）+ 既有 orchestrator/lark 测试全绿
- pre-existing telegram failures 不变（与本 PR 无关，多份历史 plan 已记录）
- `./dev/ci.sh all` 完整通过

### Step 7 — CHANGELOG-next.md（5 min）

新增条目（放在 `### Added` 段，没有则新建）：

```markdown
- **agents**: Added `[agents.<alias>].classifier_provider` (`ModelProviderRef`)
  to route the reply-intent classifier (`classify_channel_reply_intent`) to a
  separate, cheaper provider/model than the main answering model. Empty (default)
  preserves pre-release behavior: the classifier reuses the main agent's
  `model_provider`. Non-empty values must reference a configured
  `[providers.models.<type>.<alias>]` entry (validated at config-load fail-loud
  through the same `typed_provider_refs` check that covers `tts_provider` and
  `transcription_provider`). ACP channels skip the classifier entirely and
  are unaffected.

  Example: route classification through a free fast model while answering
  with the premium model:

      [providers.models.custom.default]
      api_key  = "..."
      model    = "qwen3.6-plus"
      uri      = "https://coding.dashscope.aliyuncs.com/v1"
      wire_api = "chat_completions"

      [providers.models.custom.kimi-k2-5]    # alias may NOT contain '.';
      api_key  = "..."                       # write 'kimi-k2-5' not 'kimi-k2.5'
      model    = "kimi-k2.5"                 # the model string CAN contain '.'
      uri      = "https://coding.dashscope.aliyuncs.com/v1"
      wire_api = "chat_completions"

      [agents.default]
      model_provider      = "custom.default"
      classifier_provider = "custom.kimi-k2-5"
```

### Step 8 — Atomic commit + push（10 min）

```bash
git status --short
git add crates/zeroclaw-config/src/schema.rs \
        crates/zeroclaw-channels/src/orchestrator/mod.rs \
        CHANGELOG-next.md \
        .sisyphus/plans/kanmars.req.20260522.001.plan.md
git diff --stat HEAD                # 期望 4 文件

git commit -F - <<'EOF'
feat(agents): add per-agent `classifier_provider` to route reply-intent precheck to a cheaper model

The orchestrator's `classify_channel_reply_intent` (channels/orchestrator
/mod.rs:2254) currently reuses the main agent's `active_model_provider`
and `route.model` for every inbound channel message's REPLY/NO_REPLY
classification — meaning operators who run the main agent on an
expensive model (e.g. `qwen3.6-plus`) also pay that model's per-token
cost for a one-shot Boolean-ish classification on every message.

This patch adds an optional per-agent override:

  [agents.default]
  model_provider      = "custom.default"      # answers (qwen3.6-plus)
  classifier_provider = "custom.kimi-k2-5"    # classifies (kimi-k2.5)

The field is a `ModelProviderRef` (typed alias reference, transparent
`String`) — same type as `tts_provider` and `transcription_provider`.
Empty value (the default for all existing configs) means "inherit the
main agent's resolved route" — exactly the pre-PR behavior.

Wiring:

  * `AliasedAgentConfig.classifier_provider: ModelProviderRef` added
    next to `transcription_provider` with `#[serde(default)]`.
    Backward compatible: any config without the field loads and
    behaves identically to master.

  * `Config::validate()` extended by adding a single tuple to the
    existing `typed_provider_refs` validation loop (schema.rs:14581),
    reusing the same `DanglingReference` / `InvalidFormat` error
    surfaces that already cover `tts_provider` and
    `transcription_provider`.

  * New module-private async helper `resolve_classifier_route(ctx, ref_)`
    in channels/orchestrator/mod.rs: when ref is non-empty, looks up the
    referenced `[providers.models.<type>.<alias>]` via the existing
    `ModelProviders::find` API, reads `.model`, fetches/builds the
    provider via the SHARED `provider_cache`. Returns `None` (soft
    fallback to main agent) on any resolution error, with a `WARN`
    log line so operators can see the misconfiguration.

  * Caller at mod.rs:3477 swaps `(active_model_provider, route.model)`
    for the resolver result when `Some(_)`, identical when `None`.

SINGLE SOURCE OF TRUTH compliance: the new field is a reference, not
a copy. All concrete values (api_key, uri, model, temperature, max_tokens,
...) remain in the referenced `[providers.models.<type>.<alias>]` entry
as the unique source. The runtime `provider_cache` (LRU keyed by
`<type>.<alias>`) is the existing materialized view, not a new cache.
No `ChannelRuntimeContext` field caches a resolved classifier provider.

Not in this PR (deferred to follow-ups):
  * `classifier_temperature` override (current code inherits
    `runtime_defaults.temperature`; deterministic 0.0 can be achieved
    today by pointing classifier_provider at a dedicated alias entry
    with `temperature = 0.0` in its `[providers.models.*.*]` block)
  * `classifier_history_window` (small models may run out of context
    on long histories — separate issue, separate PR)
  * `QueryClassificationConfig` (Layer 2.a rule-based router) keeps
    its current global-config shape; only Layer 2.b LLM classifier
    gets the per-agent override

ACP channels (IDE-direct) hit the `is_acp_channel` early-return at
mod.rs:3503 and skip the classifier entirely, so the new field has no
effect on ACP traffic.

Risk: Low-Medium (Beta `zeroclaw-config` + Experimental
`zeroclaw-channels`; new field is opt-in default-off; no trait /
security / gateway boundary impact). See plan
`.sisyphus/plans/kanmars.req.20260522.001.plan.md` for the full
rationale.

Co-authored-by: Sisyphus <sisyphus@ohmyopencode.local>
EOF
git push -u origin feat/classifier-provider-per-agent-override
```

**预期**：push 在沙箱可能凭证失败，由用户手动 push。

---

## 4. 验证清单（PR 提交前必须全绿）

| # | 项 | 命令 | 预期 |
|---|---|---|---|
| V1 | Schema 字段添加 | `grep -n "classifier_provider" crates/zeroclaw-config/src/schema.rs` | ≥ 2 行（字段声明 + `typed_provider_refs` tuple） |
| V2 | ModelProviderRef 类型正确 | `grep -nE "classifier_provider: crate::providers::ModelProviderRef" crates/zeroclaw-config/src/schema.rs` | 1 行 |
| V3 | resolver 函数存在 | `grep -n "async fn resolve_classifier_route" crates/zeroclaw-channels/src/orchestrator/mod.rs` | 1 行 |
| V4 | 调用点改造 | `grep -nE "resolve_classifier_route\(ctx" crates/zeroclaw-channels/src/orchestrator/mod.rs` | 1 行（仅在 process_channel_message_body 内调用） |
| V5 | `route.model` 调用点数 | `grep -nE "route\.model\.(as_str\|clone)\(\)" crates/zeroclaw-channels/src/orchestrator/mod.rs` | ≥ 7 行（PR 前 7 处 + Step 4 改写后 fallback 内 1 处 `route.model.clone()` —— 总数应增加或不变） |
| V6 | classify_channel_reply_intent 签名不变 | `grep -nA6 "async fn classify_channel_reply_intent" crates/zeroclaw-channels/src/orchestrator/mod.rs` | 签名行参数与 master 完全一致 |
| V7 | 不引入新依赖 | `git diff kanmars_main -- Cargo.toml crates/*/Cargo.toml` | 无变更 |
| V8 | 不动 zeroclaw-api | `git diff kanmars_main -- crates/zeroclaw-api/` | 无变更 |
| V9 | SSOT 检查：未在 ctx 加缓存字段 | `grep -nE "classifier_provider:" crates/zeroclaw-channels/src/orchestrator/mod.rs` | 仅出现在 `ctx.agent_cfg.classifier_provider` 读取位置 + resolver 参数 + 调用点 + 测试，**不**出现在 `struct ChannelRuntimeContext` 定义内 |
| V10 | Format | `cargo fmt --all -- --check` | exit 0 |
| V11 | Lint zeroclaw-config | `cargo clippy -p zeroclaw-config --all-targets -- -D warnings` | exit 0 |
| V12 | Lint zeroclaw-channels | `cargo clippy -p zeroclaw-channels --features channel-lark --all-targets -- -D warnings` | exit 0 |
| V13 | 新 schema 测试 | `cargo test -p zeroclaw-config classifier_provider` | 3 passed |
| V14 | 新 resolver 测试 | `cargo test -p zeroclaw-channels --features channel-lark resolve_classifier_route` | 2 passed（或 5b helper 不可用时 跳过） |
| V15 | 既有 channels 测试不回归 | `cargo test -p zeroclaw-channels --features channel-lark process_channel_message` | 全绿（pre-existing telegram 2 failures 不变） |
| V16 | 完整 CI | `./dev/ci.sh all` | exit 0 |
| V17 | 改动文件数 | `git diff --stat HEAD~1..HEAD` | 4 文件（schema.rs + mod.rs + CHANGELOG + plan）|
| V18 | CHANGELOG 已写 | `grep -nE "classifier_provider" CHANGELOG-next.md` | ≥ 1 行 |
| V19 | typed_provider_refs 数组已扩展 | `grep -nE "classifier_provider.*trim\(\)" crates/zeroclaw-config/src/schema.rs` | 1 行（在 14581 一带 typed_provider_refs 数组内） |

### 4.1 线上回归验证（PR merge + 部署后用户实测）

| 场景 | 配置 | 期望 |
|---|---|---|
| **R1 — 老用户零感知** | 不写 `classifier_provider` 字段 | 行为与 PR 前完全一致；日志无 `classifier_provider` 相关 warn |
| **R2 — kimi 分类 + qwen 回答** | 用户原 TOML（`classifier_provider = "custom.kimi-k2-5"`） | 飞书发"今天几号" → ① 分类器调用日志显示走 `kimi-k2.5` ② 主 agent 回答日志显示走 `qwen3.6-plus` ③ 用户在飞书看到正常回答 |
| **R3 — NO_REPLY 路径** | 同 R2 | 飞书群里发与 bot 无关的闲聊 → kimi 分类器返回 `NO_REPLY[INFO]` → bot 不回复，仅留 NoReply marker（沿用现有行为）|
| **R4 — 配置错误 fail-loud** | 故意写 `classifier_provider = "custom.no-such-alias"` | 启动期 `cargo run` / 部署 init 直接 panic + 报错信息 `agents.default.classifier_provider = "custom.no-such-alias" but providers.models.custom.no-such-alias is not configured` |
| **R5 — 运行期 hot-reload 错误软降级** | 启动后才把 `[providers.models.custom.kimi-k2-5]` 整段删掉（不重启）| 下一条消息 → resolver 解析失败 → warn 日志 + 自动降级到主 agent 跑分类器；bot 仍能正常回答 |
| **R6 — ACP 不受影响** | 在 IDE 中通过 ACP 协议直连，配置同 R2 | ACP turn 不调分类器，直接进 agent loop；kimi 调用计数 0 |
| **R7 — 成本节省** | 同 R2，连续 100 条飞书消息 | provider usage 日志中 kimi 调用 = 100；qwen 调用 = NO_REPLY 之外的 reply 路径数（通常 < 100）→ 至少省下 NO_REPLY 路径的主模型成本 |

---

## 5. 风险与缓解

| # | 风险 | 严重性 | 缓解 |
|---|---|---|---|
| R1 | **小模型（kimi/qwen-turbo）输出格式漂移**：返回不是规范的 `REPLY` / `NO_REPLY[...]` token，解析失败 | 中 | [`parse_reply_intent` @ mod.rs:2308](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L2308) 已有兜底：未识别 token 默认返回 `Reply(String::new())` → 失败时退化为"按 REPLY 处理"，最差不会让 bot 沉默。线上观察 1 周如果 NO_REPLY 命中率显著下降，回滚或换更稳的小模型 |
| R2 | **小模型 context window 不足**：分类器把整段 system prompt + history 喂进去，免费小模型可能超 8K context | 中 | 当前实现保持 `history[0].content` + `&history` 不变（与主模型 prompt 同源）；如线上发现超 context 报错 → 后续 PR 加 `classifier_history_window` 字段截断历史。本 PR 不处理 |
| R3 | **SSOT 违反**：开发者在 `ChannelRuntimeContext` 加 `classifier_provider: Arc<dyn ModelProvider>` 缓存字段 | 高（一旦 merge，未来维护者会复制此模式） | §0.5 #1 明示禁止；V9 grep 校验；PR description 显式声明"resolver 走 provider_cache 即可，禁止在 ctx 加字段" |
| R4 | **`provider_cache` LRU 容量太小 → kimi provider 被驱逐 → 每次重新建** | 低 | 现有 `ProviderCacheMap` size 由 `runtime` 配置（grep `ProviderCacheMap::new` 确认 capacity）；如默认 capacity = 1 会有问题。如发现，调大 cache 是另一独立优化 |
| R5 | **resolver 在每条消息上的额外延迟**：解析路径 = 字符串 split + HashMap lookup + cache hit | 极低 | 全部是 µs 级别；与 `get_or_create_provider` 主路径同款开销 |
| R6 | **多 agent 用户体验不一致**：用户配了多个 `[agents.*]`，只给一个加 `classifier_provider` | 极低 | 这是 per-agent 设计的有意特性（粒度由用户掌控）；文档明示 |
| R7 | **TOML 命名陷阱**：用户照 `kimi-k2.5` 写 → 表名嵌套错乱 → 启动 panic | 中 | §1.2 + Step 7 CHANGELOG 显式警告；validate 错误信息已能定位到 `classifier_provider` 字段，间接帮助调试 |
| R8 | **配置热更**：用户改 `classifier_provider` 不重启 daemon | 低 | resolver 每次读 `ctx.prompt_config`，热更生效（前提是 prompt_config 本身支持 hot-reload —— 现有架构已支持）|
| R9 | **未来 cron / heartbeat / delegate 也想要分类器配置** | 极低 | 它们走的不是 channel orchestrator 路径，本字段对它们透明（自动不读）；真要加是 follow-up PR |
| R10 | **`typed_provider_refs` 数组未来重构破坏新字段**：上游可能把这个数组改成 derive 宏 | 低 | 上游若重构会一并迁移所有现有字段，新字段也会跟着走；V19 grep 在 CI 阶段会捕获意外断裂 |

### 5.1 回退方案

如 PR merge + 部署后线上出现严重问题：

1. **配置层回退**（最快）：用户删除自己 TOML 里的 `classifier_provider = "..."` 行 → 行为立即回到 PR 前（不需要重新部署）
2. **代码层回退**：`git revert <commit_sha>`（单 commit，4 个文件）
3. **回退影响**：所有用户回到"分类器走主模型"行为；无 schema / 数据迁移

### 5.2 升级路径

如果 `classifier_provider` 大规模采用后用户反映还想细调：

1. follow-up PR：加 `classifier_temperature: Option<f64>`（约 +20 行）
2. follow-up PR：加 `classifier_history_window: Option<usize>` 解决小模型 context 不足
3. follow-up PR：把同款字段加到 `consolidation` / `reflection` 配置块（如果未来这些也想要独立模型）

---

## 6. 后续工作（不在本 PR 范围）

| 编号 | 待解决问题 | 建议优先级 |
|---|---|---|
| F1 | **`classifier_temperature: Option<f64>`** 字段 —— 让用户能强制锁 0 提升分类输出稳定性 | Medium |
| F2 | **`classifier_history_window: Option<usize>`** —— 小模型上下文不足时截断历史 | Medium（取决于 R2 是否真发生）|
| F3 | **memory consolidation 同款字段** —— [`consolidate_turn` @ memory/consolidation.rs:55](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-memory/src/consolidation.rs#L55) 现在也是继承主 agent，同样适合换小模型 | Low |
| F4 | **reflection / heartbeat 同款字段** —— 如果未来引入 reflection pass（参见 [README.kanmars.md §6.1](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/README.kanmars.md#61-后台反思机制对话-n-句后自动总结--生成skill)），同款覆盖入口 | Low |
| F5 | **回提上游**：本 PR 是行为优化 + 向后兼容，可考虑回提 zeroclaw-labs upstream | Low |
| F6 | **observability 增强** —— 在 SSE `/api/events` 流里区分 "classifier llm_request" vs "main llm_request"，便于成本追踪 | Low |

---

## 7. 工作量估算 & 时间线（rev1 修订）

### 7.1 双轨估算（rev1.1 修订 — 区分人工/AI 节奏）

代码量 **+175 / -7**（不是表头早期版的 88，那是写错的）。但工作量不能简单按"行数/分钟"线性算 —— **CI wall-clock 是 fixed cost，写代码部分才能并行加速**。

| 阶段 | 行数 | Sisyphus-Junior 人工估 | AI agent 执行估 | 备注 |
|---|---|---|---|---|
| Step 0（分支 + 5 个前置 grep） | — | ✅ 已完成 | ✅ 已完成 | — |
| Step 1（schema 字段 + 文档注释） | +25 / 0 | 5 min | **1 min** | 一个 edit |
| Step 2（validate：加 1 行 tuple 进 typed_provider_refs） | +5 / 0 | 2 min | **0.5 min** | 一个 edit |
| Step 3（resolver helper） | +40 / 0 | 15 min | **3 min** | 一个 edit + LSP 验证 |
| Step 4（调用点改写） | +10 / -7 | 5 min | **1 min** | 一个 edit |
| Step 5（3 schema 测试 + 2 resolver 测试） | +75 / 0 | 25 min | **8 min** | 测试 TOML/mock 构造最难压缩；**第二轮 assertion 文案微调 buffer 已含**|
| Step 6（fmt + clippy + test + CI） | — | 10 min | **15-20 min** ⚠️ | wall-clock 不可压缩。zeroclaw workspace 大，`cargo test -p zeroclaw-channels` 5-10 min，`./dev/ci.sh all` 15-20 min |
| Step 7（CHANGELOG-next.md） | +20 / 0 | 5 min | **1 min** | |
| Step 8（commit + push） | — | 10 min | **1 min** | push 由用户做（沙箱无 gitee 凭证） |
| **合计** | **≈ +175 / -7** | **≈ 77 min** | **≈ 30-35 min**（含 CI 等待 15-20 min） | |

**关键洞察**：

- 真正"动手写代码"的 AI agent 时间 = **~15 min**（Step 1-5 + Step 7 + 8）
- CI wall-clock = **15-20 min**（fmt + clippy + test + `dev/ci.sh all`），**这是物理下限**
- 所以"30 min 完成全 PR" 是激进但可达的目标；"40-50 min" 是稳健估算（含第二轮 cargo test 失败调试 buffer）
- **77 min 是人工节奏的保守估算**（来自历史 0516 系列 plan 的 Sisyphus-Junior 节奏，含频繁手动 grep + cargo check + 来回验证）

### 7.2 关键不确定项（可能放大工时）

| # | 不确定项 | 触发后增量 |
|---|---|---|
| U1 | Step 5b 找不到 `make_test_runtime_context` helper，需要自己构造 mock context | +10-15 min |
| U2 | `cargo test -p zeroclaw-channels --features channel-lark` 首跑失败，需要看错误调 assertion 文案 | +5 min × N 轮 |
| U3 | clippy 抱怨 `Arc::clone(&active_model_provider)` 的写法（应该 `active_model_provider.clone()`） | +2 min |
| U4 | 沙箱上 `./dev/ci.sh all` 跑出 pre-existing 失败之外的新问题 | +15-30 min 排查 |
| U5 | clippy 抱怨 `match resolve_classifier_route ... { Some(pair) => pair, None => (...) }` 应该改 `unwrap_or_else` | +1 min |

**乐观（无不确定项触发）**：30 min
**预期（U1-U3 各触发一次）**：45-50 min
**悲观（U4 触发）**：60-75 min

### 7.3 与 rev0 对比

| 项 | rev0 | rev1 |
|---|---|---|
| 总工作量 | 95 min | 77 min 人工 / 30-35 min AI |
| 节省 | — | 18 min（Step 2 简化） |
| 代码量 | +182 / -7 | +175 / -7 |

---

## 8. 待用户决策项（已全部敲定）

| # | 项 | 决策 | 说明 |
|---|---|---|---|
| Q1 | 字段名 | **`classifier_provider`** ✅ | 用户已明确确认 |
| Q2 | 是否同 PR 加 `classifier_temperature` | **不加** ✅ | 用户已明确否决；留 follow-up F1 |
| Q3 | 字段类型 | `ModelProviderRef`（非 `Option<ModelProviderRef>`）| 与 `tts_provider` / `transcription_provider` 同款；`Default::default() = empty string` 已表达"未配置" |
| Q4 | 配置粒度 | **per-agent**（放在 `AliasedAgentConfig`）| 与现有 `tts_provider` / `transcription_provider` 字段对称；多 agent 用户灵活 |
| Q5 | 字段插入位置（rev1 确定） | **紧挨 `transcription_provider` 之后**（schema.rs:2779 之后） | 与其他 optional typed provider refs 集中；逻辑分组清晰 |
| Q6 | 解析失败行为 | **软降级 + warn log**（运行期）+ **fail-loud**（启动期 validate） | 启动期错配立即报错；运行期 hot-reload 错配不致命 |
| Q7 | validate 实现（rev1 大简化） | **复用现有 `typed_provider_refs` 数组**，加 1 行 tuple | 错误码 / 文本 / 表单绑定与 tts_provider 完全一致 |
| Q8 | resolver 放哪 | orchestrator/mod.rs 同文件 module-private | 单一调用点，无跨 crate 复用需求 |
| Q9 | ACP 是否受影响 | **否**（ACP 走 [L3503](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-channels/src/orchestrator/mod.rs#L3503) 强制 REPLY，根本不调分类器） | 文档明示 |
| Q10 | 基线分支 | **`kanmars_main`**（不是 `master`）✅ | 用户明确要求；基线 `bf5049e2` |
| Q11 | 沙箱 push 失败处理 | 沿用历次：本地 commit 后用户手动 push | 与 0516 系列同款 |

---

## 9. 关联文档 / 参考

- 需求源对话：本会话第 1-5 轮（用户问"飞书分类是否独有 + 能否指定模型 + 详细方案" + 字段名敲定 + 基于 kanmars_main 拉新分支）
- 分析报告：本会话第 4 轮（Sisyphus 给出三层分类架构 + 模式 A/B/C 对比 + 推荐方案）
- [zeroclaw AGENTS.md — ABSOLUTE RULE SINGLE SOURCE OF TRUTH](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md)
- [zeroclaw AGENTS.md — Anti-Patterns / Stability Tiers / Risk Tiers](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/AGENTS.md)
- 同款参考实现：[MemoryConfig.embedding_model](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L8269) + [resolve_embedding_config](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-memory/src/lib.rs#L219-L285)
- 同款 typed ref：[AliasedAgentConfig.model_provider / tts_provider / transcription_provider @ schema.rs:2738/2769/2778](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L2738)
- ★ **rev1 关键引用**：[typed_provider_refs 通用化校验数组 @ schema.rs:14581-14612](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/schema.rs#L14581-L14612)
- ★ **rev1 关键引用**：[ModelProviders::find 方法 @ providers.rs:332](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-config/src/providers.rs#L332)
- 现成 provider 工厂：[create_routed_model_provider_with_options](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/crates/zeroclaw-providers/src/lib.rs#L1326)
- 二创版本说明：[README.kanmars.md](file:///Users/kanmars/workspace/kanmars_zeroclaw_github/README.kanmars.md)

---

## 10. Sign-off

| 角色 | 状态 |
|---|---|
| 起草人（组件管理员）| ✅ 已完成（rev1：Step 0 已实地跑完，发现 `typed_provider_refs` 通用化数组可直接复用，Step 2 从 12 行降到 1 行；字段位置改到 `transcription_provider` 之后；基线分支锁定 `kanmars_main` @ `bf5049e2`；`ModelProviders::find` 签名已确认） |
| 计划审阅人（用户）| ✅ 已确认（rev1 通过 `/start-work` 授权实施）|
| 实施授权 | ✅ 已授权 |
| 实施状态 | ✅ Step 1-7 全部完成 + 验证通过；Step 8 (git commit + push) 由用户手动 push |

**当前模式**：plan 实施完毕，等用户 push 到 gitee。

## 10.1 实施记录（2026-05-22T15:47 起草，约 X 分钟完成）

### 完成清单（实际执行）

- [x] Step 0 — 分支 `feat/classifier-provider-per-agent-override` 已创建（基于 `kanmars_main` @ `bf5049e2`）+ 5 个前置 grep 已跑完，rev1 修订完成
- [x] Step 1 — `classifier_provider: ModelProviderRef` 字段插入 schema.rs:2780-2805（24 行文档注释 + #[serde(default)] + 字段声明）+ **Default impl 补全** schema.rs:2934（必要修复，否则 E0063 不编译）
- [x] Step 2 — `typed_provider_refs` 数组扩展，1 行 tuple `("providers.models", "classifier_provider", agent.classifier_provider.trim())` 追加于 schema.rs:14616-14621
- [x] Step 3 — `resolve_classifier_route` async fn 插入 orchestrator/mod.rs:2343-2404（60 行，含 SSOT 合规注释 + 两处 WARN log + 软降级返回 None）
- [x] Step 4 — 调用点重写 orchestrator/mod.rs:3539-3558（match resolver + Arc::clone 主路 fallback + classify_channel_reply_intent 传新 provider/model）
- [x] Step 5a — 3 个 schema validate 测试 @ schema.rs:22061-22146；用 `#[test] async fn` 因 `use tokio::test;` 文件本地重绑定；测试 TOML 含 `risk_profile = "default"` + `[risk_profiles.default]` 以通过上游 mandatory 校验
- [x] Step 5b — **意外发现** `router_test_ctx()` helper @ mod.rs:8087（plan §5b 兜底分支未触发）；2 个 resolver 测试 @ mod.rs:8153-8167 复用此 helper（两个测试都期望 `None`，不需真 provider）
- [x] Step 6.2 — `cargo clippy -p zeroclaw-config -D warnings`: exit 0, 23.31s
- [x] Step 6.3 — `cargo clippy -p zeroclaw-channels --features channel-lark -D warnings`: 5 pre-existing errors in lark.rs:3155-3167（`anyhow::anyhow!` disallowed_macros + redundant closure）；**git stash 验证**这 5 个错误在 baseline `kanmars_main` 上完全一致，**与本 PR 无关**
- [x] Step 6.4 — `cargo test -p zeroclaw-config`: 88 passed; 0 failed; 0 ignored
- [x] Step 6.5 — `cargo test -p zeroclaw-channels --features channel-lark --lib`: **1191 passed; 0 failed; 0 ignored** in 7.13s（plan rev1 预期的 "pre-existing telegram failures" 实际为 0）
- [x] Step 6.6 — V1-V19 grep verification 全部通过：
  - V1: 14 hits（字段 + tuple + 3 测试 + ...）
  - V2: 字段在 2805 + Default impl 在 2934
  - V3: resolver fn @ 2354 + 2 测试名
  - V4: 3 处调用（prod 3545 + 2 tests）
  - V5: 7 处 route.model（plan 预期 ≥7）
  - V6: classify_channel_reply_intent 签名 **完全未变**
  - V7: Cargo.toml 0 diff
  - V8: zeroclaw-api 0 diff
  - V9: ChannelRuntimeContext 内**无** classifier_provider 字段（SSOT 合规）
  - V18: CHANGELOG 命中
  - V19: typed_provider_refs 含 classifier_provider.trim()
- [x] Step 7 — CHANGELOG-next.md:20-50 创建 `### Added` section + classifier_provider 条目（+31 行，含 TOML alias 命名陷阱警告）

### 实际改动

- `crates/zeroclaw-config/src/schema.rs`: **+121 行**（plan 预估 +105，差 16 行来自 Default impl 补全 + 测试 TOML 加 risk_profile 适配）
- `crates/zeroclaw-channels/src/orchestrator/mod.rs`: **+93 / -7 行**（plan 预估 +50/-7）
- `CHANGELOG-next.md`: **+31 行**（plan 预估 +20，差 11 行来自完整 TOML 例子 + `### Added` heading）
- `.sisyphus/plans/kanmars.req.20260522.001.plan.md`: 本节追加 + §10 状态更新
- `.sisyphus/boulder.json`: 重写为本 plan（旧 0516.004 状态归档到 `.sisyphus/boulder.json.kanmars.req.20260516.004-completed-2026-05-22`）
- `.sisyphus/notepads/kanmars.req.20260522.001/learnings.md`: 实施 sub-agent 写入了详细 deviation report + cargo output + 验证矩阵

### 实施 sub-agent 调度记录

| Task | Agent | Duration | Session ID |
|---|---|---|---|
| `bg_b542e130` zeroclaw-config 改动 | Sisyphus-Junior (unspecified-high) | 10m 51s | `ses_1af9db416ffe9cSMva7Fx9pTnC` |
| `bg_8ddcade3` zeroclaw-channels 改动 | Sisyphus-Junior (unspecified-high) | 7m 20s | `ses_1af9d025dffejBPbx7iv1Xxa3c` |
| `bg_514bf4d9` CHANGELOG entry | Sisyphus-Junior (writing) | 1m 24s | `ses_1af9cb5bfffe4orzhCSPmUk6R0` |

**并行调度**：3 个 task 同时 fire，wall-clock = max(10m51s, 7m20s, 1m24s) = **~11 min**（不是串行的 19m35s）。

Atlas 验证（Phase 1 读代码 + Phase 2 自动化 + Phase 4 gate）约 5 min。

**实际总耗时（不含 Step 0 + Step 8）≈ 16 min**，落在 rev1 §7.1 估算的"AI agent 30-35 min（含 CI 等待 15-20 min）"区间内、且偏 wall-clock 的下半边 —— Step 6.5 cargo test 实际 7.13s 远快于估算的 15-20 min（zeroclaw-channels 没有重 IO 集成测试）。

### Step 8 待用户行动

1. 由 atlas 接下来 delegate 给 git-master skill：`git add` 4 个文件（schema.rs + mod.rs + CHANGELOG + 本 plan）+ atomic commit（用 plan §3 Step 8 准备好的 60 行 commit message）
2. push 由用户在本机执行：`git push -u origin feat/classifier-provider-per-agent-override`（沙箱无 gitee 凭证）
3. 创建 MR / PR 给用户审

### 关键审阅点（用户审 commit + MR 时重点）

1. **SSOT 合规审计**：V9 已验证 `ChannelRuntimeContext` 未加 `classifier_provider` 字段；resolver 每次 re-read `ctx.prompt_config`；新字段仅是 `ModelProviderRef` 字符串引用，未复制 api_key/uri/temperature
2. **Default impl 必要补全**：schema.rs:2934 的 `classifier_provider: ModelProviderRef::default()` 是 `AliasedAgentConfig` 现有 Default::default() 要求的（E0063），不是 scope creep
3. **5 个 clippy errors 是 pre-existing**：git stash 验证过，全部在 lark.rs:3155-3167，来自 PR 002/003/004 系列代码，本 PR **不引入**任何新 clippy 错误
4. **测试 TOML 加 risk_profile**：上游 validate 强制要求 `agents.{alias}.risk_profile` 非空且引用配置，plan §3 Step 5a snippet 未含此字段；实际跑测试时不加会失败（Test 2 + 3）；Test 1 不加也会通过（DanglingReference 在 risk_profile 校验之前 fire）但保持一致性
5. **行号 shift**：Step 4 调用点从 plan 写的 3477 shift 到 3545，因为 Step 3 插入了 60 行 resolver 在前面
6. **5b 实际可行**：rev1 §3 Step 5b 写"如果 helper 不存在则跳过"，实际找到 `router_test_ctx()` @ mod.rs:8087，5b 测试上线无需跳过

**rev1 相对 rev0 的关键变动**：

1. **基线分支**：`master` → `kanmars_main`，基线 commit 锁定 `bf5049e2`
2. **目标分支**：从"待创建"改为"已创建"（Step 0 已执行）
3. **Step 1 字段插入位置**：从"`model_provider` 之后" → "`transcription_provider` 之后"（与其他 optional typed provider refs 集中）
4. **Step 2 validate**：从"手写 12 行 fail-loud block" → "加 1 行 tuple 进 `typed_provider_refs` 数组"（**最大改进**，省 10 min + 5 行 diff）
5. **Step 3 resolver**：删除"`find` 方法签名以 grep 结果为准"caveat，已确认存在
6. **新增 §1.4**：详细解释 `typed_provider_refs` 数组复用机制
7. **新增 §10 rev1 修订说明**：本节
8. **工作量**：95 min → 77 min（不含已完成 Step 0）
9. **代码量**：+182/-7 → +175/-7
10. **验证清单 V19**：新增 grep 校验 `typed_provider_refs` 数组扩展

**审阅时建议重点确认**（rev1 简化版）：

1. **§3 Step 2 方案**：复用 `typed_provider_refs` 数组（错误信息文本通用化）vs 手写自定义 fail-loud block（错误信息可定制）—— 推荐前者（已采纳）
2. **§3 Step 1 字段位置**：放在 `transcription_provider` 之后 vs `model_provider` 之后 —— 推荐前者（已采纳）
3. **§5 风险表 R1（小模型输出格式漂移）**：是否要在本 PR 加一个"非规范返回值告警计数器"指标？还是依赖 parse 兜底 + 线上观察就够？
4. **§3 Step 5b resolver 单测**：如果 `make_test_runtime_context` helper 在 mod.rs 内不存在，是接受跳过 5b（由线上验收覆盖），还是阻塞直到加 helper？

实施授权后将严格按 §3 Step 1 → Step 8 顺序执行（Step 0 已完成），每个 Step 完成在 todo 列表里实时打钩。
