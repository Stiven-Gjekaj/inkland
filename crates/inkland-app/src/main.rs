//! The inkland desktop app.
//!
//! Usage: `inkland-app [CONFIG.json]`.
//!
//! With the environment variable `INKLAND_APP_SNAPSHOT=FILE.png`, the app
//! writes an image of its window to the file when the first map shows, and
//! stops. This is a check that the app starts and draws a map. With
//! `INKLAND_APP_SNAPSHOT_OPEN=TAB/TOOL,...` the image shows a tab of the
//! menu and some dialogs (see `inkland_app::snapshot`).
//!
//! The app edits a config with the commands of `inkland::History`, makes the
//! world again on a worker thread, and shows the SVG of the selected
//! variant. A second worker thread renders the SVG for the map view.

mod ui {
    #![allow(unsafe_code, clippy::all)]
    slint::include_modules!();
}

use inkland::{Config, World};
use inkland_app::catalog;
use inkland_app::model::{Editor, error_text};
use inkland_app::options::option_text;
use inkland_app::pins::{Shape, ShapeKind, anchor_at, waypoint_at};
use inkland_app::variants::crop_from_drag;
use inkland_app::view::{View, distance};
use inkland_app::worker::Worker;
use inkland_render::page::Page;
use inkland_render::png::{FontDb, Scene};
use serde_json::Value;
use slint::{ComponentHandle, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;
use ui::CheckRow;
use ui::{
    Actions, AppState, AppWindow, Dialogs, LayerRow, Lists, Opts, TableRow, Theme, ToolRow, Ui,
};
use ui::{PinRow, Tool};

/// The color around the page in the map view.
const DESK: [u8; 3] = [0x8a, 0x8f, 0x94];
/// A click this near to a pin (in screen pixels) selects the pin.
const PIN_HIT_PX: f64 = 9.0;
/// The pin list of an editor id.
fn pin_list(list: &str) -> Option<inkland::PinList> {
    match list {
        "burgs" => Some(inkland::PinList::Points),
        "regions" => Some(inkland::PinList::Regions),
        "rivers" => Some(inkland::PinList::Rivers),
        "routes" => Some(inkland::PinList::Roads),
        _ => None,
    }
}

/// The kind of a new region: an index in `catalog::REGION_KINDS`.
fn region_kind(i: i32) -> &'static str {
    catalog::REGION_KINDS
        .get(usize::try_from(i).unwrap_or(0))
        .copied()
        .unwrap_or("hills")
}

/// The tools of the Tools tab that the app has now.
const IMPLEMENTED: &[&str] = &[
    "burgs",
    "heightmap",
    "add-burg",
    "print",
    "regions",
    "add-region",
    "rivers",
    "add-river",
    "routes",
    "add-route",
    "submap",
    "variants",
    "labels",
    "units",
    "states",
    "cultures",
    "religions",
    "provinces",
    "cells",
    "emblems",
    "diplomacy",
    "markets",
    "goods",
    "stability",
    "zones",
    "notes",
    "markers",
    "military",
];
/// The config of a new map, when no file is given.
const NEW_CONFIG: &str = r#"{ "version": 1, "seed": 1, "frame": { "width": 400, "height": 300 },
  "world": { "cells": 8000, "template": "continents" } }"#;

/// A generated map: the world, the parsed SVG of the variant, and its
/// report checks.
struct Generated {
    world: World,
    config: Config,
    scene: Scene,
    checks: Vec<(String, bool, String)>,
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
    Pan {
        last: [f64; 2],
    },
    Pin {
        id: String,
        start: [f64; 2],
    },
    Brush {
        last: [f64; 2],
    },
    /// A box, a circle, or a crop: from `start` (world) to the pointer.
    Shape {
        start: [f64; 2],
    },
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
    /// The points (world) of the shape, river, or road that the user draws.
    draft: Vec<[f64; 2]>,
    /// The pointer in the map view, if it is there.
    pointer: Option<[f64; 2]>,
}

struct Controller {
    ui: slint::Weak<AppWindow>,
    state: RefCell<State>,
    generator: Worker<GenJob>,
    rasterizer: Worker<RasterJob>,
    results: Receiver<Msg>,
    idle: slint::Timer,
    /// The file for the window image of `INKLAND_APP_SNAPSHOT`.
    snapshot: RefCell<Option<PathBuf>>,
}

