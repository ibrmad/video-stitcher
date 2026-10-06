# Module 8: parity sweep (compact plan)

> Run inline (owner: compact plan, no review pause; TDD, checks and a
> self-review stay). Branch `feat/desktop-m8-parity`.

**Goal (DESIGN.md, Modules):** every PARITY.md item ticked with evidence,
a polish and performance pass, then the Slint app retired with the
owner's sign-off.

## Tasks

1. **Inventory sweep.** Walk `docs/slint-inventory.md` §1–§9 (and the
   DESIGN.md "Slint issues to fix, not copy" list) against PARITY.md and
   the app. Every Slint behaviour either has a ticked PARITY line with
   evidence, or gets built and checked now, or gets a ruling the owner
   sees at sign-off. Result goes in PARITY.md's Module 8 section.
2. **Open items.** Tick "Export progress, status text and Cancel; Show
   in folder after an export" with check_m6 evidence. The "ROI points
   over the lens preview" ruling goes to the owner at sign-off.
3. **Performance pass** (`tools/check_m8.py perf`), on the alfheim pair
   and the 5.3K pair, against DESIGN.md Rule 8:
   - first window frame under 1 s after start (a `--perf-log` line);
   - zero-copy preview in use (no CPU copies);
   - playing: the picture keeps the source rate; panning: the picture
     re-renders every frame; UI draw under 4 ms a frame;
   - about 0% CPU while paused.
   Numbers go in PARITY.md.
4. **Polish pass.** One full suite run (theme, check_m0–m8), one look at
   every screenshot, the findings fixed in one batch, the affected checks
   rerun.
5. **Sign-off and retirement.** Screenshots, parity summary, rulings and
   numbers to the owner. On sign-off only: one commit removes
   `crates/reco-gui` and moves the workspace, `deny.toml`, the CI and
   release workflows and the docs to reco-desktop. (The checkout holds
   another session's uncommitted edits to `crates/reco-gui/ui/main.slint`:
   the owner decides what happens to them first.)
