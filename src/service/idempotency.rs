use crate::store::MemoryStore;

pub async fn insert_if_absent(store: &MemoryStore, ev: crate::domain::event::Event) -> (crate::domain::event::EventRecord, bool) {
    store.insert_if_absent(ev).await
}
