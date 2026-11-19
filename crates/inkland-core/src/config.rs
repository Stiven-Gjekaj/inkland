//! The config of one map (version 1).
//!
//! `inkland schema` makes the JSON Schema from these types. The doc comment
//! of each field is its description in the schema, so each doc comment gives
//! the unit of the field.
//!
//! All coordinates are in config units (`units.name`), in the frame of
//! `frame`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The only config version that this tool reads.
pub const CONFIG_VERSION: u32 = 1;

/// A point `[x, y]` in config units.
pub type Point = [f64; 2];

/// The config of one map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Config format version. Must be 1.
    pub version: u32,
    /// Master seed. Each layer of generation gets its own seed from it.
    pub seed: u64,
    /// The distance unit of all coordinates and lengths.
    #[serde(default)]
    pub units: Units,
    /// The size and the axes of the world.
    pub frame: Frame,
    /// Generation options.
    #[serde(default)]
    pub world: World,
    /// Hard constraints. Generation fits around them and never moves them.
    #[serde(default)]
    pub pins: Pins,
    /// Named overlays. A variant shows the base world and one overlay.
    #[serde(default)]
    pub overlays: BTreeMap<String, Overlay>,
    /// The maps to render. Default: one variant `default`, full frame, on a
    /// 167 x 250 mm page.
    #[serde(default = "default_variants")]
    pub variants: Vec<Variant>,
    /// Render options for all variants.
    #[serde(default)]
    pub render: Render,
    /// More directories with font files for the PNG preview. The preview
    /// also has the free fonts of the style presets built in, and it uses
    /// the fonts of the system. The SVG never embeds a font.
    #[serde(default)]
    pub font_dirs: Vec<String>,
}

/// The distance unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Units {
    /// Name of the unit, for example `mile`. The report and the scale bar use
    /// this name.
    #[serde(default = "default_unit_name")]
    pub name: String,
    /// Wanted mean width of a cell, in units. Optional. When it is given, it
    /// sets the number of cells and replaces `world.cells`. The `--cells`
    /// flag removes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub per_cell_hint: Option<f64>,
}

impl Default for Units {
    fn default() -> Self {
        Self {
            name: default_unit_name(),
            per_cell_hint: None,
        }
    }
}

/// The frame of the world: its size and the direction of its axes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// The corner where x = 0 and y = 0.
    #[serde(default)]
    pub origin: Origin,
    /// The direction in which x increases. Only `east`.
    #[serde(default)]
    pub x_axis: XAxis,
    /// The direction in which y increases: `south` with origin `top-left`,
    /// `north` with origin `bottom-left`.
    #[serde(default)]
    pub y_axis: YAxis,
    /// Width of the world, in units. Must be more than 0.
    pub width: f64,
    /// Height of the world, in units. Must be more than 0.
    pub height: f64,
}

/// The corner of the origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    #[default]
    TopLeft,
    BottomLeft,
}

/// The direction of the x axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum XAxis {
    #[default]
    East,
}

/// The direction of the y axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum YAxis {
    #[default]
    South,
    North,
}

/// Generation options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct World {
    /// Number of cells in the first grid. 1000 to 100000.
    #[serde(default = "default_cells")]
    #[schemars(range(min = 1000, max = 100000))]
    pub cells: u32,
    /// The lattice of the points of the first grid: `square`, `hex-pointy`,
    /// `hex-flat`, `diamond`, or `triangle`.
    #[serde(default, skip_serializing_if = "Lattice::is_square")]
    pub lattice: Lattice,
    /// How far each point of the first grid can move from its lattice
    /// position, as a fraction of half the distance between neighbor
    /// points. 0 to 0.9. 0 gives a regular lattice.
    #[serde(default = "default_jitter", skip_serializing_if = "is_default_jitter")]
    #[schemars(range(min = 0.0, max = 0.9))]
    pub jitter: f64,
    /// Heightmap template: a template name, or a list of steps.
    #[serde(default)]
    pub template: Template,
    /// Sea level on the height scale 0 to 100. Cells below it are water.
    #[serde(default = "default_sea_level")]
    #[schemars(range(min = 1, max = 99))]
    pub sea_level: u8,
    /// `true`: all water of the template and the brushes becomes low land,
    /// 1 to 3 above sea level. Only the pinned `sea` and `lake` regions,
    /// and the lakes that rivers make, are water. Use it to keep the whole
    /// frame as land except the pinned seas. Default `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub fill_land: bool,
    /// `true`: the water of the template and the brushes gets deep: each
    /// height below sea level becomes 0.4 of it. The hills of the seabed
    /// then do not show as pale shallows in open ocean. The pinned `sea`
    /// and `lake` regions keep their own depth, and their `shelf` makes the
    /// shallow water along a coast. Default `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub deep_sea: bool,
    /// `false`: generation adds no roads or trails between the places.
    /// Only the pinned roads and the roads of the overlays stay. The sea
    /// routes between the ports stay. Default `true`.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub generated_roads: bool,
    /// The most rivers that generation keeps, besides the pinned rivers.
    /// It keeps the rivers with the most water. 0: only the pinned rivers.
    /// Default: all rivers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub river_limit: Option<u32>,
    /// A land low point becomes a lake when its water cannot reach other
    /// water through cells lower than its height plus this limit, on the
    /// height scale 0 to 100. 0 to 80. 80 makes no such lakes.
    #[serde(
        default = "default_lake_elevation_limit",
        skip_serializing_if = "is_default_lake_elevation_limit"
    )]
    #[schemars(range(min = 0, max = 80))]
    pub lake_elevation_limit: u8,
    /// Climate options.
    #[serde(default)]
    pub climate: Climate,
    /// Number of cultures.
    #[serde(default = "default_cultures")]
    pub cultures: u32,
    /// Name of the culture set: `world`, `european`, `oriental`, `english`,
    /// `antique`, `high-fantasy`, `dark-fantasy`, or `random`.
    #[serde(default = "default_culture_set")]
    pub culture_set: String,
    /// How much the sizes of cultures and states differ. 0 to 10. A bigger
    /// value gives a bigger random part of the expansionism.
    #[serde(default = "default_size_variety")]
    #[schemars(range(min = 0.0, max = 10.0))]
    pub size_variety: f64,
    /// How far cultures and states grow. 0.1 to 2. Cultures and states stop
    /// at a total cost that this value multiplies.
    #[serde(default = "default_growth_rate")]
    #[schemars(range(min = 0.1, max = 2.0))]
    pub growth_rate: f64,
    /// How much the state borders wander. 0 to 1. Fractal noise from the
    /// seed adds a cost to the growth of the states, so that a border does
    /// not follow a straight line. 0: the costs of Azgaar's Fantasy Map
    /// Generator. Default 0.5.
    #[serde(
        default = "default_border_noise",
        skip_serializing_if = "is_default_border_noise"
    )]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub border_noise: f64,
    /// How much the state borders follow the terrain. 0 to 1. A river cell
    /// and a ridge cell cost more to enter, so a state stops at a river or
    /// at a ridge, and the border goes along it. 0: the costs of Azgaar's
    /// Fantasy Map Generator. Default 0.5.
    #[serde(
        default = "default_border_terrain",
        skip_serializing_if = "is_default_border_terrain"
    )]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub border_terrain: f64,
    /// Number of states. A point pin with `capital_of` is the capital of
    /// its state; generation adds the other capitals as cities. 0 and no
    /// `capital_of`: the world has no states and no capitals. With
    /// `towns: 0` too, the only places are the pins.
    #[serde(default = "default_states")]
    pub states: u32,
    /// Number of towns that generation adds, or `auto`.
    #[serde(default)]
    pub towns: Towns,
    /// Provinces for each 100 places of a state. 0 to 100. A state gets at
    /// least 2 provinces when it has 2 places or more. 100: each place is
    /// the seat of a province.
    #[serde(default = "default_provinces_ratio")]
    #[schemars(range(min = 0, max = 100))]
    pub provinces_ratio: u8,
    /// Number of religions.
    #[serde(default = "default_religions")]
    pub religions: u32,
    /// The shield shape of the coats of arms: `culture` (the shield of the
    /// culture of each item, as Azgaar's Fantasy Map Generator does),
    /// `state` (the shield of the state), `random` (a random shield for
    /// each culture), or one shield name for all, for example `heater`,
    /// `spanish`, `round`, or `banner`.
    #[serde(
        default = "default_emblem_shape",
        skip_serializing_if = "is_default_emblem_shape"
    )]
    pub emblem_shape: String,
    /// Options for places that generation adds.
    #[serde(default)]
    pub places: PlaceOptions,
    /// Heightmap brush strokes, applied in order after the template and
    /// before the pins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub brushes: Vec<Brush>,
    /// Land or sea along each frame edge, applied after the brushes and
    /// before the pins. Default: the template decides.
    #[serde(default, skip_serializing_if = "FrameEdges::is_empty")]
    pub edges: FrameEdges,
    /// Changes to the goods catalogue of Azgaar's Fantasy Map Generator, in
    /// order. Empty: the default catalogue of 71 goods.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goods: Vec<GoodEntry>,
    /// Changes to the military units of Azgaar's Fantasy Map Generator, in
    /// order. Empty: the default roster of 6 units.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub military_units: Vec<UnitEntry>,
    /// Changes to the journey transports of Azgaar's Fantasy Map Generator,
    /// in order. Empty: the default list of 20 transports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transports: Vec<TransportEntry>,
    /// Names and arms for generated states, cultures, religions, and
    /// provinces. Generation applies them after the coats of arms, so the
    /// later stages use the new names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub renames: Vec<RenameEntry>,
    /// Notes of the map, as the Notes editor of Azgaar's Fantasy Map
    /// Generator writes them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<NoteEntry>,
    /// `false`: no default ruler across the largest landmass.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub default_ruler: bool,
    /// Measurers after the default ruler: rulers, opisometers, and
    /// planimeters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measurers: Vec<MeasurerEntry>,
    /// Legend boxes: a title and colored items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legends: Vec<LegendEntry>,
    /// Changes to the zones: new zones, and changes to or removal of the
    /// generated zones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<ZoneEntry>,
    /// Changes to the finance of the states: taxes, the market fee, the
    /// budget that sets the state orders, and the tariffs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finance: Vec<FinanceEntry>,
    /// Changes to the markets: add, move, or remove a market center.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markets: Vec<MarketEntry>,
    /// Changes to the regiments: add, change, move, split, or remove.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regiments: Vec<RegimentEntry>,
    /// Changes to the journeys: add a journey, or remove one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub journeys: Vec<JourneyEntry>,
}

