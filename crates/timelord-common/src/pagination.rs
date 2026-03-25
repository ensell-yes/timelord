use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    pub page_token: Option<String>,
}

fn default_page_size() -> i64 {
    50
}

impl PaginationParams {
    pub fn limit(&self) -> i64 {
        self.page_size.clamp(1, 200)
    }

    pub fn offset(&self) -> i64 {
        self.page_token
            .as_ref()
            .and_then(|t| t.parse::<i64>().ok())
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub next_page_token: Option<String>,
    pub total_count: Option<i64>,
}

impl<T> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, offset: i64, limit: i64) -> Self {
        let next_page_token = if items.len() as i64 == limit {
            Some((offset + limit).to_string())
        } else {
            None
        };
        Self {
            items,
            next_page_token,
            total_count: None,
        }
    }
}
