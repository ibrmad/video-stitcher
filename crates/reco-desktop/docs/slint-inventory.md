# Reco desktop GUI (`crates/reco-gui`): inventory for a screen-by-screen port

> The Slint app was removed on 2026-10-06, after the owner signed off the
> parity sweep ([PARITY.md](../PARITY.md)). This inventory stays as the record
> of what was ported; `git log --diff-filter=D -- crates/reco-gui/ui/main.slint` finds the commit, and its parent has the app.

**Scope and caveats**
- Everything is under `crates/reco-gui/`.
- File sizes: `ui/main.slint` 2,862 lines, `src/main.rs` 4,710, `src/export.rs` 388, `src/playback.rs` 260, `src/preview.rs` 216, `src/settings.rs` 196, `src/toast.rs` 240, `src/telemetry_client.rs` 343, `build.rs` 16.
- Slint is `~1.15` with features `std, backend-winit, renderer-femtovg-wgpu, renderer-software, accessibility, compat-1-2, unstable-wgpu-28`.
- Line numbers refer to commit 684ac91 (branch feat/makepad-desktop-ui), which includes the colour-match and export-default edits.

---

## 1. Window and layout skeleton

```
RecoApp inherits Window ............................................ 438–2862
│  title "Reco Video Stitcher", preferred 1280×820, min 720×600, background bg-base
│  theme tokens 445–474 · public API (properties and callbacks) 476–803
│  forward-focus: key-scope; public function refocus() 811–812
├─ key-scope := FocusScope (100%×100%, global shortcuts) ............ 814–2852
│  │  init (focus + Palette.color-scheme) 821–824, key-pressed 825–847
│  ├─ VerticalLayout spacing 0 ...................................... 854–1776
│  │  ├─ Toolbar Rectangle h=42 ..................................... 858–913
│  │  ├─ HorizontalLayout (vertical-stretch 1) ...................... 916–1595
│  │  │  ├─ [if files-panel-open] Left "Files" panel ................ 921–1220
│  │  │  │     resize handle 926–940, ScrollView 942–1219
│  │  │  ├─ Preview area Rectangle .................................. 1226–1342
│  │  │  │     empty state 1230–1254, preview-box 1260–1312, drag TouchArea 1314–1341
│  │  │  └─ [if controls-open] Right "Controls" panel ............... 1347–1593
│  │  │        resize handle 1352–1366, ScrollView 1368–1592
│  │  ├─ Transport bar Rectangle h=44 ............................... 1598–1697
│  │  └─ Status bar Rectangle min-h=28 .............................. 1700–1775
│  ├─ [if recent-dialog-open]     Recent files modal ................ 1783–1961
│  ├─ [if shortcuts-dialog-open]  Keyboard shortcuts modal .......... 1964–2036
│  ├─ [if prefs-dialog-open]      Preferences modal ................. 2043–2198
│  ├─ [if bug-dialog-open]        Report-a-bug modal ................ 2201–2265
│  ├─ [if export-dialog-open]     Export modal ...................... 2274–2744
│  └─ [if lens-picker-open]       Lens profile picker modal ......... 2746–2851
└─ ToastStack overlay (direct child of Window, outside FocusScope) .. 2857–2861
```

- **What is absent:** no menu bar, no context menus, no tooltips, no tabs (`TabWidget` is imported but unused). There is no timeline track; the "timeline" is a std `Slider` in the transport bar.
- **Shared modal pattern (all 6 dialogs):**
  - A full-window scrim `Rectangle` (`bg-scrim`) with a full-size `TouchArea`; clicking it sets `*-open = false`.
  - A panel centred by Slint's default placement (`bg-surface`, radius 12, 1px `border-mid`), containing an empty `TouchArea {}` that swallows clicks.
  - Dialogs are instantiated with `if`, so their local state is destroyed on close.
  - No Esc key, no focus trap, and global shortcuts stay active behind the modal.
- **App states that gate the UI:**

| State | Trigger / effect |
|---|---|
| no files (`files-loaded=false`) | Files panel auto-opened at startup (main.rs 4008–4012); preview shows the empty state; Export, transport and the right-panel toggle are disabled |
| `has-both-videos` | Calibration block appears in the Files panel |
| `calibrating` | Step text shown in the toolbar; calibrate button disabled |
| `files-loaded` | Pipeline built: Stitching/ROI sections, preview image, transport and Export enabled |
| `export-in-progress` | Preview overlay; status bar becomes a progress bar plus Cancel |
| `recording` | Record button shows "Stop"; quality combo hidden |
| `lens-preview-active` | Camera and Stats sections hidden; ROI dots shown |
| `cal-dirty \|\| lens-dirty` | "Save Calibration" button shown |

---

## 2. Screens, panels and dialogs

Notation: ⇄ = two-way binding (`<=>`); → = one-way binding; **cb** = callback into Rust; UI = handled entirely in Slint.

