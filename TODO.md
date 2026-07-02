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

## 3. Easier way to view a rendered clip

Right now, seeing the finished render means copying a file path/URL and
pasting it somewhere manually. Want an in-app preview/player, or at minimum
an "Open" / "Reveal in folder" button next to a finished render.

---
*Started 2026-07-01.*
