use crate::AppInfo;

/// Presentation matching includes display-name substrings and exact aliases;
/// mutation lookup requires an exact identifier and optional bundle identity.
#[derive(Debug, Clone, Copy)]
pub enum AppInventoryQuery<'a> {
    All,
    NameOrAlias(&'a str),
    Exact {
        name: &'a str,
        bundle_id: Option<&'a str>,
    },
}

impl AppInventoryQuery<'_> {
    /// Matches the listed name; adapters add their verified exact aliases.
    pub fn matches_display_name(self, actual: &str) -> bool {
        match self {
            Self::All => true,
            Self::NameOrAlias(name) => crate::tree::search_text::contains(
                actual,
                &crate::tree::search_text::normalize(name),
            ),
            Self::Exact { name, .. } => crate::app_name_matches(actual, name),
        }
    }

    /// Whether `app` answers this query: any record for `All`, a display-name
    /// substring or an exact identifier for `NameOrAlias`, and an exact
    /// identifier plus the bundle identifier when one is given for `Exact`.
    pub fn admits(self, app: &AppInfo) -> bool {
        match self {
            Self::All => true,
            Self::NameOrAlias(name) => {
                self.matches_display_name(&app.name) || app.matches_identifier(name)
            }
            Self::Exact { name, bundle_id } => {
                app.matches_identifier(name)
                    && bundle_id.is_none_or(|expected| {
                        app.bundle_id
                            .as_deref()
                            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
                    })
            }
        }
    }

    /// The answer to this query from a complete list of running applications.
    pub fn answer(self, apps: Vec<AppInfo>) -> crate::AppInventory {
        crate::AppInventory {
            apps: self.select(apps),
            skipped: Vec::new(),
        }
    }

    /// The records of `apps` this query admits. For `Exact`, an application
    /// that carries the name itself outranks one that only has it as an
    /// alias; two that both only alias it stay, so the caller sees the
    /// ambiguity instead of a silent pick.
    pub fn select(self, mut apps: Vec<AppInfo>) -> Vec<AppInfo> {
        apps.retain(|app| self.admits(app));
        if let Self::Exact { name, .. } = self
            && apps.iter().any(|app| app.is_named(name))
        {
            apps.retain(|app| app.is_named(name));
        }
        apps
    }
}

#[cfg(test)]
#[path = "app_inventory_query_tests.rs"]
mod tests;
