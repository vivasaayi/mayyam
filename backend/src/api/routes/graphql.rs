use actix_web::{web, HttpResponse};
use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use async_graphql::{EmptySubscription, Schema};

// We'll define our GraphQL schema later
// For now, let's create a placeholder for the configuration

pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = web::scope("/api/graphql")
        .route("", web::post().to(graphql_handler))
        .route("/playground", web::get().to(graphql_playground));
    
    cfg.service(scope);
}

// Placeholder for GraphQL schema and handlers
// Will implement the actual schema later

pub type AppSchema = Schema<Query, Mutation, EmptySubscription>;

// Define basic Query and Mutation roots
pub struct Query;
pub struct Mutation;

async fn graphql_handler() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "data": {
            "hello": "Hello from GraphQL API!"
        }
    }))
}

async fn graphql_playground() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(
            r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Mayyam GraphQL Playground</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/graphql-playground-react/build/static/css/index.css" />
  <link rel="shortcut icon" href="https://cdn.jsdelivr.net/npm/graphql-playground-react/build/favicon.png" />
  <script src="https://cdn.jsdelivr.net/npm/graphql-playground-react/build/static/js/middleware.js"></script>
</head>
<body>
  <div id="root">
    <style>
      body {
        background-color: rgb(23, 42, 58);
        font-family: Open Sans, sans-serif;
        height: 90vh;
      }
      #root {
        height: 100%;
        width: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
      }
      .loading {
        font-size: 32px;
        font-weight: 200;
        color: rgba(255, 255, 255, .6);
        margin-left: 20px;
      }
      img {
        width: 78px;
        height: 78px;
      }
      .title {
        font-weight: 400;
      }
    </style>
    <img src='https://cdn.jsdelivr.net/npm/graphql-playground-react/build/logo.png' alt=''>
    <div class="loading"> Loading
      <span class="title">Mayyam GraphQL Playground</span>
    </div>
  </div>
  <script>window.addEventListener('load', function (event) {
      GraphQLPlayground.init(document.getElementById('root'), {
        endpoint: '/api/graphql',
        settings: {
          'request.credentials': 'include',
        }
      })
    })</script>
</body>
</html>"#,
        )
}
