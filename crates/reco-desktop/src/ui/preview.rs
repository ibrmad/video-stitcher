//! The live preview: draws the frame the worker rendered, letterboxed to
//! the preview aspect, and turns drags, the wheel and keys into commands.
//!
//! Zero-copy frames arrive as a ring of raw `MTLTexture` pointers, adopted
//! once into Makepad textures; a shown slot goes back to the worker only
//! after `RETIRE_BEATS` display beats (Makepad has no fence with the
//! worker's queue). Readback frames arrive as CPU pixels.

use std::sync::mpsc::SyncSender;
use std::time::Instant;

use makepad_widgets::*;
use reco_app::preview::slots::{Retirement, RING_SLOTS};
use reco_app::preview::view::{fit, render_size, PreviewAspect};
use reco_app::preview::worker::{PreviewCommand, PreviewEvent};

use crate::keys::{command_for_key, repeats, KeyCommand};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    set_type_default() do #(DrawPreview::script_shader(vm)){
        ..mod.draw.DrawQuad
        tex: texture_2d(float)
        pixel: fn() {
            return self.tex.sample(self.pos)
        }
    }

    mod.widgets.RecoPreviewBase = #(RecoPreview::register_widget(vm))
    mod.widgets.RecoPreview = set_type_default() do mod.widgets.RecoPreviewBase{
        width: Fill
        height: Fill
    }
}

/// Samples the frame texture.
#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawPreview {
    #[deref]
    draw_super: DrawQuad,
}

/// What the preview asks of the App.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum PreviewAction {
    /// F or F11: toggle fullscreen.
    ToggleFullscreen,
    #[default]
    None,
}

/// The App passes this in the scope's props with each key: while a sheet
/// is open the preview leaves keys alone (DESIGN.md Rule 9).
pub struct SheetOpen(pub bool);

/// Take a ring texture by its raw `MTLTexture` pointer (retained, no copy).
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
fn adopt(
    cx: &mut Cx,
    texture: &Texture,
    raw: usize,
    width: u32,
    height: u32,
) -> Result<(), String> {
    use makepad_widgets::makepad_platform::makepad_objc_sys::runtime::ObjcId;
    texture.adopt_metal_bgra(cx, raw as ObjcId, width as usize, height as usize)
}

/// Off Apple platforms there is no ring to adopt: the worker reads frames
/// back there, because the UI has no Metal device to match.
#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "tvos")))]
fn adopt(
    _cx: &mut Cx,
    _texture: &Texture,
    _raw: usize,
    _width: u32,
    _height: u32,
) -> Result<(), String> {
    Err("zero-copy frames need Metal".into())
}

/// Degrees of FOV per point of wheel scroll. reco-gui zooms by −dy / 40
/// with macOS's sign (positive away); Makepad's `scroll.y` is the negated
/// delta, so scrolling away (negative here) zooms in.
const WHEEL_DEG_PER_POINT: f64 = 1.0 / 40.0;

/// The live preview widget.
#[derive(Script, ScriptHook, Widget)]
pub struct RecoPreview {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    /// The drawn frame, letterboxed: what the remote snapshot reports (no
    /// rect until a frame is shown).
    #[area]
    #[live]
    draw_frame: DrawPreview,
    #[visible]
    #[live(true)]
    visible: bool,
    /// The whole widget: what redraws (the frame quad has no area until
    /// its first draw, so redrawing it alone would never show a first
    /// frame) and where input hits.
    #[redraw]
    #[rust]
    area: Area,
    #[rust]
    commands: Option<SyncSender<PreviewCommand>>,
    #[rust]
    outbox: Vec<PreviewCommand>,
    #[rust]
    pan: (f64, f64),
    #[rust]
    zoom: f64,
    #[rust]
    ring: Vec<Texture>,
    #[rust]
    generation: u64,
    #[rust]
    retirement: Retirement,
    #[rust]
    pixels: Option<Texture>,
    #[rust]
    frame_size: Option<(u32, u32)>,
    #[rust]
    showing_pixels: bool,
    #[rust]
    aspect: PreviewAspect,
    #[rust]
    sent_size: Option<(u32, u32)>,
    #[rust]
    drag_from: Option<DVec2>,
    #[rust]
    beat: NextFrame,
    /// Playback keys do nothing (while exporting).
    #[rust]
    transport_locked: bool,
}

