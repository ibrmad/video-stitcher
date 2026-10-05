//! A two-column table of fixed rows (the Keyboard shortcuts sheet's keys
//! and what they do). Rows come from the `row` template; the App fills its
//! `keys` and `does` labels (`set_rows`).

use std::collections::HashMap;

use makepad_widgets::widget_tree::CxWidgetExt;
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoKeyTableBase = #(RecoKeyTable::register_widget(vm))
    mod.widgets.RecoKeyTable = set_type_default() do mod.widgets.RecoKeyTableBase{
        width: Fill height: Fit flow: Down
        row := RecoRow{
            RecoLabelCell{keys := RecoStrong{text: ""}}
            does := RecoText{width: Fill text: ""}
        }
    }
}

/// Rows of keys and what they do.
#[derive(Script, Widget)]
pub struct RecoKeyTable {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    /// The row template, by name.
    #[rust]
    templates: HashMap<LiveId, ScriptObjectRef>,
    /// One widget per row, made from the template.
    #[rust]
    items: Vec<WidgetRef>,
    #[rust]
    rows: Vec<(String, String)>,
    #[redraw]
    #[rust]
    area: Area,
}

impl ScriptHook for RecoKeyTable {
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

impl RecoKeyTable {
    /// Show these rows: (keys, what they do).
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
        for (item, (keys, does)) in self.items.iter().zip(&rows) {
            item.label(cx, ids!(keys)).set_text(cx, keys);
            item.label(cx, ids!(does)).set_text(cx, does);
        }
        self.rows = rows;
        self.area.redraw(cx);
    }
}

impl Widget for RecoKeyTable {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        for item in self.items.iter().take(self.rows.len()) {
            item.draw_all(cx, scope);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
