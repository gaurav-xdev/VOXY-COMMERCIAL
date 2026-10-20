use crate::components::icons::IconOsmooLogo;
use dioxus::prelude::*;

#[component]
pub fn StartupSplash(progress: f32, status_text: String) -> Element {
    let pct = (progress * 100.0).clamp(0.0, 100.0) as u32;

    rsx! {
        div { class: "splash-screen-container",
            div { class: "splash-ambient-radiance" }

            div { class: "splash-branding-box",
                // Authentic Osmiora Helical Aperture Logo Mark
                div { class: "splash-logo-mark",
                    IconOsmooLogo { size: 68 }
                }

                h1 { class: "splash-brand-title", "OSMOO" }
                div { class: "splash-brand-subtitle", "OPERATING SYSTEM MACHINE OPTIMIZATION OPERATOR" }
                div { class: "splash-brand-parent", "OSMIORA COMPUTATIONAL SYSTEMS" }

                // Machined Loading Track
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
