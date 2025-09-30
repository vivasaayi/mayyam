use std::sync::Arc;
use actix_web::{web, Scope};

use crate::controllers::sync_run::{self, SyncRunController};

pub fn configure(controller: Arc<SyncRunController>) -> Scope {
    web::scope("/api/sync-runs")
        .app_data(web::Data::new(controller))
        .route("", web::post().to(sync_run::create_sync_run))
        .route("", web::get().to(sync_run::list_sync_runs))
        .route("/{id}", web::get().to(sync_run::get_sync_run))
}
