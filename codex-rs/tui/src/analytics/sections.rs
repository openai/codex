//! Account-specific section order, request eligibility, and independent report ranges.
//! Resolving the account precedes report loading; refreshing drops all account-owned data.

use super::AnalyticsView;
use super::chats;
use super::data::Load;
use super::models::AccountKind;

/// Stable identities preserve selections while account layouts change their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Usage,
    Plugins,
    Credits,
    Chats,
    Activity,
    Skills,
    Plan,
    Summary,
}

impl Section {
    pub(super) fn report(self) -> Option<super::models::AccountAnalyticsReport> {
        use super::models::AccountAnalyticsReport as Report;
        match self {
            Self::Usage => Some(Report::Usage),
            Self::Plugins => Some(Report::Plugins),
            Self::Credits => Some(Report::Credits),
            Self::Chats | Self::Plan | Self::Summary => None,
            Self::Activity => Some(Report::Messages),
            Self::Skills => Some(Report::Skills),
        }
    }
}

/// Section-only indexing keeps report identity separate from array positions.
pub(super) struct SectionStates(pub(super) [SectionState; 8]);

impl std::ops::Index<Section> for SectionStates {
    type Output = SectionState;
    fn index(&self, section: Section) -> &Self::Output {
        &self.0[section as usize]
    }
}

impl std::ops::IndexMut<Section> for SectionStates {
    fn index_mut(&mut self, section: Section) -> &mut Self::Output {
        &mut self.0[section as usize]
    }
}

/// A report and its selection are replaced independently of the other sections.
pub(super) struct SectionState {
    pub(super) history: Load<super::models::AccountAnalyticsHistory>,
    pub(super) cursor: usize,
    pub(super) detail: Option<usize>,
    pub(super) group: usize,
}

impl Default for SectionState {
    fn default() -> Self {
        Self {
            history: Load::Unavailable,
            cursor: 6,
            detail: None,
            group: 0,
        }
    }
}

/// Usage, product activity, and tools each share one range across their related sections.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RangeGroup {
    Usage,
    Activity,
    Tools,
}

impl AnalyticsView {
    pub(super) fn poll_reports(&mut self) {
        for section in &mut self.sections.0 {
            section.history.poll();
        }
        self.chats.poll();
        self.tasks.poll();
        self.profile.poll();
        self.plan.poll();
        self.account.poll();
        self.start_reports();
        if !self.business()
            && !self.consumer_attribution()
            && ![0, 2].contains(&self.sections[Section::Usage].group)
        {
            self.sections[Section::Usage].group = 0;
        }
        if self
            .group_picker
            .is_some_and(|choice| choice >= self.group_options().len())
        {
            self.group_picker = None;
        }
        if let Some(chats) = self.tasks.ready() {
            self.sections[Section::Chats].cursor = self.sections[Section::Chats]
                .cursor
                .min(chats.rows.len().saturating_sub(/*rhs*/ 1));
        }
        if let Some(chats) = self.chats.ready() {
            self.sections[Section::Chats].cursor = self.sections[Section::Chats]
                .cursor
                .min(chats.rows.len().saturating_sub(/*rhs*/ 1));
        }
    }

    pub(super) fn range_group(&self, section: Section) -> RangeGroup {
        match section {
            Section::Usage if self.business() => RangeGroup::Activity,
            Section::Usage
            | Section::Credits
            | Section::Chats
            | Section::Plan
            | Section::Summary => RangeGroup::Usage,
            Section::Activity => RangeGroup::Activity,
            Section::Plugins | Section::Skills => RangeGroup::Tools,
        }
    }

