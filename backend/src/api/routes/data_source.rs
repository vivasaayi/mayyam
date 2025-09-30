use actix_web::{web, HttpResponse, Result};
use std::sync::Arc;
use uuid::Uuid;

use crate::controllers::data_source::{
    CreateDataSourceRequest, DataSourceController, DataSourceQueryParams, UpdateDataSourceRequest,
};

pub fn configure(cfg: &mut web::ServiceConfig, controller: Arc<DataSourceController>) {
    cfg.service(
        web::scope("/api/v1/data-sources")
            .app_data(web::Data::new(controller))
            .route("", web::get().to(list_data_sources))
            .route("", web::post().to(create_data_source))
            .route("/{id}", web::get().to(get_data_source))
            .route("/{id}", web::put().to(update_data_source))
            .route("/{id}", web::delete().to(delete_data_source))
            .route("/{id}/test", web::post().to(test_data_source_connection))
            .route("/search", web::get().to(search_data_sources)),
    );
}

async fn list_data_sources(
    controller: web::Data<DataSourceController>,
    query: web::Query<DataSourceQueryParams>,
) -> Result<HttpResponse> {
    match controller.list_data_sources(query.into_inner()).await {
        Ok(data_sources) => Ok(HttpResponse::Ok().json(data_sources)),
        Err(e) => {
            tracing::error!("Failed to list data sources: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to list data sources",
                "details": e.to_string()
            })))
        }
    }
}

async fn create_data_source(
    controller: web::Data<DataSourceController>,
    request: web::Json<CreateDataSourceRequest>,
) -> Result<HttpResponse> {
    match controller.create_data_source(request.into_inner()).await {
        Ok(data_source) => Ok(HttpResponse::Created().json(data_source)),
        Err(e) => {
            tracing::error!("Failed to create data source: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create data source",
                "details": e.to_string()
            })))
        }
    }
}

async fn get_data_source(
    controller: web::Data<DataSourceController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let id = path.into_inner();

    match controller.get_data_source(id).await {
        Ok(Some(data_source)) => Ok(HttpResponse::Ok().json(data_source)),
        Ok(None) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Data source not found"
        }))),
        Err(e) => {
            tracing::error!("Failed to get data source {}: {:?}", id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get data source",
                "details": e.to_string()
            })))
        }
    }
}

async fn update_data_source(
    controller: web::Data<DataSourceController>,
    path: web::Path<Uuid>,
    request: web::Json<UpdateDataSourceRequest>,
) -> Result<HttpResponse> {
    let id = path.into_inner();

    match controller
        .update_data_source(id, request.into_inner())
        .await
    {
        Ok(data_source) => Ok(HttpResponse::Ok().json(data_source)),
        Err(e) => {
            tracing::error!("Failed to update data source {}: {:?}", id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to update data source",
                "details": e.to_string()
            })))
        }
    }
}

async fn delete_data_source(
    controller: web::Data<DataSourceController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let id = path.into_inner();

    match controller.delete_data_source(id).await {
        Ok(()) => Ok(HttpResponse::NoContent().finish()),
        Err(e) => {
            tracing::error!("Failed to delete data source {}: {:?}", id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to delete data source",
                "details": e.to_string()
            })))
        }
    }
}

async fn test_data_source_connection(
    controller: web::Data<DataSourceController>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let id = path.into_inner();

    match controller.test_data_source_connection(id).await {
        Ok(result) => Ok(HttpResponse::Ok().json(result)),
        Err(e) => {
            tracing::error!("Failed to test data source connection {}: {:?}", id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to test data source connection",
                "details": e.to_string()
            })))
        }
    }
}

async fn search_data_sources(
    controller: web::Data<DataSourceController>,
    query: web::Query<DataSourceQueryParams>,
) -> Result<HttpResponse> {
    match controller.search_data_sources(query.into_inner()).await {
        Ok(data_sources) => Ok(HttpResponse::Ok().json(data_sources)),
        Err(e) => {
            tracing::error!("Failed to search data sources: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to search data sources",
                "details": e.to_string()
            })))
        }
    }
}
