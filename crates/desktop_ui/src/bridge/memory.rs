use std::sync::Arc;

use voxy_memory::{MemoryApi, MemoryId, MemoryItem, MemoryQuery};

#[derive(Clone)]
pub struct MemoryBridge {
    api: Arc<dyn MemoryApi>,
}

impl MemoryBridge {
    pub fn new(api: Arc<dyn MemoryApi>) -> Self {
        Self { api }
    }

    pub async fn search(
        &self,
        query: MemoryQuery,
    ) -> Result<Vec<voxy_memory::SearchResult>, String> {
        self.api
            .search(&query)
            .await
            .map_err(|e| format!("Search failed: {}", e))
    }

    pub async fn store(&self, item: MemoryItem) -> Result<MemoryId, String> {
        self.api
            .store(item)
            .await
            .map_err(|e| format!("Store failed: {}", e))
    }

    pub async fn retrieve(&self, id: &MemoryId) -> Result<MemoryItem, String> {
        self.api
            .retrieve(id)
            .await
            .map_err(|e| format!("Retrieve failed: {}", e))
    }

    pub async fn delete(&self, id: &MemoryId) -> Result<(), String> {
        self.api
            .delete(id)
            .await
            .map_err(|e| format!("Delete failed: {}", e))
    }

    pub async fn stats(&self) -> Result<voxy_memory::MemoryStats, String> {
        self.api
            .stats()
            .await
            .map_err(|e| format!("Stats failed: {}", e))
    }

    pub async fn consolidate(&self) -> Result<usize, String> {
        self.api
            .consolidate()
            .await
            .map_err(|e| format!("Consolidate failed: {}", e))
    }
}
