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
            g {
                path {
                    d: "M 73.5,46.2 L 69.1,41.6 L 63.2,38.5 L 66.9,43.7 L 68.8,50.1 L 69.0,58.4 L 67.4,65.3 L 63.2,72.9 L 58.7,77.8 L 52.4,81.9 L 45.0,84.3 L 39.3,84.6 L 32.9,83.4 L 26.6,80.3 L 21.3,75.7 L 16.6,69.0 L 14.0,61.8 L 9.1,53.7 L 7.1,48.1 L 6.0,41.9 L 5.3,47.2 L 5.7,56.3 L 7.2,63.2 L 11.2,72.8 L 14.3,77.6 L 19.4,83.2 L 26.0,87.8 L 31.9,90.1 L 38.4,91.3 L 45.1,91.2 L 51.9,89.6 L 58.7,86.3 L 65.0,81.3 L 69.9,75.3 L 73.5,67.8 L 75.4,60.0 L 75.6,52.6 Z",
                }
                path {
                    d: "M 26.9,31.0 L 24.4,41.2 L 24.4,47.4 L 25.4,51.6 L 27.2,54.9 L 30.1,57.9 L 37.4,62.1 L 32.9,55.9 L 30.9,47.9 L 30.9,42.8 L 32.2,36.2 L 36.9,27.1 L 43.4,20.7 L 52.1,16.6 L 60.3,15.7 L 69.4,17.9 L 76.0,22.1 L 82.1,29.0 L 85.0,34.9 L 86.0,38.7 L 91.2,47.2 L 94.0,58.1 L 94.7,51.0 L 94.3,44.1 L 92.9,37.6 L 90.3,30.3 L 84.1,20.6 L 78.4,15.3 L 72.5,11.9 L 61.6,9.1 L 53.8,9.4 L 45.9,11.6 L 38.8,15.6 L 33.4,20.6 L 29.4,26.0 Z",
                }
                path {
                    d: "M 45.3,5.0 L 37.1,6.5 L 31.0,8.7 L 23.7,12.9 L 17.9,18.1 L 14.3,22.9 L 11.5,28.8 L 10.0,35.7 L 10.0,42.4 L 11.5,49.3 L 14.1,55.1 L 17.9,60.6 L 22.8,65.4 L 29.7,70.1 L 36.8,73.2 L 42.9,74.7 L 49.7,75.1 L 55.4,74.1 L 59.4,71.6 L 61.9,68.4 L 64.3,62.4 L 57.9,66.6 L 50.9,68.4 L 42.9,67.9 L 35.4,65.4 L 28.2,61.0 L 21.8,54.3 L 18.2,47.6 L 16.6,40.0 L 17.1,33.8 L 18.8,28.5 L 21.5,24.1 L 25.7,19.6 L 42.6,9.0 L 49.7,6.3 L 54.7,5.4 Z",
                }
                path {
                    d: "M 54.7,25.3 L 48.4,25.4 L 42.1,27.8 L 38.2,32.1 L 35.7,37.8 L 39.0,35.1 L 42.6,33.4 L 51.5,31.8 L 59.1,32.8 L 68.1,36.5 L 73.2,40.1 L 77.4,44.4 L 80.0,48.4 L 82.6,55.1 L 83.2,63.7 L 80.4,72.6 L 74.4,80.1 L 57.8,91.0 L 50.9,93.8 L 46.2,94.9 L 54.1,94.9 L 62.6,93.2 L 70.0,90.4 L 75.9,87.1 L 82.6,81.0 L 85.7,76.8 L 88.7,70.3 L 90.0,63.8 L 89.9,56.9 L 88.1,49.7 L 85.6,44.4 L 81.5,38.7 L 77.1,34.4 L 71.0,30.3 L 64.0,27.2 Z",
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
