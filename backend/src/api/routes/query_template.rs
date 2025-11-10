// Copyright (c) 2025 Rajan Panneer Selvam
//
// Licensed under the Business Source License 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.mariadb.com/bsl11
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use crate::controllers::query_template;
use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/query-templates")
            .service(
                web::resource("")
                    .route(web::get().to(query_template::list_templates))
                    .route(web::post().to(query_template::create_template)),
            )
            .service(
                web::resource("/common")
                    .route(web::get().to(query_template::list_common_templates)),
            )
            .service(
                web::resource("/connection-type/{type}")
                    .route(web::get().to(query_template::list_templates_by_type)),
            )
            .service(
                web::resource("/{id}")
                    .route(web::get().to(query_template::get_template))
                    .route(web::put().to(query_template::update_template))
                    .route(web::delete().to(query_template::delete_template)),
            ),
    );
}