impl RecoPreview {
    /// Lock or unlock the playback keys (Space, the arrows' seeks).
    pub fn lock_transport(&mut self, locked: bool) {
        self.transport_locked = locked;
    }

    /// Start sending commands to a worker.
    pub fn attach(&mut self, cx: &mut Cx, commands: SyncSender<PreviewCommand>) {
        self.commands = Some(commands);
        self.sent_size = None;
        self.area.redraw(cx);
    }

    /// Letterbox to `aspect` (the render target follows on the next draw).
    pub fn set_aspect(&mut self, cx: &mut Cx, aspect: PreviewAspect) {
        self.aspect = aspect;
        self.area.redraw(cx);
    }

    /// Take the frame events; hand every other event back to the App.
    pub fn on_event(&mut self, cx: &mut Cx, event: PreviewEvent) -> Option<PreviewEvent> {
        match event {
            PreviewEvent::Ring {
                generation,
                width,
                height,
                textures,
                shown,
            } => {
                while self.ring.len() < RING_SLOTS {
                    self.ring.push(Texture::new_video_external(cx));
                }
                for (texture, raw) in self.ring.iter().zip(&textures) {
                    if let Err(e) = adopt(cx, texture, *raw, width, height) {
                        error!("preview: could not adopt a ring texture: {e}");
                    }
                }
                self.generation = generation;
                self.retirement.clear();
                self.retirement.show(shown, Instant::now());
                self.frame_size = Some((width, height));
                self.showing_pixels = false;
                self.queue(PreviewCommand::Adopted { generation });
                self.flush(cx);
                self.area.redraw(cx);
                None
            }
            PreviewEvent::Frame { generation, slot } => {
                if generation == self.generation {
                    self.retirement.show(slot, Instant::now());
                    self.area.redraw(cx);
                    self.request_beat(cx);
                } else {
                    // A slot of an older ring: the worker already let it go.
                }
                None
            }
            PreviewEvent::Pixels {
                width,
                height,
                data,
            } => {
                let texture = self.pixels.get_or_insert_with(|| {
                    Texture::new_with_format(
                        cx,
                        TextureFormat::VecBGRAu8_32 {
                            width: width as usize,
                            height: height as usize,
                            data: None,
                            updated: TextureUpdated::Full,
                        },
                    )
                });
                texture.set_data_u32(cx, width as usize, height as usize, data);
                self.frame_size = Some((width, height));
                self.showing_pixels = true;
                self.area.redraw(cx);
                None
            }
            other => Some(other),
        }
    }

    fn queue(&mut self, command: PreviewCommand) {
        self.outbox.push(command);
    }

    /// Send what is queued (coalesced pan and zoom first); keep anything
    /// the full queue refused for the next beat.
    fn flush(&mut self, cx: &mut Cx) {
        let Some(tx) = self.commands.clone() else {
            return;
        };
        if self.pan != (0.0, 0.0) {
            let (dx, dy) = std::mem::take(&mut self.pan);
            self.outbox.insert(
                0,
                PreviewCommand::Pan {
                    dx: dx as f32,
                    dy: dy as f32,
                },
            );
        }
        if self.zoom != 0.0 {
            let degrees = std::mem::take(&mut self.zoom) as f32;
            self.outbox.insert(0, PreviewCommand::Zoom { degrees });
        }
        let mut kept = Vec::new();
        for command in self.outbox.drain(..) {
            if let Err(std::sync::mpsc::TrySendError::Full(c)) = tx.try_send(command) {
                kept.push(c);
            }
        }
        self.outbox = kept;
        if !self.outbox.is_empty() {
            self.request_beat(cx);
        }
    }

    fn request_beat(&mut self, cx: &mut Cx) {
        if !cx.next_frame_is_pending(self.beat) {
            self.beat = cx.new_next_frame();
        }
    }

