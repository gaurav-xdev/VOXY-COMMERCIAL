use dioxus::prelude::*;

#[component]
pub fn StartupSplash(progress: f32, status_text: String) -> Element {
    let pct = (progress * 100.0).clamp(0.0, 100.0) as u32;

    rsx! {
        div { class: "splash-screen-container",
            div { class: "splash-ambient-radiance" }

            div { class: "splash-branding-box",
                // Osmiora Master Monogram
                div { class: "splash-logo-mark",
                    svg {
                        width: "64",
                        height: "64",
                        view_box: "0 0 64 64",
                        fill: "none",
                        circle { cx: "32", cy: "32", r: "28", stroke: "rgba(56, 189, 248, 0.3)", stroke_width: "1.5" }
                        circle { cx: "32", cy: "32", r: "18", stroke: "rgba(56, 189, 248, 0.8)", stroke_width: "2" }
                        circle { cx: "32", cy: "32", r: "6", fill: "#38bdf8" }
                    }
                }

                h1 { class: "splash-brand-title", "OSMOO" }
                div { class: "splash-brand-subtitle", "OPERATING SYSTEM MACHINE OPTIMIZATION OPERATOR" }
                div { class: "splash-brand-parent", "OSMIORA COMPUTATIONAL SYSTEMS" }

                // Cinematic Loading Bar
                div { class: "splash-progress-track",
                    div {
                        class: "splash-progress-fill",
                        style: "width: {pct}%;",
                    }
                }

                div { class: "splash-telemetry-row",
                    span { class: "splash-status-msg", "{status_text}" }
                    span { class: "splash-pct", "{pct}%" }
                }
            }
        }
    }
}
