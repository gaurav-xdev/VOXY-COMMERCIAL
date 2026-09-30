use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[component]
pub fn PluginsView() -> Element {
    let bridge = use_context::<AppBridge>();
    let plugin_list = use_signal(Vec::<String>::new);
    let plugin_states = use_signal(std::collections::HashMap::<String, String>::new);

    let refresh = {
        let pm = bridge.plugins.clone();
        let list = plugin_list;
        let states = plugin_states;
        move |_: Event<MouseData>| {
            let p = pm.clone();
            let mut l = list;
            let mut s = states;
            spawn(async move {
                let ids = p.list_plugins().await;
                let mut state_map = std::collections::HashMap::new();
                for id in &ids {
                    let state = match p.get_state(id).await {
                        Some(st) => format!("{:?}", st),
                        None => "Unknown".to_string(),
                    };
                    state_map.insert(id.clone(), state);
                }
                l.set(ids);
                s.set(state_map);
            });
        }
    };

    rsx! {
        div {
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;",
                div {
                    style: "font-size: 13px; color: var(--text-secondary);",
                    "{plugin_list.read().len()} plugins registered"
                }
                div { style: "display: flex; gap: 8px;",
                    button { class: "btn btn-primary", onclick: refresh, "Refresh" }
                }
            }

            if plugin_list.read().is_empty() {
                div { class: "empty-state",
                    div { class: "empty-state-icon", "\u{1F50C}" }
                    div { class: "empty-state-title", "No Plugins Loaded" }
                    div { class: "empty-state-desc",
                        "The PluginManager is active. Load plugins via manifest to see them here."
                    }
                }
            } else {
                for id in plugin_list.read().iter() {
                    PluginCard { id: id.clone(), plugin_states: plugin_states }
                }
            }
        }
    }
}

#[component(eq = false)]
fn PluginCard(
    id: String,
    plugin_states: Signal<std::collections::HashMap<String, String>>,
) -> Element {
    let state_str = plugin_states
        .read()
        .get(&id)
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());
    rsx! {
        div { class: "plugin-card",
            div { class: "plugin-header",
                span { class: "plugin-name", "{id}" }
                span { class: "badge badge-info", "{state_str}" }
            }
        }
    }
}
