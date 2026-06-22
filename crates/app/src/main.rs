mod app;
mod persist;
mod tick;

fn main() {
    console_error_panic_hook::set_once();
    dioxus::launch(app::App);
}