/// A change to the journeys. With `remove`, the entry removes the journey
/// `id`. Else it adds a journey through its stops, in order: one segment
/// for each pair of stops, with one transport.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JourneyEntry {
    /// The journey to remove.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// Remove the journey `id`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove: bool,
    /// The name. Default: the first and the last stop, with an arrow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The stops `[[x, y], ...]`, in units: the cell at each point. 2 or
    /// more.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stops: Vec<Point>,
    /// The name of the transport. Default: the first land transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
}

/// A change to the regiments. With `add`, the entry adds a regiment of the
/// state that holds `at`, at that point; it must give `units`. Else it
/// changes the regiment nearest to `at`: its name, units, or fleet flag,
/// moves it with `to`, splits it in two halves with `split`, or removes it
/// with `remove`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegimentEntry {
    /// A point `[x, y]`, in units.
    pub at: Point,
    /// Add a new regiment.
    #[serde(default, skip_serializing_if = "is_false")]
    pub add: bool,
    /// The name. Default for a new regiment: the name that FMG gives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The troops of each unit, in people, by unit name. A unit that the
    /// entry does not name keeps its troops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<BTreeMap<String, f64>>,
    /// `true`: a fleet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub naval: Option<bool>,
    /// Move the regiment to this point `[x, y]`. Its base stays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Point>,
    /// Split the regiment in two halves. The new one stands below.
    #[serde(default, skip_serializing_if = "is_false")]
    pub split: bool,
    /// Remove the regiment.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove: bool,
}

/// A change to the market centers. `at` names the place nearest to the
/// point. Without `remove` and `to`, the entry adds a market with this
/// place as its center. With `remove`, it removes the market of this
/// center. With `to`, it moves the market of this center to the place
/// nearest to `to`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarketEntry {
    /// A point `[x, y]`: the place nearest to it.
    pub at: Point,
    /// Remove the market of the place.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove: bool,
    /// The new center: the place nearest to this point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Point>,
}

/// The budget of a state, in percents of the taxes collected. A value that
/// the entry does not give keeps the generated value.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BudgetEntry {
    /// The court, in percent. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub court: Option<f64>,
    /// The administration, in percent. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub administration: Option<f64>,
    /// The infrastructure, in percent. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infrastructure: Option<f64>,
    /// The military, in percent. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub military: Option<f64>,
    /// The percent of each expense that officials pocket. 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corruption: Option<f64>,
}

/// The rate of a tariff: a share of the price, or `embargo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TariffRate {
    /// A share of the price, 0 or more.
    Rate(f64),
    /// The word `embargo`: no import.
    Word(String),
}

/// An import rule of a state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TariffEntry {
    /// The point pins in the states that the rule is for. Default: all
    /// states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub states: Option<Vec<String>>,
    /// The names of the goods that the rule is for. Default: all goods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goods: Option<Vec<String>>,
    /// The rate: a share of the price, or `embargo`.
    pub rate: TariffRate,
}

/// A change to the finance of a state. The entry finds its state by the
/// point `at` or by the point pin `pin`. Give exactly one.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FinanceEntry {
    /// A point `[x, y]` in the state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    /// The id of a point pin in the state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// The sales tax, a share of the price. 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sales_tax: Option<f64>,
    /// The poll tax a day for each population point. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_tax: Option<f64>,
    /// The license fee a day from each market of the state. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_fee: Option<f64>,
    /// The budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<BudgetEntry>,
    /// The tariffs. A list replaces the generated tariffs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tariffs: Option<Vec<TariffEntry>>,
}

/// A change to the zones. With a shape (`polygon`, `box`, or `circle`),
/// the entry adds a zone of the cells in the shape; it must give `name`
/// and `kind`. With `at`, it changes the generated zone that holds the
/// point, or removes it with `remove`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoneEntry {
    /// A point `[x, y]` in the generated zone to change or to remove.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    /// Remove the zone at `at`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove: bool,
    /// The outline of a new zone `[[x, y], ...]`, in units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<Point>>,
    /// The rectangle of a new zone `[x0, y0, x1, y1]`, in units.
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub rect: Option<[f64; 4]>,
    /// The circle of a new zone, in units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circle: Option<Circle>,
    /// The name of the zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The zone type, for example `Invasion`, `Disease`, or `Flood`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The fill: a hatch pattern of FMG, for example `url(#hatch1)`, or a
    /// color as `#rrggbb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The legend text of the zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

/// A note of the map.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoteEntry {
    /// The title of the note.
    pub name: String,
    /// The text of the note.
    #[serde(default)]
    pub note: String,
}

/// A measurer: its kind and its points `[x, y]`, in units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeasurerEntry {
    /// The kind: `ruler`, `opisometer`, `route-opisometer`, or
    /// `planimeter`.
    pub kind: crate::annotations::MeasurerKind,
    /// 2 or more points. A planimeter closes its points into an area.
    pub points: Vec<Point>,
}

/// One item of a legend box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LegendItemEntry {
    /// The color of the swatch, as `#rrggbb`.
    pub color: String,
    /// The text of the item.
    pub text: String,
}

/// A legend box.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LegendEntry {
    /// The title of the box.
    pub name: String,
    /// The items, in order.
    #[serde(default)]
    pub items: Vec<LegendItemEntry>,
    /// The top left corner of the box `[x, y]`, in units. Default: the
    /// next free place in the bottom right of the map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
}

/// The kind of a generated area that a rename names.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AreaKind {
    #[default]
    State,
    Culture,
    Religion,
    Province,
}

impl AreaKind {
    pub fn name(self) -> &'static str {
        match self {
            AreaKind::State => "state",
            AreaKind::Culture => "culture",
            AreaKind::Religion => "religion",
            AreaKind::Province => "province",
        }
    }
}

/// A name or arms for a generated area. The entry finds its area by a
/// point: the area of its kind that holds the cell at `at`, or at the point
/// pin `pin`. Give exactly one of `at` and `pin`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameEntry {
    /// The kind of the area.
    pub kind: AreaKind,
    /// A point `[x, y]` in the area, in units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    /// The id of a point pin in the area, for example its capital.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    /// The new name. Not empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The new full name of a state or a province, for example
    /// `Kingdom of Ostrey`. Default: the full name with the old name
    /// changed to the new name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    /// The coat of arms of a state or a province, in the JSON shape of
    /// Azgaar's Fantasy Map Generator. Without `shield`, the arms keep the
    /// generated shield shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coa: Option<crate::emblems::Coa>,
    /// The color of a state, as `#rrggbb`. It replaces the generated color
    /// in the colour SVG and in `world.json`. Only for a state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(regex(pattern = r"^#[0-9a-fA-F]{6}$"))]
    pub color: Option<String>,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// A change to the goods catalogue. An entry with the name of a good in the
/// list changes the values that it gives, or removes the good with
/// `remove`. An entry with a new name adds a good at the end of the list.
/// The good ids follow the order of the list that results.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoodEntry {
    /// The name of the good, for example `Wood`.
    pub name: String,
    /// Remove the good from the list.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove: bool,
    /// Tags, for example `construction`, `fuel`, `ore`, `luxury`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// The icon id, for example `goods-wood`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The color, as `#rrggbb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The base price. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The price to carry one unit over a decima. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carriage: Option<f64>,
    /// The unit name, for example `pile`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The least and the most deposits on the map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deposits: Option<[f64; 2]>,
    /// The spread expression of the deposits, for example
    /// `biome(5, 6) && minHeight(30)`. An empty text gives no deposits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distribution: Option<String>,
    /// The least development that a cell needs to make the good. 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub development: Option<u8>,
    /// The multiplier on the output of each deposit. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deposit_yield: Option<f64>,
    /// The chance that a deposit lands next to the one before it. 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clustering: Option<f64>,
    /// Units for each rural population point, by biome id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biome_output: Option<BTreeMap<u8, f64>>,
    /// How much of each need one unit covers: `food`, `utilities`,
    /// `construction`, `military`, `luxury`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<BTreeMap<String, f64>>,
    /// The production factors by culture type, for example `Hunting`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub culture_type: Option<BTreeMap<String, f64>>,
    /// Alternative input sets: the amount of each good, by good name. Each
    /// recipe makes one unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipes: Option<Vec<BTreeMap<String, f64>>>,
}

/// The type of a military unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UnitKind {
    Levies,
    Melee,
    Ranged,
    Mounted,
    Machinery,
    Naval,
}

impl UnitKind {
    /// The FMG name of the type.
    pub fn name(self) -> &'static str {
        match self {
            UnitKind::Levies => "levies",
            UnitKind::Melee => "melee",
            UnitKind::Ranged => "ranged",
            UnitKind::Mounted => "mounted",
            UnitKind::Machinery => "machinery",
            UnitKind::Naval => "naval",
        }
    }
}

/// A change to the military units. An entry with the name of a unit in the
/// list changes the values that it gives, or removes the unit with
/// `remove`. An entry with a new name adds a unit; it must give `kind`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnitEntry {
    /// The name of the unit, for example `archers`.
    pub name: String,
    /// Remove the unit from the list.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove: bool,
    /// The glyph of the icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The percent of the rural people that serve. 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rural: Option<f64>,
    /// The percent of the urban people that serve. 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub urban: Option<f64>,
    /// The people of one unit. More than 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crew: Option<f64>,
    /// The battle power of one unit. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<f64>,
    /// The unit type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<UnitKind>,
    /// `true` if the unit forms regiments of its own, as fleets do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separate: Option<bool>,
    /// The names of the goods that equip the unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goods: Option<Vec<String>>,
}

/// The travel domain of a transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TransportDomain {
    Land,
    Water,
    Air,
    Stay,
}

