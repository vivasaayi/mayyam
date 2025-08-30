use actix_web::web;
use crate::controllers::query_template;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/query-templates")
            .service(web::resource("")
                .route(web::get().to(query_template::list_templates))
                .route(web::post().to(query_template::create_template)))
            .service(web::resource("/common")
                .route(web::get().to(query_template::list_common_templates)))
            .service(web::resource("/connection-type/{type}")
                .route(web::get().to(query_template::list_templates_by_type)))
            .service(web::resource("/{id}")
                .route(web::get().to(query_template::get_template))
                .route(web::put().to(query_template::update_template))
                .route(web::delete().to(query_template::delete_template)))
    );
}
