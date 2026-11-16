//! The step lists of the built-in heightmap templates.
//!
//! The step lists come from Azgaar's Fantasy Map Generator,
//! `src/data/heightmap-templates.ts` (MIT license, see
//! THIRD_PARTY_NOTICES.md). The code that runs the steps is inkland code.

use crate::config::TemplateName;

/// The steps of a built-in template, one step in each line.
pub fn steps(name: TemplateName) -> &'static str {
    match name {
        TemplateName::Volcano => VOLCANO,
        TemplateName::HighIsland => HIGH_ISLAND,
        TemplateName::LowIsland => LOW_ISLAND,
        TemplateName::Continents => CONTINENTS,
        TemplateName::Archipelago => ARCHIPELAGO,
        TemplateName::Atoll => ATOLL,
        TemplateName::Mediterranean => MEDITERRANEAN,
        TemplateName::Peninsula => PENINSULA,
        TemplateName::Pangea => PANGEA,
        TemplateName::Isthmus => ISTHMUS,
        TemplateName::Shattered => SHATTERED,
        TemplateName::Taklamakan => TAKLAMAKAN,
        TemplateName::OldWorld => OLD_WORLD,
        TemplateName::Fractious => FRACTIOUS,
    }
}

const VOLCANO: &str = "\
Hill 1 90-100 44-56 40-60\n\
Multiply 0.8 50-100 0 0\n\
Range 1.5 30-55 45-55 40-60\n\
Smooth 3 0 0 0\n\
Hill 1.5 35-45 25-30 20-75\n\
Hill 1 35-55 75-80 25-75\n\
Hill 0.5 20-25 10-15 20-25\n\
Mask 3 0 0 0\n\
";

const HIGH_ISLAND: &str = "\
Hill 1 90-100 65-75 47-53\n\
Add 7 all 0 0\n\
Hill 5-6 20-30 25-55 45-55\n\
Range 1 40-50 45-55 45-55\n\
Multiply 0.8 land 0 0\n\
Mask 3 0 0 0\n\
Smooth 2 0 0 0\n\
Trough 2-3 20-30 20-30 20-30\n\
Trough 2-3 20-30 60-80 70-80\n\
Hill 1 10-15 60-60 50-50\n\
Hill 1.5 13-16 15-20 20-75\n\
Range 1.5 30-40 15-85 30-40\n\
Range 1.5 30-40 15-85 60-70\n\
Pit 3-5 10-30 15-85 20-80\n\
";

const LOW_ISLAND: &str = "\
Hill 1 90-99 60-80 45-55\n\
Hill 1-2 20-30 10-30 10-90\n\
Smooth 2 0 0 0\n\
Hill 6-7 25-35 20-70 30-70\n\
Range 1 40-50 45-55 45-55\n\
Trough 2-3 20-30 15-85 20-30\n\
Trough 2-3 20-30 15-85 70-80\n\
Hill 1.5 10-15 5-15 20-80\n\
Hill 1 10-15 85-95 70-80\n\
Pit 5-7 15-25 15-85 20-80\n\
Multiply 0.4 20-100 0 0\n\
Mask 4 0 0 0\n\
";

const CONTINENTS: &str = "\
Hill 1 80-85 60-80 40-60\n\
Hill 1 80-85 20-30 40-60\n\
Hill 6-7 15-30 25-75 15-85\n\
Multiply 0.6 land 0 0\n\
Hill 8-10 5-10 15-85 20-80\n\
Range 1-2 30-60 5-15 25-75\n\
Range 1-2 30-60 80-95 25-75\n\
Range 0-3 30-60 80-90 20-80\n\
Strait 2 vertical 0 0\n\
Strait 1 vertical 0 0\n\
Smooth 3 0 0 0\n\
Trough 3-4 15-20 15-85 20-80\n\
Trough 3-4 5-10 45-55 45-55\n\
Pit 3-4 10-20 15-85 20-80\n\
Mask 4 0 0 0\n\
";

const ARCHIPELAGO: &str = "\
Add 11 all 0 0\n\
Range 2-3 40-60 20-80 20-80\n\
Hill 5 15-20 10-90 30-70\n\
Hill 2 10-15 10-30 20-80\n\
Hill 2 10-15 60-90 20-80\n\
Smooth 3 0 0 0\n\
Trough 10 20-30 5-95 5-95\n\
Strait 2 vertical 0 0\n\
Strait 2 horizontal 0 0\n\
";

