use std::{collections::HashMap, path::Path};

use aionui_db::models::AgentUsageCost;
use serde::{Deserialize, Serialize};

const PRICING_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum ModelPricingError {
    #[error("model pricing JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("model pricing file could not be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("model pricing version {0} is unsupported")]
    UnsupportedVersion(u32),
    #[error("model pricing for {model} has an invalid {field}")]
    InvalidRate { model: String, field: &'static str },
}

#[derive(Debug, Clone, Deserialize)]
struct PricingDocument {
    version: u32,
    models: HashMap<String, ModelPricing>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_usd_per_million: f64,
    pub output_usd_per_million: f64,
    /// Codex reports input_tokens including cached read/write tokens; Claude's
    /// input_tokens excludes them. Set this true for inclusive backends so the
    /// cache buckets are not charged twice.
    #[serde(default)]
    pub input_includes_cached_tokens: Option<bool>,
    #[serde(default)]
    pub cached_read_usd_per_million: Option<f64>,
    #[serde(default)]
    pub cached_write_usd_per_million: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct ModelPricingCatalog {
    models: HashMap<String, ModelPricing>,
    source: Option<String>,
    updated_at: Option<String>,
}

impl ModelPricingCatalog {
    pub fn from_json(input: &str) -> Result<Self, ModelPricingError> {
        let document: PricingDocument = serde_json::from_str(input)?;
        if document.version != PRICING_VERSION {
            return Err(ModelPricingError::UnsupportedVersion(document.version));
        }
        for (model, pricing) in &document.models {
            for (field, value) in [
                ("input_usd_per_million", pricing.input_usd_per_million),
                ("output_usd_per_million", pricing.output_usd_per_million),
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(ModelPricingError::InvalidRate {
                        model: model.clone(),
                        field,
                    });
                }
            }
            for (field, value) in [
                ("cached_read_usd_per_million", pricing.cached_read_usd_per_million),
                ("cached_write_usd_per_million", pricing.cached_write_usd_per_million),
            ] {
                if value.is_some_and(|rate| !rate.is_finite() || rate < 0.0) {
                    return Err(ModelPricingError::InvalidRate {
                        model: model.clone(),
                        field,
                    });
                }
            }
        }
        Ok(Self {
            models: document.models,
            source: document.source,
            updated_at: document.updated_at,
        })
    }

    /// Load optional user-maintained pricing. Missing configuration means an
    /// empty catalog, so callers can distinguish unknown cost from zero cost.
    pub fn load_from_data_dir(data_dir: &Path) -> Result<Self, ModelPricingError> {
        let path = std::env::var_os("AIONUI_MODEL_PRICING_FILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| data_dir.join("model-pricing.json"));
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::from_json(&contents),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Estimate a turn only when the catalog has an exact model and all token
    /// categories have configured rates. Cache-aware pricing is opt-in; if a
    /// backend reports cache tokens without cache rates, cost remains unknown.
    pub fn estimate(
        &self,
        model: Option<&str>,
        input_tokens: u64,
        output_tokens: u64,
        cached_read_tokens: u64,
        cached_write_tokens: u64,
    ) -> Option<f64> {
        self.quote(
            model,
            input_tokens,
            output_tokens,
            cached_read_tokens,
            cached_write_tokens,
        )
        .ok()
        .map(|quote| quote.cost_usd)
    }

    /// Return a price snapshot or a stable, exportable reason for unknown cost.
    pub fn quote(
        &self,
        model: Option<&str>,
        input_tokens: u64,
        output_tokens: u64,
        cached_read_tokens: u64,
        cached_write_tokens: u64,
    ) -> Result<AgentUsageCost, &'static str> {
        let model = model
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("model_missing")?;
        let pricing = self.models.get(model).ok_or("model_price_missing")?;
        if cached_read_tokens > 0 && pricing.cached_read_usd_per_million.is_none() {
            return Err("cache_read_price_missing");
        }
        if cached_write_tokens > 0 && pricing.cached_write_usd_per_million.is_none() {
            return Err("cache_write_price_missing");
        }
        if (cached_read_tokens > 0 || cached_write_tokens > 0) && pricing.input_includes_cached_tokens.is_none() {
            return Err("input_cache_semantics_missing");
        }
        let billable_input_tokens = if pricing.input_includes_cached_tokens == Some(true) {
            input_tokens
                .checked_sub(
                    cached_read_tokens
                        .checked_add(cached_write_tokens)
                        .ok_or("cache_token_count_invalid")?,
                )
                .ok_or("cache_token_count_invalid")?
        } else {
            input_tokens
        };
        let total = (billable_input_tokens as f64 * pricing.input_usd_per_million
            + output_tokens as f64 * pricing.output_usd_per_million
            + cached_read_tokens as f64 * pricing.cached_read_usd_per_million.unwrap_or(0.0)
            + cached_write_tokens as f64 * pricing.cached_write_usd_per_million.unwrap_or(0.0))
            / 1_000_000.0;
        if !total.is_finite() {
            return Err("cost_out_of_range");
        }
        let pricing_snapshot = serde_json::to_string(&serde_json::json!({
            "version": PRICING_VERSION, "model": model, "currency": "USD",
            "source": self.source.as_deref().unwrap_or("user_configuration"),
            "updated_at": self.updated_at, "rates": pricing,
            "estimated_at_ms": aionui_common::now_ms(),
        }))
        .map_err(|_| "pricing_snapshot_invalid")?;
        let cost_usd = (total * 1_000_000.0).round() / 1_000_000.0;
        if !cost_usd.is_finite() {
            return Err("cost_out_of_range");
        }
        Ok(AgentUsageCost {
            cost_usd,
            source: "configured_estimate".to_owned(),
            pricing_snapshot,
        })
    }
}
