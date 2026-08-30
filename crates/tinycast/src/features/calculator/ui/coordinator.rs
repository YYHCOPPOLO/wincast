use tinycast_pure::calc::{CalcResult, CalculatorHistoryStore};

use super::card;
use crate::features::launcher::ui::coordinator::copy_text;

pub fn copy_calculator_result(history: &mut CalculatorHistoryStore, result: &CalcResult) -> bool {
    if !card::is_actionable(result) {
        return false;
    }
    history.record(result.expression.clone(), result.display.clone());
    copy_text(&result.copy_text).is_ok()
}

pub fn copy_history_result(result: &str) -> bool {
    let plain = result.replace(',', "");
    copy_text(&plain).is_ok()
}
