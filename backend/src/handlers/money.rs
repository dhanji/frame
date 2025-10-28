use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::middleware::auth::AuthenticatedUser;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub account_name: String,
    pub account_type: String,
    pub balance: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddTransactionRequest {
    pub account_id: String,
    pub description: String,
    pub amount: f64,
    pub transaction_type: String,
    pub category: Option<String>,
    pub transaction_date: String,
}

pub async fn list_accounts(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> HttpResponse {
    let user_id = user.user_id;

    let result = sqlx::query_as::<_, (String, String, String, f64, String, Option<String>)>(
        r#"
        SELECT id, account_name, account_type, balance, currency, last_sync
        FROM money_accounts
        WHERE user_id = ?
        ORDER BY created_at DESC
        "#
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await;

    match result {
        Ok(rows) => {
            let accounts: Vec<serde_json::Value> = rows.iter().map(|(id, name, acc_type, balance, currency, last_sync)| {
                serde_json::json!({
                    "id": id,
                    "account_name": name,
                    "account_type": acc_type,
                    "balance": balance,
                    "currency": currency,
                    "last_sync": last_sync,
                })
            }).collect();

            let total_balance: f64 = rows.iter().map(|(_, _, _, balance, _, _)| balance).sum();

            HttpResponse::Ok().json(serde_json::json!({
                "accounts": accounts,
                "total_balance": total_balance,
            }))
        }
        Err(e) => {
            log::error!("Failed to list accounts: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to list accounts"}))
        }
    }
}

pub async fn create_account(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    body: web::Json<CreateAccountRequest>,
) -> HttpResponse {
    let user_id = user.user_id;
    let account_id = uuid::Uuid::new_v4().to_string();
    let balance = body.balance.unwrap_or(0.0);

    let result = sqlx::query(
        r#"
        INSERT INTO money_accounts (id, user_id, account_type, account_name, balance, currency, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 'USD', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#
    )
    .bind(&account_id)
    .bind(user_id)
    .bind(&body.account_type)
    .bind(&body.account_name)
    .bind(balance)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({
                "id": account_id,
                "account_name": body.account_name,
            }))
        }
        Err(e) => {
            log::error!("Failed to create account: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to create account"}))
        }
    }
}

pub async fn list_transactions(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let user_id = user.user_id;
    let account_id = query.get("account_id");

    let mut sql = r#"
        SELECT t.id, t.account_id, t.transaction_date, t.description, t.amount, t.category, t.transaction_type
        FROM money_transactions t
        JOIN money_accounts a ON t.account_id = a.id
        WHERE a.user_id = ?
    "#.to_string();

    if account_id.is_some() {
        sql.push_str(" AND t.account_id = ?");
    }
    sql.push_str(" ORDER BY t.transaction_date DESC LIMIT 100");

    let mut query_builder = sqlx::query_as::<_, (String, String, String, String, f64, Option<String>, String)>(&sql)
        .bind(user_id);

    if let Some(acc_id) = account_id {
        query_builder = query_builder.bind(acc_id);
    }

    let result = query_builder.fetch_all(pool.get_ref()).await;

    match result {
        Ok(rows) => {
            let transactions: Vec<serde_json::Value> = rows.iter().map(|(id, acc_id, date, desc, amount, cat, tx_type)| {
                serde_json::json!({
                    "id": id,
                    "account_id": acc_id,
                    "transaction_date": date,
                    "description": desc,
                    "amount": amount,
                    "category": cat,
                    "transaction_type": tx_type,
                })
            }).collect();

            HttpResponse::Ok().json(transactions)
        }
        Err(e) => {
            log::error!("Failed to list transactions: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to list transactions"}))
        }
    }
}

pub async fn add_transaction(
    pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
    body: web::Json<AddTransactionRequest>,
) -> HttpResponse {
    let user_id = user.user_id;

    // Verify account ownership
    let owner_check = sqlx::query_scalar::<_, i64>(
        "SELECT user_id FROM money_accounts WHERE id = ?"
    )
    .bind(&body.account_id)
    .fetch_optional(pool.get_ref())
    .await;

    match owner_check {
        Ok(Some(owner_id)) if owner_id == user_id => {
            let transaction_id = uuid::Uuid::new_v4().to_string();

            let result = sqlx::query(
                r#"
                INSERT INTO money_transactions (id, account_id, transaction_date, description, amount, category, transaction_type, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                "#
            )
            .bind(&transaction_id)
            .bind(&body.account_id)
            .bind(&body.transaction_date)
            .bind(&body.description)
            .bind(body.amount)
            .bind(&body.category)
            .bind(&body.transaction_type)
            .execute(pool.get_ref())
            .await;

            match result {
                Ok(_) => {
                    // Update account balance
                    let balance_change = match body.transaction_type.as_str() {
                        "income" => body.amount,
                        "expense" => -body.amount,
                        _ => 0.0,
                    };

                    let _ = sqlx::query(
                        "UPDATE money_accounts SET balance = balance + ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
                    )
                    .bind(balance_change)
                    .bind(&body.account_id)
                    .execute(pool.get_ref())
                    .await;

                    HttpResponse::Ok().json(serde_json::json!({
                        "id": transaction_id,
                        "success": true,
                    }))
                }
                Err(e) => {
                    log::error!("Failed to add transaction: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({"error": "Failed to add transaction"}))
                }
            }
        }
        Ok(Some(_)) => HttpResponse::Forbidden().json(serde_json::json!({"error": "Access denied"})),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({"error": "Account not found"})),
        Err(e) => {
            log::error!("Failed to verify account ownership: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn sync_accounts(
    _pool: web::Data<SqlitePool>,
    user: AuthenticatedUser,
) -> HttpResponse {
    let _user_id = user.user_id;

    // TODO: Implement actual account sync with external services
    // For now, just return success
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Account sync not yet implemented"
    }))
}