/// A change to the journey transports. An entry with the name of a
/// transport in the list changes the values that it gives, or removes the
/// transport with `remove`. An entry with a new name adds a transport; it
/// must give `speed` and `domain`. The list must keep a transport of the
/// domain `stay`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransportEntry {
    /// The name of the transport, for example `Rowboat`.
    pub name: String,
    /// Remove the transport from the list.
    #[serde(default, skip_serializing_if = "is_false")]
    pub remove: bool,
    /// The speed in km/h. 0 or more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    /// The travel domain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<TransportDomain>,
    /// The hours of travel that one day sustains. More than 0, to 24.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hours_per_day: Option<f64>,
}

/// One heightmap brush stroke: it adds `delta` at its centre, and less with
/// distance, to 0 at `radius`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Brush {
    /// Centre `[x, y]`, in units.
    pub at: Point,
    /// Radius, in units. More than 0.
    pub radius: f64,
    /// Height change at the centre, on the 0 to 100 scale. -100 to 100.
    pub delta: f64,
}

/// Land or sea along the frame edges. The compass sides follow the axes of
/// the frame.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrameEdges {
    /// The terrain along the north edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub north: Option<EdgeKind>,
    /// The terrain along the south edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub south: Option<EdgeKind>,
    /// The terrain along the west edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub west: Option<EdgeKind>,
    /// The terrain along the east edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub east: Option<EdgeKind>,
    /// The mean width of the band along an edge, in units. Fractal noise
    /// makes the inner side of the band a natural coast. More than 0.
    /// Default: a tenth of the smaller side of the frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub width: Option<f64>,
}

impl FrameEdges {
    pub fn is_empty(&self) -> bool {
        self.north.is_none() && self.south.is_none() && self.west.is_none() && self.east.is_none()
    }
}

/// The terrain along a frame edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    /// Open sea: the band is water, deeper at the edge.
    Sea,
    /// Land that goes on past the frame: the water of the band becomes low
    /// land.
    Land,
}

impl Default for World {
    fn default() -> Self {
        Self {
            cells: default_cells(),
            lattice: Lattice::default(),
            jitter: default_jitter(),
            template: Template::default(),
            sea_level: default_sea_level(),
            river_limit: None,
            fill_land: false,
            deep_sea: false,
            generated_roads: true,
            lake_elevation_limit: default_lake_elevation_limit(),
            climate: Climate::default(),
            cultures: default_cultures(),
            culture_set: default_culture_set(),
            size_variety: default_size_variety(),
            growth_rate: default_growth_rate(),
            border_noise: default_border_noise(),
            border_terrain: default_border_terrain(),
            states: default_states(),
            towns: Towns::default(),
            provinces_ratio: default_provinces_ratio(),
            religions: default_religions(),
            emblem_shape: default_emblem_shape(),
            places: PlaceOptions::default(),
            brushes: Vec::new(),
            edges: FrameEdges::default(),
            goods: Vec::new(),
            military_units: Vec::new(),
            transports: Vec::new(),
            renames: Vec::new(),
            notes: Vec::new(),
            default_ruler: true,
            measurers: Vec::new(),
            legends: Vec::new(),
            zones: Vec::new(),
            finance: Vec::new(),
            markets: Vec::new(),
            regiments: Vec::new(),
            journeys: Vec::new(),
        }
    }
}

/// The lattice of the points of the first grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Lattice {
    /// Rows and columns of squares.
    #[default]
    Square,
    /// Hexagons with a point at the top. Odd rows move half a step.
    HexPointy,
    /// Hexagons with a flat top.
    HexFlat,
    /// Squares turned 45 degrees.
    Diamond,
    /// Triangles that point up and down in turn.
    Triangle,
}

impl Lattice {
    pub fn is_square(&self) -> bool {
        *self == Lattice::Square
    }
}

/// A heightmap template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Template {
    /// A named template.
    Named(TemplateName),
    /// A list of template steps, one step in each string, for example
    /// `"Hill 1 90-99 60-80 45-55"`.
    Steps {
        /// The steps, in order.
        steps: Vec<String>,
    },
    /// A raster of heights, for example from an imported map.
    Raster {
        /// The raster. Each cell gets the height of the raster value nearest
        /// to its site. The lake stages do not change these heights.
        raster: Raster,
    },
    /// A PNG image of heights, as a raster. The path is relative to the
    /// folder of the config file. Black is height 0 and white is height
    /// 100. A colour image gives the brightness of each pixel. The image
    /// covers the full frame: its top row is at the low y edge. Like a
    /// `raster`, the lake stages do not change these heights. A change of
    /// the image does not change the config hash.
    Image {
        /// The path of the PNG file.
        image: String,
        /// The decoded heights. The program that reads the config file
        /// reads the image and sets them.
        #[serde(skip)]
        #[schemars(skip)]
        heights: Option<Raster>,
    },
}

impl Template {
    /// The raster of heights of the template: the `raster`, or the decoded
    /// heights of an `image`.
    pub fn raster(&self) -> Option<&Raster> {
        match self {
            Template::Raster { raster } => Some(raster),
            Template::Image { heights, .. } => heights.as_ref(),
            _ => None,
        }
    }

    /// `true` for a template of heights: a `raster` or an `image`.
    pub fn is_raster(&self) -> bool {
        matches!(self, Template::Raster { .. } | Template::Image { .. })
    }
}

/// A raster of heights that covers the full frame.
///
/// The raster divides the frame into `cols` x `rows` equal rectangles. Each
/// value is the height of one rectangle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Raster {
    /// Number of columns, from the low x edge to the high x edge. 2 or more.
    #[schemars(range(min = 2))]
    pub cols: u32,
    /// Number of rows, from the low y edge to the high y edge. 2 or more.
    #[schemars(range(min = 2))]
    pub rows: u32,
    /// The heights on the scale 0 to 100, row by row. The first row is at
    /// the low y edge of the frame. In each row, the first value is at the
    /// low x edge. The list has `cols` x `rows` values.
    pub values: Vec<u8>,
}

impl Raster {
    /// The value of the rectangle that contains the position `[x, y]` of a
    /// frame of `width` x `height` units. A position outside the frame
    /// gets the value of the nearest edge rectangle. The raster must have
    /// `cols` x `rows` values.
    pub fn sample(&self, x: f64, y: f64, width: f64, height: f64) -> u8 {
        let index = |v: f64, size: f64, n: u32| -> usize {
            let i = (v / size * f64::from(n)).floor();
            if i.is_nan() || i < 0.0 {
                0
            } else {
                (i as usize).min(n as usize - 1)
            }
        };
        let col = index(x, width, self.cols);
        let row = index(y, height, self.rows);
        self.values[row * self.cols as usize + col]
    }
}

impl Default for Template {
    fn default() -> Self {
        Template::Named(TemplateName::Continents)
    }
}

/// The names of the built-in heightmap templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateName {
    Volcano,
    HighIsland,
    LowIsland,
    Continents,
    Archipelago,
    Atoll,
    Mediterranean,
    Peninsula,
    Pangea,
    Isthmus,
    Shattered,
    Taklamakan,
    OldWorld,
    Fractious,
}

/// Climate options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Climate {
    /// Latitude of the south edge and of the north edge, in degrees.
    #[serde(default = "default_latitude_range")]
    pub latitude_range: [f64; 2],
    /// The size and the position of the map on the globe, as in FMG. If it
    /// is given, it sets the latitudes, and `latitude_range` has no effect.
    /// A value that it does not give comes from a random draw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geography: Option<Geography>,
    /// Winds, one for each 30 degree latitude band from 90 north to 90
    /// south: the direction to which the wind blows, in degrees clockwise
    /// from north. Default: `[225, 45, 225, 315, 135, 315]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wind: Option<[f64; 6]>,
    /// A climate for the whole map: `temperate`, `cool`, `cold`, `warm`, or
    /// `tropical`. It sets the sea-level temperature at the middle of the
    /// map. The latitudes still make the north and the south a little
    /// different, and the height still cools the land. Without it, the
    /// latitudes alone set the temperature, as in FMG, and a map at 50
    /// degrees is near 0 C at sea level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<ClimatePreset>,
    /// The sea-level temperature at the middle of the map, in degrees C.
    /// -30 to 40. It replaces the temperature of `preset`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = -30.0, max = 40.0))]
    pub temperature: Option<f64>,
}

impl Default for Climate {
    fn default() -> Self {
        Self {
            latitude_range: default_latitude_range(),
            geography: None,
            wind: None,
            preset: None,
            temperature: None,
        }
    }
}

impl Climate {
    /// The sea-level temperature at the middle of the map that `temperature`
    /// or `preset` sets, in degrees C.
    pub fn middle_temperature(&self) -> Option<f64> {
        self.temperature
            .or(self.preset.map(ClimatePreset::temperature))
    }
}

/// A climate for the whole map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ClimatePreset {
    /// 18 C at sea level: dry grassland, savanna, and broadleaf forest.
    Warm,
    /// 11 C: broadleaf forest and grassland, taiga only on high ground.
    Temperate,
    /// 5 C: taiga and tundra on high ground.
    Cool,
    /// -2 C: taiga and tundra, glacier on high ground.
    Cold,
    /// 26 C: rainforest, savanna, and hot desert.
    Tropical,
}

impl ClimatePreset {
    /// The sea-level temperature at the middle of the map, in degrees C.
    pub fn temperature(self) -> f64 {
        match self {
            ClimatePreset::Warm => 18.0,
            ClimatePreset::Temperate => 11.0,
            ClimatePreset::Cool => 5.0,
            ClimatePreset::Cold => -2.0,
            ClimatePreset::Tropical => 26.0,
        }
    }
}

/// The size and the position of the map on the globe.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geography {
    /// The map size in percent of the world. 1 to 100. 100 is the full
    /// globe from pole to pole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1.0, max = 100.0))]
    pub map_size: Option<f64>,
    /// The north-south position in percent. 0 to 100. 0 puts the north
    /// edge at the north pole, and 100 puts the south edge at the south
    /// pole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 100.0))]
    pub latitude: Option<f64>,
    /// The west-east position in percent. 0 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 100.0))]
    pub longitude: Option<f64>,
}

/// A count, or `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Towns {
    /// Generation chooses the number from the land area.
    Auto(AutoKeyword),
    /// A fixed number of towns.
    Count(u32),
}

impl Default for Towns {
    fn default() -> Self {
        Towns::Auto(AutoKeyword::Auto)
    }
}

/// The keyword `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AutoKeyword {
    Auto,
}

