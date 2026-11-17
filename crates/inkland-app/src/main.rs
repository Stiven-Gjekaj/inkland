//! The inkland desktop app.
//!
//! Usage: `inkland-app [CONFIG.json]`.
//!
//! The app edits a config with the commands of `inkland::History`, makes the
//! world again on a worker thread, and shows the SVG of the selected
//! variant. A second worker thread renders the SVG for the map view.

mod ui {
    #![allow(unsafe_code, clippy::all)]
    slint::include_modules!();
}

use inkland::{Config, World};
use inkland_app::model::{Editor, error_text};
use inkland_app::view::{View, distance};
use inkland_app::worker::Worker;
use inkland_core::config::LayerName;
use inkland_render::page::Page;
use inkland_render::png::{FontDb, Scene};
use slint::{ComponentHandle, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;
use ui::{Actions, AppState, AppWindow, LayerRow};
use ui::{PinRow, Tool};

/// The color around the page in the map view.
const DESK: [u8; 3] = [0x8a, 0x8f, 0x94];
/// A click this near to a pin (in screen pixels) selects the pin.
const PIN_HIT_PX: f64 = 9.0;
/// The kinds of the "New pin kind" list, in its order.
const PIN_KINDS: [&str; 5] = ["village", "town", "city", "fortress", "ruin"];
/// The config of a new map, when no file is given.
const NEW_CONFIG: &str = r#"{ "version": 1, "seed": 1, "frame": { "width": 400, "height": 300 },
  "world": { "cells": 8000, "template": "continents" } }"#;

/// A generated map: the world, the parsed SVG of the variant, and its
/// report checks.
struct Generated {
    world: World,
    config: Config,
    scene: Scene,
    warnings: usize,
}

/// A result of a worker thread.
enum Msg {
    Generated {
        rev: u64,
        result: Result<Box<Generated>, String>,
    },
    Rastered {
        rev: u64,
        view: View,
        size: [f64; 2],
        pixels: SharedPixelBuffer<Rgba8Pixel>,
    },
}

struct GenJob {
    rev: u64,
    config: Config,
    variant: String,
}

struct RasterJob {
    rev: u64,
    scene: Arc<Scene>,
    view: View,
    /// The size of the viewport in logical pixels.
    size: [f64; 2],
    /// Physical pixels for each logical pixel.
    scale_factor: f64,
    /// SVG pixels (CSS pixels) for each mm of the page.
    px_per_mm: f64,
}

/// The map that the view shows now.
struct Shown {
    world: Arc<World>,
    config: Config,
    scene: Arc<Scene>,
    px_per_mm: f64,
}

/// What a drag of the pointer does.
enum Drag {
    None,
    Pan { last: [f64; 2] },
    Pin { id: String, start: [f64; 2] },
}

struct State {
    editor: Editor,
    path: Option<PathBuf>,
    view: View,
    viewport: [f64; 2],
    /// Fit the page into the view when the next map comes.
    fit_next: bool,
    shown: Option<Shown>,
    /// The view and the logical size of the shown image.
    rendered: Option<(View, [f64; 2])>,
    /// The number of the latest generation request, and of the latest
    /// result.
    rev: u64,
    done_rev: u64,
    drag: Drag,
}

struct Controller {
    ui: slint::Weak<AppWindow>,
    state: RefCell<State>,
    generator: Worker<GenJob>,
    rasterizer: Worker<RasterJob>,
    results: Receiver<Msg>,
    idle: slint::Timer,
}

