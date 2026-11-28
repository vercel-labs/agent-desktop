use super::*;

struct IncompleteTreeAdapter {
    deadline: bool,
}

impl ObservationOps for IncompleteTreeAdapter {
    fn list_windows(
        &self,
        filter: &WindowFilter,
        deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        StaticTreeAdapter {
            tree: window_node(Vec::new()),
        }
        .list_windows(filter, deadline)
    }

    fn observe_tree(
        &self,
        root: crate::ObservationRoot<'_>,
        _request: &crate::ObservationRequest,
    ) -> Result<crate::ObservedTree, AdapterError> {
        let mut tree = crate::adapter::observed_tree(&root, window_node(Vec::new()))?;
        tree.structurally_complete = false;
        tree.nodes[0].completeness.subtree_complete = false;
        if self.deadline {
            tree.stats.reads.health.deadline_exhausted = 1;
        } else {
            tree.stats.reads.health.native_read_failures = 2;
        }
        Ok(tree)
    }
}

impl ActionOps for IncompleteTreeAdapter {}
impl InputOps for IncompleteTreeAdapter {}
impl SystemOps for IncompleteTreeAdapter {}

#[test]
fn an_incomplete_observation_is_recorded_with_the_locators_kind_and_cause() {
    let _guard = HomeGuard::new();
    for (gone, deadline) in [(false, false), (true, false), (false, true), (true, true)] {
        let error = execute(
            WaitSelectorInput {
                timeout_ms: 50,
                ..base_input("button:saved", gone)
            },
            &IncompleteTreeAdapter { deadline },
            &CommandContext::default(),
        )
        .expect_err("an incomplete tree proves neither appearance nor absence");
        let AppError::Adapter(error) = error else {
            panic!("expected adapter timeout");
        };
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "wait_timeout");
        let recorded = &details["last_error"];
        assert_eq!(recorded["code"], "TIMEOUT");
        assert_eq!(
            recorded["kind"],
            if deadline {
                "locator_transient_incomplete"
            } else {
                "locator_incomplete"
            }
        );
        assert_eq!(
            recorded["cause"],
            if deadline {
                "deadline"
            } else {
                "unreadable_elements"
            }
        );
        assert!(recorded["message"].as_str().unwrap().contains(if deadline {
            "ran out of time"
        } else {
            "2 accessibility reads failed"
        }));
    }
}
