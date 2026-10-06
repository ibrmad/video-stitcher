# Panels slide open and closed (compact plan)

> Run inline (owner: compact plan, no review pause; TDD, checks and a
> self-review stay). Branch `feat/desktop-ui-pass`.

**Goal:** Setup, Adjust and the time panel's lanes slide when toggled, so
the layout change reads as one motion. Owner's ask (2026-10-06), choices:
"Slide" and "0.2 s".

**Spec (as approved):**

1. A toggle (the top bar's buttons, ⌘1/⌘2/⌘3, the View menu) slides its
   panel out past its window edge, or back in, over 0.2 s, easing out.
2. The panel keeps its layout while it moves: its rows keep their width
   (or the lanes their height) and slide; nothing re-wraps.
3. The picture grows (or gives way) smoothly with it.
4. Narrowing the window still folds panels at once, and a motion under way
   ends at once when the window changes size.
5. A toggle during a motion turns it around from where it is.
6. A panel opens to the width it had (a dragged width included), and the
   saved layout never takes a width from mid-motion.

## Tasks

1. **Motion** (`src/motion.rs`, UI-free): `Motion { from, to, start,
   duration }`, ease-out cubic, `value(now)`, `done(now)`; unit tests.
2. **The panel box** (`src/ui/panel_box.rs`): `RecoPanelBox`, a View
   wrapper whose width can be pinned (anchored to its end for Setup, so it
   slides left) and whose height can be set (the lanes, clipped from the
   bottom); it notes its natural height for the lanes' way back.
3. **Motions in the App** (`src/panel_motion.rs`): a toggle starts a
   motion; each frame sets the splitter's position (its floors relaxed for
   the motion with `script_apply_eval!`, put back from the theme after, and
   the position and fold set again) or the lanes' height; the end folds or
   leaves the panel open; `apply_shell` leaves a moving panel alone; a
   window resize ends every motion.
   - Checks: check_m0 `motion` (⌘1/⌘2/⌘3: the picture's edge passes
     through points between, the moving panel keeps its width, each ends
     folded or open; a dragged width comes back; a toggle mid-motion turns
     around).
4. **Final.** check_m0–m7, unit tests, clippy, DESIGN, self-review.
