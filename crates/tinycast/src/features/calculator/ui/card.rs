//! The calculator answer card's data. The 96 DIP card, its two columns and
//! the error line are drawn by ui/surfaces/palette.slint; this module only
//! turns a result into the row the palette model carries.
//! Selection index 0 when a result is present.

use tinycast_pure::calc::CalcResult;

use crate::features::launcher::ui::list::PaintItem;

pub fn paint_item(result: &CalcResult, selected: bool) -> PaintItem {
    PaintItem::Calc {
        expression: result.expression.clone(),
        display: result.display.clone(),
        source_badge: result.source_badge.clone(),
        target_badge: result.target_badge.clone(),
        selected,
        is_error: is_error(result),
    }
}

pub fn is_error(result: &CalcResult) -> bool {
    result.source_badge.is_none() && result.target_badge.is_none()
}

pub fn is_actionable(result: &CalcResult) -> bool {
    !is_error(result)
}
