use dioxus::prelude::*;

#[component]
pub(crate) fn CodexPanel(entries: Vec<(String, String)>) -> Element {
    // entries: (title, text). Click a title to expand its text inline.
    let mut open = use_signal(|| None::<usize>);
    rsx! {
        div { class: "panel",
            p { class: "eyebrow", "Codex" }
            for (i, (title, text)) in entries.into_iter().enumerate() {
                div {
                    class: "codex-row",
                    onclick: move |_| {
                        let next = if open() == Some(i) { None } else { Some(i) };
                        open.set(next);
                    },
                    "{title}"
                }
                if open() == Some(i) {
                    div { class: "codex-text", "{text}" }
                }
            }
        }
    }
}
