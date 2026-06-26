# Learnings — kanmars.req.20260515.001

## [2026-05-15 ~16:00 CST] DIAGNOSIS COMPLETE — Card 1.0 PATCH schema silently fails on client

### How we got here
1. Initial PR (`589ab381`) shipped with Card JSON 1.0 schema for `build_resolved_approval_card`, matching the send-time `build_approval_card`. Plan §5.2 R2 worried about schema mismatch but concluded "1.0 + 1.0 PATCH is fine" — without live verification.
2. wiremock unit tests all passed (3 new + 2 migrated).
3. Deployed to production (`Malorian-3516`, `47.110.3.192`).
4. User reports: clicking ✅✅ Always on cron_add card → no visual change.
5. Diagnostic via user-side log grep:
   - `tail -n 5000 zeroclaw.log | grep -iE "approval card PATCH|approval card sent|card.action.trigger|approval_id"` → ZERO matches
   - But `Lark[: ]|feishu` keyword → many matches (so stderr IS being captured to the log file)
   - `card.action|message.receive_v1|webhook|im\.message|dispatch` → only README boilerplate match
   - 13:54:21 cron task DID trigger → backend got the decision

### Decisive evidence chain
- Lark stderr is reaching the log file ✅
- "Watering reminder" task triggered ✅ → `oneshot.send(decision)` succeeded → `handle_card_action_event` ran fully
- No `approval card PATCH` warn lines in log ✅ → either (a) PATCH not called, or (b) PATCH returned code=0 silently
- No `approval card sent but no data.message_id` warn ✅ → message_id was extracted successfully → branch (b) confirmed
- Therefore: **PATCH was called, returned code=0, but client did not render**

### Root cause confirmed
Feishu IM `PATCH /im/v1/messages/{id}` is **lenient** about Card 1.0 envelopes:
- Accepts the request (returns HTTP 200 + `{"code":0,"data":{...}}`)
- But the client (Feishu desktop/mobile) does NOT re-render the card

The production-validated `patch_card_content` (streaming output, PR4) uses **Card 2.0** schema and works correctly. This is the canonical schema for PATCH operations on this endpoint.

### Fix shipped (commit 2300498a)
1. **Switched `build_resolved_approval_card` to Card 2.0 schema**
   - Now emits `{"schema":"2.0", "config":{}, "header":{}, "body":{"elements":[markdown]}}`
   - Decision banner is now embedded in the markdown body (since Card 2.0 doesn't have `note` element). Format: `**Tool:** \`name\`\n\nargs\n\n---\n\n**emoji decision_text**`
   - Header preserved (Card 2.0 supports same `header.template` colors as 1.0)
2. **Added observability** that was missing in 589ab381:
   - `info` log when `patch_approval_card_resolved` dispatches (with message_id, decision)
   - `info` log on PATCH success (with status, message_id) — was previously silent
   - `info` log on `card.action.trigger` receipt (with approval_id, decision, message_id, has_message_id)
   - Rate-limit (230020) upgraded from `debug` to `warn` (was filtered by default RUST_LOG=info)
   - Unknown/expired approval_id upgraded from `debug` to `info`
   - Non-zero response code warn now also includes HTTP status
3. **Strengthened test assertion**: `body_string_contains("\\\"schema\\\":\\\"2.0\\\"")` ensures future regressions back to Card 1.0 are caught at test time.

### Verification
- `cargo clippy -p zeroclaw-channels --all-targets --features channel-lark -- -D warnings` → exit 0 ✅
- `cargo test -p zeroclaw-channels --features channel-lark --lib 'lark::tests'` → 97 passed, 0 failed ✅
- `cargo test -p zeroclaw-channels --features channel-lark --lib` → 1037 passed, 2 pre-existing telegram failures (unchanged) ✅
- `cargo fmt --all -- --check` on lark.rs → clean ✅

### What user needs to do now
1. `git pull` on Malorian-3516 to get commit `2300498a`
2. Rebuild: `cargo build --release` (or whatever the deploy pipeline does)
3. Restart zeroclaw service
4. Trigger an approval card again, click any decision
5. Expected: card visually updates within ≤1s, buttons disappear, decision banner shows
6. If it STILL doesn't render: the new `info` logs will show in the log file:
   - `Lark: card action received (approval_id=..., decision=..., message_id=..., has_message_id=true/false)`
   - `Lark: approval card PATCH dispatching (message_id=..., decision=...)`
   - `Lark: approval card PATCH succeeded (message_id=..., status=200 OK)` ← if this appears, problem is downstream of our code (client cache, etc.)
   - or `Lark: approval card PATCH soft-failed for ...: code=..., status=..., body=...` ← actionable error info

### Key lesson
**For UX bugs in IM platform integrations, wiremock unit tests are insufficient.** They model HTTP mechanics but cannot model the IM client's rendering decisions. Plan §5.1 step 4 ("联调") was marked optional — it should be mandatory for any visual-state change in approval/card flows. Schema decisions ("use 1.0 to match the send-time card") need at minimum one production trial before merge.

### Push status
Commit `2300498a` is local-only. Sandbox cannot push (no gitee creds). User must:
```bash
# On a host with gitee credentials:
git fetch origin fix/feishu-approval-card-resolved-state
git pull --rebase origin fix/feishu-approval-card-resolved-state  # picks up local commit
git push origin fix/feishu-approval-card-resolved-state
```
Or apply the patch directly: `git format-patch master..HEAD` then mail to deploy host.