/// Options for places that generation adds.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceOptions {
    /// Smallest distance from a generated place to a pinned place, in units.
    /// Default: three times the mean cell width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub min_distance_from_pins: Option<f64>,
}

/// Hard constraints.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pins {
    /// Places at exact coordinates.
    #[serde(default)]
    pub points: Vec<PointPin>,
    /// Areas with a forced kind of terrain.
    #[serde(default)]
    pub regions: Vec<RegionPin>,
    /// Rivers with a fixed source, course, and mouth.
    #[serde(default)]
    pub rivers: Vec<RiverPin>,
    /// Roads between pinned places, with an optional target length.
    #[serde(default)]
    pub roads: Vec<RoadPin>,
    /// Soft areas with a character of the land, such as rough or wooded.
    /// An area has no fixed height range. It steers the heights, the
    /// biomes, and so the relief symbols, and fades out at its loose edge.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<AreaPin>,
}

/// A soft area with a character of the land. Give exactly one of
/// `polygon`, `box`, and `circle`. The outline is loose: fractal noise moves
/// it, and the effect fades out over the `edge` outside it. An area changes
/// only land: the coast stays where it is. Inside a land region, the region
/// sets the height range, and the area shapes the land in that range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AreaPin {
    /// Identifier: lower-case letters and digits, words joined by `-`.
    /// Unique across all pins.
    #[schemars(regex(pattern = r"^[a-z0-9]+(-[a-z0-9]+)*$"))]
    pub id: String,
    /// The character of the land in the area.
    pub character: Character,
    /// Outline `[[x, y], ...]`, in units. 3 or more points. The outline must
    /// not cross itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<Point>>,
    /// Rectangle `[x0, y0, x1, y1]`, in units.
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub rect: Option<[f64; 4]>,
    /// Circle, in units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circle: Option<Circle>,
    /// How strong the character is. 0 to 1. Default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub strength: Option<f64>,
    /// Width of the soft edge outside the outline, in units. The effect
    /// fades out over this width. Default: a fifth of the size of the shape
    /// (the square root of the area of its bounding box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub edge: Option<f64>,
}

/// The character of the land in an area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Character {
    /// Broken, high land: the heights rise, with sharp ridges. The relief
    /// shows mountain chains and hills.
    Rough,
    /// Forest: the land keeps its heights, and the biome becomes forest.
    Wooded,
    /// Rolling land: low, rounded hills.
    Rolling,
    /// Flat land: the heights go down to low land, with few hills.
    Flat,
}

/// A place at exact coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointPin {
    /// Identifier: lower-case letters and digits, words joined by `-`.
    /// Unique across all pins.
    #[schemars(regex(pattern = r"^[a-z0-9]+(-[a-z0-9]+)*$"))]
    pub id: String,
    /// Kind of place.
    pub kind: PlaceKind,
    /// Position `[x, y]`, in units. The place is put here exactly.
    pub at: Point,
    /// Name of the place. Default: a generated name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `true`: the cell of the place touches the sea or a lake.
    #[serde(default)]
    pub port: bool,
    /// `true`: the place can be inside a `sea` or `lake` region. The cell
    /// of the place becomes a small island of land. Only for `pins.points`.
    /// For a larger island, put a land region with a higher `priority`
    /// than the sea around the place.
    #[serde(default, skip_serializing_if = "is_false")]
    pub island: bool,
    /// Name of a state. The state forms around this place, its capital.
    /// A later pin with the same name is a seat of the state: the state
    /// also grows from it, for example on the other side of a sea.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capital_of: Option<String>,
    /// Name of the kind. Required when `kind` is `custom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_kind: Option<String>,
    /// The coat of arms of this place, in the JSON shape of Azgaar's
    /// Fantasy Map Generator. It replaces the generated arms, and the
    /// generated arms of a province with this place as its seat take from
    /// it. Without `shield`, the arms get the generated shield shape. An
    /// overlay place does not use it. Default: generated arms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coa: Option<crate::emblems::Coa>,
    /// The coat of arms of the state that this place is the capital of
    /// (`capital_of`), in the same shape as `coa`. The generated arms of the
    /// places and provinces of the state take from it. Default: generated
    /// arms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_coa: Option<crate::emblems::Coa>,
    /// How far the state of this capital (`capital_of`) grows, 0.1 to 10.
    /// It divides the cost of each cell that the state takes, so a bigger
    /// value gives a bigger state. It replaces the random expansionism of
    /// `world.size_variety`. On a seat, it counts only when the capital
    /// gives none. Default: random.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.1, max = 10.0))]
    pub expansionism: Option<f64>,
    /// A soft claim of the state of this capital (`capital_of`): a polygon
    /// of 3 or more points `[x, y]`, in units. The state grows into it
    /// first, and the other states take its cells only at a high cost. The
    /// claims of a capital and its seats add together. Default: no claim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<Vec<Point>>,
}

/// Kinds of places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PlaceKind {
    City,
    Town,
    Village,
    Ruin,
    Fortress,
    Custom,
}

/// An area with a forced kind of terrain. Give exactly one of `polygon`,
/// `box`, and `circle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegionPin {
    /// Identifier: lower-case letters and digits, words joined by `-`.
    /// Unique across all pins.
    #[schemars(regex(pattern = r"^[a-z0-9]+(-[a-z0-9]+)*$"))]
    pub id: String,
    /// Kind of terrain in the area.
    pub kind: RegionKind,
    /// Outline `[[x, y], ...]`, in units. 3 or more points. The outline must
    /// not cross itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<Point>>,
    /// Rectangle `[x0, y0, x1, y1]`, in units.
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub rect: Option<[f64; 4]>,
    /// Circle, in units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circle: Option<Circle>,
    /// Width of the soft edge outside the shape, in units. Heights blend
    /// from the forced range to the free terrain over this width. Inside the
    /// shape, the heights are always in the range.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub edge: f64,
    /// Height range `[min, max]` on the height scale 0 to 100 (sea level 20).
    /// Default: the range of the kind: `sea` and `lake` 5 to 15, `marsh` 20
    /// to 23, `plains` 21 to 35, `forest` 21 to 45, `desert` 21 to 50,
    /// `hills` 35 to 60, `mountains` 70 to 100, `land` 20 to 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<[f64; 2]>,
    /// Only for `sea` and `lake`. `false`: a `sea` must connect to the ocean
    /// at a frame edge. `true`: an inland sea or a large lake.
    #[serde(default)]
    pub landlocked: bool,
    /// Where regions overlap, only the regions with the highest priority
    /// set the height range of a cell. Regions with the same priority share
    /// it: the cell gets a height in all of their ranges. Give an inner
    /// region a higher priority than the region around it, for example hills
    /// in a lowland. Default 0.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub priority: i32,
    /// The largest distance, in units, by which fractal noise moves the
    /// outline in or out. The coast becomes natural, with bays and capes.
    /// The largest bays are about 4 times this size. A side of the outline
    /// on a frame edge does not move. Near each pinned place (a point pin
    /// or a place of an overlay), the noise fades to 0 within 3 times this
    /// value, so a place on the coast stays on it. 0: the outline is exact.
    /// Default: 0
    /// for `sea` and `lake`; for the other kinds, 3 percent of the size of
    /// the shape (the square root of the area of its bounding box), at most
    /// 20, so that a box does not show as a rectangle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub roughness: Option<f64>,
    /// The largest distance, in units, by which long, slow bends move the
    /// outline in or out. The bends are about 6 times this size, so the
    /// coast can wander far from the drawn line at map scale. Near each
    /// pinned place, the bends fade to 0 within 2 times this value, so the
    /// ports and the coast towns stay on the coast. A side of the outline
    /// on a frame edge does not move. Default 0: no bends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub wander: Option<f64>,
    /// The number of noise layers of a rough outline. 1 to 8. Each layer
    /// adds bays and capes of half the size of the layer before it. 1
    /// gives a smooth, wavy coast. 8 gives a very detailed coast. Has an
    /// effect only with a `roughness`. Default 4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 8))]
    pub coast_detail: Option<u32>,
    /// Not for `sea` and `lake`. How many small islands lie off a rough
    /// coast. 0 to 1. Most of them lie off the capes. 0: none. Has an
    /// effect only with a roughness. Default: 0.3 for `land`, 0 for the
    /// other kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub islets: Option<f64>,
    /// Only for `sea` and `lake`. The width, in units, of the shallow water
    /// along the coast. The water is at the top of the height range at the
    /// coast and goes down to the bottom of the range at this distance from
    /// it. A side on a frame edge is not coast. More than 0. Default: the
    /// heights of the template, put into the range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub shelf: Option<f64>,
    /// Name of the area, for its label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A circle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Circle {
    /// Center `[x, y]`, in units.
    pub at: Point,
    /// Radius, in units.
    #[schemars(range(min = 0.0))]
    pub r: f64,
}

/// Kinds of forced terrain.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegionKind {
    Sea,
    Lake,
    Mountains,
    Hills,
    Plains,
    Marsh,
    Forest,
    Desert,
    /// Land of any height. Without a `height`, the land of the template
    /// keeps its height, and the water of the template becomes low land:
    /// deeper water gives a little higher land. Use it to draw a coast on a
    /// template with much sea.
    Land,
}

impl RegionKind {
    /// `true` for kinds that force water.
    pub fn is_water(self) -> bool {
        matches!(self, RegionKind::Sea | RegionKind::Lake)
    }

    /// The default height range `[min, max]` of the kind, on the height
    /// scale 0 to 100 with sea level at 20. A water range ends below 20.
    pub fn default_height(self) -> [f64; 2] {
        match self {
            RegionKind::Sea | RegionKind::Lake => [5.0, 15.0],
            RegionKind::Marsh => [20.0, 23.0],
            RegionKind::Plains => [21.0, 35.0],
            RegionKind::Hills => [35.0, 60.0],
            RegionKind::Mountains => [70.0, 100.0],
            RegionKind::Forest => [21.0, 45.0],
            RegionKind::Desert => [21.0, 50.0],
            RegionKind::Land => [20.0, 100.0],
        }
    }
}

