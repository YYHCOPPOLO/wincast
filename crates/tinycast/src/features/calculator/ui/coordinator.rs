use tinycast_pure::calc::{history_copy_payload, CalcResult, CalculatorHistoryStore};

use super::card;
use crate::features::launcher::ui::coordinator::copy_text;

pub fn copy_calculator_result(history: &mut CalculatorHistoryStore, result: &CalcResult) -> bool {
    if !card::is_actionable(result) {
        return false;
    }
    history.record(result.expression.clone(), result.copy_text.clone());
    copy_text(&result.copy_text).is_ok()
}

pub fn copy_history_result(result: &str) -> bool {
    copy_text(&history_copy_payload(result)).is_ok()
}
