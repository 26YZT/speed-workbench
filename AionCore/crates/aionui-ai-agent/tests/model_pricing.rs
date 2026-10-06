use aionui_ai_agent::ModelPricingCatalog;

#[test]
fn configured_rates_estimate_cost_for_uncosted_usage() {
    let catalog = ModelPricingCatalog::from_json(
        r#"{
            "version": 1,
            "models": {
                "test-model": {
                    "input_usd_per_million": 2.0,
                    "output_usd_per_million": 4.0
                }
            }
        }"#,
    )
    .unwrap();

    assert_eq!(catalog.estimate(Some("test-model"), 1_000, 500, 0, 0), Some(0.004));
}

#[test]
fn unknown_model_or_cached_usage_stays_unknown() {
    let catalog = ModelPricingCatalog::from_json(
        r#"{"version":1,"models":{"test-model":{"input_usd_per_million":2.0,"output_usd_per_million":4.0}}}"#,
    )
    .unwrap();

    assert_eq!(catalog.estimate(Some("missing"), 1, 1, 0, 0), None);
    assert_eq!(catalog.estimate(Some("test-model"), 1, 1, 1, 0), None);
}

#[test]
fn invalid_rates_are_rejected() {
    let error = ModelPricingCatalog::from_json(
        r#"{"version":1,"models":{"test-model":{"input_usd_per_million":-1.0,"output_usd_per_million":4.0}}}"#,
    )
    .expect_err("negative pricing must be rejected");
    assert!(error.to_string().contains("input_usd_per_million"));
}

#[test]
fn cache_inclusive_input_rates_do_not_double_charge_cached_tokens() {
    let catalog = ModelPricingCatalog::from_json(
        r#"{
            "version": 1,
            "models": {
                "cached-model": {
                    "input_usd_per_million": 2.0,
                    "output_usd_per_million": 4.0,
                    "cached_read_usd_per_million": 0.1,
                    "cached_write_usd_per_million": 0.2,
                    "input_includes_cached_tokens": true
                }
            }
        }"#,
    )
    .unwrap();

    assert_eq!(
        catalog.estimate(Some("cached-model"), 1_000, 500, 600, 100),
        Some(0.00268)
    );
}

#[test]
fn quote_preserves_exact_rates_and_identifies_missing_or_invalid_inputs() {
    let catalog = ModelPricingCatalog::from_json(r#"{"version":1,"source":"test account tariff","updated_at":"2026-10-06","models":{"m":{"input_usd_per_million":2,"output_usd_per_million":4,"cached_read_usd_per_million":0.1,"input_includes_cached_tokens":true}}}"#).unwrap();
    let quote = catalog.quote(Some("m"), 100, 50, 80, 0).unwrap();
    assert_eq!(quote.cost_usd, 0.000248);
    assert_eq!(quote.source, "configured_estimate");
    let snapshot: serde_json::Value = serde_json::from_str(&quote.pricing_snapshot).unwrap();
    assert_eq!(snapshot["source"], "test account tariff");
    assert_eq!(snapshot["rates"]["input_usd_per_million"].as_f64(), Some(2.0));
    assert_eq!(catalog.quote(None, 100, 50, 0, 0).unwrap_err(), "model_missing");
    assert_eq!(
        catalog.quote(Some("m"), 100, 50, 101, 0).unwrap_err(),
        "cache_token_count_invalid"
    );
    assert_eq!(
        catalog.quote(Some("m"), 100, 50, 0, 1).unwrap_err(),
        "cache_write_price_missing"
    );
}
