//! Compile the Slint user interface.

fn main() {
    slint_build::compile("ui/app.slint").expect("the Slint files compile");
}