fn s(text: impl AsRef<str>) -> SharedString {
    SharedString::from(text.as_ref())
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

/// Send a result to the event loop and ask it to read the results.
fn post(tx: &Sender<Msg>, ui: &slint::Weak<AppWindow>, msg: Msg) {
    if tx.send(msg).is_ok() {
        let _ = ui.upgrade_in_event_loop(|ui| ui.global::<Actions>().invoke_poll());
    }
}

/// Generate the world and the variant, and parse its SVG.
fn generate(job: &GenJob, fonts: &mut Option<(Vec<String>, FontDb)>) -> Result<Generated, String> {
    let (world, out) = inkland::generate_with_world(&job.config, Some(&job.variant))
        .map_err(|e| e.first().map(error_text).unwrap_or_default())?;
    let v = out
        .variants
        .into_iter()
        .next()
        .ok_or_else(|| "the variant has no output".to_owned())?;
    let dirs = job.config.font_dirs.clone();
    if fonts.as_ref().is_none_or(|(d, _)| *d != dirs) {
        let paths: Vec<&Path> = dirs.iter().map(Path::new).collect();
        *fonts = Some((dirs.clone(), inkland_render::png::font_db(&paths)));
    }
    let db = &fonts.as_ref().expect("the fonts are loaded").1;
    let families: Vec<&str> = job
        .config
        .render
        .fonts
        .families()
        .iter()
        .map(|(_, f)| *f)
        .collect();
    let scene = Scene::parse(&v.svg, db, &families)?;
    let warnings = v.report["warnings"].as_array().map_or(0, Vec::len);
    Ok(Generated {
        world,
        config: job.config.clone(),
        scene,
        warnings,
    })
}

/// Render the visible part of the page for the view.
fn rasterize(job: &RasterJob) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
    let sf = job.scale_factor;
    let w = (job.size[0] * sf).round().max(1.0) as u32;
    let h = (job.size[1] * sf).round().max(1.0) as u32;
    let scale = job.view.zoom * sf / job.px_per_mm;
    let offset = [job.view.pan[0] * sf, job.view.pan[1] * sf];
    let rgba = job.scene.render_rgba(
        w,
        h,
        scale as f32,
        [offset[0] as f32, offset[1] as f32],
        DESK,
    )?;
    Ok(SharedPixelBuffer::clone_from_slice(&rgba, w, h))
}

fn layer_names() -> Vec<(LayerName, String)> {
    use LayerName::*;
    [
        Coast, Relief, Rivers, Regions, Roads, Features, Places, Labels,
    ]
    .into_iter()
    .map(|l| {
        let name = serde_json::to_value(l)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        (l, name)
    })
    .collect()
}

impl Controller {
    fn ui(&self) -> AppWindow {
        self.ui.upgrade().expect("the window exists")
    }

    /// The page of the shown variant, from the current config.
    fn page(st: &State) -> Option<Page> {
        let c = st.editor.config();
        c.variants.get(st.editor.variant).map(|v| Page::new(c, v))
    }