impl RegionPin {
    /// The roughness of the outline: `roughness`, or the default of the
    /// kind for a shape with the bounding box `[x0, y0, x1, y1]`.
    pub fn roughness_for(&self, bbox: [f64; 4]) -> f64 {
        match self.roughness {
            Some(r) => r,
            None if self.kind.is_water() => 0.0,
            None => {
                let size = ((bbox[2] - bbox[0]).max(0.0) * (bbox[3] - bbox[1]).max(0.0)).sqrt();
                (0.03 * size).min(20.0)
            }
        }
    }

    /// The height range of the region: `height` if it is given, else the
    /// default range of the kind.
    pub fn height_range(&self) -> [f64; 2] {
        self.height.unwrap_or_else(|| self.kind.default_height())
    }
}

/// A river with a fixed source, course, and mouth. A river has no height:
/// generation calculates the heights so that the river flows downhill.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RiverPin {
    /// Identifier: lower-case letters and digits, words joined by `-`.
    /// Unique across all pins.
    #[schemars(regex(pattern = r"^[a-z0-9]+(-[a-z0-9]+)*$"))]
    pub id: String,
    /// Name of the river.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Where the river starts: a point, a place, or a region.
    pub source: Anchor,
    /// Points or places that the river passes, in order.
    #[serde(default)]
    pub through: Vec<Anchor>,
    /// Where the river ends: a point in water, a port place, or a region of
    /// kind `sea`, `lake`, or `marsh`.
    pub mouth: Anchor,
    /// `true`: the main river. Its water flow and its width are at least
    /// those of the largest other river, so it is drawn at least as wide.
    #[serde(default, skip_serializing_if = "is_false")]
    pub main: bool,
}

/// A position: a point, a pinned place, or a pinned region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Anchor {
    /// A point `[x, y]`, in units.
    Point(Point),
    /// A pinned place.
    Place(PlaceRef),
    /// A pinned region.
    Region(RegionRef),
}

/// A reference to a pinned place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceRef {
    /// The id of a point pin.
    pub place: String,
}

/// A reference to a pinned region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegionRef {
    /// The id of a region pin.
    pub region: String,
}

/// A road between two pinned places.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoadPin {
    /// Identifier: lower-case letters and digits, words joined by `-`.
    /// Unique across all pins.
    #[schemars(regex(pattern = r"^[a-z0-9]+(-[a-z0-9]+)*$"))]
    pub id: String,
    /// The id of the place where the road starts.
    pub from: String,
    /// The id of the place where the road ends.
    pub to: String,
    /// Places (ids) or points `[x, y]` in units that the road passes, in
    /// order.
    #[serde(default)]
    pub via: Vec<Waypoint>,
    /// Kind of road.
    #[serde(default)]
    pub kind: RoadKind,
    /// Wanted length of the road, in units. Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub target_length: Option<f64>,
    /// Permitted difference from `target_length`, as a fraction (0.05 is
    /// 5 percent).
    #[serde(default = "default_tolerance")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub tolerance: f64,
}

/// A point that a road passes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Waypoint {
    /// The id of a point pin.
    Place(String),
    /// A point `[x, y]`, in units.
    Point(Point),
}

/// Kinds of roads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RoadKind {
    #[default]
    Road,
    Trail,
}

/// An overlay: what one variant adds to, removes from, or renames in the
/// base world. An overlay cannot change terrain, rivers, or coasts.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Overlay {
    /// Items that only this overlay shows.
    #[serde(default)]
    pub add: OverlayAdd,
    /// Items to hide, as `"<kind>:<id>"`, for example `"place:ostrey"`.
    #[serde(default)]
    pub remove: Vec<String>,
    /// New names, from `"<kind>:<id>"` to the name.
    #[serde(default)]
    pub rename: BTreeMap<String, String>,
    /// Label options, from `"<kind>:<id>"` to the options. The key of a
    /// state label is `"state:<id>"`, with the id of the point pin that is
    /// the capital of the state, for example `"state:solmere"`.
    #[serde(default)]
    pub labels: BTreeMap<String, LabelOptions>,
}

/// Items that an overlay adds.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverlayAdd {
    /// Places. They do not change the terrain.
    #[serde(default)]
    pub places: Vec<PointPin>,
    /// Features such as ruins, battle sites, and old borders.
    #[serde(default)]
    pub features: Vec<Feature>,
    /// Roads, routed on the shared terrain.
    #[serde(default)]
    pub roads: Vec<RoadPin>,
}

/// A feature that an overlay adds. Give exactly one of `at`, `polyline`,
/// and `polygon`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    /// Identifier: lower-case letters and digits, words joined by `-`.
    #[schemars(regex(pattern = r"^[a-z0-9]+(-[a-z0-9]+)*$"))]
    pub id: String,
    /// Kind of feature.
    pub kind: FeatureKind,
    /// Position `[x, y]`, in units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    /// Line `[[x, y], ...]`, in units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polyline: Option<Vec<Point>>,
    /// Outline `[[x, y], ...]`, in units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<Point>>,
    /// Name of the feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Kinds of overlay features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FeatureKind {
    Ruin,
    BattleSite,
    Border,
    Wall,
    Custom,
}

/// Label options.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LabelOptions {
    /// `true`: do not show the label.
    #[serde(default)]
    pub hide: bool,
    /// Font size, in points. Default: the size of the label kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub size_pt: Option<f64>,
    /// The middle of the label `[x, y]`, in units. The label goes here,
    /// level, before the other labels, which keep clear of it. Default: the
    /// placer finds a free place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
}

/// One map to render.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    /// Name of the variant: lower-case letters and digits, words joined by
    /// `-`. It is also the stem of the output file names.
    #[schemars(regex(pattern = r"^[a-z0-9]+(-[a-z0-9]+)*$"))]
    pub name: String,
    /// Name of the overlay to show. Default: no overlay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
    /// Rectangle `[x0, y0, x1, y1]` of the world to show, in units. Default:
    /// the full frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<[f64; 4]>,
    /// Page size.
    pub page: Page,
    /// Extras for this variant. A value here replaces the value in
    /// `render.extras`.
    #[serde(default)]
    pub extras: Extras,
    /// The relief symbols of this variant. A value here replaces
    /// `render.relief` for this variant, for example to leave out a forest
    /// or a marsh on the map of another age.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relief: Option<ReliefOptions>,
}

/// A page size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Page {
    /// Width, in millimetres.
    #[schemars(range(min = 0.0))]
    pub w_mm: f64,
    /// Height, in millimetres.
    #[schemars(range(min = 0.0))]
    pub h_mm: f64,
    /// Margin on all sides, in millimetres.
    #[serde(default)]
    #[schemars(range(min = 0.0))]
    pub margin_mm: f64,
}

/// Render options for all variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Render {
    /// Render mode. `print-safe` is black on white only. `colour` draws the
    /// map as Azgaar's Fantasy Map Generator does, with the style in `style`.
    #[serde(default)]
    pub mode: RenderMode,
    /// The style preset of `colour` mode. A preset gives the colours, the
    /// line widths, the fonts, and the effects of each layer. The presets
    /// come from Azgaar's Fantasy Map Generator. `print-safe` mode does not
    /// use it.
    #[serde(default, skip_serializing_if = "StyleName::is_default")]
    pub style: StyleName,
    /// Changes to the style preset, for `colour` mode. Each key is the path
    /// of one value in the style tree of Azgaar's Fantasy Map Generator, for
    /// example `ocean.groups.base.attrs.fill` or
    /// `relief.options.density`. Each value is a string, a number, a
    /// boolean, or `null`. `null` removes the attribute. `docs/STYLES.md`
    /// gives the tree.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub style_overrides: BTreeMap<String, serde_json::Value>,
    /// Coastline detail. `smooth` cuts the corners of the cell edges.
    /// `fractal` adds the fractal detail of Azgaar's Fantasy Map Generator.
    /// Default: `smooth` in `print-safe` mode, `fractal` in `colour` mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coastline: Option<CoastlineDetail>,
    /// The hand-drawn look of `print-safe` mode. `true`: the lines have a
    /// small seeded wobble, rivers and coasts taper, and the dash patterns
    /// are less regular. No line becomes thinner than `min_line_width_pt`.
    /// `false`: the lines are exact. `colour` mode does not use it.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub hand_drawn: bool,
    /// Hill shading of `colour` mode. `true`: the land is darker on the
    /// slopes that turn away from a light in the north-west, and lighter on
    /// the slopes that turn to it. `print-safe` mode does not use it.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub hill_shading: bool,
    /// Coast echo lines of `colour` mode. `true`: thin lines in the sea
    /// follow the coast. Each line is further from the last line and
    /// fainter. `print-safe` mode does not use it.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub coast_echo: bool,
    /// Font families. The SVG names each family exactly as given here. The
    /// SVG never embeds a font.
    #[serde(default)]
    pub fonts: Fonts,
    /// Font families that the SVG can use. A family in `fonts` that is not in
    /// this list is an error.
    #[serde(default = "default_fonts_allowed")]
    pub fonts_allowed: Vec<String>,
    /// Font sizes, in points.
    #[serde(default)]
    pub font_sizes_pt: FontSizes,
    /// Smallest permitted line width, in points.
    #[serde(default = "default_min_line_width_pt")]
    #[schemars(range(min = 0.0))]
    pub min_line_width_pt: f64,
    /// Extras such as the scale bar and the compass.
    #[serde(default)]
    pub extras: Extras,
    /// Layers to draw. In `print-safe` mode the layers are drawn in this
    /// order, from bottom to top. In `colour` mode the renderer draws them in
    /// the layer order of Azgaar's Fantasy Map Generator. A layer that is
    /// only for `colour` mode draws nothing in `print-safe` mode.
    #[serde(default = "default_layers")]
    pub layers: Vec<LayerName>,
    /// Options of the layer `emblems`.
    #[serde(default, skip_serializing_if = "EmblemOptions::is_default")]
    pub emblems: EmblemOptions,
    /// Rules for the labels of all variants.
    #[serde(default, skip_serializing_if = "LabelRules::is_default")]
    pub labels: LabelRules,
    /// The relief symbols of `print-safe` mode: mountains, hills, trees,
    /// marsh, and dunes. `colour` mode uses the style value
    /// `relief.options.density` instead.
    #[serde(default, skip_serializing_if = "ReliefOptions::is_default")]
    pub relief: ReliefOptions,
    /// `print-safe` mode: the depths of the dashed depth lines in the
    /// ocean, below sea level on the height scale 0 to 100. Each line goes
    /// around the ocean at least that deep. Empty: no depth lines. Default
    /// `[6, 12]`. `colour` mode uses the style value
    /// `ocean.groups.oceanLayers.options.depths` instead.
    #[serde(
        default = "default_depth_lines",
        skip_serializing_if = "is_default_depth_lines"
    )]
    pub depth_lines: Vec<f64>,
    /// How strongly the width of a river follows its water, in both render
    /// modes. 0 to 2. 0: the widths of Azgaar's Fantasy Map Generator. A
    /// larger value makes the rivers with the most water wider and the
    /// small streams thinner. Default 1.
    #[serde(
        default = "default_river_contrast",
        skip_serializing_if = "is_default_river_contrast"
    )]
    #[schemars(range(min = 0.0, max = 2.0))]
    pub river_contrast: f64,
}

