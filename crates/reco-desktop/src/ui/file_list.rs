//! A camera's files as rows under its camera row in the Setup panel: the
//! name, the length and a remove button, after Rerun's list rows. Drag a row
//! to move it. Click a row to select it; then ↑/↓ select, Delete removes and
//! Alt+↑/↓ moves. Rows come from the `row` template; the App fills them
//! (`set_rows`) and reads `FileListAction`s.

use std::collections::HashMap;

use makepad_widgets::widget_tree::CxWidgetExt;
use makepad_widgets::*;

use crate::file_rows::{drop_index, gap_at, key_command, row_at, ListCommand, ListKey};

/// How far a press moves before it drags its row (points).
const DRAG_START: f64 = 4.0;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoFileListBase = #(RecoFileList::register_widget(vm))
    mod.widgets.RecoFileList = set_type_default() do mod.widgets.RecoFileListBase{
        width: Fill height: Fit flow: Down
        row_height: theme.reco_row
        drop_line: theme.reco_focus_width
        hover_color: theme.reco_hover
        select_color: theme.reco_selection_fill
        drop_color: theme.reco_accent
        // One file: indented under the camera's name, its length in the
        // camera row's column, and a remove button.
        row := View{
            width: Fill height: theme.reco_row flow: Right spacing: theme.reco_gap align: Align{y: 0.5}
            padding: Inset{left: theme.reco_file_indent right: theme.reco_pad}
            name := RecoText{width: Fill max_lines: 1 text_overflow: TextOverflow.Ellipsis text: ""}
            length := RecoMeta{width: Fit text: ""}
            Tip{text: "Remove this file"
                remove := RecoRowIcon{
                    draw_icon +: {svg: crate_resource("self:resources/icons/close.svg")}
                }
            }
        }
    }
}

/// What the list asks of the App.
#[derive(Clone, Debug, Default)]
pub enum FileListAction {
    /// Remove the file at this row.
    Remove(usize),
    /// Move the file at `from` to `to`.
    Move {
        /// Where it is.
        from: usize,
        /// Where it goes.
        to: usize,
    },
    #[default]
    None,
}

/// A camera's files, one row each.
#[derive(Script, Widget)]
pub struct RecoFileList {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[live]
    draw_rect: DrawColor,
    #[live]
    row_height: f64,
    #[live]
    drop_line: f64,
    #[live]
    hover_color: Vec4f,
    #[live]
    select_color: Vec4f,
    #[live]
    drop_color: Vec4f,
    /// The row template, by name.
    #[rust]
    templates: HashMap<LiveId, ScriptObjectRef>,
    /// One widget per row, made from the template as rows appear.
    #[rust]
    items: Vec<WidgetRef>,
    /// Each row's name and length.
    #[rust]
    rows: Vec<(String, String)>,
    #[rust]
    hover: Option<usize>,
    #[rust]
    selected: Option<usize>,
    /// A press on a row: the row and where it started.
    #[rust]
    press: Option<(usize, f64)>,
    /// While dragging: the gap the row would drop into.
    #[rust]
    drop_gap: Option<usize>,
    /// The whole list: what redraws, and what the remote snapshot reports.
    #[redraw]
    #[rust]
    area: Area,
}

impl ScriptHook for RecoFileList {
    fn on_before_apply(
        &mut self,
        _vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        if apply.is_reload() {
            self.templates.clear();
        }
    }

    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        apply: &Apply,
        scope: &mut Scope,
        value: ScriptValue,
    ) {
        // The template is the named child (`row := ...`), as FlatList keeps
        // its templates.
        if !apply.is_eval() {
            if let Some(obj) = value.as_object() {
                vm.vec_with(obj, |vm, vec| {
                    for kv in vec {
                        if let (Some(id), Some(template)) = (kv.key.as_id(), kv.value.as_object()) {
                            self.templates
                                .insert(id, vm.bx.heap.new_object_ref(template));
                        }
                    }
                });
            }
        }
        if apply.is_reload() {
            if let Some(template) = self.templates.get(&live_id!(row)) {
                let value: ScriptValue = template.as_object().into();
                for item in &mut self.items {
                    item.script_apply(vm, apply, scope, value);
                }
            }
        }
    }
}

