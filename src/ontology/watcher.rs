use std::sync::Arc;
use std::time::Duration;

use super::manager::OntologyManager;

/// Spawns a background task that polls OWL files for external changes every 5 seconds
/// and triggers a reload in the OntologyManager if the modification time changes.
///
/// This is a local optimization; [`OntologyManager::get_or_load`] also reloads on access.
pub fn spawn_watcher(manager: Arc<OntologyManager>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        // skip the immediate first tick
        interval.tick().await;
        loop {
            interval.tick().await;
            let handles = manager.loaded_handles().await;
            for (path, handle) in handles {
                let mut api = handle.lock().await;
                match api.check_and_reload_if_modified() {
                    Ok(true) => {
                        tracing::info!("Reloaded {} after external change", path.display());
                    }
                    Ok(false) => {}
                    Err(e) => {
                        tracing::warn!("Failed to check {}: {}", path.display(), e);
                    }
                }
            }
        }
    })
}
