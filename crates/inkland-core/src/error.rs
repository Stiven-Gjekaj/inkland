//! Error codes, warning codes, and exit codes.
//!
//! The codes are part of the public contract. `docs/ERRORS.md` lists them.
//! Never rename a code and never use a code again for a different fault.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The exit codes of the `inkland` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExitCode {
    /// The command succeeded.
    Ok = 0,
    /// The output is written, but one or more checks failed.
    ChecksFailed = 1,
    /// The config is not valid.
    InvalidConfig = 2,
    /// The pins cannot be satisfied.
    Infeasible = 3,
    /// A file cannot be read or written.
    Io = 4,
    /// A bug in inkland.
    Internal = 70,
}

impl ExitCode {
    pub fn code(self) -> i32 {
        self as i32
    }
}

macro_rules! codes {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $text:literal => $exit:ident,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
        pub enum $name {
            $( $(#[$vmeta])* #[serde(rename = $text)] $variant, )*
        }

        impl $name {
            /// All codes, in the order of `docs/ERRORS.md`.
            pub const ALL: &'static [$name] = &[$($name::$variant,)*];

            /// The stable text of the code.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $text,)* }
            }

            /// The exit code that the code causes.
            pub fn exit_code(self) -> ExitCode {
                match self { $($name::$variant => ExitCode::$exit,)* }
            }
        }
    };
}

codes! {
    /// Error codes.
    ErrorCode {
        /// The command line is not valid: an unknown command or flag, or a
        /// flag without its value.
        CliUsage = "CLI_USAGE" => InvalidConfig,
        /// The file is not valid JSON.
        ConfigParse = "CONFIG_PARSE" => InvalidConfig,
        /// The JSON does not agree with the schema.
        ConfigSchema = "CONFIG_SCHEMA" => InvalidConfig,
        /// The value of `version` is not supported.
        ConfigVersion = "CONFIG_VERSION" => InvalidConfig,
        /// A `--set` flag is not `POINTER=JSON`, or its pointer does not exist.
        ConfigSetInvalid = "CONFIG_SET_INVALID" => InvalidConfig,
        /// The frame axes do not agree with the origin, or the frame size is
        /// not more than 0.
        FrameAxesInvalid = "FRAME_AXES_INVALID" => InvalidConfig,
        /// An id does not match the pattern.
        IdInvalid = "ID_INVALID" => InvalidConfig,
        /// Two items have the same id.
        IdDuplicate = "ID_DUPLICATE" => InvalidConfig,
        /// A reference names an id or an overlay that does not exist, or an
        /// item of the wrong kind.
        RefUnknown = "REF_UNKNOWN" => InvalidConfig,
        /// A region or a feature does not have exactly one shape.
        ShapeInvalid = "SHAPE_INVALID" => InvalidConfig,
        /// A polygon has fewer than 3 points or crosses itself.
        RegionPolygonInvalid = "REGION_POLYGON_INVALID" => InvalidConfig,
        /// A font family is not in `render.fonts_allowed`.
        FontNotAllowed = "FONT_NOT_ALLOWED" => InvalidConfig,
        /// A river mouth region is not of kind `marsh`, `lake`, or `sea`.
        RiverMouthRegionKind = "RIVER_MOUTH_REGION_KIND" => InvalidConfig,
        /// A value is outside its permitted range.
        ValueOutOfRange = "VALUE_OUT_OF_RANGE" => InvalidConfig,
        /// A step of a custom heightmap template cannot be read.
        TemplateInvalid = "TEMPLATE_INVALID" => InvalidConfig,
        /// A map file of Azgaar's Fantasy Map Generator cannot be read.
        MapParse = "MAP_PARSE" => InvalidConfig,
        /// The version of a map file is not one that the import can read.
        MapVersion = "MAP_VERSION" => InvalidConfig,
        /// A pin or a crop is outside the frame.
        PinOutsideFrame = "PIN_OUTSIDE_FRAME" => Infeasible,
        /// A point pin is in a `sea` or `lake` region.
        PinInForcedSea = "PIN_IN_FORCED_SEA" => Infeasible,
        /// Two point pins have the same position.
        PinSamePosition = "PIN_SAME_POSITION" => Infeasible,
        /// A port pin has no water cell that it can touch.
        PortNotNearCoast = "PORT_NOT_NEAR_COAST" => Infeasible,
        /// Two forced regions overlap with kinds that cannot agree.
        RegionConflict = "REGION_CONFLICT" => Infeasible,
        /// A `sea` region without `landlocked: true` does not connect to the
        /// ocean.
        SeaNotConnected = "SEA_NOT_CONNECTED" => Infeasible,
        /// A point mouth is not in water, or a place mouth is not a port.
        RiverMouthOnLand = "RIVER_MOUTH_ON_LAND" => Infeasible,
        /// The region heights on a river course make downhill flow
        /// impossible.
        RiverNotDownhill = "RIVER_NOT_DOWNHILL" => Infeasible,
        /// Two pinned rivers cross.
        RiverCrossesRiver = "RIVER_CROSSES_RIVER" => Infeasible,
        /// A road target is shorter than the straight line through its
        /// points.
        RoadTargetTooShort = "ROAD_TARGET_TOO_SHORT" => Infeasible,
        /// No land path joins the ends of a road.
        RoadNoPath = "ROAD_NO_PATH" => Infeasible,
        /// A road is written, but its length is outside the tolerance.
        RoadLengthOutOfTolerance = "ROAD_LENGTH_OUT_OF_TOLERANCE" => ChecksFailed,
        /// A file cannot be read.
        IoRead = "IO_READ" => Io,
        /// A file cannot be written.
        IoWrite = "IO_WRITE" => Io,
        /// The command is not available in this version of inkland.
        NotImplemented = "NOT_IMPLEMENTED" => Internal,
        /// A bug in inkland.
        Internal = "INTERNAL" => Internal,
    }
}