    fn key(&mut self, cx: &mut Cx, command: KeyCommand) {
        match command {
            KeyCommand::Pan { dx, dy } => {
                self.pan = (self.pan.0 + dx as f64, self.pan.1 + dy as f64)
            }
            KeyCommand::Zoom { degrees } => self.zoom += degrees as f64,
            KeyCommand::ResetView => self.queue(PreviewCommand::ResetView),
            KeyCommand::TogglePlay | KeyCommand::SeekBy { .. } if self.transport_locked => {}
            KeyCommand::TogglePlay => self.queue(PreviewCommand::TogglePlay),
            KeyCommand::SeekBy { seconds } => self.queue(PreviewCommand::SeekBy { seconds }),
            KeyCommand::Fullscreen => {
                cx.widget_action(self.widget_uid(), PreviewAction::ToggleFullscreen)
            }
        }
        self.flush(cx);
    }
}

impl Widget for RecoPreview {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.beat.is_event(event).is_some() {
            for slot in self.retirement.beat(Instant::now()) {
                self.queue(PreviewCommand::Release {
                    generation: self.generation,
                    slot,
                });
            }
            self.flush(cx);
            if !self.retirement.is_idle() {
                self.request_beat(cx);
            }
        }
        if self.commands.is_none() || !self.visible {
            return;
        }
        // Keys reach the preview when nothing else holds focus (a focused
        // button keeps Space and arrows for itself) and no sheet is open.
        let sheet_open = scope.props.get::<SheetOpen>().is_some_and(|open| open.0);
        if let (Event::KeyDown(ke), false) = (event, sheet_open) {
            let focus = cx.key_focus();
            if focus.is_empty() || cx.has_key_focus(self.area) {
                if let Some(command) = command_for_key(ke.key_code, &ke.modifiers) {
                    if !ke.is_repeat || repeats(command) {
                        self.key(cx, command);
                    }
                }
            }
        }
        match event.hits(cx, self.area) {
            Hit::FingerDown(fe) => {
                cx.set_key_focus(self.area);
                self.drag_from = Some(fe.abs);
            }
            Hit::FingerMove(fe) => {
                if let Some(from) = self.drag_from.replace(fe.abs) {
                    let d = fe.abs - from;
                    self.pan = (self.pan.0 + d.x, self.pan.1 + d.y);
                    self.flush(cx);
                }
            }
            Hit::FingerUp(_) => self.drag_from = None,
            Hit::FingerScroll(se) => {
                self.zoom += se.scroll.y * WHEEL_DEG_PER_POINT;
                self.flush(cx);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.visible {
            return DrawStep::done();
        }
        cx.begin_turtle(walk, Layout::default());
        let rect = cx.turtle().rect();
        let (x, y, w, h) = fit((rect.size.x, rect.size.y), self.aspect.ratio());
        let wanted = render_size(w, h, cx.current_dpi_factor());
        if self.commands.is_some() && self.sent_size != Some(wanted) && w > 0.0 && h > 0.0 {
            self.sent_size = Some(wanted);
            self.queue(PreviewCommand::Resize {
                width: wanted.0,
                height: wanted.1,
            });
            self.request_beat(cx);
        }
        let texture = if self.showing_pixels {
            self.pixels.as_ref()
        } else {
            self.retirement.shown().and_then(|slot| self.ring.get(slot))
        };
        if let (Some(texture), Some((tw, th))) = (texture, self.frame_size) {
            // Letterbox the frame itself too, so a target that lags a
            // resize never stretches.
            let (fx, fy, fw, fh) = fit((w, h), Some(tw as f64 / th.max(1) as f64));
            self.draw_frame.draw_vars.set_texture(0, texture);
            self.draw_frame.draw_abs(
                cx,
                Rect {
                    pos: dvec2(rect.pos.x + x + fx, rect.pos.y + y + fy),
                    size: dvec2(fw, fh),
                },
            );
        }
        cx.walk_turtle(Walk::fixed(rect.size.x, rect.size.y));
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