fn default_river_contrast() -> f64 {
    1.0
}

fn is_default_river_contrast(v: &f64) -> bool {
    *v == default_river_contrast()
}

fn default_depth_lines() -> Vec<f64> {
    vec![6.0, 12.0]
}

fn is_default_depth_lines(v: &[f64]) -> bool {
    v == default_depth_lines().as_slice()
}

/// Options of the layer `emblems`: the coats of arms of the states, the
/// provinces, and the places.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct EmblemOptions {
    /// `true`: draw each group of emblems. `false`: as Azgaar's Fantasy Map
    /// Generator, hide a group when its emblems are smaller than 25 or
    /// larger than 300 map pixels at the zoom of the variant. Then a full
    /// map shows only the arms of the states.
    pub show_all: bool,
    /// `true`: in `print-safe` mode, draw the emblems in black and white,
    /// with the heraldic hatching of Petra Sancta for each tincture. Each
    /// line is at least `min_line_width_pt` wide. `false`: `print-safe`
    /// mode draws no emblems. `colour` mode always draws the colours.
    pub hatching: bool,
}

impl EmblemOptions {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl Default for Render {
    fn default() -> Self {
        Self {
            mode: RenderMode::default(),
            style: StyleName::default(),
            style_overrides: BTreeMap::new(),
            coastline: None,
            hand_drawn: true,
            hill_shading: true,
            coast_echo: true,
            fonts: Fonts::default(),
            fonts_allowed: default_fonts_allowed(),
            font_sizes_pt: FontSizes::default(),
            min_line_width_pt: default_min_line_width_pt(),
            extras: Extras::default(),
            layers: default_layers(),
            emblems: EmblemOptions::default(),
            labels: LabelRules::default(),
            relief: ReliefOptions::default(),
            depth_lines: default_depth_lines(),
            river_contrast: default_river_contrast(),
        }
    }
}

/// The relief symbols of `print-safe` mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct ReliefOptions {
    /// How dense the symbols are. 0 to 1. 1 is the full density. A smaller
    /// value puts the symbols further apart: 0.25 shows about a quarter of
    /// them. 0 shows none.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub density: f64,
    /// The symbol kinds to leave out: `mountain`, `hill`, `tree` (the
    /// forest biomes), `marsh` (wetland), `dune` (the deserts), or `field`
    /// (the farmland of `print-safe` mode: small fields on open lowland
    /// near the places).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<ReliefSymbol>,
    /// Both render modes. `true`: the symbols follow the terrain. Mountains
    /// stand in chains of overlapping peaks along the ridges, larger at the
    /// crest, with hills at their feet. Trees grow in groups, denser in the
    /// highland, and the symbols change a little in size. `false`: the
    /// placement of Azgaar's Fantasy Map Generator. Default `true`.
    #[serde(skip_serializing_if = "is_true")]
    pub terrain: bool,
    /// Both render modes. The ids of region pins and area pins. The relief
    /// symbols inside these shapes are left out, for example the marsh of a
    /// fen or the trees of a wood that a map of an older age does not show.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hide_in: Vec<String>,
}

impl Default for ReliefOptions {
    fn default() -> Self {
        Self {
            density: 1.0,
            hide: Vec::new(),
            terrain: true,
            hide_in: Vec::new(),
        }
    }
}

impl ReliefOptions {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A kind of relief symbol of `print-safe` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReliefSymbol {
    Mountain,
    Hill,
    Tree,
    Marsh,
    Dune,
    Field,
}

/// Rules for the labels. A name is "generated" when the config does not
/// write it: the name of a pin, of a region, of a river, the `capital_of`
/// of a pin, a rename, or a name of an overlay. Generation gives the other
/// names to rivers, lakes, seas, islands, states, provinces, and places.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct LabelRules {
    /// `false`: show no label with a generated name. Default `true`.
    pub generated: bool,
    /// `true`: the report of each variant has the check
    /// `no_generated_labels`. It fails when the variant shows a label with
    /// a generated name. Default `false`.
    pub check_generated: bool,
    /// `print-safe` mode: `true` draws a white box behind each label that
    /// does not follow a river or a line, so that the lines under it do not
    /// cross the text. A curved area label gets boxes along its curve.
    /// `false`: no box. The text never has a stroke, because a PDF printer
    /// turns stroked text into a Type 3 font. `colour` mode draws the halo
    /// of its style. Default `true`.
    pub halo: bool,
    /// The largest turn of an area label (a state, a region, a lake, a sea,
    /// an island, or an overlay feature), in degrees, in both render modes.
    /// A straight label turns 0 or 90 degrees. A curved label has no part
    /// at a larger angle to the level than this. `0` gives only level,
    /// straight labels. Default 90.
    #[schemars(range(min = 0.0, max = 90.0))]
    pub max_angle: f64,
    /// `true`: an area label can follow a smooth curve along the middle of
    /// its area, with its letters spaced to fill part of the curve.
    /// `false`: only straight area labels. Default `true`.
    pub curved: bool,
    /// `false`: a place whose label the config leaves out has no symbol
    /// either. The config leaves out a label with a generated name when
    /// `generated` is `false`, and a label with `hide` in the label options
    /// of an overlay. A label that has no free place keeps its symbol.
    /// Default `true`.
    pub unlabelled_symbols: bool,
}

impl Default for LabelRules {
    fn default() -> Self {
        Self {
            generated: true,
            check_generated: false,
            halo: true,
            max_angle: 90.0,
            curved: true,
            unlabelled_symbols: true,
        }
    }
}

impl LabelRules {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl Config {
    /// The relief options of a variant: its own, or `render.relief`.
    pub fn relief_of<'a>(&'a self, variant: &'a Variant) -> &'a ReliefOptions {
        variant.relief.as_ref().unwrap_or(&self.render.relief)
    }

    /// The shapes of the region pins and the area pins in `hide_in` of the
    /// relief options of a variant.
    pub fn relief_hidden_areas(&self, variant: &Variant) -> Vec<crate::geom::Shape> {
        self.relief_hidden_shapes(variant)
            .into_iter()
            .map(|(_, s)| s)
            .collect()
    }

    /// The ids and the shapes of the region pins and the area pins in
    /// `hide_in` of the relief options of a variant, regions first.
    pub fn relief_hidden_shapes(&self, variant: &Variant) -> Vec<(String, crate::geom::Shape)> {
        let ids = &self.relief_of(variant).hide_in;
        let regions = self
            .pins
            .regions
            .iter()
            .filter(|r| ids.contains(&r.id))
            .filter_map(|r| crate::geom::Shape::of(r).map(|s| (r.id.clone(), s)));
        let areas = self
            .pins
            .areas
            .iter()
            .filter(|a| ids.contains(&a.id))
            .filter_map(|a| {
                crate::geom::Shape::from_fields(&a.polygon, &a.rect, &a.circle)
                    .map(|s| (a.id.clone(), s))
            });
        regions.chain(areas).collect()
    }

    /// The names that the config writes, in lower case. A label with
    /// another name has a generated name (`LabelRules`).
    pub fn written_names(&self) -> std::collections::BTreeSet<String> {
        let mut names = std::collections::BTreeSet::new();
        let mut add = |n: Option<&String>| {
            if let Some(n) = n.filter(|n| !n.trim().is_empty()) {
                names.insert(n.trim().to_lowercase());
            }
        };
        let pins = &self.pins;
        for p in &pins.points {
            add(p.name.as_ref());
            add(p.capital_of.as_ref());
        }
        for r in &pins.regions {
            add(r.name.as_ref());
        }
        for r in &pins.rivers {
            add(r.name.as_ref());
        }
        for z in &self.world.zones {
            add(z.name.as_ref());
        }
        for r in &self.world.renames {
            add(r.name.as_ref());
            add(r.full_name.as_ref());
        }
        for o in self.overlays.values() {
            for p in &o.add.places {
                add(p.name.as_ref());
                add(p.capital_of.as_ref());
            }
            for f in &o.add.features {
                add(f.name.as_ref());
            }
            for n in o.rename.values() {
                add(Some(n));
            }
        }
        names
    }
}

/// Font sizes, in points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct FontSizes {
    /// The map title in the cartouche. Default 12.
    #[schemars(range(min = 0.0))]
    pub title: f64,
    /// State names. Default 10.
    #[schemars(range(min = 0.0))]
    pub state: f64,
    /// Names of pinned regions, for example seas and mountains. Default 8.5.
    #[schemars(range(min = 0.0))]
    pub region: f64,
    /// Cities and capitals. Default 8.
    #[schemars(range(min = 0.0))]
    pub place_city: f64,
    /// Towns, fortresses, ruins, and custom places. Default 7.
    #[schemars(range(min = 0.0))]
    pub place_town: f64,
    /// Villages. Default 6.
    #[schemars(range(min = 0.0))]
    pub place_village: f64,
    /// River names. Default 6.5.
    #[schemars(range(min = 0.0))]
    pub river: f64,
    /// Scale bar, compass, and grid text. Default 7.
    #[schemars(range(min = 0.0))]
    pub other: f64,
}

impl Default for FontSizes {
    fn default() -> Self {
        Self {
            title: 12.0,
            state: 10.0,
            region: 8.5,
            place_city: 8.0,
            place_town: 7.0,
            place_village: 6.0,
            river: 6.5,
            other: 7.0,
        }
    }
}

/// Render modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RenderMode {
    #[default]
    PrintSafe,
    Colour,
}

