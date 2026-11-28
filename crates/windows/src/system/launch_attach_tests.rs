use super::*;

#[test]
fn no_instance_spawns_one_instance_attaches_and_several_are_ambiguous() {
    assert_eq!(attach_plan(0), AttachPlan::Spawn);
    assert_eq!(attach_plan(1), AttachPlan::Attach);
    assert_eq!(attach_plan(2), AttachPlan::Ambiguous);
    assert_eq!(attach_plan(5), AttachPlan::Ambiguous);
}
