use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use super::manager::OntologyManager;

/// Spawns a background task that polls OWL files for external changes every 5 seconds
/// and triggers a reload in the OntologyManager if the modification time changes.
pub fn spawn_watcher(manager: Arc<Mutex<OntologyManager>>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        // skip the immediate first tick
        interval.tick().await;
        loop {
            interval.tick().await;
            let mut mgr = manager.lock().await;
            let paths: Vec<_> = mgr.apis.keys().cloned().collect();
            for path in paths {
                if let Some(api) = mgr.apis.get_mut(&path) {
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
        }
    })
}
