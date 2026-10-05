//! A list to pick one row from, one line each (the lens picker's results):
//! a row highlights under the pointer, and a click picks it; with the
//! keyboard, ↑/↓ move the highlight and Return picks. Rows come from the
//! `row` template; the App fills them (`set_rows`) and reads
//! `PickListAction::Picked`.

use std::collections::HashMap;

use makepad_widgets::widget_tree::CxWidgetExt;
use makepad_widgets::*;

use crate::file_rows::row_at;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoPickListBase = #(RecoPickList::register_widget(vm))
    mod.widgets.RecoPickList = set_type_default() do mod.widgets.RecoPickListBase{
        width: Fill height: Fit flow: Down
        row_height: theme.reco_row
        hover_color: theme.reco_hover
        row := View{
            width: Fill height: theme.reco_row flow: Right align: Align{y: 0.5}
            padding: Inset{left: theme.reco_pad right: theme.reco_pad}
            line := RecoText{width: Fill max_lines: 1 text_overflow: TextOverflow.Ellipsis text: ""}
        }
    }
}

/// What the list tells the App.
#[derive(Clone, Debug, Default)]
pub enum PickListAction {
    /// This row was picked.
    Picked(usize),
    #[default]
    None,
}

/// Rows to pick one from.
#[derive(Script, Widget)]
pub struct RecoPickList {
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
    hover_color: Vec4f,
    /// The row template, by name.
    #[rust]
    templates: HashMap<LiveId, ScriptObjectRef>,
    /// One widget per row, made from the template as rows appear.
    #[rust]
    items: Vec<WidgetRef>,
    #[rust]
    rows: Vec<String>,
    /// The highlighted row (pointer or keys).
    #[rust]
    hover: Option<usize>,
    /// The whole list: what redraws, and what the remote snapshot reports.
    #[redraw]
    #[rust]
    area: Area,
}

impl ScriptHook for RecoPickList {
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

impl RecoPickList {
    /// Show these rows.
    pub fn set_rows(&mut self, cx: &mut Cx, rows: Vec<String>) {
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
        for (item, line) in self.items.iter().zip(&rows) {
            item.label(cx, ids!(line)).set_text(cx, line);
        }
        self.hover = None;
        self.rows = rows;
        self.area.redraw(cx);
    }

    fn pick(&mut self, cx: &mut Cx, row: usize) {
        cx.widget_action(self.widget_uid(), PickListAction::Picked(row));
    }
}

impl Widget for RecoPickList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        let rows = self.rows.len();
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                let hover = row_at(fe.abs.y - fe.rect.pos.y, self.row_height, rows);
                if hover != self.hover {
                    self.hover = hover;
                    self.area.redraw(cx);
                }
                cx.set_cursor(MouseCursor::Hand);
            }
            Hit::FingerHoverOut(_) => {
                if self.hover.take().is_some() {
                    self.area.redraw(cx);
                }
            }
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                cx.set_key_focus(self.area);
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over && !fe.cancelled => {
                if let Some(row) = row_at(fe.abs.y - fe.rect.pos.y, self.row_height, rows) {
                    self.pick(cx, row);
                }
            }
            Hit::KeyDown(ke) => match ke.key_code {
                KeyCode::ArrowDown if rows > 0 => {
                    self.hover = Some(self.hover.map_or(0, |r| (r + 1).min(rows - 1)));
                    self.area.redraw(cx);
                }
                KeyCode::ArrowUp if rows > 0 => {
                    self.hover = Some(self.hover.map_or(0, |r| r.saturating_sub(1)));
                    self.area.redraw(cx);
                }
                KeyCode::ReturnKey => {
                    if let Some(row) = self.hover {
                        self.pick(cx, row);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let top = cx.turtle().pos();
        let width = cx.turtle().rect().size.x;
        for index in 0..self.rows.len() {
            if self.hover == Some(index) {
                self.draw_rect.color = self.hover_color;
                self.draw_rect.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(top.x, top.y + index as f64 * self.row_height),
                        size: dvec2(width, self.row_height),
                    },
                );
            }
            if let Some(item) = self.items.get(index).cloned() {
                item.draw_all(cx, scope);
            }
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