### 2.1 Toolbar (857–913)
| Line | Kind | Label | Binding / action | Visible / enabled |
|---|---|---|---|---|
| 867 | Button | "◀ Files" / "Files ▶" | UI: toggles `files-panel-open` | always |
| 872 | Text (#8888ff) | → `calibration-step` | – | `if calibrating` |
| 879 | spacer | – | – | – |
| 881 | Button | "?" | UI: `shortcuts-dialog-open = true` | always |
| 882 | Button | "⚙" | cb `open-prefs-dialog()` | always |
| 884 | Button (primary) | "Export…" | cb `open-export-dialog()` | enabled `files-loaded && !export-in-progress` |
| 894–912 | custom Rect + TouchArea, 36×42 | "▶" / "◀" | UI: toggles `controls-open`; hover background `bg-hover` | enabled `files-loaded` (glyph turns `text-faint`); has no `x`, see §9 |

### 2.2 Left "Files" panel (920–1220)
Shown when `files-panel-open`. Width is `left-panel-width` (260px default). The drag handle at 926–940 enforces a 180px minimum and uses the `ew-resize` cursor. Content sits in a ScrollView with horizontal scrolling off, padding 10, spacing 8.

| Line | Kind | Label | Binding / action | Visible / enabled |
|---|---|---|---|---|
| 953 | Text (#8ae68a) | "Left" | – | always |
| 954 | Button | "+" | cb `pick-left-video()` (appends segments) | always |
| 955 | Button | "Clear" | cb `clear-left()` | enabled `left-path != ""` |
| 958 | Text | "No video selected" | – | `left-segments.length == 0` |
| 964 | SegmentList | "N. filename" rows | → `left-segments`; cb `remove-left-segment(i)`, cb `reorder-left-segment(from,to)` | always |
| 976–992 | same four controls for "Right" | | cb `pick-right-video()`, `clear-right()` (enabled `right-path != ""`), `remove-right-segment`, `reorder-right-segment`; → `right-segments` | same |
| 994/996 | divider + heading "Calibration" | | – | `has-both-videos \|\| calibration-path != ""` |
| 1000 | Button | "Calibrating..." / "Re-calibrate" (if files-loaded) / "Auto Calibrate" | cb `auto-calibrate()` | `if has-both-videos`; enabled `!calibrating` |
| 1008 | Button | "Load…" | cb `pick-calibration()` | `if has-both-videos` |
| 1017 | SectionHeader | "Advanced" (collapsed) | UI | `if has-both-videos` |
| 1028 | ComboBox ["2","4","6","8"] | "Frames:" | `current-value:` → `calibration-frames`; `selected` sets `calibration-frames = max(2, v)` | Advanced expanded; enabled `!calibrating` |
| 1036 | CheckBox | "IMU seeds" | ⇄ `use-imu-seeds` | Advanced expanded; enabled `!calibrating` |
| 1043 | LabeledSlider 0.00005–0.005, 4 decimals | "AKAZE threshold" | ⇄ `cal-akaze-threshold` (changed is a no-op) | Advanced expanded; not disabled while calibrating |
| 1052 | LabeledSlider 0–0.5 | "Detect Y min" | ⇄ `cal-detect-y-min` | Advanced expanded |
| 1061 | LabeledSlider 0.5–1.0 | "Detect Y max" | ⇄ `cal-detect-y-max` | Advanced expanded |
| 1070 | LabeledSlider 0–60 s, 0 decimals | "Skip end" | ⇄ `cal-skip-end-secs` | Advanced expanded |
| 1082–1107 | file chip: Text + "×" | → `calibration-path` (elided) | "×" → cb `clear-calibration()` (× hover #3a2020) | `calibration-path != ""` |
| 1110/1112 | divider + heading "Stitching" | | – | `files-loaded` |
| 1114 | LabeledSlider 0–0.3, 2 decimals | "Seam blend" | ⇄ `blend-width`; cb `changed-blend-width(v)` | `files-loaded` |
| 1123 | CheckBox | "Match colours" (uncommitted) | ⇄ `color-match`; cb `changed-color-match(checked)` | `files-loaded` |
| 1129 | LabeledSlider −30..30°, 1 decimal | "Rig tilt" | ⇄ `rig-tilt`; cb `changed-rig-tilt(v)` | `files-loaded` |
| 1139 | LabeledSlider −15..15° | "Rig roll" | ⇄ `rig-roll`; cb `changed-rig-roll(v)` | `files-loaded` |
| 1152 | LineEdit, 70px (`sync-input`) | "Sync offset" … "frames" | `text:` → `sync-offset`; on accepted sets `sync-offset` and cb `changed-sync-offset(int)` | `files-loaded` |
| 1158 | Button | "Apply" | same as accepted | `files-loaded` |
| 1161 | Button (primary) | "Save Calibration" | cb `save-calibration()` | `files-loaded && (cal-dirty \|\| lens-dirty)` |
| 1172 | Text | "Field ROI: loaded" (#8ae68a) / "Field ROI: none" | → `has-roi` | `files-loaded` |
| 1183 | Button | "Edit ROI..." / "Set ROI..." | cb `launch-roi-editor()` | `files-loaded` |
| 1188 | Button | "Paste ROI" | cb `paste-roi()` | `files-loaded` |
| 1194 | LineEdit | placeholder "Or paste ROI JSON here..." | on accepted: `roi-manual-json = text`, then cb `paste-roi()` | `files-loaded` |
| 1202 | Text (hint) | "Draw in browser, Copy ROI, then Paste or type JSON above." | – | `files-loaded && !has-roi` |
| 1211 | Button | "Recent…" | UI: `recent-dialog-open = true` | enabled when any `recent-*-paths` is non-empty |

### 2.3 Preview area (1222–1342)
- **Empty state (1230–1254), when `!files-loaded`:**
  - "Reco Video Stitcher" (20px, #555).
  - Three-step instructions (1242): "1. Open Files panel… 2. Auto Calibrate or load… 3. Preview, adjust, then Export".
  - `status-text`, if non-empty.
- **`preview-box` (1260–1312):**
  - Aspect comes from `preview-aspect`: "16:9", "4:3" or "21:9"; anything else fills the area.
  - The box is centred, and its size feeds `out preview-area-width/height` (477–478).
  - `Image` (1277), `if files-loaded`: `source: preview-frame`, `image-fit: contain`.
  - Export overlay (1286–1297), `if files-loaded && export-in-progress`: #000000bb with "Preview paused during export".
  - ROI vertex dots (1301–1311): `for idx in roi-points-x.length` draws 8px green circles (#0f08 fill, #0f0 border) at normalized point × preview-box size. `visible: lens-preview-active && has-roi`.
- **`drag` TouchArea (1314–1341), `enabled: files-loaded`:** pan by drag and zoom by wheel; see §6.

### 2.4 Right "Controls" panel (1344–1593)
Shown when `controls-open`; it is not gated by `files-loaded`. Width is `right-panel-width` (280px default). The handle at 1352–1366 enforces a 200px minimum. Content sits in a ScrollView with padding 12, spacing 10.

| Line | Kind | Label | Binding / action | Visible / enabled |
|---|---|---|---|---|
| 1377 | Text heading | "Camera" | – | `!lens-preview-active` |
| 1384 | LabeledSlider 20–150°, 0 decimals | "Field of view" | ⇄ `fov`; cb `changed-fov(v)` | `!lens-preview-active` |
| 1394 | CheckBox | "Constrained look" | ⇄ `use-constrained-look`; cb `changed-constrained-look()` | `!lens-preview-active` |
| 1400 | Button | "Reset View" | cb `reset-view()` | `!lens-preview-active` |
| 1408 | SectionHeader (collapsed) | "Lens - {lens-left-camera}" if `lens-info-available`, else "Lens" | UI | always |
| 1418–1430 | Text rows | "L" + "{lens-left-source} - {lens-left-camera}", "R" + same for right | → | expanded && `files-loaded` && `lens-info-available` |
| 1432 | Text | "Auto-calibrate to detect lens." | – | `!lens-info-available` |
| 1437 | Button | "Browse profiles..." | UI: `lens-picker-open = true` | Lens content |
| 1444 | CheckBox | "Lens correction" | `checked:` → `lens-correction-amount > 0.5`; toggled sets the amount to 1/0 and cb `changed-lens-correction(amount)` | Lens content |
| 1453 | CheckBox | "Lens preview (single camera)" | ⇄ `lens-preview-active`; cb `changed-lens-preview()` | Lens content |
| 1461–1462 | Buttons (primary = selected) | "Left" / "Right" | set `lens-preview-side`, then cb `changed-lens-preview()` | `lens-preview-active` |
| 1466 | SectionHeader | "Fine-tune" | `expanded:` → `lens-preview-active` (initial binding) | Lens content |
| 1476–1478 | Buttons (primary = selected) | "Left" / "Right" / "Both" | UI: set `lens-selected-camera` | Fine-tune expanded |
| 1483–1490 | 8× LabeledSlider | fx, fy, cx, cy (ranges `lens-fx-min..max` etc., 0 decimals); k1–k4 (±`lens-k-range`, 2 decimals) | ⇄ `lens-left-*`; each fires cb `changed-lens-param()` | selected "left" or "both" |
| 1495–1502 | 8× LabeledSlider | same eight | ⇄ `lens-right-*`; cb `changed-lens-param()` | selected "right" |
| 1505 | Button | "Reset Lens" | cb `reset-lens()` | enabled `lens-dirty` |
| 1512 | SectionHeader (collapsed) | "Calibration" | UI | always |
| 1520 | LabeledSlider −1..1, 3 decimals | "Intersect" | ⇄ `cal-intersect`; cb `changed-cal-intersect(v)` | Calibration expanded |
| 1529 | LabeledSlider −0.6..0.6 | "Camera axis offset" | ⇄ `cal-camera-axis-offset`; cb `changed-cal-camera-axis-offset(v)` | Calibration expanded |
| 1538 | LabeledSlider −0.1..0.1 | "x_ty" | ⇄ `cal-x-ty`; cb `changed-cal-x-ty(v)` | Calibration expanded |
| 1552 | Button | "Reset" | cb `reset-calibration()` | enabled `cal-dirty` |
| 1565 | SectionHeader (collapsed) | "Stats" | UI | `!lens-preview-active` |
| 1573–1588 | read-only Texts | see below | → `telem-*` | Stats expanded |

Stats texts (1573–1588):
- "FPS: avg / recent"; "Frame: total / p99 ms"; Decode, Stitch, Readback and Submit in ms.
- "Bottleneck: …" (#e8a060) if non-empty; GPU name if non-empty; "Dropped: N" (#f88) if > 0.
- "Ball %, Tracks, Det/f"; "Detection: … ms" if > 0.
- "Cal: …% conf, N matches, reproj …" if `telem-cal-matches > 0`.

### 2.5 Transport bar (1597–1697)
| Line | Kind | Label | Binding / action | Enabled / visible |
|---|---|---|---|---|
| 1610 | Button | "\|<" | cb `step-backward()` | `files-loaded` |
| 1611 | Button | "\|\|" / "▶" (→ `playing`) | cb `toggle-playback()` | `files-loaded` |
| 1612 | Button | ">\|" | cb `step-forward()` | `files-loaded` |
| 1614 | Text, 44px, right-aligned | → `current-time-text` | – | always |
| 1628–1639 | Rect (#4a7a4a33) | export-range tint behind the slider | position/size from `export-start-secs`, `export-end-secs`, `clip-duration-secs` | `export-start-secs > 0 \|\| export-end-secs < clip-duration-secs` |
| 1641 | Slider 0..`total-frames` | timeline | `value:` → `1.0 * current-frame`; on released, cb `seek(val / total-frames)` | `files-loaded` |
| 1653 | Text | → `total-time-text` | – | always |
| 1662–1680 | custom record button (Rect + TouchArea + dot) | "Rec*" / "Stop" | cb `toggle-recording(recording-codec, recording-quality)`; colours #2a1a1a, hover #3a2020, active #4a1010 / #e04040 | TouchArea enabled `files-loaded && !export-in-progress` (no visual disabled state) |
| 1682 | ComboBox [fast, balanced, high], 82px | – | ⇄ `recording-quality` | `!recording`; enabled `files-loaded && !export-in-progress` |
| 1689 | ComboBox [auto, 16:9, 4:3, 21:9], 78px | – | ⇄ `preview-aspect`; cb `changed-preview-aspect(v)` | enabled `files-loaded` |

### 2.6 Status bar (1699–1775)
| Line | Kind | Content / action | Visible |
|---|---|---|---|
| 1709 | Text (word-wrap, #666) | → `status-text`; this is also the primary error channel | `!export-in-progress` |
| 1721 | Text (#8f8, 600 weight) | "Exporting: " + `export-status-text` | `export-in-progress` |
| 1729 | ProgressIndicator | → `export-progress` | `export-in-progress` |
| 1735 | Button | "Cancel" → cb `cancel-export()` | `export-in-progress` |
| 1740 | Button | "Show in folder" → cb `show-in-folder(last-output-path)` | `!export-in-progress && last-output-path != ""` |
| 1745 | Text | "{avg} / {recent} fps" from `telem-fps-*` | `!export-in-progress && telem-fps-avg > 0` |
| 1752 | Text | → `version` | `version != ""` |
| 1759–1773 | custom link (Rect + TouchArea) | "Report Bug" → UI `bug-dialog-open = true` (hover #333) | always |

### 2.7 Recent files modal (1778–1961), `recent-dialog-open`, fixed 720×460
- Title "Recent files".
- Button "Clear all" (1812): cb `clear-recent-files()`, then closes.
- Three columns, each a ScrollView of 28px rows (hover #2d2d2d, full path elided). Clicking a row calls the column's callback and closes:
  - "Left videos": `for entry in recent-left-paths` → cb `load-recent-left(entry)` (1844–1863)
  - "Right videos": → cb `load-recent-right` (1885–1904)
  - "Calibrations": → cb `load-recent-calibration` (1926–1945)
- Button "Close" (1954). The backdrop also closes.

### 2.8 Keyboard shortcuts modal (1963–2036), `shortcuts-dialog-open`, fixed 420×360
- Title "Keyboard shortcuts".
- Static rows from an inline struct array (1994–2002): Space, Arrows, "+ / -", Scroll, Drag, "[ / ]", R.
- Buttons: "Website" → cb `open-website()`; "Forum" → cb `open-forum()`; "Close" (UI).

### 2.9 Preferences modal (2038–2198), `prefs-dialog-open`, 480 × min(h−60, 680), scrollable
`open-prefs-dialog` seeds the values from settings and then opens the dialog.

| Line | Kind | Label | Binding |
|---|---|---|---|
| 2075 | ComboBox (model `available-codecs`) | "Default codec" | ⇄ `prefs-default-codec` |
| 2081 | ComboBox [fast, balanced, high] | "Default quality" | ⇄ `prefs-default-quality` |
| 2086/2091 | Text + Slider 0–0.3 | "Default seam blend (x.xx)" | ⇄ `prefs-default-blend-width` |
| 2100/2105 | LineEdit + Button | "AI model (ONNX)", placeholder "Path to .onnx, or leave empty"; "Browse…" | ⇄ `prefs-ai-model-path`; cb `pick-prefs-model()` |
| 2121 | ComboBox (`available-codecs`) | "Recording defaults" → "Codec" | ⇄ `recording-codec` (live property, not scratch) |
| 2130 | ComboBox [fast, balanced, high] | "Quality" | ⇄ `recording-quality` (live) |
| 2140/2145 | LineEdit + Button | "Recording folder", placeholder "Same as source video (default)"; "Browse…" | ⇄ `recording-folder` (live); cb `pick-recording-folder()` |
| 2155 | CheckBox | "Appearance" → "Dark mode" | ⇄ `prefs-dark-mode` |
| 2164 | CheckBox | "Privacy" → "Send anonymous usage data (opt-in)", plus hint text | ⇄ `prefs-telemetry-enabled` |
| 2182 | Button | "Cancel" | UI close; does not revert the live recording-* properties |
| 2186 | Button (primary) | "Save" | cb `save-prefs()`, then close |

### 2.10 Report-a-bug modal (2200–2265), `bug-dialog-open`, 500 × min(h−60, 420)
- Title "Report a Bug" plus an explanation text.
- `bug-msg` LineEdit (local, single-line): "What went wrong?", placeholder "e.g. The video freezes when I click Export".
- `bug-contact` LineEdit ⇄ `bug-user-contact`: "How can we reach you? (optional)", placeholder "Forum username or email".
- `bug-share-logs` CheckBox "Include system info and logs" (local, `checked: true`), plus hint text.
- Buttons: "Cancel"; "Send Report" (primary, enabled `bug-msg.text != ""`) → cb `submit-bug-report(msg, contact, share_logs)`, then close.

### 2.11 Export modal (2269–2744), `export-dialog-open`, 560 × min(h−60, 720), ScrollView → VerticalBox (padding 24, spacing 14)
Header: "Export Stitched Video" plus a subtitle (2299–2311).

| Line | Kind | Label | Binding / action | Visible / enabled |
|---|---|---|---|---|
| 2323 | LineEdit | "Output file", placeholder "Pick a destination file, or type a path…" | ⇄ `export-output-path` | always |
| 2328 | Button | "Save to…" | cb `pick-export-output()` | always |
| 2343 | ComboBox ["1080p (1920x1080)", "720p (1280x720)", "2K (2560x1440)", "4K (3840x2160)"] | "Resolution" | `current-index` derived from `export-width`; `selected` sets `export-width/height` (UI) | always |
| 2369 | Text | "{w} x {h}" | → | always |
| 2381 | ComboBox (model `available-codecs`) | "Codec" | `current-index:` hevc→1, av1→2, else 0; `selected` sets `export-codec` | always |
| 2394 | ComboBox [fast, balanced, high] | "Quality" | `current-index` mapping; `selected` sets `export-quality` | always |
| 2415 | Slider 0..max(1, clip) | "Processing range" → "Start (s)" | ⇄ `export-start-secs`; `changed` clamps to ≤ end | enabled `clip-duration-secs > 0` |
| 2424 | LineEdit, placeholder "0:00" | Start | `text:` → (start==0 ? "" : start); `edited` sets start = min(max(0, v), end) | always |
| 2437 | Slider | "End (s)" | ⇄ `export-end-secs`; `changed` clamps to ≥ start | enabled `clip > 0` |
| 2446 | LineEdit | End | `text:` → end; `edited` sets end = max(min(v, clip), start) | always |
| 2456 | Text | "Duration: X.Xs" or "Duration: 0s (nothing to export)" (red) | derived | always |
| 2465 | CheckBox | "Record replay (stacked raw input for re-stitching)" | ⇄ `export-replay-enabled` | always |
| 2470 | CheckBox | "Save AI debug events (.jsonl for visualize_detections.py)" | ⇄ `export-events-enabled` | always |
| 2489 | CheckBox | "AI Tracking" → "Enable" | ⇄ `export-autocam-enabled` | enabled `ai-available` |
| 2496 | Text | → `ai-status` (#8f8 if available, else #f88) | – | always |
| 2509/2515 | LineEdit (read-only) + Button | placeholder "Pick a YOLO ONNX model…"; "Model…" | → `export-model-path`; cb `pick-export-model()` | `export-autocam-enabled` |
| 2524 | Text warning (#e0a020) | "⚠ Pick a model - AI tracking needs one, and export is blocked until you do." | – | autocam on and `export-model-path == ""` |
| 2539 | ComboBox [field, ball, sweep] | "Tracking mode" | index mapping; `selected` sets `export-tracking-mode` | autocam on |
| 2552 | ComboBox ["1","3","5","10","15","30"] | "Detect every N frames" | index mapping; `selected` sets `export-detection-interval` | autocam on |
| 2573 | ComboBox [broadcast, action, frame_all] | "Style preset" | `selected` sets `export-panner-preset`, then cb `apply-panner-preset(v)` | autocam on |
| 2594 | ComboBox [action, frame_all] | "Framing" | `selected` sets `export-framing` | autocam on |
| 2605 | CheckBox | "Pitch" → "Lock (horizontal-only)" | ⇄ `export-lock-pitch` | autocam on |
| 2616 | VramRiskSlider 0–2.5 s | "Lookahead (smoothness)" | ⇄ `export-lookahead-secs`; zones from `lookahead-green-max`, `lookahead-red-min`, `lookahead-risk-active` | autocam on |
| 2627 | SectionHeader (collapsed) | "Advanced panner" | UI | autocam on |
| 2643 | ComboBox [density, trimmed_mean] | "Cluster mode" | `selected` sets `export-cluster-mode` | Advanced panner expanded |
| 2653 | LabeledSlider 0–1 | "Ball weight" | ⇄ `export-ball-weight` | Advanced panner expanded |
| 2663 | LabeledSlider 0.1–0.6 rad | "Cluster bandwidth" | ⇄ `export-cluster-bandwidth` | Advanced panner expanded |
| 2671 | LabeledSlider 0–0.5 rad | "Dead-zone" | ⇄ `export-dead-zone` | Advanced panner expanded |
| 2684/2691/2698 | LabeledSlider 10–40 / 20–60 / 40–90 | "Field of view (degrees)": "Tight" / "Default" / "Wide" | ⇄ `export-fov-tight` / `-default` / `-wide` | Advanced panner expanded |
| 2717 | Text (#e06060) | → `export-error-text` | – | non-empty |
| 2725 | Button | "Cancel" | UI: close and clear `export-error-text` | always |
| 2730 | Button (primary) | "Start Export" | cb `start-export()`; Rust closes the dialog on success | enabled `export-output-path != "" && !(autocam && model == "")` |

### 2.12 Lens profile picker modal (2745–2851), `lens-picker-open`, 520 × min(h−80, 540)
- Title "Browse Lens Profiles".
- "Apply to:" ComboBox [Both, Left, Right] (2774): `current-value:` derived from `lens-pick-side`; `selected` sets `lens-pick-side` to left/right/both.
- LineEdit (2784) ⇄ `lens-search-query`, placeholder "Search by camera, lens, resolution...", `edited` → cb `lens-search-changed(text)`.
- ScrollView list (2790–2835):
  - Empty states: "No profiles match your search." and "Type to search across 4200+ camera profiles."
  - Rows (32px, hover #333 / #282828), `for result[idx] in lens-search-results`; click → cb `lens-pick(idx, lens-pick-side)`, then close.
- Buttons: "Load from file..." → cb `lens-pick-file()`; "Close".

### 2.13 Toast overlay (2854–2861)
- `ToastStack { toasts: root.toasts; right-inset: controls-open ? right-panel-width : 0 }`.
- Dismissing a card → cb `toast-dismissed(id)`.

### 2.14 External screen: browser ROI editor
- Template `resources/roi_editor.html` (202 lines), embedded with `include_str!` at main.rs:2058.
- Placeholders: `{{LEFT_IMAGE_DATA}}`, `{{RIGHT_IMAGE_DATA}}` (base64 PNG), `{{CAL_JSON}}`, `{{CAL_PATH}}`.
- Controls: side selector, click to add a point, right-click to undo, Undo, Clear, and "Copy ROI" (writes the FieldRoi JSON to the clipboard).
- The user then returns to the app and uses "Paste ROI" or the JSON field. There is no in-app ROI drawing.

---

## 3. Custom components (main.slint)

| Component | Lines | What it draws / does | Notable styling |
|---|---|---|---|
| `LabeledSlider` (inherits VerticalLayout) | 6–49 | Label + formatted value (`decimals` 0–5 via a scale lookup, plus `suffix`) above a std `Slider` (`value <=> root.value`); forwards `changed(float)` | Label #bbb, value #888, 11px; 33 usages |
| `VramRiskSlider` | 51–166 | Fully custom slider: 16px track (#2b2b2b, radius 8, clipped) with three risk zones when `risk-active`: green #2e7d32 up to `green-max/maximum`, amber #b8932a up to `red-min`, red #b13b2e beyond. 14px white handle with #222 border. TouchArea (pointer cursor) sets the value from mouse-x on down/move (`apply-x`). Status word "safe" (#5cd65c) / "tight" (#e6c14d) / "may fail" (#e35c5c); footnote "fits ≈ Xs here (VRAM)" | No keyboard, focus or a11y. Zones assume `minimum == 0`. `changed` is declared but unused by the one consumer (2616) |
| `SectionHeader` (inherits Rectangle) | 168–203 | 30px collapsible header with an in-out `expanded` that its own TouchArea toggles; chevron "▾"/"▸" plus title (13px, 600, #eee) | Hover background #2e2e2e vs #1e1e1e, pointer cursor. Consumers gate content with `if x.expanded`. 6 instances: cal-advanced, lens-section, lens-tune, cal-section, stats-section, panner-advanced |
| `export struct Toast` | 205–214 | `{id:int, severity:string ("info"/"warn"/"error"), title, body}`; Rust uses the generated struct (`crate::Toast`) | – |
| `ToastCard` | 216–290 | 360px card: title (12px, 600, wrap), optional body (11px #bbb), 20px round "×" dismiss → `dismissed(id)` | Background #1f1f1f, 1px #333 border, radius 6, drop shadow (blur 6, #000000aa). 4px left accent bar: error #e06060, warn #e0c060, info #6080c0. Dismiss hover #3a3a3a |
| `ToastStack` | 292–314 | Full-size overlay; VerticalLayout positioned explicitly bottom-right (x = width − preferred − 16 − `right-inset`, y = height − preferred − 16), spacing 8, one `ToastCard` per toast | No enter/exit animations |
| `SegmentList` | 316–436 | Fixed-height rows (26px + 4px gap). Each row: grip "⠿" (TouchArea, `grab` cursor, enabled when count > 1), "N. name" (10px #ccc, elided), "×" remove (hover #3a2020) → `remove(idx)`. Drag state is `drag-from` / `drag-to` (insertion gap); the dragged row dims to opacity 0.4; a 2px #8ae68a insertion line with a 6px dot is drawn in the gap (420–435). On pointer-up it emits `reorder(from, to)`; cancel resets | Grip colour #8ae68a on hover/drag, else #5a5a5a |

- No `animate`, `states`, `Path`, `PopupWindow` or image assets anywhere.
- All icons are Unicode glyphs: ◀ ▶ ▾ ▸ ⚙ ⠿ × ⚠ |< || >| ▶. Rendering depends on font fallback.
- Hover states exist on: the six components above, the panel toggle, both resize handles, the calibration-chip ×, the record button, Report Bug, recent rows and lens rows.

---

## 4. Theme and styling

- **Style:** `build.rs:2` compiles with `with_style("fluent-dark")`. The comments at main.slint:2 and main.rs:5 still say "Material dark" (stale).
- **Std widgets imported (4):** Button, VerticalBox, HorizontalBox, Slider, ProgressIndicator, LineEdit, ComboBox, CheckBox, ScrollView, TabWidget (unused), Palette, StyleMetrics (unused).
- **Std widget instance counts:** 46 Button, 12 CheckBox, 16 ComboBox, 11 LineEdit, 5 raw Slider, 8 ScrollView, 1 ProgressIndicator.
- **Dark/light switching:**
  - `in-out dark-mode` (446) drives 25 private colour tokens (448–472). Rust seeds it from settings (main.rs:1447) and re-applies it live on save-prefs (2433).
  - `Palette.color-scheme` is set only once, in FocusScope `init` (823), so std widgets do not follow a runtime toggle. They are likely locked dark under fluent-dark anyway.

**Colour tokens (dark / light):**

| Group | Token: dark / light |
|---|---|
| Backgrounds | bg-base #121212/#f5f5f5 · bg-panel #1a1a1a/#eaeaea · bg-bar #1e1e1e/#e0e0e0 · bg-surface #222/#fff · bg-card #242424/#ddd · bg-hover #2e2e2e/#c8c8c8 · bg-preview #0a0a0a/#444 · bg-scrim #000000aa (both) |
| Borders | border-subtle #333/#ccc · border-mid #444/#aaa |
| Text | text-primary #eee/#111 · text-body #ccc/#222 · text-secondary #aaa/#333 · text-muted #888/#444 · text-dim #666/#555 · text-faint #555/#666 · text-heading #fff/#111 |
| Accents | accent #8ae68a/#2a7a2a · accent-info #8f8/#2a8a2a · accent-warn #e8a060/#c07020 · accent-error #f88/#c44 |
| Record button | rec-bg #2a1a1a/#f0e0e0 · rec-border #553333/#c08080 · rec-active-bg #4a1010/#f0c0c0 · rec-active-border #e04040/#c03030 |

- **Token usage gaps:**
  - Nine tokens are never used: text-primary, accent, accent-info, accent-warn, accent-error, and all four rec-* tokens.
  - About 116 hard-coded hex literals in the `RecoApp` body bypass the tokens. The most common are #ccc ×27, #aaa ×13, #888 ×10, #666 ×9, #fff ×5, #8ae68a ×5. All component internals are hard-coded too. Light mode is therefore inconsistent.
- **Brand accent:** green #8ae68a (section headings, grip, ROI status, insertion line); shortcut keys #a0e8a0; calibrating text #8888ff.
- **Fonts:** no `font-family` or `default-font`, so the platform default is used. Sizes: 12px ×40, 11px ×35, 10px ×21, 13px ×6, 16px ×6 (dialog titles), 14px ×2, 18px ×2 (Export/Bug titles), 20px ×1 (empty state). Headings use `font-weight: 600` (27 uses).
- **Spacing conventions:**
  - Spacing: mostly 4px (×30), then 8/6/10px.
  - Padding: Files panel 10, Controls panel 12, dialogs 20 (24 for Export and Bug).
  - Radii: 4 for rows/cards, 6 for toasts/columns, 12 for dialogs.
  - Heights: toolbar 42px, transport 44px, status ≥28px, rows 26/28/32px.
  - Cursors: `pointer` ×3, `ew-resize` ×2, `grab` ×1.

---

## 5. Rust ↔ UI bridge

**Wiring pattern:**
- `slint::include_modules!()` at main.rs:53.
- `AppState` lives in `Rc<RefCell<…>>` (1400). Every handler captures `Rc::clone(&state)` plus `app.as_weak()`.
- All 170 public properties are `in-out` except two `out`; there are no `in` properties.
- 61 callbacks and 1 public function (`refocus()` at 812, never invoked from Rust). Automation calls `app.invoke_start_export()` (3816, 4190).

Direction key used below:

| Mark | Meaning |
|---|---|
| R→ | Rust writes the property |
| ←R | Rust reads the property |
| UI | Never touched by Rust |
| set-only | Written by Rust but never displayed |
| DEAD | Declared but unused |

### 5.1 Properties by screen

**Window / global**
- `out preview-area-width`, `out preview-area-height` (477–478): ←R 4037–4038.
- `dark-mode` (446): R→ 1447, 2433.
- `version` (580): R→ 1443.
- `status-text` (488): R→ about 25 sites, plus toast.rs:183.
- `ai-status`, `ai-available` (577–578): R→ 1422–1423.
- `available-codecs [string]` (533): R→ 1508.
- `toasts [Toast]` (630): R→ toast.rs:174.
- `files-loaded` (484): R→ many sites; ←R 4010.
- `files-panel-open` (800): R→ 4011.
- UI-only: `controls-open` (525), `left-panel-width` (802, 260px), `right-panel-width` (803, 280px).
- DEAD: `progress` (489), `stats-panel-open` (634).

**Toolbar**
- `calibrating` (495): R→ 2527, 4479.
- `calibration-step` (496): R→ 2528, 2608.
- UI-only: `shortcuts-dialog-open` (620).

**Files panel**
- Display labels: `left-path`, `right-path`, `calibration-path` (485–487): R→.
- `has-both-videos` (497): UI-only, a derived binding over the labels.
- `left-segments`, `right-segments [string]` (789–790): R→ 1202–1207.
- Calibration parameters, ←R 2516–2521: `calibration-frames` (601), `use-imu-seeds` (600), `cal-akaze-threshold` (602), `cal-detect-y-min/max` (603–604), `cal-skip-end-secs` (605).
- `blend-width` (509): R→ 4356, 4623; ←R 3469.
- `color-match` (511): ←R 3470 only.
- `rig-tilt`, `rig-roll` (512–513, degrees): R→ 4350/4353, 4617/4620.
- `sync-offset` (514): R→ 4362, 4629.
- `cal-dirty` (522), `lens-dirty` (685): R→.
- `has-roi` (498): R→ 2169, 3943, 4316, 4593.
- `roi-manual-json` (503): ←R 2118, cleared at 2119.
- UI-only: `recent-dialog-open` (614).

**Preview**
- `preview-frame image` (482): R→ 2664, 2696, 2820, 3982, 4071, 4341, 4608.
- `preview-aspect` (785): R→ 1523.
- `roi-points-x`, `roi-points-y [float]` (499–500): R→ 1225–1226.
- set-only: `yaw` (506) R→ 4087, `pitch` (507) R→ 4088.

**Controls panel**
- `fov` (508): R→ 4090, 4364, 4631.
- `use-constrained-look` (611): ←R 3245.
- Lens profile labels `lens-left-camera/source`, `lens-right-camera/source` (587–592): R→ 1119–1134, 3146–3151, 3206–3209.
- set-only: `lens-left-lens`, `lens-right-lens`, `lens-candidates-count` (593).
- `lens-info-available` (594): R→.
- `lens-left-fx…k4`, `lens-right-fx…k4` (659–674): R→ 1075–1091, 2972–2979; ←R 2950–3011.
- Slider ranges `lens-fx-min…lens-cy-max`, `lens-k-range` (676–684): R→ 1065–1073.
- `lens-selected-camera` (687): ←R 2932.
- `lens-preview-active`, `lens-preview-side` (736–737): ←R 3266–3267.
- `lens-correction-amount` (738): R→ 3288, 4359, 4626.
- `cal-intersect`, `cal-camera-axis-offset`, `cal-x-ty` (519–521): R→ 2911–2913, 4344–4346, 4611–4613.
- Telemetry:
  - R→ from export.rs 81–99 only: `telem-fps-avg/recent, decode/stitch/readback/submit/total/p99/detection-ms, active-tracks, ball-pct, det-per-frame, gpu-name, bottleneck` (635–649).
  - Never written: `telem-frames-dropped` (647), `telem-cal-confidence`, `telem-cal-reproj-err`, `telem-cal-matches` (650–652). The bug report reads these at 387–405.

**Transport bar**
- `current-frame`, `total-frames`, `current-time-text`, `total-time-text` (490–493): R→ 1161–1164.
- `playing` (483): R→ 2646, 2674, 2705, 3524, 4078; ←R 4001.
- `recording` (777): R→ 3583, 3597.
- `recording-quality`, `recording-codec` (778–779): R→ 1513–1514, 2392–2393; ←R 2423–2424.
- set-only: `fps` (494).

**Status bar**
- `last-output-path` (581): R→ 3584, 3773.
- UI-only: `bug-dialog-open` (772).

**Recent files dialog**
- `recent-left-paths`, `recent-right-paths`, `recent-calibration-paths [string]` (615–617): R→ 1183–1185.

**Preferences dialog**
- `prefs-dialog-open` (623): R→ 2404.
- `prefs-default-codec`, `prefs-default-quality`, `prefs-default-blend-width`, `prefs-ai-model-path` (624–627): R→ 2381–2384 and 2472; ←R 2414–2417.
- `prefs-telemetry-enabled`, `prefs-dark-mode` (761–762): R→ 2402–2403; ←R 2431, 2435.
- `recording-folder` (780): R→ 1515, 2394, 2482; ←R 2425.

**Bug dialog**
- `bug-user-contact` (773): UI-only, never persisted.

**Export dialog**
- `export-dialog-open` (528): R→ 3339, 3537.
- `export-output-path` (529): R→ 3356, 4388; ←R 3415.
- `export-width`, `export-height` (530–531): ←R 3465–3466.
- `export-codec` (532): R→ 3325; ←R 3467, 3777, 3880.
- `export-quality` (534): R→ 3326; ←R 3468.
- `export-start-secs`, `export-end-secs` (536–537): R→ 4334/4335, 3337; ←R 3471–3472, 3336.
- `clip-duration-secs` (538): R→ 3335, 4333.
- `export-model-path` (539): R→ 3328, 3370; ←R 3476.
- `export-tracking-mode` (540), `export-detection-interval` (541), `export-autocam-enabled` (542), `export-panner-preset` (546): ←R 3475–3480.
- `export-lookahead-secs` (547): R→ 4306; ←R 3479, 4304.
- `lookahead-green-max`, `lookahead-red-min`, `lookahead-risk-active` (551–553): R→ 4288–4290, 4314.
- `export-framing`, `export-lock-pitch`, `export-cluster-mode`, `export-cluster-bandwidth`, `export-dead-zone`, `export-ball-weight`, `export-fov-tight/wide/default` (554–562): R→ 3395–3403 (preset); ←R 3481–3489.
- `export-replay-enabled`, `export-events-enabled` (563–564): ←R 3491–3492.
- `export-error-text` (572): R→ 3430, 3531.
- DEAD: `export-blend-width` (535).

**Export progress**
- `export-in-progress` (567): R→ 3532, 3764.
- `export-progress` (568): R→ 3533, 3765, export.rs:224.
- `export-frames-done` (569): set-only; R→ 3534, export.rs:221.
- `export-frames-total` (570): R→ 3535, export.rs:176; ←R export.rs:222.
- `export-status-text` (571): R→ 3536, 3768, 3834, 3851, export.rs:148/234/266/363.

**Lens picker**
- `lens-picker-open` (730): R→ 3204.
- `lens-search-query` (731): ←R 3093.
- `lens-search-results [string]` (732): R→ 3083.
- `lens-pick-side` (733): UI-only; passed as a callback argument.

### 5.2 Callbacks: declaration line → Rust handler (main.rs) → what it does

| Callback (decl line) | Handler | Summary |
|---|---|---|
| `pick-left-video()` (691) | 1613 | rfd `pick_files` (mp4/mov/avi/mkv), sorted. Appends to the existing input (Single→Chained). Unloads the pipeline if the first path changed. Sets the label "name (N segments)", pushes to MRU and saves, then `try_init_and_update` (4215) |
| `pick-right-video()` (692) | 1686 | Mirror of the left handler |
| `pick-calibration()` (693) | 1759 | rfd JSON picker; unloads if changed; sets label; MRU; `try_init_and_update` |
| `auto-calibrate()` (694) | 2497 | Reads cal parameters and the current playback time (used as `skip_start_secs`). Preserves the current lens params. Sets `calibrating` / step / status. Spawns the calibration thread (2569); the result arrives over `cal_rx` |
| `toggle-playback()` (695) | 2638 | Ignored while exporting; `Playback::toggle`; sets `playing` |
| `step-forward()` (696) | 2652 | Pauses, `step_forward`, renders synchronously, `sync_frame_display`, `playing = false` |
| `step-backward()` (697) | 2680 | Pauses, seeks to `frame_index − 2` (seek decodes one frame), renders, syncs |
| `seek(float)` (698) | 2710 | Drops echoes (< 2 frames apart) and stores `pending_seek`; the timer commits it after 120ms (3972–3992) |
| `seek-relative(float)` (700) | 2812 | `AppState::seek_relative` (1029) by ±seconds; synchronous render and sync |
| `pan(float,float)` (702) | 2748 | `apply_pan` (856): `PoseControl::apply_drag`, coverage clamp, marks dirty |
| `zoom(float)` (704) | 2753 | `apply_zoom` (867): `PoseIntent::DeltaFovDeg`, clamp, dirty |
| `reset-view()` (706) | 2758 | `reset_view` (1006): `PoseIntent::Reset` |
| `changed-blend-width(float)` (708) | 2769 | `set_blend_width` (916): renderer, clamped 0..0.5; `cal-dirty = true` |
| `changed-color-match(bool)` (709) | 2763 | `set_color_match` (924): renderer colour match (uncommitted feature) |
| `changed-rig-tilt(float)` (710) | 2780 | `set_rig_tilt` (932): calibration (radians) + renderer; clamp; `cal-dirty` |
| `changed-rig-roll(float)` (711) | 2789 | `set_rig_roll` (992); `cal-dirty` |
| `changed-sync-offset(int)` (712) | 2798 | `set_sync_offset` (943): validates against total frames, reopens playback (position resets); `cal-dirty` is set even if rejected |
| `changed-fov(float)` (713) | 2806 | `set_fov` (875): `PoseIntent::SetFovDeg`, clamp |
| `changed-cal-intersect(float)` (715) | 2835 | `layout.intersect` → `apply_layout` (580) → `renderer.update_layout`; `cal-dirty` |
| `changed-cal-camera-axis-offset(float)` (716) | 2849 | Same, for `camera_axis_offset` |
| `changed-cal-x-ty(float)` (717) | 2863 | Same, for `x_ty` |
| `save-calibration()` (718) | 2877 | `AppState::save_calibration` (592): folds in renderer lens params, blend width and lens correction, overwrites `calibration_path`, clears both dirty flags, toast and status |
| `reset-calibration()` (719) | 2907 | Restores `cal_baseline_layout`, reseeds the 3 sliders, `cal-dirty = false` |
| `changed-lens-param()` (723) | 2927 | Builds `CameraParams` from `lens-left-*` / `lens-right-*` according to `lens-selected-camera` ("both" mirrors left into the right properties). Writes the calibration and calls `renderer.update_camera_params`; `lens-dirty = true` |
| `reset-lens()` (724) | 3041 | Restores baseline params → `set_lens_sliders` (1049) and renderer; `lens-dirty = false` |
| `lens-search-changed(string)` (727) | 3068 | `LensDatabase::embedded().search(q, in_w, in_h)` → `lens-search-results` rows "camera - lens - WxH" |
| `lens-pick(int,string)` (728) | 3089 | Re-runs the search to resolve the index, `load_by_summary`, scales to the input resolution, applies to the chosen side(s) in calibration and renderer. Labels source "Picker", reseeds sliders, sets `lens-dirty` and `lens-info-available` |
| `lens-pick-file()` (729) | 3163 | rfd JSON → `lens_database::load_from_file`, scales, applies to both sides, closes the picker, labels "File", toast |
| `changed-lens-preview()` (739) | 3261 | Copies active/side into AppState, `sync_roi_points` (1210), dirty |
| `changed-lens-correction(float)` (740) | 3274 | Snaps to 0/1; `pipeline.set_lens_correction_amount` (only when not in lens preview); sets the property and `cal-dirty` |
| `changed-constrained-look()` (746) | 3241 | Reads `use-constrained-look` into AppState; re-clamps if enabled; dirty |
| `load-recent-left(string)` (749) | 1794 | Single input from the path; unload if changed; MRU; `try_init_and_update` |
| `load-recent-right(string)` (750) | 1823 | Mirror |
| `load-recent-calibration(string)` (751) | 1849 | Same flow for calibration |
| `clear-recent-files()` (752) | 1874 | Clears the 3 MRU lists, saves, syncs |
| `open-prefs-dialog()` (756) | 2376 | Seeds `prefs-*`, `recording-*`, telemetry and dark mode from settings; opens the dialog |
| `save-prefs()` (757) | 2409 | Writes properties to settings; applies `dark-mode`; starts or drops the TelemetryClient; `save()` |
| `pick-prefs-model()` (758) | 2465 | rfd ONNX → `prefs-ai-model-path` |
| `open-website()` (759) | 2456 | `open::that("https://github.com/reco-project/video-stitcher")` |
| `open-forum()` (760) | 2460 | `open::that("https://forum.reco-project.org/")` |
| `open-export-dialog()` (764) | 3319 | Seeds codec, quality and model from saved defaults; sets `clip-duration-secs` and `export-end-secs` (if 0); opens |
| `pick-export-output()` (765) | 3344 | rfd `save_file` (MP4/MOV/MKV filters); appends `.mp4` if there is no extension |
| `pick-export-model()` (766) | 3363 | rfd ONNX → `export-model-path`; persists `ai_model_path` |
| `start-export()` (767) | 3411 | Validates the output dir (sets `export-error-text`). Snapshots every export property into `AutocamUiConfig`; persists codec/quality/blend defaults. Pauses playback and releases the preview pipeline (`reset_pipeline`, frees VRAM). Sets progress properties, closes the dialog, spawns the export thread (3541) |
| `apply-panner-preset(string)` (769) | 3380 | `FieldPannerConfig::from_preset_name` → writes 9 panner properties (`autocam` feature only) |
| `cancel-export()` (770) | 3569 | `export_interrupted.store(true)` |
| `submit-bug-report(string,string,bool)` (771) | 3628 | Builds the report (+ `build_bug_report` at 355 when logs are included). Sends it via TelemetryClient, creating one even without opt-in. Copies it to the clipboard; toast |
| `show-in-folder(string)` (774) | 3618 | `open::that(parent dir)` |
| `toggle-recording(string,string)` (775) | 3577 | `start_recording` (691) or `stop_recording` (753); sets `recording` and `last-output-path`; toasts (8s TTL on save) |
| `pick-recording-folder()` (776) | 2477 | rfd `pick_folder` → `recording-folder` |
| `changed-preview-aspect(string)` (786) | 2487 | Persists `preview_aspect` |
| `remove-left-segment(int)` (791) | 2182 | Removes from the InputPath (collapses to Single/None); unloads the pipeline (no re-init); updates label and segments; `files-loaded = false` |
| `remove-right-segment(int)` (792) | 2235 | Mirror |
| `reorder-left-segment(int,int)` (793) | 2292 | Moves within the Chained list; updates label and segments; `Timer::single_shot(40ms)` → `reopen_source` (969) |
| `reorder-right-segment(int,int)` (794) | 2332 | Mirror |
| `clear-left()` (795) | 1889 | Unloads the pipeline, clears the left input; `files-loaded = false`; status "Left video cleared. Calibration preserved." |
| `clear-right()` (796) | 1906 | Mirror |
| `clear-calibration()` (797) | 1923 | Unloads; clears calibration and path; `files-loaded = false` |
| `launch-roi-editor()` (501) | 1939 | Needs L+R+calibration (else a toast). Extracts the current frame from each video synchronously, converts YUV→RGB PNG into `$XDG_CACHE_HOME`/`~/.cache/reco/roi`, base64-injects it into the HTML template, writes `roi_editor.html`, `open::that`; toasts |
| `paste-roi()` (502) | 2112 | Uses `roi-manual-json` (then clears it), else the clipboard via arboard. Parses `FieldRoi`, sets `cal.field_roi`, `save_calibration()` (overwrites the file), sets `has-roi`, `sync_roi_points`; toast |
| `toast-dismissed(int)` (631) | 3301 | `ToastManager::dismiss`, then `sync_to_ui` |

### 5.3 Models

- Every array is rebuilt wholesale as `ModelRc::new(VecModel::from(vec))`; no persistent VecModel is kept:
  - `available-codecs` (1508)
  - `recent-*-paths` (`sync_recent_paths`, 1175–1186)
  - `left/right-segments` (`sync_segments`, 1190–1208)
  - `roi-points-x/y` (`sync_roi_points`, 1210–1227)
  - `lens-search-results` (3083)
  - `toasts` (`ToastManager::to_model`, toast.rs:144–156; `sync_to_ui`, toast.rs:173)
- Static in-Slint models: all ComboBox string lists and the shortcuts struct array (1994).

### 5.4 Timers and notifiers

| Mechanism | Location | Role |
|---|---|---|
| `set_rendering_notifier` | 1544–1607 | `RenderingSetup` captures Slint's wgpu-28 device and queue, then retries `try_init_and_update`. `BeforeRendering` runs `vsync_render_tick` (4022) |
| Repeated `slint::Timer`, 2ms (`TICK_INTERVAL_MS`) | 3692–4006 | Automation autoload; update toast; poll `cal_rx` and `export_rx`; debounced window-size persistence; dead ROI-reload branch; toast expiry; debounced seek commit; `request_redraw()` while playing, a seek is pending, or the preview is dirty (4003) |
| `Timer::single_shot` | 2325, 2363 (40ms reorder); 3806 (800ms), 3824 (1500ms), 3842 / 3892 (1000ms) | Deferred source reopen; automation next-run and quit |

---

## 6. Preview viewport input

- **Mouse (`drag` TouchArea, main.slint 1314–1341):**
  - Covers the whole outer preview rectangle, including letterbox bars. It is declared after `preview-box`, so it sits on top.
  - `enabled: files-loaded`; default arrow cursor.
  - Pointer down records `last-x/y`. While pressed, `moved` calls `root.pan(dx_px, dy_px)` with deltas in logical px.
  - `scroll-event` calls `root.zoom(-e.delta-y / 40px)` (degrees of FOV) and returns `accept`.
  - No right-click, middle-click, double-click or pinch handling.
- **Keyboard (FocusScope key-pressed, 825–847):**

| Key | Action |
|---|---|
| Space | `toggle-playback()` |
| ← / → / ↑ / ↓ | `pan(∓20, 0)` / `pan(0, ∓20)` |
| `+` / `=` | `zoom(-5)` (zoom in) |
| `-` / `_` | `zoom(5)` (zoom out) |
| `[` / `]` | `seek-relative(∓5)` |
| r / R | `reset-view()` |
| f / F / F11 | toggles `root.full-screen` (UI only) |

  Focus is grabbed at `init` (822) and `forward-focus` is set (811). `refocus()` (812) is never called.
- **Rust pose path:**
  - `apply_pan` (856) → `PoseControl::apply_drag`. Config is built in `AppState::new` (476–491) and again in `reset_pipeline` (530–542): `DRAG_DEG_PER_PIXEL` 0.287 (78), `POSE_SMOOTHING` 0.25 (84), FOV 20–150, default 75 (70–72), `invert_drag_x: true`.
  - `clamp_targets` (1013) applies `clamp_via_coverage` when `use_constrained_look`.
  - Zoom, FOV and reset go through `IntentTranslator` (867, 875, 1006).
  - Smoothing runs per vsync in `smooth_camera` (886), which also pushes FOV to `pipeline.set_fov` (FRICTION N19).
  - `vsync_render_tick` writes `yaw`, `pitch` and `fov` back to the UI on every rendered frame (4086–4091), so the FOV slider tracks the wheel.
- **Render target sizing (4033–4044):**
  - Reads `preview-area-width/height` (logical px), clamps to 320–1920 × 240–1080, and calls `bridge.resize` only when the change exceeds 16px.
  - Skipped while recording.
  - The bridge allocates a fresh `wgpu::Texture` (RENDER_ATTACHMENT | TEXTURE_BINDING, Rgba8Unorm) each frame and hands it over with `slint::Image::try_from` (preview.rs 123–161). Lens preview goes through `render_lens_preview` (preview.rs 193–215; main.rs 781–796).
- **ROI:**
  - In the app there are only vertex dots, shown in lens-preview mode (main.slint 1301–1311), fed by `sync_roi_points` (main.rs 1210) from `cal.field_roi.left/right` for the selected side.
  - Editing happens in the external browser editor (§2.14), then Paste.
- **Not present:** in-app ROI drawing, calibration point picking, rubber-band or selection tools, drag-and-drop.

---

## 7. Background work and threading

| Work | Thread / mechanism | Flow back to the UI |
|---|---|---|
| UI, AppState, render | Slint winit event loop (main thread); `Rc<RefCell<AppState>>` | Direct property setters |
| Vsync render tick | `BeforeRendering` notifier → `vsync_render_tick` (4022–4095), on the UI thread | Runs `Playback::tick` (playback.rs 135, non-blocking `try_next_frame`, drift-free anchor), camera smoothing, then `render_current` (773) → `set_preview_frame`. Syncs frame/time display and the Finished state ("Playback finished"). Skipped while exporting |
| Decoding | Inside reco-io `FfmpegFileSource` | `Playback::open`, `seek` and `step_forward` (playback.rs 72/217/100) block the UI thread (decoder reinit) |
| Auto-calibration | `std::thread::spawn` (2569) → `reco_calibrate::video::calibrate_videos` | Progress closure → `invoke_from_event_loop` (2606) sets `calibration-step` (Debug name of the step) and `status-text` "Calibrating: {detail}". Result goes over `mpsc::channel` (2533); `cal_rx` is polled by the timer (3736) → `handle_calibration_result` (4473), which inits the pipeline, autosaves JSON, reseeds sliders and adds low-confidence / fallback-lens warning toasts. The cancel `AtomicBool` (2532) is never exposed |
| Export | `std::thread::spawn` (3541) → `export::run_export` (export.rs 107), `StitchJob::run(&interrupted)` | `invoke_from_event_loop`: telemetry sink every 30 frames → `telem-*` (export.rs 75–103); status strings (146: "Probing source...", "Opening encoder and decoders...", "Seeking…"); `export-frames-total` (174); progress → `frames-done`, `export-progress`, "Frame N (F fps) - ~m:ss remaining" (219–236); "Finalizing output file..." (264); AI banner (357). Completion goes over `mpsc` (3511); `export_rx` is polled (3745–3903): join, rebuild preview, outcome toasts, telemetry, automation repeats. Cancel: `Arc<AtomicBool>` (3572) |
| Preview recording | Encoder thread (732) fed by `sync_channel(4)` of NV12 `RecordingFrame` | NV12 readback runs on the UI thread in `render_current` (809–829) with `try_send` (drops when full). Display refreshes every 5th frame. `stop_recording` drops the sender and joins on the UI thread (753–764) |
| Update check | Thread (1455): ureq GET GitHub `releases/latest` | `Arc<Mutex<Option<String>>>` polled by the timer (3717): 30s toast plus auto `open::that(release URL)` (3732) |
| Telemetry | `TelemetryClient` thread (telemetry_client.rs 79), `mpsc::Sender<Event>`, ureq POST to a Cloud Run endpoint (13), 5s timeout | Fire-and-forget; no UI feedback |
| Segment reorder | `Timer::single_shot(40ms)` → `reopen_source` (969) | Deferred on the UI thread |

**Telemetry events and call sites:**
- `app_open`: 1413, 2445
- `context`: 1430, 4402
- `source_info`: 4411
- `calibration_complete`: 4487
- `calibration_error`: 4688
- `export_complete`: 3778
- `export_error`: 3881
- `bug_report`: 3665 (report capped at 16KB, oldest log lines dropped first; telemetry_client.rs 215–267)

---

## 8. Platform and integration features

- **Graphics backend (hard porting constraint):**
  - `BackendSelector::require_wgpu_28` (1388–1397) with downlevel limits and `GpuContext::select_backends()`.
  - The UI and reco-core share one wgpu device.
  - Preview frames are zero-copy textures (preview.rs). A target framework must support importing wgpu textures, or the port needs CPU readback.
  - `AdapterInfo` is fabricated (1566–1578; FRICTION N8).
- **rfd file dialogs:**

| Line | Dialog |
|---|---|
| 1614 | Left videos (multi-select; mp4/mov/avi/mkv) |
| 1687 | Right videos (same) |
| 1760 | Calibration JSON |
| 2466 | Prefs ONNX model |
| 2478 | Recording folder (`pick_folder`) |
| 3164 | Lens profile JSON |
| 3345 | Export output (`save_file`; MP4/MOV/MKV; adds `.mp4`) |
| 3364 | Export ONNX model |

- **Clipboard (arboard):** read for Paste ROI (2126); write of the bug report (3675). LineEdits use Slint's native clipboard.
- **Opening URLs and paths (`open` crate):** website 2457, forum 2461, ROI HTML 2085, show-in-folder (parent dir) 3621, update release page 3732.
- **Not present:** file drag-and-drop, menus, context menus, tooltips, system tray, multiple windows.
- **Keyboard shortcuts:** §6. Fullscreen via `full-screen` (843).
- **Window:**
  - Static title "Reco Video Stitcher" (439); never changed.
  - Preferred 1280×820, min 720×600.
  - Size and maximized state are persisted in the timer (3909–3927, saved after 2s of stability), but restore is disabled (1531–1537).
  - Release builds use `windows_subsystem="windows"` (main.rs:1).
- **Toasts:**
  - `ToastManager` (toast.rs): TTL info 4s, warn 7s, error 10s, plus custom values (8s, 30s); at most 4 visible; ids are monotonic.
  - Expired in the timer (3960–3965).
  - `sync_to_ui` (toast.rs 173–185) also copies the latest toast title into `status-text` with "[ERROR] " / "[WARN] " prefixes.
- **Settings:**
  - `GuiSettings` (settings.rs 25–90) via `reco_io::settings` at `<ProjectDirs("","","reco").config_dir()>/gui.json` (namespace "gui"; override `RECO_CONFIG_DIR`). MRU lists cap at 8.
  - Fields: `recent_left/right/calibration`, `default_codec/quality/blend_width`, `ai_model_path`, `window_size`, `window_maximized`, `recording_codec/quality/folder`, `preview_aspect`, `telemetry_enabled`, `telemetry_client_id` (UUID created at startup, 502–505), `dark_mode`.
  - When each is saved:

| What | Saved at |
|---|---|
| MRU lists | `push_*` on pick/load/auto-cal; clear-recent |
| Codec / quality / blend defaults | save-prefs, start-export (3503–3506) |
| AI model path | save-prefs, pick-export-model |
| Recording codec / quality / folder, dark mode, telemetry | save-prefs only |
| Preview aspect | on change |

  - Not persisted: panel widths and open states, export resolution, AI/panner/lookahead options, color-match, constrained look, calibration Advanced parameters, bug contact.
- **Calibration persistence:**
  - Save Calibration and Paste ROI overwrite the loaded JSON.
  - Auto-calibration writes `<left_stem>_calibration.json` next to the left video (4557–4584).
  - The export output is suggested as `<left_stem>_stitched.mp4` (4379–4389).
- **Automation (cargo feature `automation`, compiled out by default):**
  - Environment variables:

| Variable | Meaning |
|---|---|
| `RECO_AUTOLOAD` | `"left[;l2],right[;r2],cal.json"` (parsed at 143–188) |
| `RECO_AUTOEXPORT` | Output path |
| `RECO_AUTOEXPORT_MODEL` | Model path |
| `RECO_AUTOEXPORT_LOOKAHEAD` | Lookahead seconds (default 0) |
| `RECO_AUTOEXPORT_REPEAT` | Number of exports (default 1) |
| `RECO_VRAM_BUDGET_GB` | VRAM budget override (4205) |

  - Fired once from the timer after the GPU is captured (3700–3712) → `run_autoload` (4129) → `try_init_and_update`, then `invoke_start_export`.
  - A repeat driver with `_N` output names (4116) and nvidia-smi VRAM logging (4100) runs in the completion handler (3794–3831); `quit_event_loop` ends the run.
  - Other env inputs: `RUST_LOG` (EnvFilter, 1241), `XDG_CACHE_HOME`/`HOME` (ROI temp dir), `XDG_STATE_HOME`/`HOME` (log file).
- **Logging:**
  - tracing (1237–1317). Release builds write to a file: Windows next to the exe, macOS `~/Library/Logs/reco-gui.log`, Linux `~/.local/state/reco/reco-gui.log`. The file is truncated above 2MB.
  - Panic hook at 1354.
  - The log tail is attached to bug reports (422–437).
- **Accessibility:**
  - Slint's `accessibility` feature is enabled, but `main.slint` has zero `accessible-*` properties.
  - All custom controls are bare TouchAreas, so they are not focusable or announced: SectionHeader, VramRiskSlider, segment grip and ×, record button, panel toggle, toast ×, Report Bug, calibration-chip ×, recent rows, lens rows.
  - There is no keyboard path for segment reorder or the lookahead slider.

---

## 9. Line counts and fragile or unusual points

### main.slint (2,862 lines)
| Region | Lines | Count |
|---|---|---|
| Header and imports | 1–4 | 4 |
| LabeledSlider / VramRiskSlider / SectionHeader | 6–49 / 51–166 / 168–203 | 44 / 116 / 36 |
| Toast struct / ToastCard / ToastStack | 205–214 / 216–290 / 292–314 | 10 / 75 / 23 |
| SegmentList | 316–436 | 121 |
| RecoApp window attributes and theme tokens | 438–474 | 37 |
| RecoApp property/callback API | 476–803 | 328 |
| Focus, keys, layout wrapper | 805–856 | 52 |
| Toolbar | 857–913 | 57 |
| Left Files panel | 920–1220 | 301 |
| Preview area | 1222–1342 | 121 |
| Right Controls panel | 1344–1593 | 250 |
| Transport bar | 1597–1697 | 101 |
| Status bar | 1699–1775 | 77 |
| Recent / Shortcuts / Prefs / Bug modals | 1778–1961 / 1963–2036 / 2038–2198 / 2200–2265 | 184 / 74 / 161 / 66 |
| Export modal | 2269–2744 | 476 |
| Lens picker modal | 2745–2851 | 107 |
| ToastStack instance | 2854–2861 | 8 |

### main.rs (4,710 lines)
| Region | Lines | Count |
|---|---|---|
| Docs, consts, types, automation spec | 1–189 | 189 |
| AppState struct | 191–302 | 112 |
| AI probe, bug report, RecordingFrame | 304–447 | 144 |
| `impl AppState` (pipeline, pose, recording, render) | 449–1041 | 593 |
| UI sync helpers | 1043–1227 | 185 |
| Tracing, log path, panic hook | 1229–1376 | 148 |
| `main()` setup (backend, probes, update thread, codecs, seeding, notifier) | 1378–1607 | 230 |
| Callback registrations | 1609–3686 | 2,078 |
| Housekeeping timer | 3688–4006 | 319 |
| Auto-open panel and run | 4008–4016 | 9 |
| `vsync_render_tick` | 4018–4095 | 78 |
| Automation helpers | 4097–4192 | 96 |
| `budget_for_lookahead` | 4194–4213 | 20 |
| `try_init_and_update` | 4215–4446 | 232 |
| `classify_init_error` | 4448–4470 | 23 |
| `handle_calibration_result` | 4472–4710 | 239 |

Breakdown of the callback block (1609–3686):

| Handlers | Lines | Count |
|---|---|---|
| File pickers | 1609–1784 | 176 |
| Recent files | 1786–1883 | 98 |
| Clear L/R/calibration | 1885–1935 | 51 |
| ROI editor + paste | 1937–2178 | 242 |
| Segment remove/reorder | 2180–2366 | 187 |
| Prefs, links, model/folder pickers, aspect | 2368–2491 | 124 |
| Auto-calibrate | 2493–2632 | 140 |
| Playback and seek | 2634–2734 | 101 |
| View and stitch params | 2736–2825 | 90 |
| Live calibration | 2827–2916 | 90 |
| Lens tuning | 2918–3062 | 145 |
| Lens picker | 3064–3233 | 170 |
| Constrained look / lens preview / correction | 3235–3292 | 58 |
| Toast dismiss | 3294–3307 | 14 |
| Export dialog / start / cancel | 3309–3573 | 265 |
| Recording + show-in-folder | 3575–3624 | 50 |
| Bug report | 3626–3686 | 61 |

### Other files and Slint coupling
- **Slint-coupled:**
  - `preview.rs` 216 (`slint::Image::try_from`).
  - `export.rs` 388, of which about 70 lines touch `slint::Weak` or `invoke_from_event_loop`.
  - `toast.rs` 240 (uses the generated `Toast`, `ModelRc`; includes 54 lines of tests).
- **Framework-agnostic:** `playback.rs` 260, `settings.rs` 196, `telemetry_client.rs` 343 (62 lines of tests).
- Roughly 3,300 lines of `main.rs` call the Slint API directly; `AppState` (449–1041) is mostly portable except that `render_current` returns `slint::Image`.

### Fragile or unusual (verify before or while porting)

**Stale documentation**
1. main.slint:2 and main.rs:5 say "Material dark", but `build.rs` uses `fluent-dark`.
2. The README claims `reco-control::KeyboardTransport` routing and FOV/director/detection-interval persistence; neither exists.

**Theme**
3. Theming is partial: Palette is set once at init (823), about 116 hard-coded colours bypass the tokens, and 9 of 25 tokens are unused.

**Toasts**
4. Code comments call the toast overlay unreliable: main.slint 1714–1716 says "toasts are broken"; toast.rs:175–176 says the overlay "isn't rendering reliably".
5. `sync_to_ui` overwrites `status-text`, which forces ordering hacks (3873–3885).

**Dead or unwired API**
6. Unused or only set:
   - Unused: `progress`, `stats-panel-open`, `export-blend-width`.
   - Written but never displayed: `fps`, `yaw`, `pitch`, `lens-*-lens`, `lens-candidates-count`, `export-frames-done`.
   - Never written: `telem-frames-dropped`, `telem-cal-*`. So the Stats "Dropped"/"Cal:" lines and the bug report's Calibration section can never appear.
7. Never called or never read:
   - `refocus()` is never invoked.
   - `roi_reload_pending` is never armed, so the timer branch at 3929–3957 is dead.
   - `export_last_progress_at` is written but never read.
   - `TabWidget` and `StyleMetrics` are imported but unused.
   - `_seek_str` (1995) is unused.

**Stats panel**
8. Stats only ever receives export telemetry (export.rs sink). The live preview never feeds `telem-*`, and the preview is released during export.

**Shortcuts and focus**
9. The shortcuts dialog (1996) promises arrow-key frame stepping, which is not implemented. F/F11 fullscreen is undocumented.
10. Std widgets keep focus after a click, so Space can re-fire the last-clicked button (for example "Reset View").
11. Shortcuts stay active behind modals.

**Calibration**
12. Calibration cannot be cancelled. The Advanced sliders stay enabled while calibrating.

**Bindings**
13. One-way bindings sit on interactive widgets:
    - Timeline `value:` (1645), sync-offset LineEdit `text:` (1153), export start/end LineEdits (2425/2447).
    - Lens-correction `checked:` (1446), frames `current-value:` (1030), Fine-tune `expanded:` (1468), export ComboBox `current-index:` expressions.
    - In Slint, user interaction overwrites such bindings. Several only work because their `if` subtree is recreated.
14. The Rust seek comment (2712–2723) refers to `changed` echoes, but the UI now uses `released` (1647), so the 120ms debounce only adds latency.
15. The export Codec combo maps fixed indices (hevc→1, av1→2, 2383–2385) onto the runtime-probed `available-codecs`, so the selection is wrong if hevc is unavailable.

**Instantiation and layout**
16. `if`-instantiated panels and dialogs lose expanded flags, typed text and scroll position on close.
17. Clicking the Export backdrop does not clear `export-error-text`.
18. Opening the export dialog re-seeds codec and quality from saved defaults every time.
19. The toolbar panel toggle (894) has no `x`. Slint centres unpositioned children, so it likely renders mid-toolbar, not "pinned to the far right" as the comment says. Confirm with a screenshot.
20. Resize handles (926, 1352) are declared before, and overlapped by, their ScrollViews. They have no maximum width and are not persisted. FRICTION N18: sliders inside a ScrollView lose pointer tracking.
21. `has-both-videos` is an in-out property bound to display labels (497).
22. Fixed-size modals: Recent is 720px wide, which equals the minimum window width.

**Recording**
23. NV12 readback on the UI thread (FRICTION N16), frames silently dropped by `try_send`, and the encoder thread is joined on the UI thread.
24. Preferences Cancel does not revert the live `recording-*` properties, and opening Preferences resets the transport quality combo.

**UI-thread blocking**
25. Heavy synchronous work on the UI thread:
    - ROI extraction with per-pixel YUV→RGB, PNG and base64 (1996–2052).
    - Decoder reopen and seek in callbacks.
    - Lens database search on every keystroke (3068) and again on pick (3098).

**Rendering and ROI**
26. ROI dots are placed on preview-box dimensions while the image uses `contain` and the render target is clamped to 1920×1080, so they misalign when aspects differ.
27. The ROI HTML injects the raw calibration path into a JS string (2063); Windows backslashes become escapes.
28. The render target uses logical px, so it is not DPI-aware.
29. After an export, the preview rebuild reopens playback, so the position resets to frame 0 (3754–3762).

**Side effects**
30. The update check auto-opens the browser (3732).
31. Bug reports are always sent, even without telemetry opt-in, and silently copied to the clipboard.
32. A comment says "Save contact for next time" (3667), but the contact is never saved.
33. Paste ROI, Save Calibration and the auto-calibration autosave all write calibration JSON to disk. The first two overwrite the loaded file in place; the autosave goes next to the left video.

**State and data handling**
34. Removing a segment or clearing an input unloads the pipeline without re-init; the user must re-load or re-calibrate. "+" always appends segments rather than replacing.
35. Window size is persisted but never restored.
36. The uncommitted `color-match` is persisted nowhere (settings or calibration) and resets to true on launch.
37. Panner defaults are duplicated in Slint (546–562) and must mirror `FieldPannerConfig::broadcast()` (`crates/reco-autocam/src/panners/field.rs` 216–270). They currently match, but only after the uncommitted edit.
38. Native dialogs are opened before AppState is borrowed. Keep that ordering in the port, because a nested event loop that fires the render notifier while AppState is borrowed would panic in the RefCell.
