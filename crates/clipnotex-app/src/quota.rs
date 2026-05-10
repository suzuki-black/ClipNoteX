use arc_swap::ArcSwap;
use clipnotex_core::{
    settings::{EvictionPolicy as Policy, HistoryConfig},
    Result,
};
use clipnotex_store::{EvictionPolicy as StorePolicy, StoreService};
use std::sync::Arc;

pub struct QuotaManager {
    store: Arc<StoreService>,
    config: Arc<ArcSwap<HistoryConfig>>,
}

impl QuotaManager {
    pub fn new(store: Arc<StoreService>, config: HistoryConfig) -> Arc<Self> {
        Arc::new(Self {
            store,
            config: Arc::new(ArcSwap::from_pointee(config)),
        })
    }

    pub fn replace_config(&self, cfg: HistoryConfig) {
        self.config.store(Arc::new(cfg));
    }

    pub fn enforce(&self) -> Result<u64> {
        let cfg = self.config.load();
        let target = match cfg.eviction_policy {
            Policy::CountPriority => StorePolicy::UntilCount(cfg.max_items),
            Policy::SizePriority => StorePolicy::UntilBytes(cfg.max_bytes),
        };
        self.store.evict(target)
    }
}
