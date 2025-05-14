use actix_web::{web, App, HttpServer};
use actix_cors::Cors;
use tracing::{info, error};
use std::error::Error;

use crate::config::Config;
use crate::api::routes;
use crate::middleware::auth::AuthMiddleware;

pub async fn run_server(host: String, port: u16, config: Config) -> Result<(), Box<dyn Error>> {
    let addr = format!("{}:{}", host, port);
    
    info!("Starting Mayyam server on http://{}", addr);
    
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
            
        App::new()
            .wrap(cors)
            .wrap(AuthMiddleware::new(&config))
            .configure(routes::auth::configure)
            .configure(routes::database::configure)
            .configure(routes::kafka::configure)
            .configure(routes::cloud::configure)
            .configure(routes::kubernetes::configure)
            .configure(routes::chaos::configure)
            .configure(routes::ai::configure)
            .configure(routes::graphql::configure)
    })
    .bind(addr)?
    .run()
    .await?;
    
    Ok(())
}
