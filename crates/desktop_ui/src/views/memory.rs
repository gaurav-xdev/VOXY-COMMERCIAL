use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[component]
pub fn MemoryView() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut search_query = use_signal(String::new);
    let search_results = use_signal(Vec::<String>::new);
    let stats = use_signal(|| None::<String>);
    let is_searching = use_signal(|| false);
    let mut store_text = use_signal(String::new);
    let store_status = use_signal(|| None::<String>);

    let do_search = {
        let results = search_results;
        let mut searching = is_searching;
        let memory = bridge.memory.clone();
        let query = search_query;
        move |_: Event<MouseData>| {
            let q = query.read().trim().to_string();
            if q.is_empty() {
                return;
            }
            searching.set(true);
            let mut res = results;
            let mem = memory.clone();
            spawn(async move {
                let memory_query = voxy_memory::MemoryQuery {
                    query_text: Some(q),
                    memory_types: None,
                    states: None,
                    tags: None,
                    time_range: None,
                    min_importance: None,
                    max_results: 20,
                    include_embeddings: false,
                    source_filter: None,
                };
                match mem.search(&memory_query).await {
                    Ok(items) => {
                        let formatted: Vec<String> = items
                            .iter()
                            .map(|sr| {
                                format!(
                                    "[{:.0}%] {:?} - {:?}",
                                    sr.score * 100.0,
                                    sr.item.memory_type,
                                    sr.item.content
                                )
                            })
                            .collect();
                        res.set(formatted);
                    }
                    Err(e) => {
                        res.set(vec![format!("Search error: {}", e)]);
                    }
                }
                searching.set(false);
            });
        }
    };

    let load_stats = {
        let memory = bridge.memory.clone();
        move |_: Event<MouseData>| {
            let mem = memory.clone();
            let mut s = stats;
            spawn(async move {
                match mem.stats().await {
                    Ok(st) => {
                        s.set(Some(format!(
                            "Total: {} | Working: {} | ShortTerm: {} | Episodic: {} | Semantic: {} | Procedural: {} | Graph: {} nodes, {} edges",
                            st.total_items, st.working_count, st.short_term_count,
                            st.episodic_count, st.semantic_count, st.procedural_count,
                            st.graph_nodes, st.graph_edges
                        )));
                    }
                    Err(e) => {
                        s.set(Some(format!("Stats error: {}", e)));
                    }
                }
            });
        }
    };

    let do_store = {
        let status = store_status;
        let memory = bridge.memory.clone();
        let text = store_text;
        move |_: Event<MouseData>| {
            let t = text.read().trim().to_string();
            if t.is_empty() {
                return;
            }
            let mem = memory.clone();
            let mut st = status;
            let mut txt = text;
            spawn(async move {
                let now = chrono::Utc::now();
                let item = voxy_memory::MemoryItem {
                    id: voxy_memory::MemoryId(format!("mem_{}", uuid::Uuid::new_v4())),
                    memory_type: voxy_memory::MemoryType::Working,
                    state: voxy_memory::MemoryState::Active,
                    content: serde_json::json!({ "text": t }),
                    importance: 0.5,
                    timestamp: now,
                    last_accessed: now,
                    access_count: 0,
                    context_tags: vec!["user_input".to_string()],
                    source: "desktop_ui".to_string(),
                    version: 1,
                    ttl: None,
                    metadata: std::collections::HashMap::new(),
                    embedding: None,
                    parent_id: None,
                    related_ids: vec![],
                };
                match mem.store(item).await {
                    Ok(id) => {
                        st.set(Some(format!("Stored: {}", id.0)));
                        txt.set(String::new());
                    }
                    Err(e) => {
                        st.set(Some(format!("Store error: {}", e)));
                    }
                }
            });
        }
    };

    rsx! {
        div {
            div { style: "display: flex; gap: 12px; margin-bottom: 20px;",
                input {
                    class: "input",
                    placeholder: "Search memories...",
                    value: "{search_query}",
                    oninput: move |e| search_query.set(e.value()),
                    style: "flex: 1;",
                }
                button { class: "btn btn-primary", onclick: do_search,
                    if *is_searching.read() { "Searching..." } else { "Search" }
                }
                button { class: "btn btn-secondary", onclick: load_stats, "Stats" }
            }

            if let Some(s) = stats.read().as_ref() {
                div { style: "margin-bottom: 16px; padding: 12px; background: var(--bg-secondary); border-radius: var(--radius-md); border: 1px solid var(--border);",
                    div { style: "font-size: 13px; color: var(--text-secondary);", "{s}" }
                }
            }

            div { style: "display: flex; gap: 12px; margin-bottom: 20px;",
                input {
                    class: "input",
                    placeholder: "Store a new memory...",
                    value: "{store_text}",
                    oninput: move |e| store_text.set(e.value()),
                    style: "flex: 1;",
                }
                button { class: "btn btn-primary", onclick: do_store, "+ Store" }
            }

            if let Some(s) = store_status.read().as_ref() {
                div { style: "margin-bottom: 16px; font-size: 12px; color: var(--success);", "{s}" }
            }

            if search_results.read().is_empty() && !*is_searching.read() {
                div { class: "empty-state",
                    div { class: "empty-state-icon", "\u{1F4E6}" }
                    div { class: "empty-state-title", "Memory Engine Connected" }
                    div { class: "empty-state-desc",
                        "Search or store memories. The SQLite memory engine is active."
                    }
                }
            } else {
                for result in search_results.read().iter() {
                    div { class: "download-item",
                        div { class: "download-icon", "\u{1F50D}" }
                        div { class: "download-info",
                            div { class: "download-name", "{result}" }
                        }
                    }
                }
            }
        }
    }
}
