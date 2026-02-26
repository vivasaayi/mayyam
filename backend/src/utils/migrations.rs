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

use rust_embed::RustEmbed;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement};
use tracing::{info, warn, error};

#[derive(RustEmbed)]
#[folder = "migrations/"]
struct Asset;

/// Run all embedded SQL migrations in alphabetical/numerical order.
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    info!("Initializing connection and checking for migrations table...");

    // Create a table to track applied migrations
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS _mayyam_migrations (
            id SERIAL PRIMARY KEY,
            filename VARCHAR(255) NOT NULL UNIQUE,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
    "#;

    db.execute(Statement::from_string(
        DbBackend::Postgres,
        create_table_sql.to_string(),
    ))
    .await?;

    // Get list of applied migrations
    let applied_stmt = r#"SELECT filename FROM _mayyam_migrations"#;
    
    // We use a raw query and simple mapping since we just need the strings
    let applied_migrations_result = db.query_all(Statement::from_string(
        DbBackend::Postgres,
        applied_stmt.to_string(),
    )).await?;
    
    let mut applied_files = Vec::new();
    for row in applied_migrations_result {
        if let Ok(filename) = row.try_get::<String>("", "filename") {
            applied_files.push(filename);
        }
    }

    // Get all embedded files and sort them alphabetically
    let mut script_files: Vec<String> = Asset::iter().map(|f| f.to_string()).collect();
    script_files.sort();

    let mut applied_count = 0;

    for filename in script_files {
        if !filename.ends_with(".sql") {
            continue;
        }

        if applied_files.contains(&filename) {
            continue;
        }

        info!("Applying migration: {}", filename);
        
        let file = Asset::get(&filename).expect("Failed to get embedded migration file");
        let sql = std::str::from_utf8(file.data.as_ref())
            .expect("Migration file is not valid UTF-8");
        
        // Execute the migration script
        match db.execute(Statement::from_string(DbBackend::Postgres, sql.to_string())).await {
            Ok(_) => {
                // Record the migration
                let record_sql = format!(
                    "INSERT INTO _mayyam_migrations (filename) VALUES ('{}');",
                    filename
                );
                db.execute(Statement::from_string(DbBackend::Postgres, record_sql)).await?;
                applied_count += 1;
                info!("Successfully applied migration: {}", filename);
            }
            Err(e) => {
                error!("Failed to apply migration {}: {}", filename, e);
                return Err(e);
            }
        }
    }

    if applied_count > 0 {
        info!("Successfully applied {} new migrations", applied_count);
    } else {
        info!("Database schema is up to date. No new migrations applied.");
    }

    Ok(())
}