/// The style presets of `colour` mode. Each preset is a style of Azgaar's
/// Fantasy Map Generator.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum StyleName {
    #[default]
    Default,
    Ancient,
    Atlas,
    Cinderwood,
    Clean,
    Cyberpunk,
    DarkSeas,
    Frostbite,
    Gloom,
    Ink,
    Light,
    Monochrome,
    Night,
    Pale,
    Watercolor,
}

impl StyleName {
    /// All presets, in a fixed order.
    pub const ALL: [StyleName; 15] = [
        StyleName::Default,
        StyleName::Ancient,
        StyleName::Atlas,
        StyleName::Cinderwood,
        StyleName::Clean,
        StyleName::Cyberpunk,
        StyleName::DarkSeas,
        StyleName::Frostbite,
        StyleName::Gloom,
        StyleName::Ink,
        StyleName::Light,
        StyleName::Monochrome,
        StyleName::Night,
        StyleName::Pale,
        StyleName::Watercolor,
    ];

    /// `true` for the default preset.
    pub fn is_default(&self) -> bool {
        *self == StyleName::Default
    }
}

/// Coastline detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CoastlineDetail {
    /// Cut the corners of the cell edges.
    Smooth,
    /// Add the fractal detail of Azgaar's Fantasy Map Generator.
    Fractal,
}

/// Font families.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    /// Family of the map title.
    #[serde(default = "default_font_fell")]
    pub title: String,
    /// Family of the map labels.
    #[serde(default = "default_font_fell")]
    pub labels: String,
    /// Small-capital family of the map labels, for example region names.
    #[serde(default = "default_font_fell_sc")]
    pub labels_sc: String,
    /// Family of all other text: scale bar, compass letters, grid, notes.
    #[serde(default = "default_font_other")]
    pub other: String,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            title: default_font_fell(),
            labels: default_font_fell(),
            labels_sc: default_font_fell_sc(),
            other: default_font_other(),
        }
    }
}

impl Fonts {
    /// Each family with the name of its field, in a fixed order.
    pub fn families(&self) -> [(&'static str, &str); 4] {
        [
            ("title", &self.title),
            ("labels", &self.labels),
            ("labels_sc", &self.labels_sc),
            ("other", &self.other),
        ]
    }
}

/// Extras. A missing value means "not set".
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Extras {
    /// Show a scale bar in config units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_bar: Option<bool>,
    /// Show a compass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compass: Option<bool>,
    /// Show a title cartouche.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cartouche: Option<Cartouche>,
    /// Show a grid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<Grid>,
}

/// A title cartouche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Cartouche {
    /// The title text.
    pub title: String,
}

/// A grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Grid {
    /// Distance between grid lines, in units.
    #[schemars(range(min = 0.0))]
    pub step: f64,
}

/// Names of the map layers.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum LayerName {
    Coast,
    Relief,
    Rivers,
    Regions,
    Roads,
    /// The features that an overlay adds.
    Features,
    Places,
    Labels,
    /// The ocean: the water colour and the depth bands. In `print-safe`
    /// mode, the water lines along the coast.
    Ocean,
    /// The land colour. Only `colour` mode.
    Landmass,
    /// A bitmap texture. The renderer has no bitmaps, so it draws nothing.
    Texture,
    /// Height shading. Only `colour` mode.
    Heightmap,
    /// Lakes. Only `colour` mode.
    Lakes,
    /// Biomes. Only `colour` mode.
    Biomes,
    /// The outlines of the cells.
    Cells,
    /// A grid of hexagons or squares over the map.
    Grid,
    /// Lines of latitude and longitude, with labels.
    Coordinates,
    /// A compass rose.
    Compass,
    /// Religions. Only `colour` mode.
    Religions,
    /// Cultures. Only `colour` mode.
    Cultures,
    /// The areas of the states. Only `colour` mode.
    States,
    /// The areas of the provinces. Only `colour` mode.
    Provinces,
    /// Zones. Only `colour` mode. Draws nothing when the world has no zones.
    Zones,
    /// The borders of states and provinces.
    Borders,
    /// Roads and trails.
    Routes,
    /// Temperature. Only `colour` mode.
    Temperature,
    /// The coastline.
    Coastline,
    /// Glaciers and icebergs. Only `colour` mode. Draws nothing when the
    /// world has no ice.
    Ice,
    /// Goods: the output of the cells, the pies of the places, and the
    /// deposit icons (FMG `goods`). Only `colour` mode. Draws nothing when
    /// the world has no economy.
    Goods,
    /// The markets: the area of each market and a pin at its center (FMG
    /// `markets`). Only `colour` mode. Draws nothing when the world has no
    /// markets.
    Markets,
    /// Trade: the goods that each route link carried in the run, as lines
    /// with a width by the units. FMG animates the trade; inkland draws its
    /// traffic lines (FMG `trade`). Only `colour` mode. Draws nothing when
    /// no goods travel.
    Trade,
    /// Precipitation. Only `colour` mode.
    Precipitation,
    /// Population. Only `colour` mode. Draws nothing when the world has no
    /// population.
    Population,
    /// Coats of arms. Only `colour` mode. Draws nothing when the world has
    /// no emblems.
    Emblems,
    /// The symbols of the places.
    BurgIcons,
    /// The regiments of the states: a box in the colour of the state with
    /// the troops and the icon (FMG `military`). Only `colour` mode. Draws
    /// nothing when the world has no regiments.
    Military,
    /// Markers. Only `colour` mode. Draws nothing when the world has no
    /// markers.
    Markers,
    /// Journeys: the path of each journey, a curve for each segment in the
    /// colour of the journey (FMG `journeys`). Only `colour` mode. Draws
    /// nothing when the world has no journeys.
    Journeys,
    /// Rulers: the default ruler of FMG across the largest landmass, with
    /// its length (FMG `rulers`). Only `colour` mode. Draws nothing when
    /// the world has no measurers.
    Rulers,
    /// A scale bar.
    ScaleBar,
    /// A dark edge around the map. Only `colour` mode.
    Vignette,
    /// A legend box. Only `colour` mode.
    Legend,
}

impl LayerName {
    /// All layer names, in the order of their declaration.
    pub const ALL: [LayerName; 42] = [
        LayerName::Coast,
        LayerName::Relief,
        LayerName::Rivers,
        LayerName::Regions,
        LayerName::Roads,
        LayerName::Features,
        LayerName::Places,
        LayerName::Labels,
        LayerName::Ocean,
        LayerName::Landmass,
        LayerName::Texture,
        LayerName::Heightmap,
        LayerName::Lakes,
        LayerName::Biomes,
        LayerName::Cells,
        LayerName::Grid,
        LayerName::Coordinates,
        LayerName::Compass,
        LayerName::Religions,
        LayerName::Cultures,
        LayerName::States,
        LayerName::Provinces,
        LayerName::Zones,
        LayerName::Borders,
        LayerName::Routes,
        LayerName::Temperature,
        LayerName::Coastline,
        LayerName::Ice,
        LayerName::Goods,
        LayerName::Markets,
        LayerName::Trade,
        LayerName::Precipitation,
        LayerName::Population,
        LayerName::Emblems,
        LayerName::BurgIcons,
        LayerName::Military,
        LayerName::Markers,
        LayerName::Journeys,
        LayerName::Rulers,
        LayerName::ScaleBar,
        LayerName::Vignette,
        LayerName::Legend,
    ];

    /// The name of the layer in the config, for example `burg-icons`.
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    /// `true` if the layer has a `print-safe` drawing. Other layers draw
    /// nothing in `print-safe` mode.
    pub fn is_print_safe(self) -> bool {
        use LayerName::*;
        matches!(
            self,
            Coast
                | Relief
                | Rivers
                | Regions
                | Roads
                | Features
                | Places
                | Labels
                | Ocean
                | Texture
                | Cells
                | Grid
                | Coordinates
                | Compass
                | Borders
                | Routes
                | Coastline
                | BurgIcons
                | ScaleBar
        )
    }
}

fn default_unit_name() -> String {
    "mile".to_owned()
}
fn default_border_terrain() -> f64 {
    0.5
}

fn is_default_border_terrain(v: &f64) -> bool {
    *v == default_border_terrain()
}

fn default_border_noise() -> f64 {
    0.5
}

fn is_default_border_noise(v: &f64) -> bool {
    *v == default_border_noise()
}

fn default_cells() -> u32 {
    10_000
}
fn default_jitter() -> f64 {
    0.9
}
fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}

fn is_default_jitter(v: &f64) -> bool {
    *v == default_jitter()
}
fn default_lake_elevation_limit() -> u8 {
    crate::lakes::DEFAULT_ELEVATION_LIMIT
}
fn is_default_lake_elevation_limit(v: &u8) -> bool {
    *v == default_lake_elevation_limit()
}
fn default_sea_level() -> u8 {
    20
}
fn default_cultures() -> u32 {
    6
}
fn default_culture_set() -> String {
    "european".to_owned()
}
fn default_size_variety() -> f64 {
    4.0
}
fn default_growth_rate() -> f64 {
    1.0
}
fn default_states() -> u32 {
    5
}
fn default_emblem_shape() -> String {
    "culture".to_owned()
}

fn is_default_emblem_shape(v: &str) -> bool {
    v == "culture"
}

