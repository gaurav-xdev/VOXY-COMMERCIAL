use dioxus::prelude::*;

#[component]
pub fn IconMic(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" }
            path { d: "M19 10v2a7 7 0 0 1-14 0v-2" }
            line { x1: "12", x2: "12", y1: "19", y2: "22" }
        }
    }
}

#[component]
pub fn IconMicOff(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "2", x2: "22", y1: "2", y2: "22" }
            path { d: "M18.89 13.23A7.12 7.12 0 0 0 19 12v-2" }
            path { d: "M5 10v2a7 7 0 0 0 12 5" }
            path { d: "M15 9.34V5a3 3 0 0 0-5.68-1.33" }
            path { d: "M9 9v3a3 3 0 0 0 5.12 2.12" }
            line { x1: "12", x2: "12", y1: "19", y2: "22" }
        }
    }
}

#[component]
pub fn IconSpeaker(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { width: "16", height: "20", x: "4", y: "2", rx: "2" }
            circle { cx: "12", cy: "14", r: "4" }
            line { x1: "12", x2: "12.01", y1: "6", y2: "6" }
        }
    }
}

#[component]
pub fn IconCpu(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { width: "16", height: "16", x: "4", y: "4", rx: "2" }
            rect { width: "6", height: "6", x: "9", y: "9", rx: "1" }
            path { d: "M15 2v2" }
            path { d: "M15 20v2" }
            path { d: "M2 15h2" }
            path { d: "M2 9h2" }
            path { d: "M20 15h2" }
            path { d: "M20 9h2" }
            path { d: "M9 2v2" }
            path { d: "M9 20v2" }
        }
    }
}

#[component]
pub fn IconSettings(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" }
            circle { cx: "12", cy: "12", r: "3" }
        }
    }
}

#[component]
pub fn IconShield(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" }
            path { d: "m9 12 2 2 4-4" }
        }
    }
}

#[component]
pub fn IconSend(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "m22 2-7 20-4-9-9-4Z" }
            path { d: "M22 2 11 13" }
        }
    }
}

#[component]
pub fn IconLayers(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z" }
            path { d: "m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65" }
            path { d: "m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65" }
        }
    }
}

#[component]
pub fn IconMessageSquare(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
        }
    }
}

#[component]
pub fn IconCheck(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "20 6 9 17 4 12" }
        }
    }
}

#[component]
pub fn IconChevronRight(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "m9 18 6-6-6-6" }
        }
    }
}

#[component]
pub fn IconAlertCircle(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "10" }
            line { x1: "12", x2: "12", y1: "8", y2: "12" }
            line { x1: "12", x2: "12.01", y1: "16", y2: "16" }
        }
    }
}

#[component]
pub fn IconStopCircle(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "10" }
            rect { width: "6", height: "6", x: "9", y: "9" }
        }
    }
}

#[component]
pub fn IconRefresh(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
            path { d: "M3 3v5h5" }
            path { d: "M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" }
            path { d: "M16 16h5v5" }
        }
    }
}

#[component]
pub fn IconSparkles(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(20);
    let c = class.unwrap_or_else(|| "svg-icon".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275Z" }
            path { d: "M5 3v4" }
            path { d: "M19 17v4" }
            path { d: "M3 5h4" }
            path { d: "M17 19h4" }
        }
    }
}
