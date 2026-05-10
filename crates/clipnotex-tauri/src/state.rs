use clipnotex_app::ExclusionFilter;
use clipnotex_donelog::DoneLogStore;
use clipnotex_paste::PasteController;
use clipnotex_store::StoreService;
use std::sync::Arc;

/// Composition root, owned by Tauri's `State`.
pub struct AppState {
    pub store: Arc<StoreService>,
    pub donelog: Arc<DoneLogStore>,
    pub filter: Arc<ExclusionFilter>,
    pub paste: Arc<PasteController>,
}