const ATOLL: &str = "\
Hill 1 75-80 50-60 45-55\n\
Hill 1.5 30-50 25-75 30-70\n\
Hill .5 30-50 25-35 30-70\n\
Smooth 1 0 0 0\n\
Multiply 0.2 25-100 0 0\n\
Hill 0.5 10-20 50-55 48-52\n\
";

const MEDITERRANEAN: &str = "\
Range 4-6 30-80 0-100 0-10\n\
Range 4-6 30-80 0-100 90-100\n\
Hill 6-8 30-50 10-90 0-5\n\
Hill 6-8 30-50 10-90 95-100\n\
Multiply 0.9 land 0 0\n\
Mask -2 0 0 0\n\
Smooth 1 0 0 0\n\
Hill 2-3 30-70 0-5 20-80\n\
Hill 2-3 30-70 95-100 20-80\n\
Trough 3-6 40-50 0-100 0-10\n\
Trough 3-6 40-50 0-100 90-100\n\
";

const PENINSULA: &str = "\
Range 2-3 20-35 40-50 0-15\n\
Add 5 all 0 0\n\
Hill 1 90-100 10-90 0-5\n\
Add 13 all 0 0\n\
Hill 3-4 3-5 5-95 80-100\n\
Hill 1-2 3-5 5-95 40-60\n\
Trough 5-6 10-25 5-95 5-95\n\
Smooth 3 0 0 0\n\
Invert 0.4 both 0 0\n\
";

const PANGEA: &str = "\
Hill 1-2 25-40 15-50 0-10\n\
Hill 1-2 5-40 50-85 0-10\n\
Hill 1-2 25-40 50-85 90-100\n\
Hill 1-2 5-40 15-50 90-100\n\
Hill 8-12 20-40 20-80 48-52\n\
Smooth 2 0 0 0\n\
Multiply 0.7 land 0 0\n\
Trough 3-4 25-35 5-95 10-20\n\
Trough 3-4 25-35 5-95 80-90\n\
Range 5-6 30-40 10-90 35-65\n\
";

const ISTHMUS: &str = "\
Hill 5-10 15-30 0-30 0-20\n\
Hill 5-10 15-30 10-50 20-40\n\
Hill 5-10 15-30 30-70 40-60\n\
Hill 5-10 15-30 50-90 60-80\n\
Hill 5-10 15-30 70-100 80-100\n\
Smooth 2 0 0 0\n\
Trough 4-8 15-30 0-30 0-20\n\
Trough 4-8 15-30 10-50 20-40\n\
Trough 4-8 15-30 30-70 40-60\n\
Trough 4-8 15-30 50-90 60-80\n\
Trough 4-8 15-30 70-100 80-100\n\
Invert 0.25 x 0 0\n\
";

const SHATTERED: &str = "\
Hill 8 35-40 15-85 30-70\n\
Trough 10-20 40-50 5-95 5-95\n\
Range 5-7 30-40 10-90 20-80\n\
Pit 12-20 30-40 15-85 20-80\n\
";

const TAKLAMAKAN: &str = "\
Hill 1-3 20-30 30-70 30-70\n\
Hill 2-4 60-85 0-5 0-100\n\
Hill 2-4 60-85 95-100 0-100\n\
Hill 3-4 60-85 20-80 0-5\n\
Hill 3-4 60-85 20-80 95-100\n\
Smooth 3 0 0 0\n\
";

const OLD_WORLD: &str = "\
Range 3 70 15-85 20-80\n\
Hill 2-3 50-70 15-45 20-80\n\
Hill 2-3 50-70 65-85 20-80\n\
Hill 4-6 20-25 15-85 20-80\n\
Multiply 0.5 land 0 0\n\
Smooth 2 0 0 0\n\
Range 3-4 20-50 15-35 20-45\n\
Range 2-4 20-50 65-85 45-80\n\
Strait 3-7 vertical 0 0\n\
Trough 6-8 20-50 15-85 45-65\n\
Pit 5-6 20-30 10-90 10-90\n\
";

const FRACTIOUS: &str = "\
Hill 12-15 50-80 5-95 5-95\n\
Mask -1.5 0 0 0\n\
Mask 3 0 0 0\n\
Add -20 30-100 0 0\n\
Range 6-8 40-50 5-95 10-90\n\
";
