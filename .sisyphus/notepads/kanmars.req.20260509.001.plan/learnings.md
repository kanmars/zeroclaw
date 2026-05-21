# Learnings — kanmars.req.20260509.001 (DEBUG_CHAT eprintln → tracing)

## 2026-05-09 PR1 完成 + 沙箱边界处理

### PR1 结果
- commit `f8a01a29` 落地 fix/debug-chat-tracing-replace 分支
- 1 file changed (+7, -17)，仅 `crates/zeroclaw-providers/src/compatible.rs`
- atlas 亲跑 6 步验证全 PASS：
  - cargo check exit 0
  - cargo clippy -D warnings exit 0
  - cargo test 807 passed; 0 failed
  - DEBUG_CHAT 字符串计数 = 6
  - eprintln! 计数 = 0
  - tracing::(debug|warn)! 计数 = 6（4 debug + 2 warn）

### 沙箱边界遇到的两个 blocker

1. **cargo 不在默认 PATH**：
   - subagent 当初在某个 shell 里能跑 cargo（位于 `~/.cargo/bin/cargo`，是 rustup symlink）
   - 默认 bash session 不 source `~/.cargo/env`
   - 修法：每个 cargo 命令前 `export PATH="$HOME/.cargo/bin:$PATH"`
   - 教训：subagent 报告 "cargo exit 0" 时可能是因为 PATH 不同；atlas 必须独立验证（即使麻烦）

2. **gitee push 需要凭证**：
   - `git push` 触发 `fatal: could not read Username for 'https://gitee.com'`
   - 沙箱禁了 terminal prompt，无法交互输入
   - 用户决定自己 push（沙箱 agent 边界外，§4.5 经验）
   - 教训：当 plan 里有"push to remote"步骤时，必须显式标 "user-side closure"

### Plan §4.5 沙箱边界教训应用
本任务剩余 13 个 unchecked = PR2 验收项，但 PR2 启动门要求 "PR1 合并 + 观察 ≥1 晚后才开始"（plan §3.1.1 PR2 line 320）。

→ **boulder loop 不应 continuation 续作 PR2**，因为：
- PR1 还没合（用户在 push）
- 观察期还没过（≥1 晚）
- 强行启动 PR2 = 违反 plan 自己写的硬门禁

→ 决策：**归档 boulder.json**（rename 加日期后缀），让 hook 不再触发 continuation。等用户合 PR1 + 观察期通过后，用户主动 `/start-work` 重新启动会话做 PR2。

### Subagent 验证经验
- subagent 报告"全 PASS"时，atlas 必须**亲跑同款命令**复现（不能只看 subagent 输出文本）
- 这次 subagent 没说谎，但 PATH 差异导致 atlas 第一次跑出 `cargo: command not found` 假阳性，差点误判 subagent 撒谎
- 修复：对 cargo/Rust 工具链命令，atlas 验证脚本里 export PATH 是必需前置
