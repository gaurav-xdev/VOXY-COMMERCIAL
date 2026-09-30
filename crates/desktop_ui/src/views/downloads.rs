use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[component]
pub fn DownloadsView() -> Element {
    let bridge = use_context::<AppBridge>();
    let downloads = use_signal(Vec::<String>::new);
    let mut url_input = use_signal(String::new);
    let mut file_input = use_signal(String::new);
    let download_status = use_signal(|| None::<String>);

    let refresh = {
        let dl = bridge.downloads.clone();
        move |_: Event<MouseData>| {
            let d = dl.clone();
            let mut dl_list = downloads;
            spawn(async move {
                let items = d.all_downloads();
                let formatted: Vec<String> = items
                    .iter()
                    .map(|p| {
                        let pct = match p.total_bytes {
                            Some(total) if total > 0 => {
                                (p.bytes_downloaded as f64 / total as f64 * 100.0) as u32
                            }
                            _ => 0,
                        };
                        format!(
                            "[{}%] {} - {} bytes - {:?}",
                            pct, p.filename, p.bytes_downloaded, p.status
                        )
                    })
                    .collect();
                dl_list.set(formatted);
            });
        }
    };

    let start_download = {
        let dl = bridge.downloads.clone();
        let url = url_input;
        let file = file_input;
        let mut status = download_status;
        let dl_list = downloads;
        move |_: Event<MouseData>| {
            let u = url.read().trim().to_string();
            let f = file.read().trim().to_string();
            if u.is_empty() || f.is_empty() {
                status.set(Some("URL and filename required".to_string()));
                return;
            }
            let d = dl.clone();
            let mut st = status;
            let mut dl_list = dl_list;
            spawn(async move {
                match d.download(&u, &f).await {
                    Ok(id) => {
                        st.set(Some(format!("Download started: ID {}", id)));
                        let items = d.all_downloads();
                        let formatted: Vec<String> = items
                            .iter()
                            .map(|p| format!("{} - {:?}", p.filename, p.status))
                            .collect();
                        dl_list.set(formatted);
                    }
                    Err(e) => st.set(Some(format!("Download error: {}", e))),
                }
            });
        }
    };

    rsx! {
        div {
            div { style: "display: flex; gap: 12px; margin-bottom: 20px;",
                input {
                    class: "input",
                    placeholder: "Download URL",
                    value: "{url_input}",
                    oninput: move |e| url_input.set(e.value()),
                    style: "flex: 1;",
                }
                input {
                    class: "input",
                    placeholder: "Filename",
                    value: "{file_input}",
                    oninput: move |e| file_input.set(e.value()),
                    style: "width: 200px;",
                }
                button { class: "btn btn-primary", onclick: start_download, "Download" }
                button { class: "btn btn-secondary", onclick: refresh, "Refresh" }
            }

            if let Some(s) = download_status.read().as_ref() {
                div { style: "margin-bottom: 16px; font-size: 12px; color: var(--info);", "{s}" }
            }

            div { style: "font-size: 13px; color: var(--text-secondary); margin-bottom: 12px;",
                "Download directory: {bridge.downloads.download_dir().display()}"
            }

            if downloads.read().is_empty() {
                div { class: "empty-state",
                    div { class: "empty-state-icon", "\u{2B07}" }
                    div { class: "empty-state-title", "No Active Downloads" }
                    div { class: "empty-state-desc",
                        "Enter a URL and filename to start a download."
                    }
                }
            } else {
                for item in downloads.read().iter() {
                    div { class: "download-item",
                        div { class: "download-icon", "\u{2B07}" }
                        div { class: "download-info",
                            div { class: "download-name", "{item}" }
                        }
                    }
                }
            }
        }
    }
}
