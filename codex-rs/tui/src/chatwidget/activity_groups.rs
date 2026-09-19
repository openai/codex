//! Keep hidden reasoning and terminal bookkeeping inside compatible activity groups.
//! Detailed output retains the original order while compact activity avoids artificial breaks.

use super::*;

impl ChatWidget {
    pub(super) fn absorb_activity_detail(
        &mut self,
        cell: Box<dyn HistoryCell>,
    ) -> Result<(), Box<dyn HistoryCell>> {
        let hidden_reasoning = cell
            .as_any()
            .downcast_ref::<history_cell::ReasoningSummaryCell>()
            .is_some_and(history_cell::ReasoningSummaryCell::is_transcript_only);
        if !hidden_reasoning
            && !cell
                .as_any()
                .is::<history_cell::UnifiedExecInteractionCell>()
        {
            return Err(cell);
        }
        let Some(active) = self.transcript.active_cell.as_mut() else {
            return Err(cell);
        };
        if let Some(computer) = active
            .as_any_mut()
            .downcast_mut::<history_cell::ComputerActivityCell>()
        {
            computer.group.push_detail(Arc::from(cell));
            self.bump_active_cell_revision();
            return Ok(());
        }
        if let Some(exploration) = active.as_any_mut().downcast_mut::<ExecCell>()
            && exploration.is_exploring_cell()
        {
            exploration.group.push_detail(Arc::from(cell));
            self.bump_active_cell_revision();
            return Ok(());
        }
        Err(cell)
    }
}
