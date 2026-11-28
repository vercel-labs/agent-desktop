use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, EvidenceRequirements, LocatorEvidence, LocatorField,
};

pub(crate) fn collection_target(
    element: &crate::tree::AXElement,
    root: &crate::tree::AXElement,
    value: &str,
    deadline: Deadline,
) -> Result<Option<crate::tree::AXElement>, AdapterError> {
    let (read, stats) = candidate_read(element, false, deadline)?;
    let named = is_named_target(element, &read.evidence, value, stats, deadline)?;
    if !named && !text_value_matches(element, &read.attrs, value, deadline)? {
        return Ok(None);
    }
    collection_owner(
        element,
        root,
        named,
        |node| {
            Ok((
                crate::tree::surface_read::string(node, "AXRole", deadline)?,
                crate::tree::surface_read::element(node, "AXParent", deadline)?,
            ))
        },
        crate::tree::properties::capabilities::same_element,
    )
}

fn collection_owner<T: Clone>(
    element: &T,
    root: &T,
    named: bool,
    mut read: impl FnMut(&T) -> Result<(Option<String>, Option<T>), AdapterError>,
    same: impl Fn(&T, &T) -> bool,
) -> Result<Option<T>, AdapterError> {
    let mut selectable = None;
    let mut row = None;
    let mut current = element.clone();
    for _ in 0..8 {
        if same(&current, root) {
            return Ok(row.or(selectable));
        }
        let (role, parent) = read(&current)?;
        if row.is_none() && role.as_deref() == Some("AXRow") {
            row = Some(current.clone());
        }
        if role.as_deref().is_some_and(|role| {
            super::container_select::role_activates_by_selection(
                crate::tree::roles::ax_role_to_str(role),
            )
        }) {
            selectable.get_or_insert_with(|| current.clone());
        } else if named
            && selectable.is_none()
            && same(&current, element)
            && !matches!(
                role.as_deref(),
                Some("AXTextField" | "AXTextArea" | "AXSecureTextField")
            )
        {
            selectable = Some(current.clone());
        }
        let Some(parent) = parent else {
            return Err(incomplete("owner_parent_missing"));
        };
        current = parent;
    }
    Err(incomplete("owner_depth_limit"))
}

fn text_value_matches(
    element: &crate::tree::AXElement,
    attrs: &crate::tree::NodeAttrs,
    value: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if attrs.role.as_deref() != Some("AXTextField") {
        return Ok(false);
    }
    if attrs.subrole.as_deref() == Some("AXSecureTextField") {
        return Ok(false);
    }
    Ok(
        crate::tree::surface_read::string(element, "AXValue", deadline)?
            .is_some_and(|text| text.eq_ignore_ascii_case(value)),
    )
}

pub(crate) fn matches(
    element: &crate::tree::AXElement,
    value: &str,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let (read, stats) = candidate_read(element, false, deadline)?;
    is_named_target(element, &read.evidence, value, stats, deadline)
}

fn is_named_target(
    element: &crate::tree::AXElement,
    evidence: &LocatorEvidence,
    value: &str,
    stats: agent_desktop_core::LocatorStats,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let verdict = name_verdict(evidence, value);
    if verdict == NameVerdict::Differs {
        return Ok(false);
    }
    let (read, _) = candidate_read(element, true, deadline)?;
    decide(verdict, &read.evidence.ref_evidence.available_actions).map_err(|mut error| {
        attach_stats(&mut error, stats);
        error
    })
}

fn candidate_read(
    element: &crate::tree::AXElement,
    with_actions: bool,
    deadline: Deadline,
) -> Result<
    (
        crate::tree::query::node_read::NodeRead,
        agent_desktop_core::LocatorStats,
    ),
    AdapterError,
> {
    let mut usage = crate::tree::observation_usage::ObservationUsage::with_defaults();
    let mut stats = agent_desktop_core::LocatorStats::default();
    let mut requirements = EvidenceRequirements {
        role: true,
        name: true,
        description: true,
        ..Default::default()
    };
    requirements.ref_evidence.actions = with_actions;
    let read = crate::tree::query::node_read::read_node(
        element,
        crate::tree::query::node_read_context::NodeReadContext {
            tree: &crate::tree::TreeBuildContext::empty(false),
            stats: &mut stats,
            usage: &mut usage,
            requirements,
            deadline,
            child_plan: crate::tree::query::child_read_plan::ChildReadPlan::boundary_aware(
                0,
                crate::tree::child_labels::MAX_LABEL_ELEMENTS,
                1,
                0,
            ),
        },
    )?;
    if read.invalid_element {
        return Err(AdapterError::stale_ref_because(
            "Selection candidate became invalid",
        ));
    }
    Ok((read, stats))
}

