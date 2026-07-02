# Backlog / Future Improvements

Ideas to come back to later. Not scheduled, just captured so they don't get lost.

## 1. Recut a clip with a different caption style — IN PROGRESS (2026-07-01/02)

Caption style used to only be chosen once, at initial project/media upload.
There was no way to re-render an existing clip candidate with a different
caption choice without redoing the whole import/transcribe/detect pipeline.

**Backend — done, working, compiles clean:**
- `clips` table now has its own `caption_style` column (migration in `db.rs`)
- `render_flat_clip_for_candidate` takes an optional `caption_style` override;
  falls back to the project's default style if not given
- Resolved style is persisted on successful render so re-opening a project
  remembers what style each clip was actually cut with

**Frontend — first attempt didn't land, needs a different approach:**
Tried a per-candidate dropdown next to the Cut/Re-cut button (custom-built,
not a native `<select>`, since native select popups ignore the app's dark
theme on Windows/WebView2 — confirmed that the hard way, screenshot showed a
white native popup). Custom dropdown fixed the native-styling problem but
introduced new ones: background text bleeds through the transparent popover,
and the options are awkward to click reliably.

**Agreed next direction (pick up here tomorrow):**
Ditch the inline dropdown. Instead, clicking "Re-cut" should open a **full
modal** — same visual style-card grid used in the import wizard (`style-card`,
`style-preview-box`, `preview-text-*` CSS classes already exist and work well
there) — let the user pick a style, then confirm to actually re-render.
Also add a **"No Subtitles"** option to the style choices (currently all 7
styles always burn captions; there's no way to render a clip with none).

Relevant files: `src/main.tsx` (candidate card UI, `CAPTION_STYLES` array,
`captionStyleForCandidate`, `cutCandidate`), `src/styles.css` (style-card /
preview classes to reuse), `src-tauri/src/lib.rs` (`render_flat_clip_for_candidate`,
`build_drawtext_filters` — will need a "none" branch that skips captions
entirely once the no-subtitles option exists).

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
