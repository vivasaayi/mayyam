use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/cloud")
            .route("/providers", web::get().to(|| async { "List cloud provider configurations" }))
            .service(
                web::scope("/aws")
                    .route("/accounts", web::get().to(|| async { "List AWS accounts" }))
                    .route("/ec2/instances", web::get().to(|| async { "List EC2 instances" }))
                    .route("/ec2/instances/{id}", web::get().to(|| async { "Get EC2 instance details" }))
                    .route("/ec2/instances/{id}/start", web::post().to(|| async { "Start EC2 instance" }))
                    .route("/ec2/instances/{id}/stop", web::post().to(|| async { "Stop EC2 instance" }))
                    .route("/s3/buckets", web::get().to(|| async { "List S3 buckets" }))
                    .route("/s3/buckets/{name}", web::get().to(|| async { "Get S3 bucket details" }))
                    .route("/cloudwatch/metrics", web::get().to(|| async { "Get CloudWatch metrics" }))
                    .route("/cost-explorer", web::get().to(|| async { "Get cost data" }))
            )
            .service(
                web::scope("/azure")
                    .route("/subscriptions", web::get().to(|| async { "List Azure subscriptions" }))
                    .route("/vms", web::get().to(|| async { "List Azure VMs" }))
                    .route("/vms/{id}", web::get().to(|| async { "Get Azure VM details" }))
                    .route("/vms/{id}/start", web::post().to(|| async { "Start Azure VM" }))
                    .route("/vms/{id}/stop", web::post().to(|| async { "Stop Azure VM" }))
                    .route("/storage", web::get().to(|| async { "List Azure storage accounts" }))
                    .route("/storage/{id}", web::get().to(|| async { "Get Azure storage account details" }))
                    .route("/metrics", web::get().to(|| async { "Get Azure metrics" }))
                    .route("/cost-analysis", web::get().to(|| async { "Get Azure cost analysis" }))
            )
    );
}
