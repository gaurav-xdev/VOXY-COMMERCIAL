use dioxus::prelude::*;

#[component]
pub fn IconOsmooLogo(class: Option<String>, size: Option<u32>) -> Element {
    let s = size.unwrap_or(36);
    let c = class.unwrap_or_else(|| "osmoo-logo-mark".to_string());
    rsx! {
        svg {
            class: "{c}",
            width: "{s}",
            height: "{s}",
            view_box: "0 0 100 100",
            fill: "currentColor",
            xmlns: "http://www.w3.org/2000/svg",
            // Precise 6-blade helical aperture iris derived from reference image 3
            g {
                transform: "translate(50, 50)",
                // Blade 0
                path {
                    d: "M 0,-46 C 22,-46 41,-28 46,-5 C 38,-15 25,-22 10,-24 C 6,-32 1,-40 0,-46 Z",
                }
                // Blade 1 (60 deg)
                path {
                    transform: "rotate(60)",
                    d: "M 0,-46 C 22,-46 41,-28 46,-5 C 38,-15 25,-22 10,-24 C 6,-32 1,-40 0,-46 Z",
                }
                // Blade 2 (120 deg)
                path {
                    transform: "rotate(120)",
                    d: "M 0,-46 C 22,-46 41,-28 46,-5 C 38,-15 25,-22 10,-24 C 6,-32 1,-40 0,-46 Z",
                }
                // Blade 3 (180 deg)
                path {
                    transform: "rotate(180)",
                    d: "M 0,-46 C 22,-46 41,-28 46,-5 C 38,-15 25,-22 10,-24 C 6,-32 1,-40 0,-46 Z",
                }
                // Blade 4 (240 deg)
                path {
                    transform: "rotate(240)",
                    d: "M 0,-46 C 22,-46 41,-28 46,-5 C 38,-15 25,-22 10,-24 C 6,-32 1,-40 0,-46 Z",
                }
                // Blade 5 (300 deg)
                path {
                    transform: "rotate(300)",
                    d: "M 0,-46 C 22,-46 41,-28 46,-5 C 38,-15 25,-22 10,-24 C 6,-32 1,-40 0,-46 Z",
                }
            }
        }
    }
}

#[component]
pub fn IconRobot(class: Option<String>, size: Option<u32>) -> Element {
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
            rect { x: "4", y: "8", width: "16", height: "12", rx: "3" }
            line { x1: "12", y1: "2", x2: "12", y2: "5" }
            circle { cx: "12", cy: "2", r: "1", fill: "currentColor" }
            line { x1: "9", y1: "13", x2: "9.01", y2: "13" }
            line { x1: "15", y1: "13", x2: "15.01", y2: "13" }
            path { d: "M2 13h2" }
            path { d: "M20 13h2" }
        }
    }
}

#[component]
pub fn IconOrb(class: Option<String>, size: Option<u32>) -> Element {
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
            stroke_width: "1.75",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "9" }
            path { d: "M12 3a14.5 14.5 0 0 0 0 18" }
            path { d: "M12 3a14.5 14.5 0 0 1 0 18" }
            line { x1: "3", y1: "12", x2: "21", y2: "12" }
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
            path { d: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" }
        }
    }
}

#[component]
pub fn IconEye(class: Option<String>, size: Option<u32>) -> Element {
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
            path { d: "M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" }
            circle { cx: "12", cy: "12", r: "3" }
        }
    }
}

#[component]
pub fn IconEyeOff(class: Option<String>, size: Option<u32>) -> Element {
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
            path { d: "M9.88 9.88a3 3 0 1 0 4.24 4.24" }
            path { d: "M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68" }
            path { d: "M6.61 6.61A13.526 13.526 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61" }
            line { x1: "2", y1: "2", x2: "22", y2: "22" }
        }
    }
}

#[component]
pub fn IconLock(class: Option<String>, size: Option<u32>) -> Element {
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
            rect { width: "18", height: "11", x: "3", y: "11", rx: "2", ry: "2" }
            path { d: "M7 11V7a5 5 0 0 1 10 0v4" }
        }
    }
}

#[component]
pub fn IconArrowLeft(class: Option<String>, size: Option<u32>) -> Element {
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
            path { d: "m12 19-7-7 7-7" }
            path { d: "M19 12H5" }
        }
    }
}

#[component]
pub fn IconSliders(class: Option<String>, size: Option<u32>) -> Element {
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
            line { x1: "4", x2: "4", y1: "21", y2: "14" }
            line { x1: "4", x2: "4", y1: "10", y2: "3" }
            line { x1: "12", x2: "12", y1: "21", y2: "12" }
            line { x1: "12", x2: "12", y1: "8", y2: "3" }
            line { x1: "20", x2: "20", y1: "21", y2: "16" }
            line { x1: "20", x2: "20", y1: "12", y2: "3" }
            line { x1: "1", x2: "7", y1: "14", y2: "14" }
            line { x1: "9", x2: "15", y1: "8", y2: "8" }
            line { x1: "17", x2: "23", y1: "16", y2: "16" }
        }
    }
}

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
            line { x1: "22", x2: "11", y1: "2", y2: "13" }
            polygon { points: "22 2 15 22 11 13 2 9 22 2" }
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
