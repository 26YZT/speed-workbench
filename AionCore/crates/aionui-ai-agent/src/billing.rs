//! Billing counters are separate from the last-request context indicator.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Receipt-bound attribution stays immutable across a later Send or picker change.
#[derive(Debug, Clone)]
pub(crate) struct TurnAttribution {
    pub task_id: Option<String>,
    pub turn_id: Option<String>,
    pub usage_attempt_id: Option<String>,
    pub model: Option<String>,
    model_revision: u64,
}

pub(crate) struct UsageAttributions {
    model: Option<String>,
    requested_model: Option<String>,
    revision: u64,
    turns: BTreeMap<u64, TurnAttribution>,
}

impl UsageAttributions {
    pub(crate) fn new(model: Option<String>) -> Self {
        Self {
            model,
            requested_model: None,
            revision: 0,
            turns: BTreeMap::new(),
        }
    }

    pub(crate) fn freeze(
        &self,
        task_id: Option<String>,
        turn_id: Option<String>,
        usage_attempt_id: Option<String>,
    ) -> TurnAttribution {
        TurnAttribution {
            task_id,
            turn_id,
            usage_attempt_id,
            model: self.model.clone(),
            model_revision: self.revision,
        }
    }

    pub(crate) fn bind(&mut self, epoch: u64, attribution: TurnAttribution) {
        // NoTurn inputs must never reassign the already-running wire turn.
        self.turns.entry(epoch).or_insert(attribution);
        while self.turns.len() > 256 {
            self.turns.pop_first();
        }
    }

    pub(crate) fn get(&self, epoch: u64) -> Option<TurnAttribution> {
        self.turns.get(&epoch).cloned()
    }

    pub(crate) fn request_model(&mut self, model: String) {
        self.revision += 1;
        self.model = None;
        self.requested_model = Some(model);
    }

    pub(crate) fn confirm_selection(&mut self, model: String) {
        if self
            .requested_model
            .as_ref()
            .is_some_and(|requested| requested != &model)
        {
            return; // an older confirmation cannot validate a newer request
        }
        if self.requested_model.is_none() && self.model.as_ref() != Some(&model) {
            self.revision += 1;
        }
        self.requested_model = None;
        self.model = Some(model);
    }

    pub(crate) fn confirm_initial_model(&mut self, model: String) {
        if self.revision != 0 {
            return;
        }
        self.model = Some(model.clone());
    }

