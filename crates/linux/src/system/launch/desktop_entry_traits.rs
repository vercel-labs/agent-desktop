/// Which kind of launcher a desktop entry describes and how it shows up.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct EntryTraits {
    pub(crate) application: bool,
    pub(crate) terminal: bool,
    pub(crate) hidden: bool,
    pub(crate) wm_class: Option<String>,
    pub(crate) no_display: bool,
    pub(crate) only_show_in: Option<String>,
    pub(crate) not_show_in: Option<String>,
}

impl EntryTraits {
    /// Whether the entry is an application the user sees in `desktops`, the
    /// `XDG_CURRENT_DESKTOP` list: shown, and allowed by `OnlyShowIn` and
    /// `NotShowIn`.
    pub(crate) fn shown_in(&self, desktops: &[&str]) -> bool {
        let lists = |list: &Option<String>| {
            list.as_deref().map(|list| {
                list.split(';').filter(|name| !name.is_empty()).any(|name| {
                    desktops
                        .iter()
                        .any(|desktop| desktop.eq_ignore_ascii_case(name))
                })
            })
        };
        self.application
            && !self.hidden
            && !self.no_display
            && lists(&self.only_show_in).unwrap_or(true)
            && !lists(&self.not_show_in).unwrap_or(false)
    }
}
