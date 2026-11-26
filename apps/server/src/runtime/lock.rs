use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;
#[derive(Clone, Default)]
pub struct SessionCommandLocks {
    pub locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    pub active: Arc<Mutex<HashSet<String>>>,
}
impl SessionCommandLocks {
    pub async fn lock_for(&self, id: &str) -> Arc<Mutex<()>> {
        let mut all = self.locks.lock().await;
        all.entry(id.into())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
    pub async fn try_register_active(&self, id: &str) -> bool {
        let mut active = self.active.lock().await;
        active.insert(id.into())
    }
    pub async fn clear_active(&self, id: &str) {
        self.active.lock().await.remove(id);
    }
}
