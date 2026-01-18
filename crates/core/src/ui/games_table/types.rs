//! Type definitions for games table

#[derive(Clone, Copy, PartialEq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    LastPlayed,
    Playtime,
    AchievementsTotal,
    AchievementsPercent,
    TimeToBeat,
    Votes,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggle(&self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }
}

/// Tri-state filter: All, Only With, Only Without
#[derive(Clone, Copy, PartialEq, Default)]
pub enum TriFilter {
    #[default]
    All,
    With,
    Without,
}

impl TriFilter {
    pub fn cycle(&self) -> Self {
        match self {
            TriFilter::All => TriFilter::With,
            TriFilter::With => TriFilter::Without,
            TriFilter::Without => TriFilter::All,
        }
    }
    
    pub fn label(&self, with_text: &str, without_text: &str) -> String {
        match self {
            TriFilter::All => "All".to_string(),
            TriFilter::With => with_text.to_string(),
            TriFilter::Without => without_text.to_string(),
        }
    }
}
