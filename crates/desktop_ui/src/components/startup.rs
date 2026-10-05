use dioxus::prelude::*;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntroStage {
    Black,
    OsmooLogo,
    OsmioraParent,
    FadeOut,
    Done,
}

#[component]
pub fn CinematicStartup(on_complete: EventHandler<()>) -> Element {
    let mut stage = use_signal(|| IntroStage::Black);

    use_effect(move || {
        spawn(async move {
            // Phase A: Clean Black Screen (600ms)
            tokio::time::sleep(Duration::from_millis(600)).await;
            stage.set(IntroStage::OsmooLogo);

            // Phase B: OSMOO Logo reveal (1400ms)
            tokio::time::sleep(Duration::from_millis(1400)).await;
            stage.set(IntroStage::OsmioraParent);

            // Phase C: Osmiora parent reveal (1600ms)
            tokio::time::sleep(Duration::from_millis(1600)).await;
            stage.set(IntroStage::FadeOut);

            // Phase D: Smooth Dissolve into Next State (600ms)
            tokio::time::sleep(Duration::from_millis(600)).await;
            stage.set(IntroStage::Done);
            on_complete.call(());
        });
    });

    let current = *stage.read();

    if current == IntroStage::Done {
        return rsx! {};
    }

    let is_logo_visible = matches!(current, IntroStage::OsmooLogo | IntroStage::OsmioraParent | IntroStage::FadeOut);
    let is_parent_visible = matches!(current, IntroStage::OsmioraParent | IntroStage::FadeOut);
    let is_fading_out = matches!(current, IntroStage::FadeOut);

    rsx! {
        div {
            class: if is_fading_out { "cinematic-viewport fade-exit" } else { "cinematic-viewport" },

            div { class: "cinematic-brand-group",
                // Precision Monolithic OSMOO Mark
                div {
                    class: if is_logo_visible { "osmoo-brand-mark visible" } else { "osmoo-brand-mark" },
                    div { class: "brand-mark-glyph",
                        div { class: "mark-orbit-ring" }
                        div { class: "mark-core-dot" }
                    }
                    div { class: "brand-wordmark", "OSMOO" }
                }

                // Understated Parent Company Typography
                div {
                    class: if is_parent_visible { "osmiora-parent-label visible" } else { "osmiora-parent-label" },
                    "Osmiora"
                }
            }
        }
    }
}
