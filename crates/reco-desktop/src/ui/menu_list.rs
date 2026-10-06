//! A menu's rows in Reco's own style (the app menu and the Recent menu, in
//! a `RecoMenu` popover): items, separators and section lines, at the app's
//! row height with its padding, an item lit under the pointer as Rerun's
//! menus light theirs. A click picks an item; with the keyboard (the
//! popover hands it the focus), ↑/↓ move between items and Return picks.
//! Rows come from the `item`, `choice`, `section` and `separator`
//! templates; the App fills them (`set_entries`) and reads
//! `MenuListAction::Picked`. A choice is an item that can carry a ✓ at
//! the content edge (the record menu's quality), so no row keeps an empty
//! mark column.

use std::collections::HashMap;

use makepad_widgets::widget_tree::CxWidgetExt;
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoMenuListBase = #(RecoMenuList::register_widget(vm))
    mod.widgets.RecoMenuList = set_type_default() do mod.widgets.RecoMenuListBase{
        width: Fill height: Fit flow: Down
        row_height: theme.reco_row
        hover_inset: theme.reco_gap_s
        draw_hover +: {
            color: theme.reco_hover
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, theme.corner_radius)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        item := View{
            width: Fill height: theme.reco_row flow: Right align: Align{y: 0.5}
            padding: Inset{left: theme.reco_pad right: theme.reco_pad}
            label := RecoText{width: Fill max_lines: 1 text_overflow: TextOverflow.Ellipsis text: ""}
        }
        choice := View{
            width: Fill height: theme.reco_row flow: Right spacing: theme.reco_gap align: Align{y: 0.5}
            padding: Inset{left: theme.reco_pad right: theme.reco_pad}
            label := RecoText{width: Fill max_lines: 1 text_overflow: TextOverflow.Ellipsis text: ""}
            mark := RecoText{text: "" draw_text +: {color: theme.reco_accent}}
        }
        section := View{
            width: Fill height: theme.reco_row flow: Right align: Align{y: 0.5}
            padding: Inset{left: theme.reco_pad right: theme.reco_pad}
            label := RecoSubdued{width: Fill max_lines: 1 text_overflow: TextOverflow.Ellipsis text: ""}
        }
        separator := View{
            width: Fill height: Fit
            padding: Inset{top: theme.reco_gap_s bottom: theme.reco_gap_s}
            RecoSeparator{}
        }
    }

    // A menu that drops from its button (or from any anchor, `open_at`):
    // the panel Reco's sheets and menus share, the rows a `RecoMenuList` in
    // `content`. Escape or a press outside closes it.
    mod.widgets.RecoMenu = PopoverFlat{
        // The anchor doesn't clip its button: a row icon's face reaches past
        // its box, so its ink ends on the content edge (Rule 11).
        clip_x: false clip_y: false
        placement: BottomStart
        trap_focus: true
        offset: theme.reco_gap_xs
        panel_padding: Inset{top: theme.reco_gap_s bottom: theme.reco_gap_s}
        draw_panel +: {
            color: theme.reco_band
            border_color: theme.reco_widget
            border_radius: theme.container_corner_radius
        }
        content +: {width: theme.reco_menu_width}
    }
}

/// One row of a menu.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    /// A command, picked by its id.
    Item(LiveId, String),
    /// One of a set (a quality), picked by its id; the chosen one shows a ✓.
    Choice(LiveId, String, bool),
    /// A line between groups.
    Separator,
    /// A quiet line that names something (the version), not picked.
    Section(String),
}

impl MenuEntry {
    fn template(&self) -> LiveId {
        match self {
            MenuEntry::Item(..) => live_id!(item),
            MenuEntry::Choice(..) => live_id!(choice),
            MenuEntry::Separator => live_id!(separator),
            MenuEntry::Section(_) => live_id!(section),
        }
    }

    fn id(&self) -> Option<LiveId> {
        match self {
            MenuEntry::Item(id, _) | MenuEntry::Choice(id, _, _) => Some(*id),
            _ => None,
        }
    }

    /// A choice's mark: a ✓ when it is the chosen one.
    fn mark(&self) -> &'static str {
        match self {
            MenuEntry::Choice(_, _, true) => "✓",
            _ => "",
        }
    }
}

/// What the list tells the App.
#[derive(Clone, Debug, Default)]
pub enum MenuListAction {
    /// This item was picked.
    Picked(LiveId),
    #[default]
    None,
}

/// A menu's rows.
#[derive(Script, Widget)]
pub struct RecoMenuList {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[live]
    draw_hover: DrawColor,
    /// An item's height (the `item` template's).
    #[live]
    row_height: f64,
    /// How far the lit row's fill keeps from the menu's sides.
    #[live]
    hover_inset: f64,
    /// The row templates, by name.
    #[rust]
    templates: HashMap<LiveId, ScriptObjectRef>,
    /// One widget per row, and the template it came from.
    #[rust]
    rows: Vec<(LiveId, WidgetRef)>,
    #[rust]
    entries: Vec<MenuEntry>,
    /// The lit item (pointer or keys).
    #[rust]
    hover: Option<usize>,
    #[redraw]
    #[rust]
    area: Area,
}

