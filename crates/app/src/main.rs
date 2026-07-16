mod app;
mod components;
mod persist;
mod screens;
mod style;
mod theme;
mod tick;
mod vm;

fn main() {
    console_error_panic_hook::set_once();
    dioxus::launch(app::App);
}