fn default_provinces_ratio() -> u8 {
    20
}
fn default_religions() -> u32 {
    4
}
fn default_latitude_range() -> [f64; 2] {
    [30.0, 55.0]
}
fn default_tolerance() -> f64 {
    0.05
}
fn default_min_line_width_pt() -> f64 {
    0.5
}
fn default_font_fell() -> String {
    "IM Fell English".to_owned()
}
fn default_font_fell_sc() -> String {
    "IM Fell English SC".to_owned()
}
fn default_font_other() -> String {
    "Caladea".to_owned()
}
fn default_fonts_allowed() -> Vec<String> {
    ["Caladea", "Inter", "IM Fell English", "IM Fell English SC"]
        .map(str::to_owned)
        .to_vec()
}
fn default_layers() -> Vec<LayerName> {
    vec![
        LayerName::Coast,
        LayerName::Relief,
        LayerName::Rivers,
        LayerName::Regions,
        LayerName::Roads,
        LayerName::Features,
        LayerName::Places,
        LayerName::Labels,
    ]
}
fn default_variants() -> Vec<Variant> {
    vec![Variant {
        name: "default".to_owned(),
        overlay: None,
        crop: None,
        page: Page {
            w_mm: 167.0,
            h_mm: 250.0,
            margin_mm: 0.0,
        },
        extras: Extras::default(),
        relief: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal() -> serde_json::Value {
        json!({ "version": 1, "seed": 7, "frame": { "width": 400, "height": 300 } })
    }

    #[test]
    fn relief_can_hide_in_an_area() {
        let mut v = minimal();
        v["pins"] = json!({
            "regions": [{ "id": "fen", "kind": "marsh", "box": [10, 10, 50, 50] }],
            "areas": [{ "id": "kingsholm-wood", "character": "wooded", "box": [100, 100, 150, 150] }]
        });
        v["render"] = json!({ "relief": { "hide_in": ["kingsholm-wood"] } });
        let c: Config = serde_json::from_value(v).unwrap();
        let hidden = c.relief_hidden_areas(&c.variants[0]);
        assert_eq!(hidden.len(), 1);
        assert!(hidden[0].contains([120.0, 120.0]));
        assert!(!hidden[0].contains([20.0, 20.0]));
    }

    #[test]
    fn minimal_config_gets_the_defaults() {
        let c: Config = serde_json::from_value(minimal()).unwrap();
        assert_eq!(c.units.name, "mile");
        assert_eq!(c.frame.origin, Origin::TopLeft);
        assert_eq!(c.world.cells, 10_000);
        assert_eq!(c.world.template, Template::Named(TemplateName::Continents));
        assert_eq!(c.variants.len(), 1);
        assert_eq!(c.variants[0].page.w_mm, 167.0);
        assert_eq!(c.render.fonts.labels_sc, "IM Fell English SC");
        assert_eq!(c.render.mode, RenderMode::PrintSafe);
    }

    #[test]
    fn unknown_field_is_refused() {
        let mut v = minimal();
        v["colour"] = json!("red");
        assert!(serde_json::from_value::<Config>(v).is_err());
    }

    #[test]
    fn region_box_is_read_from_the_box_key() {
        let mut v = minimal();
        v["pins"] = json!({ "regions": [
            { "id": "spine", "kind": "mountains", "box": [200, 40, 260, 140] }
        ]});
        let c: Config = serde_json::from_value(v).unwrap();
        assert_eq!(c.pins.regions[0].rect, Some([200.0, 40.0, 260.0, 140.0]));
    }

    #[test]
    fn river_anchors_accept_points_places_and_regions() {
        let mut v = minimal();
        v["pins"] = json!({ "rivers": [{
            "id": "brann",
            "source": { "region": "wolds" },
            "through": [{ "place": "brannocks-ford" }, [140, 90]],
            "mouth": { "region": "low-fen" }
        }]});
        let c: Config = serde_json::from_value(v).unwrap();
        let r = &c.pins.rivers[0];
        assert_eq!(
            r.source,
            Anchor::Region(RegionRef {
                region: "wolds".into()
            })
        );
        assert_eq!(
            r.through[0],
            Anchor::Place(PlaceRef {
                place: "brannocks-ford".into()
            })
        );
        assert_eq!(r.through[1], Anchor::Point([140.0, 90.0]));
    }

    #[test]
    fn towns_accept_auto_and_a_count() {
        let mut v = minimal();
        v["world"] = json!({ "towns": "auto" });
        let c: Config = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(c.world.towns, Towns::Auto(AutoKeyword::Auto));
        v["world"] = json!({ "towns": 30 });
        let c: Config = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(c.world.towns, Towns::Count(30));
        v["world"] = json!({ "towns": "many" });
        assert!(serde_json::from_value::<Config>(v).is_err());
    }

    #[test]
    fn region_height_comes_from_the_kind_or_the_config() {
        let mut v = minimal();
        v["pins"] = json!({ "regions": [
            { "id": "teeth", "kind": "mountains", "box": [0, 0, 1, 1] },
            { "id": "wolds", "kind": "hills", "box": [0, 0, 1, 1], "height": [35, 50] }
        ]});
        let c: Config = serde_json::from_value(v).unwrap();
        assert_eq!(c.pins.regions[0].height_range(), [70.0, 100.0]);
        assert_eq!(c.pins.regions[1].height_range(), [35.0, 50.0]);
    }

    #[test]
    fn a_forest_or_desert_region_is_lowland_by_default() {
        use RegionKind::*;
        for k in [Forest, Desert, Plains, Marsh] {
            let [_, hi] = k.default_height();
            assert!(hi <= Hills.default_height()[0] + 15.0, "{k:?}");
            assert!(hi < Mountains.default_height()[0], "{k:?}");
        }
    }

    #[test]
    fn water_ranges_end_below_sea_level_and_land_ranges_start_at_it() {
        use RegionKind::*;
        for k in [Sea, Lake, Mountains, Hills, Plains, Marsh, Forest, Desert] {
            let [lo, hi] = k.default_height();
            assert!(lo <= hi);
            if k.is_water() {
                assert!(hi < 20.0, "{k:?}");
            } else {
                assert!(lo >= 20.0, "{k:?}");
            }
        }
    }

    #[test]
    fn template_accepts_a_raster() {
        let mut v = minimal();
        v["world"] = json!({ "template": { "raster": {
            "cols": 2, "rows": 2, "values": [0, 10, 20, 30]
        }}});
        let c: Config = serde_json::from_value(v).unwrap();
        let Template::Raster { raster } = &c.world.template else {
            panic!("not a raster: {:?}", c.world.template);
        };
        assert_eq!((raster.cols, raster.rows), (2, 2));
        let again: Config = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(c, again);
    }

    #[test]
    fn template_accepts_an_image_and_writes_only_its_path() {
        let mut v = minimal();
        v["world"] = json!({ "template": { "image": "heights.png" } });
        let mut c: Config = serde_json::from_value(v).unwrap();
        assert_eq!(
            c.world.template,
            Template::Image {
                image: "heights.png".into(),
                heights: None
            }
        );
        if let Template::Image { heights, .. } = &mut c.world.template {
            *heights = Some(Raster {
                cols: 2,
                rows: 2,
                values: vec![0, 1, 2, 3],
            });
        }
        let out = serde_json::to_value(&c).unwrap();
        assert_eq!(out["world"]["template"], json!({ "image": "heights.png" }));
        assert!(c.world.template.raster().is_some());
    }

    #[test]
    fn raster_sample_takes_the_rectangle_of_the_position() {
        let r = Raster {
            cols: 3,
            rows: 2,
            values: vec![1, 2, 3, 4, 5, 6],
        };
        // Frame 300 x 100: each rectangle is 100 x 50.
        assert_eq!(r.sample(0.0, 0.0, 300.0, 100.0), 1);
        assert_eq!(r.sample(99.9, 49.9, 300.0, 100.0), 1);
        assert_eq!(r.sample(100.0, 0.0, 300.0, 100.0), 2);
        assert_eq!(r.sample(250.0, 10.0, 300.0, 100.0), 3);
        assert_eq!(r.sample(10.0, 50.0, 300.0, 100.0), 4);
        assert_eq!(r.sample(300.0, 100.0, 300.0, 100.0), 6);
        // Outside the frame: the nearest edge rectangle.
        assert_eq!(r.sample(-5.0, -5.0, 300.0, 100.0), 1);
        assert_eq!(r.sample(900.0, 900.0, 300.0, 100.0), 6);
    }

    #[test]
    fn style_fields_are_left_out_of_json_when_they_have_the_default() {
        let c: Config = serde_json::from_value(minimal()).unwrap();
        let v = serde_json::to_value(&c).unwrap();
        for key in ["style", "style_overrides", "coastline"] {
            assert!(v["render"].get(key).is_none(), "{key}");
        }
        let mut v = minimal();
        v["render"] = json!({ "mode": "colour", "style": "dark-seas", "coastline": "fractal",
                              "style_overrides": { "ocean.groups.base.attrs.fill": "#123456" } });
        let c: Config = serde_json::from_value(v).unwrap();
        assert_eq!(c.render.style, StyleName::DarkSeas);
        assert_eq!(c.render.coastline, Some(CoastlineDetail::Fractal));
        let again: Config = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(c, again);
    }

    #[test]
    fn each_layer_name_is_in_the_list_once() {
        let names: std::collections::BTreeSet<LayerName> = LayerName::ALL.into_iter().collect();
        assert_eq!(names.len(), LayerName::ALL.len());
        let json = serde_json::to_value(LayerName::BurgIcons).unwrap();
        assert_eq!(json, json!("burg-icons"));
        assert!(default_layers().iter().all(|l| l.is_print_safe()));
    }

    #[test]
    fn a_config_survives_a_round_trip() {
        let mut v = minimal();
        v["pins"] = json!({
            "points": [{ "id": "ostrey", "kind": "city", "at": [120, 88], "port": true }],
            "roads": [{ "id": "kings-road", "from": "ostrey", "to": "harrowgate",
                        "via": ["mid", [10, 20]], "target_length": 72 }]
        });
        let c: Config = serde_json::from_value(v).unwrap();
        let again: Config = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(c, again);
    }

    #[test]
    fn the_hand_drawn_look_is_on_by_default_and_can_be_turned_off() {
        let c: Config = serde_json::from_value(minimal()).unwrap();
        assert!(c.render.hand_drawn);
        // The default is not written back.
        let v = serde_json::to_value(&c).unwrap();
        assert!(v["render"].get("hand_drawn").is_none());
        let mut off = minimal();
        off["render"] = json!({ "hand_drawn": false });
        let c: Config = serde_json::from_value(off).unwrap();
        assert!(!c.render.hand_drawn);
        assert_eq!(
            serde_json::to_value(&c).unwrap()["render"]["hand_drawn"],
            false
        );
    }

    #[test]
    fn the_colour_effects_are_on_by_default_and_can_be_turned_off() {
        let c: Config = serde_json::from_value(minimal()).unwrap();
        assert!(c.render.hill_shading && c.render.coast_echo);
        let mut off = minimal();
        off["render"] = json!({ "hill_shading": false, "coast_echo": false });
        let c: Config = serde_json::from_value(off).unwrap();
        assert!(!c.render.hill_shading && !c.render.coast_echo);
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["render"]["hill_shading"], false);
        assert_eq!(v["render"]["coast_echo"], false);
    }
}