/// A scan that ran out of time reports the scan, not the application: the
/// application answered every read, the list was larger than the budget.
pub(crate) fn name_scan_deadline(
    error: AdapterError,
    scanned: usize,
    deadline: Deadline,
) -> AdapterError {
    let evidence_cut = error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .is_some_and(|kind| kind == "selection_evidence");
    if error.code != ErrorCode::Timeout && !(evidence_cut && deadline.is_expired()) {
        return error;
    }
    AdapterError::timeout(format!(
        "The select scan read {scanned} accessibility nodes before its deadline"
    ))
    .with_details(serde_json::json!({
        "kind": "select_scan_deadline",
        "nodes_scanned": scanned,
        "complete": false,
    }))
    .with_suggestion("Retry with a larger --timeout-ms; a long list needs time to scan whole")
    .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
}

fn attach_stats(error: &mut AdapterError, stats: agent_desktop_core::LocatorStats) {
    if let Some(details) = error
        .details
        .as_mut()
        .and_then(serde_json::Value::as_object_mut)
    {
        details.insert("query_stats".into(), serde_json::json!(stats));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameVerdict {
    Match,
    Differs,
    Unknown(&'static str),
}

fn name_verdict(evidence: &LocatorEvidence, value: &str) -> NameVerdict {
    let fields = [
        (&evidence.name, "name_unknown"),
        (&evidence.description, "description_unknown"),
    ];
    if fields.iter().any(|(field, _)| {
        field
            .known()
            .is_some_and(|name| name.eq_ignore_ascii_case(value))
    }) {
        return NameVerdict::Match;
    }
    fields
        .iter()
        .find(|(field, _)| field.is_unknown())
        .map_or(NameVerdict::Differs, |(_, phase)| {
            NameVerdict::Unknown(phase)
        })
}

fn decide(verdict: NameVerdict, actions: &LocatorField<Vec<String>>) -> Result<bool, AdapterError> {
    let clickable = actions.known().map(|known| {
        known
            .iter()
            .any(|action| action == agent_desktop_core::actions::capability::CLICK)
    });
    match (verdict, clickable) {
        (NameVerdict::Differs, _) => Ok(false),
        (NameVerdict::Match, Some(clickable)) => Ok(clickable),
        (NameVerdict::Match, None) => Err(incomplete("actions_unknown")),
        (NameVerdict::Unknown(_), Some(false)) => Ok(false),
        (NameVerdict::Unknown(phase), _) => Err(incomplete(phase)),
    }
}

fn incomplete(phase: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Selection candidate evidence was incomplete",
    )
    .with_details(
        serde_json::json!({ "kind": "selection_evidence", "phase": phase, "complete": false }),
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{IdentifierEvidence, LocatorField, LocatorRefEvidence};

    fn owner(roles: &[&str], named: bool) -> Result<Option<usize>, AdapterError> {
        collection_owner(
            &0,
            &(roles.len() - 1),
            named,
            |index| Ok((Some(roles[*index].into()), Some(index + 1))),
            |left, right| left == right,
        )
    }

    #[test]
    fn collection_label_selects_its_row_not_the_editable_field_or_wrapper_cell() {
        assert_eq!(
            owner(&["AXTextField", "AXCell", "AXRow", "AXOutline"], false).unwrap(),
            Some(2)
        );
        assert_eq!(
            owner(&["AXTextField", "AXCell", "AXTable"], false).unwrap(),
            Some(1)
        );
        assert_eq!(owner(&["AXTextField", "AXOutline"], false).unwrap(), None);
        assert_eq!(owner(&["AXTextField", "AXOutline"], true).unwrap(), None);
        assert_eq!(owner(&["AXButton", "AXList"], true).unwrap(), Some(0));
        assert_eq!(
            owner(&["AXRow", "AXRow", "AXOutline"], true).unwrap(),
            Some(0)
        );
    }

    #[test]
    fn collection_owner_must_reach_the_requested_root_before_delivery() {
        let missing = collection_owner(
            &0,
            &2,
            true,
            |_| Ok((Some("AXRow".into()), None)),
            |left, right| left == right,
        );
        assert_eq!(missing.unwrap_err().code, ErrorCode::AppUnresponsive);
        let mut reads = 0;
        let cycle = collection_owner(
            &0,
            &2,
            true,
            |_| {
                reads += 1;
                Ok((Some("AXRow".into()), Some(0)))
            },
            |left, right| left == right,
        );
        assert_eq!(cycle.unwrap_err().code, ErrorCode::AppUnresponsive);
        assert_eq!(reads, 8);
    }

    fn evidence(name: LocatorField<String>, description: LocatorField<String>) -> LocatorEvidence {
        LocatorEvidence {
            role: LocatorField::Known("treeitem".into()),
            name,
            description,
            value: LocatorField::Unknown,
            identifiers: IdentifierEvidence::absent(),
            states: LocatorField::Unknown,
            ref_evidence: LocatorRefEvidence {
                window_bounds: LocatorField::Absent,
                bounds: LocatorField::Unknown,
                available_actions: LocatorField::Unknown,
                descriptors: Default::default(),
            },
        }
    }

    fn known(text: &str) -> LocatorField<String> {
        LocatorField::Known(text.into())
    }

    fn click() -> LocatorField<Vec<String>> {
        LocatorField::Known(vec!["Click".into()])
    }

    #[test]
    fn a_scan_cut_by_its_deadline_names_the_scan() {
        let cut = name_scan_deadline(
            AdapterError::timeout(
                "Accessibility surface resolution exceeded its absolute deadline",
            ),
            300,
            Deadline::after(1_000).unwrap(),
        );
        assert_eq!(cut.code, ErrorCode::Timeout);
        assert_eq!(cut.details.unwrap()["nodes_scanned"], 300);
        let other = name_scan_deadline(
            AdapterError::new(ErrorCode::ActionFailed, "x"),
            1,
            Deadline::after(1_000).unwrap(),
        );
        assert_eq!(other.code, ErrorCode::ActionFailed);
    }

    #[test]
    fn a_label_is_compared_before_any_action_is_read() {
        let named = evidence(known("Math"), LocatorField::Absent);
        assert_eq!(name_verdict(&named, "math"), NameVerdict::Match);
        assert_eq!(name_verdict(&named, "All"), NameVerdict::Differs);
        let described = evidence(known("x"), known("Math"));
        assert_eq!(name_verdict(&described, "Math"), NameVerdict::Match);
    }

    #[test]
    fn a_candidate_whose_readable_label_differs_never_reads_actions_or_aborts() {
        let differs = name_verdict(&evidence(known("Row"), LocatorField::Absent), "Math");
        assert_eq!(differs, NameVerdict::Differs);
        assert!(!decide(differs, &LocatorField::Unknown).unwrap());
    }

    #[test]
    fn a_matching_label_needs_known_actions_with_click() {
        assert!(decide(NameVerdict::Match, &click()).unwrap());
        assert!(!decide(NameVerdict::Match, &LocatorField::Known(vec![])).unwrap());
        assert_eq!(
            decide(NameVerdict::Match, &LocatorField::Unknown)
                .unwrap_err()
                .code,
            ErrorCode::AppUnresponsive
        );
    }

    #[test]
    fn an_unknown_label_aborts_only_when_the_candidate_could_still_be_the_target() {
        let unknown = name_verdict(
            &evidence(LocatorField::Unknown, LocatorField::Absent),
            "Math",
        );
        assert_eq!(unknown, NameVerdict::Unknown("name_unknown"));
        assert!(!decide(unknown, &LocatorField::Known(vec![])).unwrap());
        assert_eq!(
            decide(unknown, &LocatorField::Unknown).unwrap_err().code,
            ErrorCode::AppUnresponsive
        );
        assert_eq!(
            decide(unknown, &click()).unwrap_err().code,
            ErrorCode::AppUnresponsive
        );
    }
}