fn s(text: impl AsRef<str>) -> SharedString {
    SharedString::from(text.as_ref())
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

/// The rows of an editor table.
fn table(rows: Vec<inkland_app::rows::Row>) -> ModelRc<TableRow> {
    model(
        rows.into_iter()
            .map(|r| TableRow {
                id: s(r.id),
                cells: model(r.cells.into_iter().map(s).collect()),
                pinned: r.pinned,
                color: r
                    .color
                    .as_deref()
                    .and_then(inkland_app::rows::hex_color)
                    .map_or(slint::Color::from_argb_u8(0, 0, 0, 0), |[r, g, b]| {
                        slint::Color::from_rgb_u8(r, g, b)
                    }),
            })
            .collect(),
    )
}

/// Send a result to the event loop and ask it to read the results.
fn post(tx: &Sender<Msg>, ui: &slint::Weak<AppWindow>, msg: Msg) {
    if tx.send(msg).is_ok() {
        let _ = ui.upgrade_in_event_loop(|ui| ui.global::<Actions>().invoke_poll());
    }
}

/// A new seed for New Map, from the clock: 1 to 999999999, as in FMG.
fn new_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    // Mix the bits, so that two clicks close in time give far seeds.
    let mut x = nanos as u64 ^ 0x9e37_79b9_7f4a_7c15;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x % 999_999_999 + 1
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
    let checks = v.report["checks"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|c| {
                    let detail = match &c["count"] {
                        Value::Number(n) if c["pass"] == false => format!(" ({n})"),
                        _ => String::new(),
                    };
                    (
                        c["name"].as_str().unwrap_or("?").to_owned(),
                        c["pass"] == true,
                        detail,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let warnings = v.report["warnings"].as_array().map_or(0, Vec::len);
    Ok(Generated {
        world,
        config: job.config.clone(),
        scene,
        checks,
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
        let radius = f64::from(g.get_brush_radius());
        g.set_brush_view_radius((radius * page.scale * st.view.zoom) as f32);
        self.update_marks(&st, &page);
    }

    /// The marks of the open editors and of the drawing over the map.
    fn update_marks(&self, st: &State, page: &Page) {
        use inkland_app::overlay::{self, Paths};
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let d = ui.global::<Dialogs>();
        let l = ui.global::<Lists>();
        let c = st.editor.config();
        let f = |p: [f64; 2]| st.view.world_to_screen(page, p);
        let mut all = Paths::default();
        let mut push = |p: Paths| {
            for (to, from) in [(&mut all.normal, p.normal), (&mut all.selected, p.selected)] {
                if !from.is_empty() {
                    if !to.is_empty() {
                        to.push(' ');
                    }
                    to.push_str(&from);
                }
            }
        };
        if d.get_regions() || g.get_tool() == Tool::Region {
            push(overlay::region_paths(c, &l.get_selected_region(), f));
        }
        if d.get_rivers() || g.get_tool() == Tool::River {
            push(overlay::river_paths(c, &l.get_selected_river(), f));
        }
        if d.get_routes() || g.get_tool() == Tool::Route {
            push(overlay::road_paths(c, &l.get_selected_route(), f));
        }
        if d.get_submap()
            && let Some(crop) = c.variants.get(st.editor.variant).and_then(|v| v.crop)
        {
            push(Paths {
                normal: String::new(),
                selected: overlay::rect(crop, &f),
            });
        }
        let draft: Vec<[f64; 2]> = st.draft.iter().map(|p| f(*p)).collect();
        let mut draft_path = String::new();
        match (&st.drag, st.pointer) {
            (Drag::Shape { start }, Some(now)) => {
                let a = f(*start);
                draft_path = if g.get_tool() == Tool::Region && g.get_region_shape() == 1 {
                    overlay::circle(a, distance(a, now))
                } else {
                    overlay::polyline(&[a, [now[0], a[1]], now, [a[0], now[1]]], true)
                };
            }
            _ if !draft.is_empty() => {
                let mut pts = draft.clone();
                if let Some(now) = st.pointer {
                    pts.push(now);
                }
                draft_path = overlay::polyline(&pts, false);
            }
            _ => {}
        }
        g.set_overlay_path(s(all.normal));
        g.set_highlight_path(s(all.selected));
        g.set_draft_path(s(draft_path));
        g.set_draft_points(model(
            draft
                .iter()
                .map(|p| PinRow {
                    id: s(""),
                    label: s(""),
                    x: p[0] as f32,
                    y: p[1] as f32,
                })
                .collect(),
        ));
    }

    /// Show the state of the editor: pins, selection, layers, variants,
    /// undo and redo.
    fn update_editor(&self) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let st = self.state.borrow();
        let e = &st.editor;
        let c = e.config();
        g.set_can_undo(e.can_undo());
        g.set_can_redo(e.can_redo());
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
            catalog::LAYERS
                .iter()
                .map(|l| LayerRow {
                    id: s(l.id),
                    label: s(l.label),
                    key: s(l.key.map(String::from).unwrap_or_default()),
                    on: catalog::is_on(c, l),
                    available: catalog::available(l),
                })
                .collect(),
        ));
        let l = ui.global::<Lists>();
        let mut presets: Vec<SharedString> = catalog::PRESETS
            .iter()
            .map(|(_, label, _)| s(label))
            .collect();
        presets.push(s(catalog::CUSTOM_PRESET));
        let mut disabled = vec![false; catalog::PRESETS.len()];
        disabled.push(true);
        l.set_presets(model(presets));
        l.set_presets_disabled(model(disabled));
        l.set_preset_index(
            catalog::current_preset(c)
                .and_then(|p| catalog::PRESETS.iter().position(|(id, _, _)| *id == p))
                .unwrap_or(catalog::PRESETS.len()) as i32,
        );
        self.update_options(c);
        self.update_lists(&st);
        g.set_variants(model(c.variants.iter().map(|v| s(&v.name)).collect()));
        g.set_variant_index(e.variant as i32);
        g.set_brush_radius_max((c.frame.width.max(c.frame.height) / 4.0).max(2.0) as f32);
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
                // Show the values of the config again in the fields.
                self.update_editor();
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
                    if !g.get_generating() {
                        self.snapshot_soon();
                    }
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
                    checks,
                    warnings,
                } = *made;
                let page = config
                    .variants
                    .iter()
                    .find(|v| v.name == st.editor.variant_name())
                    .map(|v| Page::new(&config, v));
                let px_per_mm = page.map_or(1.0, |p| f64::from(scene.size()[0]) / p.w_mm);
                let failed = checks.iter().filter(|c| !c.1).count();
                g.set_checks(model(
                    checks
                        .into_iter()
                        .map(|(name, pass, detail)| CheckRow {
                            name: s(name),
                            pass,
                            detail: s(detail),
                        })
                        .collect(),
                ));
                g.set_checks_summary(s(if failed == 0 {
                    format!("Variant {:?}: all checks pass.", st.editor.variant_name())
                } else {
                    format!(
                        "Variant {:?}: {failed} checks fail.",
                        st.editor.variant_name()
                    )
                }));
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
                self.update_lists(&self.state.borrow());
                self.update_view();
                self.rerender();
            }
        }
    }

    /// Write the window image of `INKLAND_APP_SNAPSHOT` a short time after
    /// the first map shows, then stop the app.
    fn snapshot_soon(self: &Rc<Self>) {
        let Some(path) = self.snapshot.borrow_mut().take() else {
            return;
        };
        // `INKLAND_APP_SNAPSHOT_POINTER=x,y` (config units) puts the pointer
        // there, so that the image shows the cell facts.
        let pointer = std::env::var("INKLAND_APP_SNAPSHOT_POINTER")
            .ok()
            .and_then(|t| inkland_app::snapshot::point(&t));
        if let Some(w) = pointer {
            let st = self.state.borrow();
            let screen = Self::page(&st).map(|page| st.view.world_to_screen(&page, w));
            drop(st);
            if let Some(p) = screen {
                self.pointer_move(p);
            }
        }
        let me = Rc::clone(self);
        slint::Timer::single_shot(Duration::from_millis(500), move || {
            let result = me
                .ui()
                .window()
                .take_snapshot()
                .map_err(|e| e.to_string())
                .and_then(|shot| {
                    let (w, h) = (shot.width(), shot.height());
                    inkland_render::png::rgba_to_png(shot.as_bytes().to_vec(), w, h)
                })
                .and_then(|png| std::fs::write(&path, png).map_err(|e| e.to_string()));
            match result {
                Ok(()) => eprintln!("inkland-app: wrote {}", path.display()),
                Err(e) => eprintln!("inkland-app: cannot write the snapshot: {e}"),
            }
            let _ = slint::quit_event_loop();
        });
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

    fn brush_delta(&self) -> (f64, f64) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let strength = f64::from(g.get_brush_strength()).round();
        let delta = if g.get_brush_raise() {
            strength
        } else {
            -strength
        };
        (f64::from(g.get_brush_radius()), delta)
    }

    fn stroke(&self, at: [f64; 2]) {
        let (radius, delta) = self.brush_delta();
        let r = self.state.borrow_mut().editor.brush(at, radius, delta);
        self.edited(r, &format!("brush at ({:.1}, {:.1})", at[0], at[1]));
    }

    fn pointer_down(self: &Rc<Self>, p: [f64; 2], pan: bool) {
        self.ui().invoke_focus_keys();
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
                let kind = catalog::PLACE_KINDS
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
            Tool::Brush => {
                let Some(at) = Self::world_at(&st, p) else {
                    return;
                };
                st.drag = Drag::Brush { last: at };
                drop(st);
                self.stroke(at);
            }
            Tool::Region => {
                let Some(at) = Self::world_at(&st, p) else {
                    return;
                };
                let shape = self.ui().global::<AppState>().get_region_shape();
                if shape < 2 {
                    st.drag = Drag::Shape { start: at };
                    return;
                }
                // A click near the first point closes the outline.
                let close = st.draft.len() >= 3
                    && Self::page(&st).is_some_and(|page| {
                        distance(st.view.world_to_screen(&page, st.draft[0]), p) < PIN_HIT_PX
                    });
                if close {
                    drop(st);
                    self.finish_draft();
                } else {
                    Self::push_draft(&mut st, at);
                    drop(st);
                    self.update_view();
                }
            }
            Tool::River | Tool::Route => {
                let Some(at) = Self::world_at(&st, p) else {
                    return;
                };
                Self::push_draft(&mut st, at);
                drop(st);
                self.update_view();
            }
            Tool::Crop => {
                if let Some(at) = Self::world_at(&st, p) {
                    st.drag = Drag::Shape { start: at };
                }
            }
        }
    }

    /// Add a point to the drawing. A point on the last point (a double
    /// click) is not added.
    fn push_draft(st: &mut State, at: [f64; 2]) {
        let near = Self::world_len(st, 3.0);
        if st
            .draft
            .last()
            .is_none_or(|last| distance(*last, at) > near)
        {
            st.draft.push(at);
        }
    }

    /// The world distance of `px` screen pixels.
    fn world_len(st: &State, px: f64) -> f64 {
        Self::page(st).map_or(1.0, |page| px / (page.scale * st.view.zoom))
    }

    /// Make the item that the user drew: a region outline, a river, or a
    /// road.
    fn finish_draft(self: &Rc<Self>) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let mut st = self.state.borrow_mut();
        let draft = std::mem::take(&mut st.draft);
        if draft.is_empty() {
            return;
        }
        let r = match g.get_tool() {
            Tool::Region => {
                let kind = region_kind(g.get_region_kind());
                st.editor
                    .add_region(kind, &Shape::Polygon(draft))
                    .map(|id| format!("add the {kind} region {id}"))
            }
            Tool::River => {
                let tol = Self::world_len(&st, PIN_HIT_PX);
                let c = st.editor.config();
                let anchors: Vec<_> = draft.iter().map(|p| anchor_at(c, *p, tol)).collect();
                st.editor
                    .add_river(&anchors)
                    .map(|id| format!("add the river {id}"))
            }
            Tool::Route => {
                let tol = Self::world_len(&st, PIN_HIT_PX);
                let c = st.editor.config();
                let points: Vec<_> = draft.iter().map(|p| waypoint_at(c, *p, tol)).collect();
                let kind = if g.get_road_kind() == 1 {
                    "trail"
                } else {
                    "road"
                };
                st.editor
                    .add_road_along(&points, kind)
                    .map(|id| format!("add the {kind} {id}"))
            }
            _ => Ok(String::new()),
        };
        drop(st);
        match r {
            Ok(what) => {
                g.set_tool(Tool::Select);
                self.edited(Ok(()), &what);
            }
            Err(e) => self.edited(Err(e), "add the drawn item"),
        }
        self.update_tools();
    }

    /// Stop the drawing.
    fn cancel_draft(self: &Rc<Self>) {
        let mut st = self.state.borrow_mut();
        st.draft.clear();
        st.drag = Drag::None;
        drop(st);
        self.update_view();
    }

    fn pointer_move(self: &Rc<Self>, p: [f64; 2]) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        g.set_pointer_x(p[0] as f32);
        g.set_pointer_y(p[1] as f32);
        g.set_pointer_in_map(true);
        let mut st = self.state.borrow_mut();
        st.pointer = Some(p);
        if (matches!(st.drag, Drag::Shape { .. }) || !st.draft.is_empty())
            && let Some(page) = Self::page(&st)
        {
            self.update_marks(&st, &page);
        }
        // The status bar.
        if let Some(page) = Self::page(&st) {
            let w = st.view.screen_to_world(&page, p);
            let unit = &st.editor.config().units.name;
            g.set_status_position(s(format!("x {:.2}, y {:.2} {unit}", w[0], w[1])));
            let info = st
                .shown
                .as_ref()
                .and_then(|m| inkland::query_at(&m.world, &m.config, w).ok());
            g.set_status_cell(s(match &info {
                Some(q) => inkland_app::rows::status_text(q),
                None => "outside the frame".to_owned(),
            }));
            if ui.global::<Dialogs>().get_cells()
                && let Some(q) = &info
            {
                let lines = inkland_app::rows::cell_lines(q)
                    .into_iter()
                    .map(|(label, value)| {
                        inkland_app::rows::row(label.clone(), vec![label, value], false)
                    })
                    .collect();
                ui.global::<Lists>().set_cell_lines(table(lines));
            }
        }
        match &mut st.drag {
            Drag::None | Drag::Shape { .. } => {}
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
            Drag::Brush { last } => {
                let last = *last;
                let Some(at) = Self::world_at(&st, p) else {
                    return;
                };
                let (radius, _) = self.brush_delta();
                if distance(at, last) >= radius * 0.5 {
                    st.drag = Drag::Brush { last: at };
                    drop(st);
                    self.stroke(at);
                }
            }
        }
    }

    fn pointer_up(self: &Rc<Self>, p: [f64; 2]) {
        let drag = std::mem::replace(&mut self.state.borrow_mut().drag, Drag::None);
        self.ui().global::<AppState>().set_dragging_pin(false);
        if let Drag::Shape { start } = drag {
            self.shape_done(start, p);
            return;
        }
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

    /// The end of a drag that makes a box or a circle.
    fn shape_done(self: &Rc<Self>, start: [f64; 2], p: [f64; 2]) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let mut st = self.state.borrow_mut();
        let Some(page) = Self::page(&st) else { return };
        let end = st.view.screen_to_world(&page, p);
        let f = &st.editor.config().frame;
        let end = [end[0].clamp(0.0, f.width), end[1].clamp(0.0, f.height)];
        let r = match g.get_tool() {
            Tool::Region => {
                let kind = region_kind(g.get_region_kind());
                let shape_kind = if g.get_region_shape() == 1 {
                    ShapeKind::Circle
                } else {
                    ShapeKind::Box
                };
                match Shape::from_drag(shape_kind, start, end) {
                    Some(shape) => st
                        .editor
                        .add_region(kind, &shape)
                        .map(|id| format!("add the {kind} region {id}")),
                    None => Err("drag to draw the area".to_owned()),
                }
            }
            Tool::Crop => match crop_from_drag(start, end) {
                Some(crop) => {
                    let v = st.editor.variant;
                    if g.get_crop_new() {
                        st.editor.add_variant(v, Some(crop)).map(|i| {
                            st.editor.variant = i;
                            st.fit_next = true;
                            format!("add the variant {}", st.editor.variant_name())
                        })
                    } else {
                        st.fit_next = true;
                        st.editor
                            .set_crop(v, Some(crop))
                            .map(|_| format!("crop the variant {}", st.editor.variant_name()))
                    }
                }
                None => Err("drag to draw the crop".to_owned()),
            },
            _ => Ok(String::new()),
        };
        drop(st);
        match r {
            Ok(what) => {
                g.set_tool(Tool::Select);
                self.edited(Ok(()), &what);
            }
            Err(e) => self.edited(Err(e), "draw"),
        }
        self.update_tools();
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

    /// The rows of the editor tables, from the config and from the shown
    /// world.
    /// The Emblems editor: the items of the kind, and the arms, the image,
    /// and the custom field of the selected item.
    fn update_emblems(&self, st: &State) {
        use inkland_app::emblems as em;
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let l = ui.global::<Lists>();
        l.set_emblem_kinds(model(em::KINDS.iter().map(s).collect()));
        let open = ui.global::<Dialogs>().get_emblems();
        let kind = usize::try_from(g.get_emblem_kind()).unwrap_or(0).min(2);
        let found = st.shown.as_ref().and_then(|m| {
            let w = m.world.as_ref();
            let items = em::items(w, kind);
            l.set_emblem_items(model(items.iter().map(|(_, n)| s(n)).collect()));
            let index = usize::try_from(g.get_emblem_index())
                .unwrap_or(0)
                .min(items.len().saturating_sub(1));
            g.set_emblem_index(index as i32);
            let (id, _) = items.get(index)?;
            let coa = em::coa(w, kind, *id)?.clone();
            Some((coa, em::custom_path(st.editor.config(), w, kind, *id)))
        });
        let Some((coa, path)) = found else {
            l.set_emblem_items(model(Vec::new()));
            g.set_emblem_summary(s(""));
            g.set_emblem_custom_path(s(""));
            g.set_emblem_custom(false);
            g.set_emblem_json(s(""));
            g.set_emblem_image(slint::Image::default());
            return;
        };
        let custom = path
            .as_deref()
            .and_then(|p| em::custom(st.editor.config(), p));
        g.set_emblem_summary(s(em::summary(&coa)));
        g.set_emblem_custom_path(s(path.unwrap_or_default()));
        g.set_emblem_custom(custom.is_some());
        let shown = custom.unwrap_or_else(|| coa.clone());
        g.set_emblem_json(s(serde_json::to_string(&shown).unwrap_or_default()));
        if open {
            // Two pixels for each logical pixel, for a sharp image.
            const SIZE: u32 = 392;
            match em::image(&coa, SIZE) {
                Ok(px) => g.set_emblem_image(slint::Image::from_rgba8(
                    SharedPixelBuffer::clone_from_slice(&px, SIZE, SIZE),
                )),
                Err(e) => self.message(&format!("cannot draw the arms: {e}"), true),
            }
        }
    }

    /// The read-only overviews of the shown world: the state select, and
    /// the rows of the open overviews.
    fn update_overviews(&self, st: &State) {
        use inkland_app::overviews as ov;
        use inkland_app::rows;
        let ui = self.ui();
        let l = ui.global::<Lists>();
        let g = ui.global::<AppState>();
        let d = ui.global::<Dialogs>();
        let world = st.shown.as_ref().map(|m| m.world.as_ref());
        let names = world.map(ov::state_names).unwrap_or_default();
        l.set_state_names(model(names.iter().map(s).collect()));
        let last = i32::try_from(names.len()).unwrap_or(i32::MAX) - 1;
        let index = g.get_overview_state().min(last).max(0);
        g.set_overview_state(index);
        let selected = world
            .and_then(|w| ov::state_of_index(w, index))
            .unwrap_or(0);
        // The rows of a closed overview, or without a world, are empty.
        let open = |on: bool| world.filter(|_| on);
        let rows = |w: Option<&World>, f: &dyn Fn(&World) -> Vec<rows::Row>| {
            table(w.map(f).unwrap_or_default())
        };
        let w = open(d.get_diplomacy());
        l.set_relations(rows(w, &|w| ov::relation_rows(w, selected)));
        g.set_chronicle(s(w.map(ov::chronicle_text).unwrap_or_default()));
        l.set_markets(rows(open(d.get_markets()), &ov::market_rows));
        l.set_goods(rows(open(d.get_goods()), &ov::goods_rows));
        let view = open(d.get_stability()).and_then(|w| ov::stability_view(w, selected));
        g.set_stability_total(s(view.as_ref().map_or("", |v| v.total.as_str())));
        let (parts, provinces) = view.map_or((Vec::new(), Vec::new()), |v| (v.parts, v.provinces));
        l.set_stability_parts(table(parts));
        l.set_stability_provinces(table(provinces));
        let unit = st.editor.config().units.name.clone();
        l.set_zones(rows(open(d.get_zones()), &|w| ov::zone_rows(w, &unit)));
        let w = open(d.get_notes());
        let notes = w.map(ov::note_rows).unwrap_or_default();
        // As in FMG, the first note shows when no note is selected.
        let mut sel = g.get_selected_note();
        if !notes.iter().any(|r| r.id == sel.as_str()) {
            sel = s(notes.first().map_or("", |r| r.id.as_str()));
            g.set_selected_note(sel.clone());
        }
        l.set_notes(table(notes));
        g.set_note_text(s(w.map(|w| ov::note_text(w, &sel)).unwrap_or_default()));
        let w = open(d.get_markers());
        l.set_markers(rows(w, &ov::marker_rows));
        let sel = g.get_selected_marker();
        g.set_marker_text(s(w.map(|w| ov::marker_text(w, &sel)).unwrap_or_default()));
        l.set_military_header(model(ov::military_header().iter().map(s).collect()));
        l.set_regiment_header(model(ov::regiment_header().iter().map(s).collect()));
        let w = open(d.get_military());
        l.set_military(rows(w, &ov::military_rows));
        l.set_regiments(rows(w, &|w| ov::regiment_rows(w, selected)));
    }

    fn update_lists(&self, st: &State) {
        use inkland_app::rows;
        let ui = self.ui();
        let l = ui.global::<Lists>();
        let c = st.editor.config();
        l.set_brushes(table(rows::brush_rows(c)));
        let world = st.shown.as_ref().map(|m| m.world.as_ref());
        l.set_burgs(table(rows::burg_rows(c, world)));
        l.set_regions(table(rows::region_rows(c)));
        l.set_variant_rows(table(inkland_app::variants::variant_rows(c)));
        l.set_overlay_rows(table(inkland_app::variants::overlay_rows(c)));
        let mut overlays = vec![s("none")];
        overlays.extend(c.overlays.keys().map(s));
        l.set_overlay_names(model(overlays));
        let current = c.variants.get(st.editor.variant);
        l.set_variant_crop(s(current.and_then(|v| v.crop).map_or(
            "The variant shows the full frame.".to_owned(),
            |r| {
                use inkland_app::options::number_text as n;
                format!(
                    "The variant shows {}, {} to {}, {}.",
                    n(r[0]),
                    n(r[1]),
                    n(r[2]),
                    n(r[3])
                )
            },
        )));
        let sel = l.get_selected_variant();
        match sel.parse::<usize>().ok().and_then(|i| c.variants.get(i)) {
            Some(v) => {
                use inkland_app::options::number_text as n;
                l.set_variant_name(s(&v.name));
                l.set_variant_w(s(n(v.page.w_mm)));
                l.set_variant_h(s(n(v.page.h_mm)));
                l.set_variant_margin(s(n(v.page.margin_mm)));
                l.set_variant_overlay(
                    v.overlay
                        .as_ref()
                        .and_then(|o| c.overlays.keys().position(|k| k == o))
                        .map_or(0, |i| i as i32 + 1),
                );
            }
            None => {
                l.set_selected_variant(s(""));
                for set in [
                    Lists::set_variant_name,
                    Lists::set_variant_w,
                    Lists::set_variant_h,
                    Lists::set_variant_margin,
                ] {
                    set(&l, s(""));
                }
                l.set_variant_overlay(-1);
            }
        }
        if !c.overlays.contains_key(l.get_selected_overlay().as_str()) {
            l.set_selected_overlay(s(""));
        }
        l.set_labels(table(inkland_app::variants::label_rows(
            c,
            st.editor.variant,
        )));
        let sel = l.get_selected_label();
        if inkland_app::variants::label_items(c)
            .iter()
            .any(|(k, _)| k == sel.as_str())
        {
            let setting = inkland_app::variants::label_setting(c, st.editor.variant, &sel);
            l.set_label_hide(setting.hide);
            l.set_label_size(s(setting
                .size_pt
                .map(inkland_app::options::number_text)
                .unwrap_or_default()));
        } else {
            l.set_selected_label(s(""));
            l.set_label_hide(false);
            l.set_label_size(s(""));
        }
        match world {
            Some(w) => {
                l.set_states(table(rows::state_rows(w)));
                l.set_cultures(table(rows::culture_rows(w)));
                l.set_religions(table(rows::religion_rows(w)));
                l.set_provinces(table(rows::province_rows(w)));
            }
            None => {
                for set in [
                    Lists::set_states,
                    Lists::set_cultures,
                    Lists::set_religions,
                    Lists::set_provinces,
                ] {
                    set(&l, table(Vec::new()));
                }
            }
        }
        self.update_emblems(st);
        self.update_overviews(st);
        l.set_rivers(table(rows::river_rows(c, world)));
        let sel = l.get_selected_river();
        match c.pins.rivers.iter().find(|r| r.id == sel.as_str()) {
            Some(r) => l.set_river_name(s(r.name.clone().unwrap_or_default())),
            None => {
                l.set_selected_river(s(""));
                l.set_river_name(s(""));
            }
        }
        l.set_routes(table(rows::route_rows(c, world)));
        let sel = l.get_selected_route();
        match c.pins.roads.iter().find(|r| r.id == sel.as_str()) {
            Some(r) => {
                l.set_route_target(s(r
                    .target_length
                    .map(inkland_app::options::number_text)
                    .unwrap_or_default()));
                l.set_route_tolerance(s(inkland_app::options::number_text(r.tolerance * 100.0)));
            }
            None => {
                l.set_selected_route(s(""));
                l.set_route_target(s(""));
                l.set_route_tolerance(s(""));
            }
        }
        let sel = l.get_selected_region();
        match c.pins.regions.iter().find(|r| r.id == sel.as_str()) {
            Some(r) => {
                let [lo, hi] = r.height_range();
                l.set_region_name(s(r.name.clone().unwrap_or_default()));
                l.set_region_kind(
                    catalog::REGION_KINDS
                        .iter()
                        .position(|k| *k == rows::kind_text(&r.kind))
                        .map_or(-1, |i| i as i32),
                );
                l.set_region_lo(s(inkland_app::options::number_text(lo)));
                l.set_region_hi(s(inkland_app::options::number_text(hi)));
                l.set_region_edge(s(inkland_app::options::number_text(r.edge)));
            }
            None => {
                l.set_selected_region(s(""));
                l.set_region_name(s(""));
                l.set_region_kind(-1);
                l.set_region_lo(s(""));
                l.set_region_hi(s(""));
                l.set_region_edge(s(""));
            }
        }
        let g = ui.global::<AppState>();
        let selected = st
            .editor
            .selected
            .as_ref()
            .and_then(|id| c.pins.points.iter().find(|p| &p.id == id));
        g.set_selected_kind(selected.map_or(-1, |p| {
            let kind = rows::kind_text(&p.kind);
            catalog::PLACE_KINDS
                .iter()
                .position(|k| *k == kind)
                .map_or(-1, |i| i as i32)
        }));
        g.set_selected_port(selected.is_some_and(|p| p.port));
    }

    /// The fields of the Options tab and of the Style tab.
    fn update_options(&self, c: &Config) {
        let ui = self.ui();
        let o = ui.global::<Opts>();
        let t = |key: &str| s(option_text(c, key));
        o.set_seed(t("seed"));
        o.set_width(t("width"));
        o.set_height(t("height"));
        o.set_cells(t("cells"));
        o.set_cultures(t("cultures"));
        o.set_states(t("states"));
        o.set_towns(t("towns"));
        o.set_religions(t("religions"));
        o.set_sea_level(t("sea-level"));
        o.set_lat_south(t("lat-south"));
        o.set_lat_north(t("lat-north"));
        o.set_units(t("units"));
        o.set_cell_hint(t("cell-hint"));
        o.set_size_title(t("size-title"));
        o.set_size_state(t("size-state"));
        o.set_size_region(t("size-region"));
        o.set_size_city(t("size-city"));
        o.set_size_town(t("size-town"));
        o.set_size_village(t("size-village"));
        o.set_size_river(t("size-river"));
        o.set_size_other(t("size-other"));
        o.set_min_line(t("min-line"));
        o.set_grid_step(t("grid-step"));
        o.set_title(t("title"));
        o.set_scale_bar(option_text(c, "scale-bar") == "true");
        o.set_compass(option_text(c, "compass") == "true");
        let index = |ids: &[&str], key: &str| {
            let v = option_text(c, key);
            ids.iter().position(|i| *i == v).map_or(-1, |i| i as i32)
        };
        let templates: Vec<&str> = catalog::TEMPLATES.iter().map(|t| t.0).collect();
        o.set_templates(model(catalog::TEMPLATES.iter().map(|t| s(t.1)).collect()));
        o.set_template(index(&templates, "template"));
        let sets = catalog::culture_sets();
        let set_ids: Vec<&str> = sets.iter().map(|t| t.0).collect();
        o.set_culture_sets(model(sets.iter().map(|t| s(&t.1)).collect()));
        o.set_culture_set(index(&set_ids, "culture-set"));
        let modes: Vec<&str> = catalog::RENDER_MODES.iter().map(|t| t.0).collect();
        o.set_modes(model(
            catalog::RENDER_MODES.iter().map(|t| s(t.1)).collect(),
        ));
        o.set_modes_disabled(model(
            modes
                .iter()
                .map(|m| !catalog::mode_available(c, m))
                .collect(),
        ));
        o.set_mode(index(&modes, "mode"));
        let styles: Vec<&str> = catalog::STYLE_PRESETS.iter().map(|t| t.0).collect();
        o.set_styles(model(
            catalog::STYLE_PRESETS.iter().map(|t| s(t.1)).collect(),
        ));
        o.set_styles_disabled(model(
            styles
                .iter()
                .map(|m| !catalog::style_available(c, m))
                .collect(),
        ));
        o.set_style(index(&styles, "style"));
        let fonts: Vec<&str> = c.render.fonts_allowed.iter().map(String::as_str).collect();
        o.set_fonts(model(fonts.iter().map(s).collect()));
        o.set_font_title(index(&fonts, "font-title"));
        o.set_font_labels(index(&fonts, "font-labels"));
        o.set_font_labels_sc(index(&fonts, "font-labels-sc"));
        o.set_font_other(index(&fonts, "font-other"));
    }

    /// Set an option from the text of its field.
    fn set_option(self: &Rc<Self>, key: &str, text: &str) {
        let r = self.state.borrow_mut().editor.set_option(key, text);
        self.edited(r, &format!("set {key} to {text}"));
    }

    /// An entry of a select of the Options tab or of the Style tab.
    fn choose(self: &Rc<Self>, key: &str, i: usize) {
        let id = {
            let st = self.state.borrow();
            let c = st.editor.config();
            match key {
                "template" => catalog::TEMPLATES.get(i).map(|t| t.0.to_owned()),
                "culture-set" => catalog::culture_sets().get(i).map(|t| t.0.to_owned()),
                "mode" => catalog::RENDER_MODES.get(i).map(|t| t.0.to_owned()),
                "style" => catalog::STYLE_PRESETS.get(i).map(|t| t.0.to_owned()),
                _ => c.render.fonts_allowed.get(i).cloned(),
            }
        };
        if let Some(id) = id {
            self.set_option(key, &id);
        }
    }

    /// A button of the Tools tab: open a dialog, or start a tool that adds
    /// items on the map.
    fn open_tool(self: &Rc<Self>, id: &str) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        match id {
            "add-burg" => {
                ui.global::<Dialogs>().invoke_open(s("burgs"));
                g.set_tool(if g.get_tool() == Tool::AddPin {
                    Tool::Select
                } else {
                    Tool::AddPin
                });
            }
            "add-region" => {
                ui.global::<Dialogs>().invoke_open(s("regions"));
                self.start_tool(Tool::Region);
            }
            "add-river" => {
                ui.global::<Dialogs>().invoke_open(s("rivers"));
                self.start_tool(Tool::River);
            }
            "add-route" => {
                ui.global::<Dialogs>().invoke_open(s("routes"));
                self.start_tool(Tool::Route);
            }
            _ => ui.global::<Dialogs>().invoke_open(s(id)),
        }
        self.update_tools();
    }

    /// Edit a field of an item of an editor table.
    fn edit_item(self: &Rc<Self>, list: &str, id: &str, field: &str, text: &str) {
        let mut st = self.state.borrow_mut();
        let e = &mut st.editor;
        let num = |t: &str| {
            t.trim()
                .parse::<f64>()
                .map_err(|_| format!("{t:?} is not a number"))
        };
        let index = id
            .parse::<usize>()
            .map_err(|_| format!("{id:?} is not a variant"));
        let r = match (list, field) {
            ("variants", "overlay") => {
                index.and_then(|v| e.set_variant_overlay(v, (!text.is_empty()).then_some(text)))
            }
            ("variants", "crop") => index.and_then(|v| e.set_crop(v, None)),
            ("variants", "copy") => index.and_then(|v| e.add_variant(v, None).map(|_| ())),
            ("variants", _) => index.and_then(|v| e.set_variant_field(v, field, text)),
            ("overlays", "add") => e.add_overlay(text),
            ("labels", "hide" | "size") => {
                let v = e.variant;
                let mut setting = inkland_app::variants::label_setting(e.config(), v, id);
                if field == "hide" {
                    setting.hide = text == "true";
                    Ok(())
                } else if text.trim().is_empty() {
                    setting.size_pt = None;
                    Ok(())
                } else {
                    num(text).map(|n| setting.size_pt = Some(n))
                }
                .and_then(|()| e.set_label(v, id, setting))
            }
            (_, "name") => match pin_list(list) {
                Some(l) => e.rename_in(l, id, text),
                None => Err(format!("{list:?} has no names")),
            },
            ("regions", "kind") => e.set_region_kind(id, text),
            ("regions", "height") => {
                let parts: Vec<&str> = text.split(',').collect();
                match parts.as_slice() {
                    [lo, hi] if lo.trim().is_empty() && hi.trim().is_empty() => {
                        e.set_region_height(id, None)
                    }
                    [lo, hi] => num(lo)
                        .and_then(|lo| num(hi).map(|hi| [lo, hi]))
                        .and_then(|r| e.set_region_height(id, Some(r))),
                    _ => Err("give the height as low,high".to_owned()),
                }
            }
            ("regions", "edge") => num(text).and_then(|v| e.set_region_edge(id, v)),
            ("routes", "target") if text.trim().is_empty() => e.set_road_target(id, None),
            ("routes", "target") => num(text).and_then(|v| e.set_road_target(id, Some(v))),
            ("routes", "tolerance") => num(text).and_then(|v| e.set_road_tolerance(id, v / 100.0)),
            _ => Err(format!("{list}: there is no field {field:?}")),
        };
        drop(st);
        self.edited(r, &format!("set the {field} of {id}"));
    }

    /// A hotkey of FMG (see `inkland_app::hotkeys`). The result is `true`
    /// if the key does something.
    fn key(self: &Rc<Self>, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        use inkland_app::hotkeys::{Hotkey, hotkey};
        use slint::platform::Key;
        let special = [
            (Key::F2, "F2"),
            (Key::Tab, "Tab"),
            (Key::Escape, "Escape"),
            (Key::Return, "Enter"),
            (Key::Delete, "Delete"),
            (Key::Backspace, "Backspace"),
            (Key::LeftArrow, "Left"),
            (Key::RightArrow, "Right"),
            (Key::UpArrow, "Up"),
            (Key::DownArrow, "Down"),
        ];
        let name = special
            .iter()
            .find(|(k, _)| SharedString::from(*k) == text)
            .map_or(text, |(_, n)| *n);
        let Some(action) = hotkey(name, ctrl, shift, alt) else {
            return false;
        };
        let ui = self.ui();
        let a = ui.global::<Actions>();
        let g = ui.global::<AppState>();
        match action {
            Hotkey::Undo => a.invoke_undo(),
            Hotkey::Redo => a.invoke_redo(),
            Hotkey::Save => a.invoke_save(),
            Hotkey::SaveAs => a.invoke_save_as(),
            Hotkey::Open => a.invoke_open(),
            Hotkey::NewMap => a.invoke_new_map(),
            Hotkey::ToggleMenu => {
                let u = ui.global::<Ui>();
                u.set_panel_open(!u.get_panel_open());
            }
            Hotkey::Escape => {
                let drawing = !self.state.borrow().draft.is_empty();
                if drawing || g.get_tool() != Tool::Select {
                    g.set_tool(Tool::Select);
                    self.cancel_draft();
                    self.update_tools();
                } else {
                    ui.global::<Dialogs>().invoke_close_all();
                    ui.global::<Ui>().set_panel_open(false);
                }
            }
            Hotkey::Finish => {
                if self.state.borrow().draft.is_empty() {
                    return false;
                }
                self.finish_draft();
            }
            Hotkey::Delete => {
                let selected = self.state.borrow().editor.selected.clone();
                let Some(id) = selected else { return false };
                a.invoke_remove_pin(s(id));
            }
            Hotkey::Tool(id) => self.open_tool(id),
            Hotkey::Layer(id) => {
                let on = {
                    let st = self.state.borrow();
                    catalog::layer_def(id).map(|d| catalog::is_on(st.editor.config(), d))
                };
                match on {
                    Some(on) => a.invoke_set_layer(s(id), !on),
                    None => return false,
                }
            }
            Hotkey::Zoom(f) => {
                let mut st = self.state.borrow_mut();
                let centre = [st.viewport[0] / 2.0, st.viewport[1] / 2.0];
                st.view.zoom_at(centre, f);
                drop(st);
                self.update_view();
                self.rerender_soon();
            }
            Hotkey::Fit => self.fit(),
            Hotkey::Pan(dx, dy) => {
                self.state.borrow_mut().view.pan_by([dx, dy]);
                self.update_view();
                self.rerender_soon();
            }
            Hotkey::BrushSize(up) => {
                let step = g.get_brush_radius_max() / 20.0;
                let r = g.get_brush_radius() + if up { step } else { -step };
                g.set_brush_radius(r.clamp(1.0, g.get_brush_radius_max()));
                self.update_view();
            }
        }
        true
    }

    /// Start a drawing tool, or stop it if it is on.
    fn start_tool(self: &Rc<Self>, tool: Tool) {
        let ui = self.ui();
        let g = ui.global::<AppState>();
        let on = g.get_tool() == tool;
        g.set_tool(if on { Tool::Select } else { tool });
        self.cancel_draft();
        self.update_tools();
    }

    /// The buttons of the Tools tab, by group.
    fn update_tools(&self) {
        let ui = self.ui();
        let tool = ui.global::<AppState>().get_tool();
        let mut rows = Vec::new();
        let mut counts = Vec::new();
        for group in ["Edit", "Add", "Show"] {
            let list: Vec<&catalog::ToolDef> =
                catalog::TOOLS.iter().filter(|t| t.group == group).collect();
            counts.push(list.len() as i32);
            rows.extend(list.into_iter().map(|t| ToolRow {
                id: s(t.id),
                label: s(t.label),
                hint: s(t.hint),
                enabled: t.enabled && IMPLEMENTED.contains(&t.id),
                active: (t.id == "add-burg" && tool == Tool::AddPin)
                    || (t.id == "add-region" && tool == Tool::Region)
                    || (t.id == "add-river" && tool == Tool::River)
                    || (t.id == "add-route" && tool == Tool::Route),
            }));
        }
        let l = ui.global::<Lists>();
        l.set_tools(model(rows));
        l.set_tool_counts(model(counts));
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
            draft: Vec::new(),
            pointer: None,
        }),
        generator,
        rasterizer,
        results,
        idle: slint::Timer::default(),
        snapshot: RefCell::new(std::env::var_os("INKLAND_APP_SNAPSHOT").map(PathBuf::from)),
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
    a.on_pointer_exit(move || {
        c.state.borrow_mut().pointer = None;
        c.ui().global::<AppState>().set_pointer_in_map(false);
    });
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
    a.on_set_layer(move |id, on| {
        let r = c.state.borrow_mut().editor.set_layer(&id, on);
        let what = if on { "show" } else { "hide" };
        let label = catalog::layer_def(&id).map_or("?", |l| l.label);
        c.edited(r, &format!("{what} the layer {label}"));
    });
    let c = Rc::clone(&app);
    a.on_apply_preset(move |i| {
        let Some((id, label, _)) = catalog::PRESETS.get(i as usize) else {
            return;
        };
        let r = c.state.borrow_mut().editor.apply_preset(id);
        c.edited(r, &format!("show the layers of the preset {label}"));
    });
    let c = Rc::clone(&app);
    a.on_set_option(move |key, text| c.set_option(&key, &text));
    let c = Rc::clone(&app);
    a.on_choose(move |key, i| c.choose(&key, usize::try_from(i).unwrap_or(0)));
    let c = Rc::clone(&app);
    a.on_new_map(move || {
        let seed = new_seed();
        c.set_option("seed", &seed.to_string());
    });
    let c = Rc::clone(&app);
    a.on_pin_place(move |id| {
        let place = {
            let st = c.state.borrow();
            st.shown.as_ref().and_then(|m| {
                inkland_app::rows::generated_place(&m.world, &id).map(|p| {
                    let kind = inkland_app::rows::kind_text(&p.kind);
                    let kind = if catalog::PLACE_KINDS.contains(&kind.as_str()) {
                        kind
                    } else {
                        "village".to_owned()
                    };
                    (p.at, kind, p.name.clone())
                })
            })
        };
        let r = match place {
            Some((at, kind, name)) => c
                .state
                .borrow_mut()
                .editor
                .pin_place(at, &kind, &name)
                .map(|_| ()),
            None => Err("the place is not in the map".to_owned()),
        };
        c.edited(r, "pin the place");
    });
    let c = Rc::clone(&app);
    a.on_set_pin_kind(move |i| {
        let kind = catalog::PLACE_KINDS
            .get(i as usize)
            .copied()
            .unwrap_or("village");
        let mut st = c.state.borrow_mut();
        let r = match st.editor.selected.clone() {
            Some(id) => st.editor.set_pin_field(
                inkland::PinList::Points,
                &id,
                "kind",
                Some(serde_json::json!(kind)),
            ),
            None => Err("no burg is selected".to_owned()),
        };
        drop(st);
        c.edited(r, &format!("set the kind to {kind}"));
    });
    let c = Rc::clone(&app);
    a.on_set_pin_port(move |on| {
        let mut st = c.state.borrow_mut();
        let r = match st.editor.selected.clone() {
            Some(id) => st.editor.set_pin_field(
                inkland::PinList::Points,
                &id,
                "port",
                Some(serde_json::json!(on)),
            ),
            None => Err("no burg is selected".to_owned()),
        };
        drop(st);
        c.edited(r, if on { "make a port" } else { "make not a port" });
    });
    let c = Rc::clone(&app);
    a.on_key(move |text, ctrl, shift, alt| c.key(&text, ctrl, shift, alt));
    let c = Rc::clone(&app);
    a.on_finish_draft(move || c.finish_draft());
    let c = Rc::clone(&app);
    a.on_cancel_draft(move || c.cancel_draft());
    let c = Rc::clone(&app);
    a.on_start_tool(move |t| c.start_tool(t));
    let c = Rc::clone(&app);
    a.on_selection_changed(move || {
        c.update_lists(&c.state.borrow());
        c.update_view();
    });
    let c = Rc::clone(&app);
    a.on_edit_item(move |list, id, field, text| c.edit_item(&list, &id, &field, &text));
    let c = Rc::clone(&app);
    a.on_remove_item(move |list, id| {
        let mut st = c.state.borrow_mut();
        let r = match (list.as_str(), pin_list(&list)) {
            (_, Some(l)) => st.editor.remove_in(l, &id),
            ("variants", _) => match id.parse::<usize>() {
                Ok(i) => {
                    st.fit_next = true;
                    st.editor.remove_variant(i)
                }
                Err(_) => Err(format!("{id:?} is not a variant")),
            },
            ("overlays", _) => st.editor.remove_overlay(&id),
            _ => Err(format!("{list:?} has no items")),
        };
        drop(st);
        c.edited(r, &format!("remove {id}"));
    });
    let c = Rc::clone(&app);
    a.on_remove_brush(move |id| {
        let r = match id.parse::<usize>() {
            Ok(i) => c.state.borrow_mut().editor.remove_brush(i),
            Err(_) => Err(format!("{id:?} is not a brush")),
        };
        c.edited(r, &format!("remove the brush stroke {id}"));
    });
    let c = Rc::clone(&app);
    a.on_open_tool(move |id| c.open_tool(&id));
    let c = Rc::clone(&app);
    a.on_emblem_select(move |kind, index| {
        let ui = c.ui();
        let g = ui.global::<AppState>();
        g.set_emblem_kind(kind);
        g.set_emblem_index(index);
        c.update_emblems(&c.state.borrow());
    });
    let c = Rc::clone(&app);
    a.on_emblem_set(move |text| {
        let path = c.ui().global::<AppState>().get_emblem_custom_path();
        let r = if path.is_empty() {
            Err("this item cannot have custom arms".to_owned())
        } else {
            inkland_app::emblems::parse(&text)
                .and_then(|v| c.state.borrow_mut().editor.set(&path, v))
        };
        c.edited(r, "set custom arms");
    });
    let c = Rc::clone(&app);
    a.on_emblem_reset(move || {
        let path = c.ui().global::<AppState>().get_emblem_custom_path();
        let r = if path.is_empty() {
            Err("this item has no custom arms".to_owned())
        } else {
            c.state.borrow_mut().editor.unset(&path)
        };
        c.edited(r, "use the generated arms");
    });
    let c = Rc::clone(&app);
    a.on_select_pin(move |id| {
        let mut st = c.state.borrow_mut();
        let pin = st
            .editor
            .config()
            .pins
            .points
            .iter()
            .any(|p| p.id == id.as_str());
        st.editor.selected = pin.then(|| id.to_string());
        drop(st);
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
    a.on_undo(move || {
        let r = c.state.borrow_mut().editor.undo();
        match r {
            Ok(true) => c.edited(Ok(()), "undo"),
            Ok(false) => c.message("there is nothing to undo", false),
            Err(e) => c.message(&format!("undo: {e}"), true),
        }
    });
    let c = Rc::clone(&app);
    a.on_redo(move || {
        let r = c.state.borrow_mut().editor.redo();
        match r {
            Ok(true) => c.edited(Ok(()), "redo"),
            Ok(false) => c.message("there is nothing to redo", false),
            Err(e) => c.message(&format!("redo: {e}"), true),
        }
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
    let c = Rc::clone(&app);
    a.on_save_script(move || {
        let dialog = rfd::FileDialog::new()
            .add_filter("inkland edit script", &["json"])
            .set_file_name("edits.json");
        if let Some(path) = dialog.save_file() {
            let (text, needs_config) = {
                let st = c.state.borrow();
                (st.editor.script_json(), st.editor.script_needs_config())
            };
            if c.write(&path, &text, "edit script") && needs_config {
                c.message(
                    &format!(
                        "saved the edit script to {}; it edits variants, overlays, or render \
                         options, so replay it with inkland edit --config",
                        path.display()
                    ),
                    false,
                );
            }
        }
    });
    a.on_quit(|| {
        let _ = slint::quit_event_loop();
    });
    // A change of the brush radius changes the outline in the view.
    let c = Rc::clone(&app);
    a.on_brush_changed(move || c.update_view());

    let theme = ui.global::<Theme>();
    if cfg!(target_os = "macos") {
        theme.set_mono(s("Menlo"));
        theme.set_sans(s("Helvetica"));
    } else if cfg!(target_os = "windows") {
        theme.set_mono(s("Consolas"));
        theme.set_sans(s("Arial"));
    } else {
        theme.set_mono(s("DejaVu Sans Mono"));
        theme.set_sans(s("DejaVu Sans"));
    }
    let l = ui.global::<Lists>();
    l.set_place_kinds(model(catalog::PLACE_KINDS.iter().map(s).collect()));
    l.set_region_kinds(model(catalog::REGION_KINDS.iter().map(s).collect()));
    app.update_tools();
    app.update_editor();
    match std::env::args_os().nth(1) {
        Some(path) => app.open_path(Path::new(&path)),
        None => {
            app.regenerate();
            app.message("a new map; use Open to open a config", false);
        }
    }
    if let Some(text) = std::env::var_os("INKLAND_APP_SNAPSHOT_OPEN") {
        match inkland_app::snapshot::plan(&text.to_string_lossy()) {
            Ok(plan) => {
                let u = ui.global::<Ui>();
                u.set_panel_open(plan.tab.is_some());
                u.set_tab(plan.tab.unwrap_or(0) as i32);
                for tool in &plan.tools {
                    app.open_tool(tool);
                }
            }
            Err(e) => eprintln!("inkland-app: INKLAND_APP_SNAPSHOT_OPEN: {e}"),
        }
    }
    ui.run()
}
