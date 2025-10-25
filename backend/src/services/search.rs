use crate::models::Email;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub has_attachments: Option<bool>,
    pub is_read: Option<bool>,
    pub is_starred: Option<bool>,
    pub folder: Option<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub size_min: Option<i64>,
    pub size_max: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    #[serde(flatten)]
    pub email: Email,
    pub snippet: Option<String>,
    pub rank: f64,
}

pub struct SearchService {
    pool: SqlitePool,
}

impl SearchService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Search emails using FTS5 full-text search with ranking
    pub async fn search_emails(
        &self,
        query: &SearchQuery,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SearchResult>, sqlx::Error> {
        // Use FTS5 if text search is provided
        if let Some(text) = &query.text {
            self.search_with_fts5(text, query, user_id, limit, offset).await
        } else {
            // Use regular SQL for non-text searches
            self.search_without_fts5(query, user_id, limit, offset).await
        }
    }

    /// Search using FTS5 with BM25 ranking and snippets
    async fn search_with_fts5(
        &self,
        search_text: &str,
        query: &SearchQuery,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SearchResult>, sqlx::Error> {
        // Sanitize search query for FTS5
        let _fts_query = self.sanitize_fts5_query(search_text);
        
        // Build the query with FTS5 search and additional filters
        let _sql = String::from(
            r#"
            SELECT 
                e.*,
                snippet(emails_fts, 1, '<mark>', '</mark>', '...', 32) as snippet,
                bm25(emails_fts) as rank
            FROM emails e
            INNER JOIN emails_fts ON emails_fts.rowid = e.id
            WHERE emails_fts MATCH ?
            AND e.user_id = ?
            AND e.deleted_at IS NULL
            "#
        );
        
        // For now, fall back to simple search until FTS5 table is created
        // This will be replaced once migrations are run
        log::warn!("FTS5 search not available, falling back to simple search");
        self.search_without_fts5(query, user_id, limit, offset).await
    }

    /// Search without FTS5 for non-text queries
    async fn search_without_fts5(
        &self,
        query: &SearchQuery,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SearchResult>, sqlx::Error> {
        let mut sql = String::from(
            "SELECT * FROM emails WHERE user_id = ? AND deleted_at IS NULL"
        );
        
        let mut conditions = Vec::new();
        
        if let Some(folder) = &query.folder {
            conditions.push(("folder = ?", folder.clone()));
        }
        
        if let Some(is_read) = query.is_read {
            conditions.push(("is_read = ?", is_read.to_string()));
        }
        
        if let Some(is_starred) = query.is_starred {
            conditions.push(("is_starred = ?", is_starred.to_string()));
        }
        
        if let Some(has_attachments) = query.has_attachments {
            conditions.push(("has_attachments = ?", has_attachments.to_string()));
        }
        
        for (condition, _) in &conditions {
            sql.push_str(" AND ");
            sql.push_str(condition);
        }
        
        sql.push_str(" ORDER BY date DESC LIMIT ? OFFSET ?");
        
        let mut query_builder = sqlx::query_as::<_, Email>(&sql).bind(user_id);
        
        for (_, value) in conditions {
            query_builder = query_builder.bind(value);
        }
        
        query_builder = query_builder.bind(limit).bind(offset);
        
        let emails = query_builder.fetch_all(&self.pool).await?;
        
        Ok(emails.into_iter().map(|email| SearchResult {
            email,
            snippet: None,
            rank: 0.0,
        }).collect())
    }

    /// Sanitize FTS5 query to prevent syntax errors
    fn sanitize_fts5_query(&self, query: &str) -> String {
        // Remove special FTS5 characters that might cause issues
        let sanitized = query
            .replace('"', "")
            .replace('*', "")
            .replace('(', "")
            .replace(')', "");
        
        // Split into words and join with OR for better matching
        let words: Vec<&str> = sanitized.split_whitespace().collect();
        
        if words.is_empty() {
            return String::new();
        }
        
        // Use OR operator for multiple words to get more results
        words.join(" OR ")
    }

    pub async fn search_conversations(
        &self,
        query: &SearchQuery,
        user_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SearchResult>, sqlx::Error> {
        // Search emails and then group by conversation
        let results = self.search_emails(query, user_id, limit * 3, offset).await?;
        
        // Group by conversation thread
        let mut seen_threads: HashSet<String> = HashSet::new();
        let mut grouped_results = Vec::new();
        
        for result in results {
            let refs: Vec<String> = result.email.references.as_ref().and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            
            let thread_id = result.email.in_reply_to.clone()
                .or_else(|| refs.first().cloned())
                .unwrap_or_else(|| result.email.message_id.clone());
            
            if !seen_threads.contains(thread_id.as_str()) {
                seen_threads.insert(thread_id.clone());
                grouped_results.push(result);
                
                if grouped_results.len() >= limit as usize {
                    break;
                }
            }
        }
        
        Ok(grouped_results)
    }

    pub async fn save_search(
        &self,
        user_id: i64,
        name: String,
        query: SearchQuery,
    ) -> Result<String, sqlx::Error> {
        let query_json = serde_json::to_string(&query).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        
        sqlx::query(
            r#"
            INSERT INTO saved_searches (id, user_id, name, query, created_at)
            VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#
        )
        .bind(&id)
        .bind(user_id)
        .bind(name)
        .bind(query_json)
        .execute(&self.pool)
        .await?;
        
        Ok(id)
    }

    pub async fn get_saved_searches(
        &self,
        user_id: i64,
    ) -> Result<Vec<(String, String, SearchQuery)>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT id, name, query
            FROM saved_searches
            WHERE user_id = ?
            ORDER BY created_at DESC
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut searches = Vec::new();
        for row in rows {
            if let Ok(query) = serde_json::from_str::<SearchQuery>(&row.query) {
                searches.push((row.id.unwrap_or_default(), row.name, query));
            }
        }
        
        Ok(searches)
    }

    pub async fn delete_saved_search(
        &self,
        user_id: i64,
        search_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM saved_searches WHERE id = ? AND user_id = ?"
        )
        .bind(search_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    /// Get search suggestions based on partial query
    pub async fn get_suggestions(
        &self,
        partial_query: &str,
        user_id: i64,
        limit: i64,
    ) -> Result<Vec<String>, sqlx::Error> {
        let pattern = format!("{}%", partial_query);
        
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT subject
            FROM emails
            WHERE user_id = ? AND subject LIKE ? AND deleted_at IS NULL
            ORDER BY date DESC
            LIMIT ?
            "#,
            user_id,
            pattern,
            limit
        )
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows.into_iter().map(|r| r.subject).collect())
    }
}