codes! {
    /// Warning codes. A warning does not change the exit code.
    WarningCode {
        /// A font family is not available for the PNG preview.
        FontNotFound = "FONT_NOT_FOUND" => Ok,
        /// A label cannot go on the map without an overlap.
        LabelUnplaced = "LABEL_UNPLACED" => Ok,
    }
}

/// One error or warning, with the place in the config where it applies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic<C> {
    /// The stable code.
    pub code: C,
    /// A message for a human.
    pub message: String,
    /// JSON Pointer to the value in the config, or `""` for the full config.
    pub path: String,
    /// More data. The keys depend on the code.
    pub details: BTreeMap<String, Value>,
}

/// An error.
pub type Error = Diagnostic<ErrorCode>;
/// A warning.
pub type Warning = Diagnostic<WarningCode>;

impl<C> Diagnostic<C> {
    pub fn new(code: C, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path: path.into(),
            details: BTreeMap::new(),
        }
    }

    /// Add a value to `details`.
    pub fn with(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.details.insert(key.to_owned(), value.into());
        self
    }
}

/// The exit code for a list of errors: the most basic fault wins. A config
/// error (2) comes before an infeasible pin (3), because the second can be a
/// result of the first.
pub fn exit_code_of(errors: &[Error]) -> ExitCode {
    errors
        .iter()
        .map(|e| e.code.exit_code())
        .min_by_key(|c| match c {
            ExitCode::InvalidConfig => 0,
            ExitCode::Io => 1,
            ExitCode::Infeasible => 2,
            ExitCode::Internal => 3,
            ExitCode::ChecksFailed => 4,
            ExitCode::Ok => 5,
        })
        .unwrap_or(ExitCode::Ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn code_texts_are_unique_and_upper_snake_case() {
        let texts: BTreeSet<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(texts.len(), ErrorCode::ALL.len());
        for t in texts {
            assert!(t.chars().all(|c| c.is_ascii_uppercase() || c == '_'), "{t}");
        }
    }

    #[test]
    fn serde_uses_the_code_text() {
        for &c in ErrorCode::ALL {
            assert_eq!(serde_json::to_value(c).unwrap(), Value::from(c.as_str()));
        }
    }

    #[test]
    fn config_error_wins_over_infeasible_pin() {
        let errors = [
            Error::new(ErrorCode::PinInForcedSea, "/pins/points/0", "a"),
            Error::new(ErrorCode::IdDuplicate, "/pins/points/1/id", "b"),
        ];
        assert_eq!(exit_code_of(&errors), ExitCode::InvalidConfig);
    }

    /// `docs/ERRORS.md` is the public list of codes. Each code of the enum
    /// must have a row in it with the right exit code.
    #[test]
    fn each_code_has_a_row_in_errors_md() {
        let doc = include_str!("../../../docs/ERRORS.md");
        for &c in ErrorCode::ALL {
            let row = format!("| `{}` | {} |", c.as_str(), c.exit_code().code());
            assert!(doc.contains(&row), "docs/ERRORS.md has no row {row}");
        }
        for &w in WarningCode::ALL {
            assert!(
                doc.contains(&format!("| `{}` |", w.as_str())),
                "{}",
                w.as_str()
            );
        }
    }

    #[test]
    fn no_errors_is_exit_zero() {
        assert_eq!(exit_code_of(&[]), ExitCode::Ok);
    }
}