    pub(crate) fn confirm_turn_model(&mut self, epoch: u64, model: Option<String>) {
        let Some(turn) = self.turns.get_mut(&epoch) else { return };
        turn.model = model.clone();
        if turn.model_revision == self.revision
            && self
                .requested_model
                .as_ref()
                .is_none_or(|requested| Some(requested) == model.as_ref())
        {
            self.model = model;
            if self.model.is_some() {
                self.requested_model = None;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_read_tokens: u64,
    pub cached_write_tokens: u64,
}

impl TokenTotals {
    fn delta(self, base: Self) -> Option<Self> {
        Some(Self {
            input_tokens: self.input_tokens.checked_sub(base.input_tokens)?,
            output_tokens: self.output_tokens.checked_sub(base.output_tokens)?,
            cached_read_tokens: self.cached_read_tokens.checked_sub(base.cached_read_tokens)?,
            cached_write_tokens: self.cached_write_tokens.checked_sub(base.cached_write_tokens)?,
        })
    }
}

#[derive(Clone)]
struct AttemptBilling {
    epoch: u64,
    base: Option<TokenTotals>,
    last: TokenTotals,
}

/// An older receipt may repeat its last report, but it cannot move the global
/// boundary backwards. A changed old report makes successor attribution unsafe.
pub(crate) struct TurnBilling {
    high_water: Option<TokenTotals>,
    newest_epoch: u64,
    attempts: BTreeMap<String, AttemptBilling>,
    uncertain: bool,
    unsafe_from_epoch: u64,
}

impl TurnBilling {
    pub(crate) fn new(initial: Option<TokenTotals>) -> Self {
        Self {
            high_water: initial,
            newest_epoch: 0,
            attempts: BTreeMap::new(),
            uncertain: false,
            unsafe_from_epoch: u64::MAX,
        }
    }

    pub(crate) fn restore_uncertainty(&mut self, uncertain: bool) {
        if uncertain {
            self.mark_uncertain_at(0);
        }
    }

    pub(crate) fn mark_uncertain_at(&mut self, epoch: u64) {
        self.uncertain = true;
        self.unsafe_from_epoch = self.unsafe_from_epoch.min(epoch);
    }

    pub(crate) fn uncertain(&self) -> bool {
        self.uncertain
    }

    pub(crate) fn accepted_total(&self) -> Option<TokenTotals> {
        self.high_water
    }

    pub(crate) fn attempt_ids(&self) -> Vec<String> {
        self.attempts
            .iter()
            .filter(|(_, attempt)| attempt.epoch >= self.unsafe_from_epoch)
            .map(|(id, _)| id.clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn observe(&mut self, turn: &str, total: TokenTotals) -> Result<TokenTotals, &'static str> {
        let epoch = self
            .attempts
            .get(turn)
            .map_or(self.newest_epoch + 1, |attempt| attempt.epoch);
        self.observe_epoch(turn, epoch, total)
    }

    pub(crate) fn observe_epoch(
        &mut self,
        turn: &str,
        epoch: u64,
        total: TokenTotals,
    ) -> Result<TokenTotals, &'static str> {
        if self.uncertain {
            return Err("billing_attribution_uncertain");
        }
        if let Some(attempt) = self.attempts.get(turn) {
            if attempt.epoch != epoch
                || (attempt.last != total && epoch < self.newest_epoch)
                || total.delta(attempt.last).is_none()
            {
                self.mark_uncertain_at(epoch);
                return Err("billing_attribution_uncertain");
            }
            if attempt.last == total {
                return attempt
                    .base
                    .and_then(|base| total.delta(base))
                    .ok_or("billing_baseline_missing_or_reset");
            }
        } else {
            if epoch != self.newest_epoch + 1 {
                self.mark_uncertain_at(epoch);
                return Err("billing_attribution_uncertain");
            }
            self.attempts.insert(
                turn.to_owned(),
                AttemptBilling {
                    epoch,
                    base: self.high_water,
                    last: total,
                },
            );
            self.newest_epoch = epoch;
            while self.attempts.len() > 256 {
                if let Some(oldest) = self
                    .attempts
                    .iter()
                    .min_by_key(|(_, attempt)| attempt.epoch)
                    .map(|(id, _)| id.clone())
                {
                    self.attempts.remove(&oldest);
                }
            }
        }
        if self.high_water.is_some_and(|last| total.delta(last).is_none()) {
            self.mark_uncertain_at(epoch);
            return Err("billing_attribution_uncertain");
        }
        self.high_water = Some(total);
        let attempt = self.attempts.get_mut(turn).expect("attempt just registered");
        attempt.last = total;
        attempt
            .base
            .and_then(|base| total.delta(base))
            .ok_or("billing_baseline_missing_or_reset")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn total(input: u64, output: u64, cache: u64) -> TokenTotals {
        TokenTotals {
            input_tokens: input,
            output_tokens: output,
            cached_read_tokens: cache,
            cached_write_tokens: 0,
        }
    }
    #[test]
    fn multiple_model_requests_replace_the_whole_turn_delta_and_next_turn_starts_at_previous_total() {
        let mut tracker = TurnBilling::new(Some(TokenTotals::default()));
        assert_eq!(tracker.observe("one", total(100, 10, 80)), Ok(total(100, 10, 80)));
        assert_eq!(tracker.observe("one", total(250, 25, 200)), Ok(total(250, 25, 200)));
        assert_eq!(tracker.observe("two", total(400, 40, 310)), Ok(total(150, 15, 110)));
        assert_eq!(tracker.observe("two", total(400, 40, 310)), Ok(total(150, 15, 110)));
    }
    #[test]
    fn persisted_baseline_excludes_historical_usage_after_restart() {
        let mut tracker = TurnBilling::new(Some(total(250, 25, 200)));
        assert_eq!(tracker.observe("resumed", total(400, 40, 310)), Ok(total(150, 15, 110)));
    }
    #[test]
    fn missing_or_decreased_counters_never_price_historical_tokens() {
        let mut tracker = TurnBilling::new(None);
        assert!(tracker.observe("one", total(100, 10, 80)).is_err());
        assert!(tracker.observe("one", total(250, 25, 200)).is_err());
        assert_eq!(tracker.observe("two", total(400, 40, 310)), Ok(total(150, 15, 110)));
        assert!(tracker.observe("two", total(1, 1, 0)).is_err());
    }

    #[test]
    fn late_duplicate_keeps_attempt_delta_and_newest_boundary() {
        let mut tracker = TurnBilling::new(Some(TokenTotals::default()));
        assert_eq!(
            tracker.observe_epoch("a", 1, total(100, 10, 80)),
            Ok(total(100, 10, 80))
        );
        assert_eq!(
            tracker.observe_epoch("b", 2, total(200, 20, 160)),
            Ok(total(100, 10, 80))
        );
        assert_eq!(
            tracker.observe_epoch("a", 1, total(100, 10, 80)),
            Ok(total(100, 10, 80))
        );
        assert_eq!(tracker.accepted_total(), Some(total(200, 20, 160)));
        assert_eq!(
            tracker.observe_epoch("c", 3, total(300, 30, 240)),
            Ok(total(100, 10, 80))
        );
    }

    #[test]
    fn changed_old_report_and_current_reset_fail_closed_without_regressing_boundary() {
        for (id, epoch, report) in [("a", 1, total(150, 15, 120)), ("b", 2, total(1, 1, 0))] {
            let mut tracker = TurnBilling::new(Some(TokenTotals::default()));
            tracker.observe_epoch("a", 1, total(100, 10, 80)).unwrap();
            tracker.observe_epoch("b", 2, total(200, 20, 160)).unwrap();
            assert_eq!(
                tracker.observe_epoch(id, epoch, report),
                Err("billing_attribution_uncertain")
            );
            assert_eq!(tracker.accepted_total(), Some(total(200, 20, 160)));
            assert_eq!(
                tracker.observe_epoch("c", 3, total(300, 30, 240)),
                Err("billing_attribution_uncertain")
            );
            let mut restored = TurnBilling::new(tracker.accepted_total());
            restored.restore_uncertainty(tracker.uncertain());
            assert_eq!(
                restored.observe_epoch("resumed", 1, total(400, 40, 320)),
                Err("billing_attribution_uncertain")
            );
        }
    }

    #[test]
    fn a_missing_attempt_boundary_is_not_charged_to_its_successor() {
        let mut tracker = TurnBilling::new(Some(TokenTotals::default()));
        tracker.observe_epoch("a", 1, total(100, 10, 80)).unwrap();
        assert_eq!(
            tracker.observe_epoch("c", 3, total(300, 30, 240)),
            Err("billing_attribution_uncertain")
        );
        assert_eq!(tracker.accepted_total(), Some(total(100, 10, 80)));
    }

    #[test]
    fn model_requests_and_rejection_do_not_change_the_running_turn_identity() {
        let mut state = UsageAttributions::new(Some("A".into()));
        state.bind(1, state.freeze(Some("task-a".into()), Some("turn-a".into()), None));
        state.request_model("B".into());
        // A delayed older report, or a rejected switch with no matching
        // confirmation, cannot make the next B request a verified identity.
        state.confirm_selection("A".into());
        state.bind(2, state.freeze(Some("task-b".into()), Some("turn-b".into()), None));
        assert_eq!(state.get(1).unwrap().model.as_deref(), Some("A"));
        assert_eq!(state.get(2).unwrap().model, None);
        state.confirm_selection("B".into());
        state.bind(3, state.freeze(None, Some("turn-c".into()), None));
        assert_eq!(state.get(3).unwrap().model.as_deref(), Some("B"));
        assert_eq!(state.get(1).unwrap().model.as_deref(), Some("A"));
    }

    #[test]
    fn late_turn_model_report_does_not_override_a_newer_request() {
        let mut state = UsageAttributions::new(Some("A".into()));
        state.bind(1, state.freeze(Some("old-task".into()), Some("old-turn".into()), None));
        state.request_model("B".into());
        state.bind(2, state.freeze(Some("new-task".into()), Some("new-turn".into()), None));
        state.confirm_turn_model(1, Some("actual-A".into()));
        assert_eq!(state.get(1).unwrap().turn_id.as_deref(), Some("old-turn"));
        assert_eq!(state.get(1).unwrap().model.as_deref(), Some("actual-A"));
        assert_eq!(state.get(2).unwrap().model, None);
        assert_eq!(state.freeze(None, None, None).model, None);
        state.confirm_turn_model(2, Some("actual-B".into()));
        assert_eq!(state.get(2).unwrap().model.as_deref(), Some("actual-B"));
        assert_eq!(
            state.freeze(None, None, None).model,
            None,
            "a differing pending selection is still unverified"
        );
    }

    #[test]
    fn no_turn_receipt_preserves_original_owner() {
        let mut state = UsageAttributions::new(None);
        state.bind(
            1,
            state.freeze(Some("original".into()), Some("original-turn".into()), None),
        );
        state.bind(1, state.freeze(Some("injection".into()), Some("new-turn".into()), None));
        assert_eq!(state.get(1).unwrap().task_id.as_deref(), Some("original"));
        assert_eq!(state.get(1).unwrap().turn_id.as_deref(), Some("original-turn"));
    }
}
