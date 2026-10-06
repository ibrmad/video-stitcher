# Value fields: every slider's number can be typed (compact plan)

> Run inline, as the UI pass (owner: compact plan, no review pause; TDD,
> checks and a self-review stay). Branch `feat/desktop-ui-pass`, after the
> round-2 fixes.

**Goal:** a slider's number is a field: type a value, or step it with
↑/↓, for more control than a drag gives. Asked by the owner on 2026-10-06
("we need it to be very intuitive"); before this, the export sheet's Start
and End could be typed into and the 26 other values could not.

**Spec (as approved):**

1. **Where:** every slider's value. Adjust 15 (Field of view; Seam blend;
   Tilt, Roll, Overlap, Camera depth, Vertical shift; the lens's 8),
   Setup › Calibration › Advanced 4, the export sheet's AI 7 (Lookahead
   and the 6 knobs), and Start and End (already typed; they take the same
   field). Sync (frames) has no slider and stays as it is.
2. **Look: a quiet box.** A dark box with no border, the digits on the
   right, the row's value width; a shade lighter on hover; a green ring
   while typing. Start and End drop their border to match.
3. **Typing:**
   - A click selects the digits, so typing replaces them.
   - Return, or a click anywhere else, applies: the slider moves there and
     the change works as a drag does (the preview follows, Unsaved shows).
     The keyboard then goes back to the preview (Space plays).
   - Escape puts the shown value back and gives the keyboard back. In the
     export sheet it does not close the sheet.
   - Units are optional (80 or 80°; 2.5 or 2.5 s; times as 1:30 or 90); a
     decimal comma reads as a point (0,05).
   - A value past an end of the slider goes to that end; text that is not
     a number puts the shown value back. A field left as it was changes
     nothing (a shown 0.05 does not round a 0.0523 away).
   - While a field is typed in, nothing overwrites it (zooming the
     preview moves Field of view); a field focused but not typed in
     follows its value.
4. **↑/↓ while typing:** one step of the last digit shown (Field of view
   75° → 76°, Seam blend 0.05 → 0.06, Start 0:02 → 0:03); ⇧ takes ten.
   Each step applies at once, as a drag would; the digits stay selected.
5. **Locked with their slider:** the calibration's Advanced fields while a
   calibration runs.

## Tasks

Each task: change the checks (or unit tests) first and watch them fail on
the current build, then the code, then green, then commit.

1. **How a value reads** (`src/value_text.rs`, UI-free).
   - `Reading` (decimals and a unit, or a clock; Lookahead's zero reads
     "Off"): `text(value)`, `step()`, `read(text)` (unit optional, comma
     as point, finite numbers only), and `typed(text, current)`: the
     number, or `None` when it doesn't read or is the shown text.
   - Unit tests first; the screens' own formatters become Readings, with
     the same strings as before.
2. **The field** (`src/ui/value_field.rs`): `RecoValueField`, a TextInput
   wrapper (as `RecoFold` wraps FoldHeader).
   - Select all on focus; a widget cancel scope while typing, so Escape is
     the field's; Return and a click away send `Typed(text)`, ↑/↓
     `Step(±1 | ±10)`; `set_text` keeps typed text (the rule in spec 3).
   - Theme: `reco_value_field` width, `reco_value_field_height` (3 pt
     above and below in a 28 pt row), colours from existing tokens.
   - `App::set_label` sets text through the generic widget, so the
     screens' existing value updates reach fields and labels alike.
3. **Wire every row.** Tune (6), View (Field of view), Lens (8),
   calibration options (4), AI (6 knobs + Lookahead), export range (2):
   a typed or stepped value goes through the slider (which clamps to its
   own range), then the same path a drag takes.
   - Checks: check_m4 `values` (Adjust: type, Return, the slider and
     preview follow; Escape; clamp; text that isn't a number; ↑ and ⇧↑;
     Space plays after Return; an untouched field changes nothing),
     check_m6 `rules` extended (Escape in a field keeps the sheet open;
     Lookahead "Off"; a knob typed), check_m7 or m3 for the calibration
     fields locked while calibrating.
4. **Final.** check_m0–m7 and the theme check; unit tests; clippy; DESIGN
   (the value field), PARITY, FRICTION if anything new; self-review.