    pub(super) fn group_label(&self, section: Section, group: usize) -> &'static str {
        if section == Section::Summary {
            return super::summary::VIEWS[group].label();
        }
        if group == 0
            && (section == Section::Credits
                || (section == Section::Usage && !self.consumer_attribution()))
        {
            "Product"
        } else {
            super::data::GROUP_LABELS[group]
        }
    }

    fn account_kind(&self) -> Option<AccountKind> {
        self.account
            .ready()
            .copied()
            .map(|plan| AccountKind::from(Some(plan)))
    }

    pub(super) fn business(&self) -> bool {
        matches!(
            self.account_kind(),
            Some(AccountKind::Business | AccountKind::Enterprise)
        )
    }

    pub(super) fn visible_sections(&self) -> &'static [Section] {
        match self.account_kind() {
            Some(AccountKind::Consumer) if self.plan.enabled => &[
                Section::Summary,
                Section::Usage,
                Section::Plan,
                Section::Activity,
                Section::Plugins,
                Section::Skills,
                Section::Chats,
            ],
            Some(AccountKind::Consumer) => &[
                Section::Summary,
                Section::Usage,
                Section::Activity,
                Section::Plugins,
                Section::Skills,
                Section::Chats,
            ],
            Some(AccountKind::Business | AccountKind::Enterprise)
                if super::models::thread_usage_supported(self.account.ready().copied()) =>
            {
                &[
                    Section::Summary,
                    Section::Credits,
                    Section::Usage,
                    Section::Plugins,
                    Section::Skills,
                    Section::Chats,
                ]
            }
            Some(AccountKind::Business | AccountKind::Enterprise) => &[
                Section::Summary,
                Section::Credits,
                Section::Usage,
                Section::Plugins,
                Section::Skills,
            ],
            Some(AccountKind::Unknown) => &[Section::Summary],
            None => &[],
        }
    }

    pub(super) fn start_reports(&mut self) {
        if self.reports_started || self.account.ready().is_none() {
            return;
        }
        self.reports_started = true;
        let visible = self.visible_sections();
        let Some(first) = visible.first() else {
            return;
        };
        if !visible.contains(&self.section) {
            self.section = *first;
        }
        let usage_groups: &[usize] = if self.business() {
            &[6, 2]
        } else {
            &[1, 2, 0, 3]
        };
        if !usage_groups.contains(&self.sections[Section::Usage].group) {
            self.sections[Section::Usage].group =
                if matches!(self.account_kind(), Some(AccountKind::Business)) {
                    2
                } else {
                    usage_groups[0]
                };
        }
        if visible.contains(&Section::Credits)
            && let Some(live) = &self.live
            && !live
                .credit_groups()
                .contains(&self.sections[Section::Credits].group)
        {
            self.sections[Section::Credits].group = live.credit_groups()[0];
        }
        self.load_summary();
        for section in visible {
            if section.report().is_some() {
                self.load_report(*section);
            }
        }
        if visible.contains(&Section::Plan)
            && let (Some((_, _, frame)), Some(live)) = (&self.connection, &self.live)
        {
            let live = std::sync::Arc::clone(live);
            self.plan.report = Load::start(async move { live.plan_history().await }, frame.clone());
        }
        if visible.contains(&Section::Chats) && !self.business() {
            if let (Some((_, handle, frame)), Some(live)) = (&self.connection, &self.live) {
                self.tasks = Load::start_with_timeout(
                    super::tasks::read(handle.clone(), std::sync::Arc::clone(live)),
                    frame.clone(),
                    std::time::Duration::from_secs(/*secs*/ 120),
                );
            }
        } else if visible.contains(&Section::Chats) {
            self.chats =
                if let (Some((_, handle, frame)), Some(live)) = (&self.connection, &self.live) {
                    Load::start_with_timeout(
                        chats::read(handle.clone(), std::sync::Arc::clone(live)),
                        frame.clone(),
                        std::time::Duration::from_secs(/*secs*/ 90),
                    )
                } else {
                    Load::Unavailable
                };
        }
    }

    pub(super) fn section_date_range(
        &self,
        section: Section,
    ) -> std::ops::RangeInclusive<chrono::NaiveDate> {
        let end = self.end_date;
        let range = self.ranges[self.range_group(section) as usize];
        (end - chrono::Days::new(if range == 0 { 6 } else { 29 }))..=end
    }

    pub(super) fn change_range(&mut self) {
        let index = self.range_group(self.section);
        self.ranges[index as usize] ^= 1;
        for section in self.visible_sections() {
            if matches!(section, Section::Chats | Section::Plan | Section::Summary)
                || self.range_group(*section) != index
            {
                continue;
            }
            self.sections[*section].cursor = if self.ranges[index as usize] == 1 {
                self.sections[*section].cursor + 23
            } else {
                self.sections[*section].cursor.saturating_sub(/*rhs*/ 23)
            };
            self.sections[*section].detail = self.sections[*section].detail.and_then(|day| {
                if self.ranges[index as usize] == 1 {
                    Some(day + 23)
                } else {
                    day.checked_sub(/*rhs*/ 23)
                }
            });
            if section.report().is_some() {
                self.load_report(*section);
            }
        }
    }
}
