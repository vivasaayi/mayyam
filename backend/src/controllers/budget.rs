use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::services::budget_service::BudgetService;
use crate::models::cost_budget::{BudgetDto, BudgetStatus, BudgetAlert};

#[derive(Deserialize)]
pub struct CreateBudgetRequest {
    pub name: String,
    pub description: Option<String>,
    pub budget_type: String,
    pub budget_period: String,
    pub amount: String, // BigDecimal as string
    pub currency: String,
    pub start_date: String, // Date as string
    pub end_date: Option<String>,
    pub alert_thresholds: Vec<BudgetAlertThresholdRequest>,
    pub tags: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct BudgetAlertThresholdRequest {
    pub percentage: String, // BigDecimal as string
    pub alert_type: String,
}

#[derive(Serialize)]
pub struct BudgetResponse {
    pub id: Uuid,
    pub account_id: String,
    pub name: String,
    pub description: Option<String>,
    pub budget_type: String,
    pub budget_period: String,
    pub amount: String,
    pub currency: String,
    pub start_date: String,
    pub end_date: Option<String>,
    pub alert_thresholds: Vec<BudgetAlertThresholdResponse>,
    pub tags: std::collections::HashMap<String, String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct BudgetAlertThresholdResponse {
    pub percentage: String,
    pub alert_type: String,
}

#[derive(Serialize)]
pub struct BudgetStatusResponse {
    pub budget_id: Uuid,
    pub current_spending: String,
    pub budget_amount: String,
    pub utilization_percentage: String,
    pub forecasted_spending: String,
    pub remaining_budget: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct BudgetAlertResponse {
    pub id: Uuid,
    pub budget_id: Uuid,
    pub alert_type: String,
    pub threshold_percentage: String,
    pub current_percentage: String,
    pub message: String,
    pub created_at: String,
}

impl From<crate::models::cost_budget::Budget> for BudgetResponse {
    fn from(budget: crate::models::cost_budget::Budget) -> Self {
        let alert_thresholds: Vec<crate::models::cost_budget::BudgetAlertThreshold> = serde_json::from_value(budget.alert_thresholds)
            .unwrap_or_else(|_| vec![]);

        let tags: std::collections::HashMap<String, String> = serde_json::from_value(budget.tags)
            .unwrap_or_else(|_| std::collections::HashMap::new());

        BudgetResponse {
            id: budget.id,
            account_id: budget.account_id,
            name: budget.name,
            description: budget.description,
            budget_type: budget.budget_type,
            budget_period: budget.budget_period,
            amount: budget.amount.to_string(),
            currency: budget.currency,
            start_date: budget.start_date.to_string(),
            end_date: budget.end_date.map(|d| d.to_string()),
            alert_thresholds: alert_thresholds.into_iter()
                .map(|t| BudgetAlertThresholdResponse {
                    percentage: t.percentage.to_string(),
                    alert_type: t.alert_type,
                })
                .collect(),
            tags,
            created_at: budget.created_at.to_string(),
            updated_at: budget.updated_at.to_string(),
        }
    }
}

impl From<BudgetStatus> for BudgetStatusResponse {
    fn from(status: BudgetStatus) -> Self {
        BudgetStatusResponse {
            budget_id: status.budget_id,
            current_spending: status.current_spending.to_string(),
            budget_amount: status.budget_amount.to_string(),
            utilization_percentage: status.utilization_percentage.to_string(),
            forecasted_spending: status.forecasted_spending.to_string(),
            remaining_budget: status.remaining_budget.to_string(),
            status: status.status,
        }
    }
}

impl From<BudgetAlert> for BudgetAlertResponse {
    fn from(alert: BudgetAlert) -> Self {
        BudgetAlertResponse {
            id: alert.id,
            budget_id: alert.budget_id,
            alert_type: alert.alert_type,
            threshold_percentage: alert.threshold_percentage.to_string(),
            current_percentage: alert.current_percentage.to_string(),
            message: alert.message,
            created_at: alert.created_at.to_string(),
        }
    }
}

impl CreateBudgetRequest {
    fn into_budget_dto(self, account_id: String) -> Result<BudgetDto, String> {
        use chrono::NaiveDate;

        let amount = self.amount.parse::<f64>()
            .map_err(|_| "Invalid amount format")?;

        let start_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
            .map_err(|_| "Invalid start_date format")?;

        let end_date = if let Some(end_date_str) = self.end_date {
            Some(NaiveDate::parse_from_str(&end_date_str, "%Y-%m-%d")
                .map_err(|_| "Invalid end_date format")?)
        } else {
            None
        };

        let alert_thresholds = self.alert_thresholds.into_iter()
            .map(|t| {
                let percentage = t.percentage.parse::<f64>()
                    .map_err(|_| "Invalid threshold percentage format")?;
                Ok(crate::models::cost_budget::BudgetAlertThreshold {
                    percentage,
                    alert_type: t.alert_type,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let budget_type = match self.budget_type.as_str() {
            "overall" => crate::models::cost_budget::BudgetType::Overall,
            "service" => crate::models::cost_budget::BudgetType::Service,
            "category" => crate::models::cost_budget::BudgetType::Category,
            "tag_based" => crate::models::cost_budget::BudgetType::TagBased,
            _ => return Err("Invalid budget type".to_string()),
        };

        let budget_period = match self.budget_period.as_str() {
            "monthly" => crate::models::cost_budget::BudgetPeriod::Monthly,
            "quarterly" => crate::models::cost_budget::BudgetPeriod::Quarterly,
            "yearly" => crate::models::cost_budget::BudgetPeriod::Yearly,
            "custom" => crate::models::cost_budget::BudgetPeriod::Custom,
            _ => return Err("Invalid budget period".to_string()),
        };

        Ok(BudgetDto {
            account_id,
            name: self.name,
            description: self.description,
            budget_type,
            budget_period,
            amount,
            currency: self.currency,
            start_date,
            end_date,
            alert_thresholds,
            tags: serde_json::to_value(self.tags).unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        })
    }
}

pub async fn create_budget(
    service: web::Data<BudgetService>,
    account_id: web::Path<String>,
    req: web::Json<CreateBudgetRequest>,
) -> Result<HttpResponse> {
    let dto = req.into_inner().into_budget_dto(account_id.into_inner())
        .map_err(|e| actix_web::error::ErrorBadRequest(e))?;

    let budget = service.create_budget(dto).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let response = BudgetResponse::from(budget);
    Ok(HttpResponse::Created().json(response))
}

pub async fn get_budget(
    service: web::Data<BudgetService>,
    path: web::Path<(String, Uuid)>,
) -> Result<HttpResponse> {
    let (_account_id, budget_id) = path.into_inner();

    let budget = service.get_budget(budget_id).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Budget not found"))?;

    let response = BudgetResponse::from(budget);
    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_budgets(
    service: web::Data<BudgetService>,
    account_id: web::Path<String>,
) -> Result<HttpResponse> {
    let budgets = service.get_budgets_by_account(&account_id).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let responses: Vec<BudgetResponse> = budgets.into_iter()
        .map(BudgetResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(responses))
}

pub async fn update_budget(
    service: web::Data<BudgetService>,
    path: web::Path<(String, Uuid)>,
    req: web::Json<CreateBudgetRequest>,
) -> Result<HttpResponse> {
    let (_account_id, budget_id) = path.into_inner();

    let dto = req.into_inner().into_budget_dto(_account_id.clone())
        .map_err(|e| actix_web::error::ErrorBadRequest(e))?;

    let budget = service.update_budget(budget_id, dto).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let response = BudgetResponse::from(budget);
    Ok(HttpResponse::Ok().json(response))
}

pub async fn delete_budget(
    service: web::Data<BudgetService>,
    path: web::Path<(String, Uuid)>,
) -> Result<HttpResponse> {
    let (_account_id, budget_id) = path.into_inner();

    service.delete_budget(budget_id).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    Ok(HttpResponse::NoContent().finish())
}

pub async fn get_budget_status(
    service: web::Data<BudgetService>,
    path: web::Path<(String, Uuid)>,
) -> Result<HttpResponse> {
    let (_account_id, budget_id) = path.into_inner();

    let status = service.get_budget_status(budget_id).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let response = BudgetStatusResponse::from(status);
    Ok(HttpResponse::Ok().json(response))
}

pub async fn get_budget_alerts(
    service: web::Data<BudgetService>,
    account_id: web::Path<String>,
) -> Result<HttpResponse> {
    let alerts = service.get_budget_alerts(&account_id).await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    let responses: Vec<BudgetAlertResponse> = alerts.into_iter()
        .map(BudgetAlertResponse::from)
        .collect();

    Ok(HttpResponse::Ok().json(responses))
}