impl ScriptHook for RecoMenuList {
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
        // The templates are the named children, as FlatList keeps its own.
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
            for (template, row) in &mut self.rows {
                if let Some(source) = self.templates.get(template) {
                    let value: ScriptValue = source.as_object().into();
                    row.script_apply(vm, apply, scope, value);
                }
            }
        }
    }
}

impl RecoMenuList {
    /// Show these rows.
    pub fn set_entries(&mut self, cx: &mut Cx, entries: Vec<MenuEntry>) {
        if entries == self.entries {
            return;
        }
        for (index, entry) in entries.iter().enumerate() {
            let template = entry.template();
            if self
                .rows
                .get(index)
                .is_some_and(|(kind, _)| *kind == template)
            {
                continue;
            }
            let Some(source) = self.templates.get(&template) else {
                return;
            };
            let value: ScriptValue = source.as_object().into();
            let widget = cx.with_vm(|vm| WidgetRef::script_from_value(vm, value));
            cx.widget_tree_insert_child(self.uid, LiveId(index as u64), widget.clone());
            if index < self.rows.len() {
                self.rows[index] = (template, widget);
            } else {
                self.rows.push((template, widget));
            }
        }
        for ((_, row), entry) in self.rows.iter().zip(&entries) {
            if let MenuEntry::Item(_, text)
            | MenuEntry::Choice(_, text, _)
            | MenuEntry::Section(text) = entry
            {
                row.label(cx, ids!(label)).set_text(cx, text);
            }
            if let MenuEntry::Choice(..) = entry {
                row.label(cx, ids!(mark)).set_text(cx, entry.mark());
            }
        }
        self.hover = None;
        self.entries = entries;
        self.area.redraw(cx);
    }

    /// The item whose drawn rect holds `abs`.
    fn item_under(&self, cx: &Cx, abs: Vec2d) -> Option<usize> {
        self.rows
            .iter()
            .zip(&self.entries)
            .position(|((_, row), entry)| {
                entry.id().is_some() && row.area().clipped_rect(cx).contains(abs)
            })
    }

    /// The next item from `from` going `down` (or up), if any.
    fn step(&self, from: Option<usize>, down: bool) -> Option<usize> {
        let items: Vec<usize> = (0..self.entries.len())
            .filter(|i| self.entries[*i].id().is_some())
            .collect();
        match (from, down) {
            (None, true) => items.first().copied(),
            (None, false) => items.last().copied(),
            (Some(at), true) => items.iter().copied().find(|i| *i > at).or(Some(at)),
            (Some(at), false) => items.iter().rev().copied().find(|i| *i < at).or(Some(at)),
        }
    }

    fn pick(&mut self, cx: &mut Cx, index: usize) {
        if let Some(id) = self.entries.get(index).and_then(MenuEntry::id) {
            cx.widget_action(self.widget_uid(), MenuListAction::Picked(id));
        }
    }
}

impl Widget for RecoMenuList {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(fe) | Hit::FingerHoverOver(fe) => {
                let hover = self.item_under(cx, fe.abs);
                if hover != self.hover {
                    self.hover = hover;
                    self.area.redraw(cx);
                }
            }
            // A menu opens with nothing lit: the keys start from the top
            // (or the bottom) every time, not where the last open left off.
            Hit::FingerHoverOut(_) | Hit::KeyFocusLost(_) => {
                if self.hover.take().is_some() {
                    self.area.redraw(cx);
                }
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() && fe.is_over && !fe.cancelled => {
                if let Some(index) = self.item_under(cx, fe.abs) {
                    self.pick(cx, index);
                }
            }
            Hit::KeyDown(ke) => match ke.key_code {
                KeyCode::ArrowDown | KeyCode::ArrowUp => {
                    self.hover = self.step(self.hover, ke.key_code == KeyCode::ArrowDown);
                    self.area.redraw(cx);
                }
                KeyCode::ReturnKey => {
                    if let Some(index) = self.hover {
                        self.pick(cx, index);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        for (index, (_, row)) in self.rows.iter().take(self.entries.len()).enumerate() {
            if self.hover == Some(index) {
                // Lit before the row draws over it, at the row's place.
                let top = cx.turtle().pos();
                let width = cx.turtle().rect().size.x;
                self.draw_hover.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(top.x + self.hover_inset, top.y),
                        size: dvec2(width - self.hover_inset * 2.0, self.row_height),
                    },
                );
            }
            row.draw_all(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
        cx.add_nav_stop(self.area, NavRole::DropDown, Inset::default());
        DrawStep::done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_is_picked_like_an_item_and_marks_the_chosen_one() {
        let high = MenuEntry::Choice(live_id!(high), "High".into(), true);
        assert_eq!(high.id(), Some(live_id!(high)));
        assert_eq!(high.template(), live_id!(choice));
        assert_eq!(high.mark(), "✓");
        let fast = MenuEntry::Choice(live_id!(fast), "Fast".into(), false);
        assert_eq!(fast.mark(), "", "only the chosen row is marked");
    }
}
