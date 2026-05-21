
## PR7 — refactor/lark-unify-ws-http-parser — ABORTED 2026-05-12

**Decision**: bail out, no commit, branch deleted.

**Investigation summary**:
- WS path lark.rs:918-989 (inner type match)
- HTTP audio sub-path lark.rs:1470-1560 (top-level early-return for audio only)
- HTTP main sub-path lark.rs:1647-1733 (inner type match)

**Findings**:
1. `parse_post_content_details` (L2221) and `parse_list_content` (L2311) ALREADY are module-level shared helpers; both WS and HTTP call them. No duplication left for post/list.
2. Text/image/file inner blocks total ~22 lines × 2 sites. Adding a helper fn (signature + doc + dispatch) costs ~30 lines → net-positive on lark.rs, fails AC.
3. Log strings differ ("Lark WS:" vs "Lark:") and constraints forbid changing them; merging download-fallback log paths needs a `log_prefix` param, making the helper uglier than the duplication.
4. Audio handling structurally diverges: WS inline in match arm; HTTP is a sibling top-level dispatcher (`parse_event_payload_with_audio`) that short-circuits before calling `parse_event_payload`. Not a unifiable inner-match case.
5. WS has 5 interleaved side effects (ws_seen_ids dedup, strip_at_placeholders, should_respond_in_group, ack-reaction spawn, tracing::debug); HTTP has 2. Refactor risks behavior drift unit tests can't catch.

**Conclusion**: filed as RFC technical debt. The genuine remaining duplication is small enough that "drift risk" is the only motivation, and side-effect ordering makes safe extraction net-line-positive. Plan §3 PR7 acceptance criteria explicitly allowed this outcome.