impl RecoFileList {
    /// Show these rows: each file's name and length.
    pub fn set_rows(&mut self, cx: &mut Cx, rows: Vec<(String, String)>) {
        if rows == self.rows {
            return;
        }
        let Some(template) = self.templates.get(&live_id!(row)) else {
            return;
        };
        let value: ScriptValue = template.as_object().into();
        while self.items.len() < rows.len() {
            let widget = cx.with_vm(|vm| WidgetRef::script_from_value(vm, value));
            cx.widget_tree_insert_child(self.uid, LiveId(self.items.len() as u64), widget.clone());
            self.items.push(widget);
        }
        for (item, (name, length)) in self.items.iter().zip(&rows) {
            item.label(cx, ids!(name)).set_text(cx, name);
            item.label(cx, ids!(length)).set_text(cx, length);
        }
        self.selected = self.selected.filter(|s| *s < rows.len());
        self.hover = None;
        self.rows = rows;
        self.area.redraw(cx);
    }

    fn line(&mut self, cx: &mut Cx2d, rect: Rect, color: Vec4f) {
        self.draw_rect.color = color;
        self.draw_rect.draw_abs(cx, rect);
    }

    fn command(&mut self, cx: &mut Cx, command: ListCommand) {
        let uid = self.widget_uid();
        match command {
            ListCommand::Select(row) => self.selected = Some(row),
            ListCommand::Remove(row) => cx.widget_action(uid, FileListAction::Remove(row)),
            ListCommand::Move { from, to } => {
                self.selected = Some(to);
                cx.widget_action(uid, FileListAction::Move { from, to });
            }
        }
        self.area.redraw(cx);
    }
}

impl Widget for RecoFileList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();
        // The rows first: a remove button takes its own click.
        for (index, item) in self.items.iter().enumerate().take(self.rows.len()) {
            let actions = cx.capture_actions(|cx| item.handle_event(cx, event, scope));
            if item.button(cx, ids!(remove)).clicked(&actions) {
                cx.widget_action(uid, FileListAction::Remove(index));
            }
        }
        let rows = self.rows.len();
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                let hover = row_at(fe.abs.y - fe.rect.pos.y, self.row_height, rows);
                if hover != self.hover {
                    self.hover = hover;
                    self.area.redraw(cx);
                }
                cx.set_cursor(MouseCursor::Grab);
            }
            Hit::FingerHoverOut(_) => {
                if self.hover.take().is_some() {
                    self.area.redraw(cx);
                }
            }
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                if let Some(row) = row_at(fe.abs.y - fe.rect.pos.y, self.row_height, rows) {
                    self.press = Some((row, fe.abs.y));
                    cx.set_key_focus(self.area);
                    self.command(cx, ListCommand::Select(row));
                }
            }
            Hit::FingerMove(fe) => {
                if let Some((_, start)) = self.press {
                    if self.drop_gap.is_some() || (fe.abs.y - start).abs() > DRAG_START {
                        let gap = gap_at(fe.abs.y - fe.rect.pos.y, self.row_height, rows);
                        if self.drop_gap != Some(gap) {
                            self.drop_gap = Some(gap);
                            self.area.redraw(cx);
                        }
                        cx.set_cursor(MouseCursor::Grabbing);
                    }
                }
            }
            Hit::FingerUp(fe) => {
                if let (Some((from, _)), Some(gap)) = (self.press.take(), self.drop_gap.take()) {
                    if let (false, Some(to)) = (fe.cancelled, drop_index(from, gap)) {
                        self.command(cx, ListCommand::Move { from, to });
                    }
                    self.area.redraw(cx);
                }
            }
            Hit::KeyDown(ke) => {
                let key = match ke.key_code {
                    KeyCode::ArrowUp if ke.modifiers.alt => Some(ListKey::MoveUp),
                    KeyCode::ArrowDown if ke.modifiers.alt => Some(ListKey::MoveDown),
                    KeyCode::ArrowUp => Some(ListKey::Up),
                    KeyCode::ArrowDown => Some(ListKey::Down),
                    KeyCode::Delete | KeyCode::Backspace => Some(ListKey::Delete),
                    _ => None,
                };
                if let Some(command) = key.and_then(|k| key_command(k, self.selected, rows)) {
                    self.command(cx, command);
                }
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let top = cx.turtle().pos();
        let width = cx.turtle().rect().size.x;
        for index in 0..self.rows.len() {
            let row = Rect {
                pos: dvec2(top.x, top.y + index as f64 * self.row_height),
                size: dvec2(width, self.row_height),
            };
            if self.selected == Some(index) {
                self.line(cx, row, self.select_color);
            } else if self.hover == Some(index) && self.drop_gap.is_none() {
                self.line(cx, row, self.hover_color);
            }
            if let Some(item) = self.items.get(index).cloned() {
                item.draw_all(cx, scope);
            }
        }
        if let Some(gap) = self.drop_gap {
            let y = top.y + gap as f64 * self.row_height - self.drop_line * 0.5;
            self.line(
                cx,
                Rect {
                    pos: dvec2(top.x, y),
                    size: dvec2(width, self.drop_line),
                },
                self.drop_color,
            );
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