    fn message(&self, text: &str, error: bool) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        g.set_message(s(text));
        g.set_message_is_error(error);
    }

    /// Ask the worker for a new map of the current config.
    fn regenerate(&self) {
        let mut st = self.state.borrow_mut();
        st.rev += 1;
        let job = GenJob {
            rev: st.rev,
            config: st.editor.config().clone(),
            variant: st.editor.variant_name(),
        };
        drop(st);
        self.ui().global::<AppState>().set_generating(true);
        self.generator.submit(job);
    }

    /// Ask the worker for a sharp image of the current view.
    fn rerender(&self) {
        let st = self.state.borrow();
        let Some(shown) = &st.shown else { return };
        if st.viewport[0] < 1.0 || st.viewport[1] < 1.0 {
            return;
        }
        let scale_factor = f64::from(self.ui().window().scale_factor());
        self.rasterizer.submit(RasterJob {
            rev: st.done_rev,
            scene: Arc::clone(&shown.scene),
            view: st.view,
            size: st.viewport,
            scale_factor,
            px_per_mm: shown.px_per_mm,
        });
    }

    /// Render a sharp image when the view does not change for a short time.
    fn rerender_soon(self: &Rc<Self>) {
        let me = Rc::clone(self);
        self.idle.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(120),
            move || me.rerender(),
        );
    }

    /// Put the shown image, the pins, and the brush outline where the view
    /// has them.
    fn update_view(&self) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let st = self.state.borrow();
        if let Some((rv, size)) = &st.rendered {
            let [x, y, w, h] = st.view.placement(rv, *size);
            g.set_img_x(x as f32);
            g.set_img_y(y as f32);
            g.set_img_w(w as f32);
            g.set_img_h(h as f32);
        }
        let Some(page) = Self::page(&st) else { return };
        let pins: Vec<PinRow> = st
            .editor
            .points()
            .into_iter()
            .map(|p| {
                let [x, y] = st.view.world_to_screen(&page, p.at);
                PinRow {
                    label: s(match &p.name {
                        Some(n) => format!("{n}  ({}, {})", p.id, p.kind),
                        None => format!("{}  ({})", p.id, p.kind),
                    }),
                    id: s(&p.id),
                    x: x as f32,
                    y: y as f32,
                }
            })
            .collect();
        g.set_pins(model(pins));
    }

    /// Show the state of the editor: pins, selection, layers, variants,
    /// undo and redo.
    fn update_editor(&self) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let st = self.state.borrow();
        let e = &st.editor;
        let c = e.config();
        let selected = e.selected.clone().unwrap_or_default();
        let name = e
            .points()
            .into_iter()
            .find(|p| p.id == selected)
            .and_then(|p| p.name)
            .unwrap_or_default();
        g.set_selected_id(s(&selected));
        g.set_selected_name(s(name));
        g.set_layers(model(
            layer_names()
                .into_iter()
                .map(|(l, name)| LayerRow {
                    name: s(name),
                    on: c.render.layers.contains(&l),
                })
                .collect(),
        ));
        g.set_variants(model(c.variants.iter().map(|v| s(&v.name)).collect()));
        g.set_variant_index(e.variant as i32);
        let file = st
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("new map".to_owned(), |f| f.to_string_lossy().into_owned());
        let dirty = if e.can_undo() { " (edited)" } else { "" };
        ui.set_window_title(s(format!("inkland - {file}{dirty}")));
        drop(st);
        self.update_view();
    }

    /// After an edit: show it and make the map again.
    fn edited(&self, result: Result<(), String>, what: &str) {
        match result {
            Ok(()) => self.message(what, false),
            Err(e) => {
                self.message(&format!("{what}: refused: {e}"), true);
                return;
            }
        }
        self.update_editor();
        self.regenerate();
    }

    /// Read the results of the worker threads.
    fn poll(self: &Rc<Self>) {
        while let Ok(msg) = self.results.try_recv() {
            match msg {
                Msg::Generated { rev, result } => self.generated(rev, result),
                Msg::Rastered {
                    rev,
                    view,
                    size,
                    pixels,
                } => {
                    let mut st = self.state.borrow_mut();
                    if rev != st.done_rev {
                        continue;
                    }
                    st.rendered = Some((view, size));
                    drop(st);
                    let ui = self.ui();
                    let g = ui.global::<AppState>();
                    g.set_map_image(slint::Image::from_rgba8(pixels));
                    g.set_has_map(true);
                    self.update_view();
                }
            }
        }
    }

    fn generated(self: &Rc<Self>, rev: u64, result: Result<Box<Generated>, String>) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let mut st = self.state.borrow_mut();
        if rev == st.rev {
            g.set_generating(false);
        }
        match result {
            Err(e) => {
                drop(st);
                self.message(&format!("the map cannot be made: {e}"), true);
            }
            Ok(made) => {
                if rev < st.done_rev {
                    return;
                }
                st.done_rev = rev;
                let Generated {
                    world,
                    config,
                    scene,
                    warnings,
                } = *made;
                let page = config
                    .variants
                    .iter()
                    .find(|v| v.name == st.editor.variant_name())
                    .map(|v| Page::new(&config, v));
                let px_per_mm = page.map_or(1.0, |p| f64::from(scene.size()[0]) / p.w_mm);
                let missing = scene.missing_fonts.clone();
                st.shown = Some(Shown {
                    world: Arc::new(world),
                    config,
                    scene: Arc::new(scene),
                    px_per_mm,
                });
                if st.fit_next
                    && st.viewport[0] > 0.0
                    && let Some(p) = page
                {
                    st.view = View::fit(st.viewport, [p.w_mm, p.h_mm]);
                    st.fit_next = false;
                }
                drop(st);
                if !missing.is_empty() {
                    self.message(
                        &format!(
                            "the preview uses a fallback for the fonts {}",
                            missing.join(", ")
                        ),
                        false,
                    );
                } else if warnings > 0 {
                    self.message(&format!("the report has {warnings} warnings"), false);
                }
                self.update_view();
                self.rerender();
            }
        }
    }

    /// The pin under a screen point.
    fn pin_at(st: &State, p: [f64; 2]) -> Option<String> {
        let page = Self::page(st)?;
        st.editor
            .points()
            .into_iter()
            .map(|pin| (distance(st.view.world_to_screen(&page, pin.at), p), pin.id))
            .filter(|(d, _)| *d <= PIN_HIT_PX)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// The world point under a screen point, if it is in the frame.
    fn world_at(st: &State, p: [f64; 2]) -> Option<[f64; 2]> {
        let page = Self::page(st)?;
        let w = st.view.screen_to_world(&page, p);
        let f = &st.editor.config().frame;
        (w[0] >= 0.0 && w[1] >= 0.0 && w[0] <= f.width && w[1] <= f.height).then_some(w)
    }

    fn pointer_down(self: &Rc<Self>, p: [f64; 2], pan: bool) {
        let mut st = self.state.borrow_mut();
        if pan {
            st.drag = Drag::Pan { last: p };
            return;
        }
        let tool = self.ui().global::<AppState>().get_tool();
        match tool {
            Tool::Select => match Self::pin_at(&st, p) {
                Some(id) => {
                    st.editor.selected = Some(id.clone());
                    st.drag = Drag::Pin { id, start: p };
                    drop(st);
                    self.update_editor();
                }
                None => st.drag = Drag::Pan { last: p },
            },
            Tool::AddPin => {
                let Some(at) = Self::world_at(&st, p) else {
                    drop(st);
                    self.message("a pin must be inside the frame", true);
                    return;
                };
                let kind = PIN_KINDS
                    .get(self.ui().global::<AppState>().get_new_pin_kind() as usize)
                    .copied()
                    .unwrap_or("village");
                let r = st.editor.add_pin(at, kind);
                drop(st);
                let what = match &r {
                    Ok(id) => format!("add the {kind} pin {id}"),
                    Err(_) => format!("add a {kind} pin"),
                };
                self.edited(r.map(|_| ()), &what);
            }
        }
    }

    fn pointer_move(self: &Rc<Self>, p: [f64; 2]) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let mut st = self.state.borrow_mut();
        // The status bar.
        if let Some(page) = Self::page(&st) {
            let w = st.view.screen_to_world(&page, p);
            let unit = &st.editor.config().units.name;
            g.set_status_position(s(format!("x {:.2}, y {:.2} {unit}", w[0], w[1])));
            let info = st
                .shown
                .as_ref()
                .and_then(|m| inkland::query_at(&m.world, &m.config, w).ok());
            g.set_status_cell(s(match info {
                Some(q) => format!(
                    "height {} ({}), {}",
                    q["height"],
                    if q["land"] == true { "land" } else { "water" },
                    q["biome"].as_str().unwrap_or("?")
                ),
                None => "outside the frame".to_owned(),
            }));
        }
        match &mut st.drag {
            Drag::None => {}
            Drag::Pan { last } => {
                let d = [p[0] - last[0], p[1] - last[1]];
                *last = p;
                st.view.pan_by(d);
                drop(st);
                self.update_view();
                self.rerender_soon();
            }
            Drag::Pin { start, .. } => {
                let moved = distance(*start, p) > 3.0;
                g.set_dragging_pin(moved);
                g.set_drag_x(p[0] as f32);
                g.set_drag_y(p[1] as f32);
            }
        }
    }

    fn pointer_up(self: &Rc<Self>, p: [f64; 2]) {
        let drag = std::mem::replace(&mut self.state.borrow_mut().drag, Drag::None);
        self.ui().global::<AppState>().set_dragging_pin(false);
        if let Drag::Pin { id, start } = drag
            && distance(start, p) > 3.0
        {
            let at = Self::world_at(&self.state.borrow(), p);
            let r = match at {
                Some(at) => self.state.borrow_mut().editor.move_pin(&id, at),
                None => Err("a pin must be inside the frame".to_owned()),
            };
            self.edited(r, &format!("move the pin {id}"));
        }
    }

    fn scroll(self: &Rc<Self>, p: [f64; 2], dy: f64) {
        let factor = libm::exp(dy / 300.0).clamp(0.5, 2.0);
        self.state.borrow_mut().view.zoom_at(p, factor);
        self.update_view();
        self.rerender_soon();
    }

    fn fit(self: &Rc<Self>) {
        let mut st = self.state.borrow_mut();
        if let Some(p) = Self::page(&st) {
            st.view = View::fit(st.viewport, [p.w_mm, p.h_mm]);
        }
        drop(st);
        self.update_view();
        self.rerender();
    }

    /// Start to edit a new config.
    fn load(self: &Rc<Self>, config: Config, path: Option<PathBuf>) {
        let mut st = self.state.borrow_mut();
        st.editor = Editor::new(config);
        st.path = path;
        st.fit_next = true;
        drop(st);
        self.update_editor();
        self.regenerate();
    }

    fn open_path(self: &Rc<Self>, path: &Path) {
        let config = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))
            .and_then(|text| inkland::parse_config(&text, &[]).map_err(|e| error_text(&e)));
        match config {
            Ok(c) => {
                self.load(c, Some(path.to_owned()));
                self.message(&format!("opened {}", path.display()), false);
            }
            Err(e) => self.message(&e, true),
        }
    }

    fn write(&self, path: &Path, text: &str, what: &str) -> bool {
        match std::fs::write(path, text) {
            Ok(()) => {
                self.message(&format!("saved the {what} to {}", path.display()), false);
                true
            }
            Err(e) => {
                self.message(&format!("cannot write {}: {e}", path.display()), true);
                false
            }
        }
    }

    fn save_as(self: &Rc<Self>) {
        let dialog = rfd::FileDialog::new()
            .add_filter("inkland config", &["json"])
            .set_file_name("config.json");
        if let Some(path) = dialog.save_file() {
            let text = self.state.borrow().editor.config_json();
            if self.write(&path, &text, "config") {
                self.state.borrow_mut().path = Some(path);
                self.update_editor();
            }
        }
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let (tx, results) = channel();

    let (gen_tx, gen_ui) = (tx.clone(), ui.as_weak());
    let mut fonts: Option<(Vec<String>, FontDb)> = None;
    let generator = Worker::spawn("inkland-generate", move |job: GenJob| {
        let result = generate(&job, &mut fonts).map(Box::new);
        post(
            &gen_tx,
            &gen_ui,
            Msg::Generated {
                rev: job.rev,
                result,
            },
        );
    });
    let (raster_tx, raster_ui) = (tx, ui.as_weak());
    let rasterizer = Worker::spawn("inkland-render", move |job: RasterJob| {
        if let Ok(pixels) = rasterize(&job) {
            post(
                &raster_tx,
                &raster_ui,
                Msg::Rastered {
                    rev: job.rev,
                    view: job.view,
                    size: job.size,
                    pixels,
                },
            );
        }
    });

    let app = Rc::new(Controller {
        ui: ui.as_weak(),
        state: RefCell::new(State {
            editor: Editor::new(
                inkland::parse_config(NEW_CONFIG, &[]).expect("the new config is valid"),
            ),
            path: None,
            view: View::default(),
            viewport: [0.0, 0.0],
            fit_next: true,
            shown: None,
            rendered: None,
            rev: 0,
            done_rev: 0,
            drag: Drag::None,
        }),
        generator,
        rasterizer,
        results,
        idle: slint::Timer::default(),
    });

    let a = ui.global::<Actions>();
    let pt = |x: f32, y: f32| [f64::from(x), f64::from(y)];
    let c = Rc::clone(&app);
    a.on_poll(move || c.poll());
    let c = Rc::clone(&app);
    a.on_pointer_down(move |x, y, pan| c.pointer_down(pt(x, y), pan));
    let c = Rc::clone(&app);
    a.on_pointer_move(move |x, y| c.pointer_move(pt(x, y)));
    let c = Rc::clone(&app);
    a.on_pointer_up(move |x, y| c.pointer_up(pt(x, y)));
    let c = Rc::clone(&app);
    a.on_scroll(move |x, y, dy| c.scroll(pt(x, y), f64::from(dy)));
    let c = Rc::clone(&app);
    a.on_viewport_changed(move |w, h| {
        let mut st = c.state.borrow_mut();
        let old = st.viewport;
        st.viewport = pt(w, h);
        let first = old[0] < 1.0 && st.shown.is_some() && st.fit_next;
        drop(st);
        if first {
            c.fit();
        } else {
            c.update_view();
            c.rerender_soon();
        }
    });
    let c = Rc::clone(&app);
    a.on_fit(move || c.fit());
    let c = Rc::clone(&app);
    a.on_select_variant(move |i| {
        let mut st = c.state.borrow_mut();
        st.editor.variant = usize::try_from(i).unwrap_or(0);
        st.fit_next = true;
        drop(st);
        c.update_editor();
        c.regenerate();
    });
    let c = Rc::clone(&app);
    a.on_set_layer(move |name, on| {
        let layer = layer_names()
            .into_iter()
            .find(|(_, n)| n.as_str() == name.as_str())
            .map(|(l, _)| l);
        if let Some(layer) = layer {
            let r = c.state.borrow_mut().editor.set_layer(layer, on);
            let what = if on { "show" } else { "hide" };
            c.edited(r, &format!("{what} the layer {name}"));
        }
    });
    let c = Rc::clone(&app);
    a.on_select_pin(move |id| {
        c.state.borrow_mut().editor.selected = Some(id.to_string());
        c.update_editor();
    });
    let c = Rc::clone(&app);
    a.on_rename_pin(move |id, name| {
        let r = c.state.borrow_mut().editor.rename_pin(&id, &name);
        c.edited(r, &format!("rename the pin {id}"));
    });
    let c = Rc::clone(&app);
    a.on_remove_pin(move |id| {
        let r = c.state.borrow_mut().editor.remove_pin(&id);
        c.edited(r, &format!("remove the pin {id}"));
    });
    let c = Rc::clone(&app);
    a.on_open(move || {
        let dialog = rfd::FileDialog::new().add_filter("inkland config", &["json"]);
        if let Some(path) = dialog.pick_file() {
            c.open_path(&path);
        }
    });
    let c = Rc::clone(&app);
    a.on_save(move || {
        let path = c.state.borrow().path.clone();
        match path {
            Some(path) => {
                let text = c.state.borrow().editor.config_json();
                c.write(&path, &text, "config");
            }
            None => c.save_as(),
        }
    });
    let c = Rc::clone(&app);
    a.on_save_as(move || c.save_as());
    a.on_quit(|| {
        let _ = slint::quit_event_loop();
    });

    app.update_editor();
    match std::env::args_os().nth(1) {
        Some(path) => app.open_path(Path::new(&path)),
        None => {
            app.regenerate();
            app.message("a new map; use File > Open to open a config", false);
        }
    }
    ui.run()
}
