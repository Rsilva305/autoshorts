# Backlog / Future Improvements

Ideas to come back to later. Not scheduled, just captured so they don't get lost.

## 1. Recut a clip with a different caption style — DONE (2026-07-02)

Caption style used to only be chosen once, at initial project/media upload,
with no way to change it per-clip. Fixed:

- `clips` table stores its own `caption_style`; `render_flat_clip_for_candidate`
  accepts an override, falls back to the project default, persists whatever
  was actually used on success
- Clicking Cut/Re-cut opens a modal (reuses the import wizard's style-card
  grid) to pick a style before rendering — first attempt was an inline
  dropdown, rejected for theme/hit-target problems, modal is the shipped version
- Added an 8th option, "No Subtitles" (`caption_style: "none"`), which skips
  caption burn-in entirely rather than just picking an empty-looking style

Confirmed working end-to-end in the running app 2026-07-02.

## 2. Full web app version (SaaS direction)

Rebuild autoshorts as a web app instead of (or alongside) the Tauri desktop
app — same shape as the FastAPI + Next.js pattern seen in OpenMontage's
PR #241 (job queue, SSE progress, browser-based dashboard).

This is tied to the monetization/SaaS idea discussed 2026-07-01 — see that
conversation for the licensing consideration (current codebase has no
license; a from-scratch rebuild was the direction being explored, contingent
on checking in with the upstream author first).

## 3. Easier way to view a rendered clip — DONE (2026-07-02)

Clicking the "Cut ready" badge now reveals the file in Explorer/Finder
(`revealItemInDir`, no scope needed). Added a small play-icon button next to
it that opens the video directly in the default player (`openPath`).

Needed `tauri-plugin-opener` (Rust + JS). `openPath` required an explicit
path scope in `src-tauri/capabilities/default.json` beyond just the bare
permission string — `reveal_item_in_dir` doesn't take a scope param at all,
but `open_path` does, and errors with "Not allowed to open path ..." without
one. Scoped to `$DOCUMENT/AutoShorts/**`, which covers every project's
output (see `documents_project_dir` in `lib.rs` for why that's the right base).

## 4. Occasional "parsing candidate JSON" error on Find Viral Moments — DONE (2026-07-02)

Happened intermittently when importing a new video and running moment
detection — the LLM occasionally returns text that isn't valid JSON
(truncated mid-response, stray prose, an unescaped character in a "hook"/
"rationale" string), and there was no retry, so the user had to notice the
error and manually re-click "Find Viral Moments."

Fixed in `src-tauri/src/lib.rs`: extracted the provider dispatch (was inline
in `generate_candidates`) into `request_candidates()`, which returns the raw
`anyhow::Error` instead of an already-stringified one. `generate_candidates`
now retries up to 3 attempts (800ms apart) specifically when the error is a
JSON-parse failure (`parse_candidate_json` in `llm.rs`) — auth/network/missing-key
errors still fail immediately, since retrying those wouldn't help. No UI
changes needed; this is invisible when it works and just means the
occasional flaky response gets silently retried instead of surfacing an error.

---
*Started 2026-07-01.*
