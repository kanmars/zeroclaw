## [2026-05-16T13:50] Step 0 抓包不可行 → 改为 belt-and-suspenders pointer

### 调研结论

- 代码 [`lark.rs:1121-1122`](file:///home/admin/workspace-public/kanmars/zeroclaw/crates/zeroclaw-channels/src/lark.rs#L1121-L1122) **不 dump** raw `card.action.trigger` payload (`event.event` 直接派发到 handler)
- 生产 `gloria/A/logs/zeroclaw.log` 12:27 时只有 handler 提取后的字段日志（log:774/780），没有 raw JSON
- 沙箱无飞书真实 token，无法 wiremock + 真实 token 发 Card 2.0 测试卡
- 历史日志里所有 click 都是 Card 1.0 卡产生的 → **从生产日志推断 Card 2.0 click pointer 物理上不可能**

### 决策：放弃 Step 0 抓包，改为防御性双 pointer

§3 Step 3 计划本来是"看抓包结果决定"，现在直接执行：

```rust
// handle_card_action_event L1868-L1870 改为：
let value = event_payload
    .pointer("/action/value")
    .or_else(|| event_payload.pointer("/action/behaviors/0/value"))
    .ok_or_else(|| anyhow::anyhow!(
        "card.action.trigger: missing event.action.value or event.action.behaviors[0].value"
    ))?;
```

### 理由

1. **零风险**：对当前 Card 1.0 click 行为完全不变（`/action/value` 优先命中）
2. **未来兼容**：飞书 Card 2.0 即使把 value 迁到 `behaviors[0].value`，handler 自动走兜底路径
3. **PR 仍是单文件** + 仍是 schema 修复主题（不扩大范围）
4. **省去联调**：不需要发真实卡片就能 ship

### Q3 决策

Q3（plan §8）原默认值"看抓包结果决定" → **改为强制加 fallback**（§8 Q3 备选项）

### Q1/Q2 沿用 plan 默认

- Q1 button type: `primary_filled` (Card 2.0 标准命名)
- Q2 排列: 横排 (`column_set`)
- Q4 不修 F1
- Q5 验收 3 次连续审批
- Q6 push 失败用户手动 push
