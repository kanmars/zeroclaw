# Issues / Blockers — kanmars.req.20260515.001

## [2026-05-15 ~13:55 CST] PRODUCTION REGRESSION ON DEPLOYED `589ab381`

### Symptom
User confirmed deployment via `zeroclaw --version` on `Malorian-3516`:
- `zeroclaw 0.7.4`
- `build-time: 2026-05-15 11:52:37 +0800`
- `git-commit: 589ab381`

After clicking ✅✅ Always on a `cron_add` approval card in the Feishu chat:
- Card UI does **not** update visually
- Buttons remain clickable

This is the **exact bug the PR was meant to fix**. wiremock unit tests all passed (3 new + 2 migrated). Production behavior diverges from test behavior → tests didn't cover the real failure surface.

### Root cause: UNKNOWN — narrowed to 4 suspects

| # | Suspect | Diagnostic log signature |
|---|---------|--------------------------|
| 1 | `data.message_id` empty in send response → `PendingApproval { message_id: "" }` → handler skips PATCH | `Lark: approval card sent but no data.message_id in response — post-click card update will be skipped (approval_id=...)` |
| 2 | Card 1.0 schema rejected by PATCH endpoint | `Lark: approval card PATCH soft-failed for ...: code=230003, body=...` |
| 3 | `msg_type: interactive` PATCH not supported on this tenant / SDK version | `Lark: approval card PATCH soft-failed for ...: code=230002, body=...` |
| 4 | Tenant token missing `im:message` scope | `Lark: approval card PATCH still unauthorized after token refresh ...` |

### Why our tests didn't catch it
- wiremock mocks always return `{"code": 0, "data": {"message_id": "om_appr_1"}}` — the happy path
- Never exercised real-tenant Feishu response shape for **private chat** or PATCH semantics on `msg_type: interactive` cards
- Plan §5.2 R2 worried about schema mismatch but concluded "1.0 + 1.0 PATCH is fine" without live verification

### Diagnostic attempt (2026-05-15 ~14:10 CST)

User provided WS gateway URL for `Malorian-3516`:
```
ws://47.110.3.192:17802/ws/chat?token=zc_...&session_id=feishu_oc_99bc..._oc_99bc...
```
with subprotocols `zeroclaw.v1, bearer.zc_...`.

**Attempted**: 4+ probe attempts via Python `websockets` 16.0 + raw `curl --upgrade websocket`.

**Result**: TCP connection succeeds, **but no HTTP response is ever returned from the server** — handshake hangs until client timeout (tested 6s, 25s, 35s — all silent).

**Same behavior on every endpoint tested**:
- `GET /healthz` → silent timeout
- `GET /` → silent timeout
- `GET /health` → silent timeout
- `GET /api/status` → silent timeout
- `GET /ws/chat (upgrade)` → silent timeout

**Conclusion**: `47.110.3.192:17802` accepts TCP from this sandbox but does not deliver any HTTP response. Most likely:
1. **Firewall / IP allowlist** on the remote — sandbox public IP not whitelisted (most plausible: the user's curl example was browser-paste from their own laptop session, where their IP is allowed)
2. **Reverse-proxy hung / misconfigured** — accepts socket but doesn't forward to gateway
3. **Gateway not running** despite zeroclaw process being alive

### Status
🚦 **DOUBLE BLOCKED**:
1. Cannot reach the gateway from this sandbox to query logs / interact
2. Even if I could query, the diagnostic still requires log evidence to choose between 4 fix paths

### What user needs to do
**Option A** (preferred): User runs the diagnostic from the deployment machine itself:
```bash
journalctl -u zeroclaw -n 500 --no-pager | grep -E "approval card PATCH|approval card sent|card.action.trigger" | tail -20
```
and pastes output here.

**Option B**: User adds the sandbox public IP to the gateway firewall allowlist, then I can interact via WS.

**Option C**: User runs a one-shot Feishu approval on their end while tailing logs, captures the relevant log span, and pastes it.

### Lesson
Plan §5.1 step 4 ("联调 / 用户侧验证") was marked "可选" — for a UX bug where wiremock cannot simulate real-tenant API responses, this should have been **mandatory before declaring the PR ready**. wiremock + Card-spec assumptions are insufficient verification for IM-platform integrations.
