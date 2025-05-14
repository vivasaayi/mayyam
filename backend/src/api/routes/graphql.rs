use actix_web::web;
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use actix_web::{HttpResponse, Result};
use async_graphql::{EmptySubscription, Schema};

// We'll define our GraphQL schema later
// For now, let's create a placeholder for the configuration

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/graphql")
            .route("", web::post().to(graphql_handler))
            .route("", web::get().to(graphql_playground))
    );
}

// Placeholder for GraphQL schema and handlers
// Will implement the actual schema later

pub type AppSchema = Schema<Query, Mutation, EmptySubscription>;

// Define basic Query and Mutation roots
pub struct Query;
pub struct Mutation;

async fn graphql_handler(
    schema: web::Data<AppSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphql_playground() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(playground_source(GraphQLPlaygroundConfig::new("/api/graphql")))
    )
}
