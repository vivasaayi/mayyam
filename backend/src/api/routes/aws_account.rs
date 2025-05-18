use actix_web::{web, Scope};
use crate::controllers::aws_account;

pub fn configure() -> Scope {
    web::scope("/api/aws/accounts")
        // List all AWS accounts
        .route("", web::get().to(aws_account::list_accounts))
        
        // Create new AWS account
        .route("", web::post().to(aws_account::create_account))
        
        // Get a specific AWS account
        .route("/{id}", web::get().to(aws_account::get_account))
        
        // Update an AWS account
        .route("/{id}", web::put().to(aws_account::update_account))
        
        // Delete an AWS account
        .route("/{id}", web::delete().to(aws_account::delete_account))
        
        // Sync resources for an AWS account
        .route("/{id}/sync", web::post().to(aws_account::sync_account_resources))
}
