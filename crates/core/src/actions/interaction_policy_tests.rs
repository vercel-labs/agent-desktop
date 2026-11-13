use super::*;

#[test]
fn join_headless_with_focus_fallback_gives_focus_fallback() {
    assert_eq!(
        InteractionPolicy::headless().join(InteractionPolicy::focus_fallback()),
        InteractionPolicy::focus_fallback()
    );
}

#[test]
fn join_headless_with_headed_gives_headed() {
    assert_eq!(
        InteractionPolicy::headless().join(InteractionPolicy::headed()),
        InteractionPolicy::headed()
    );
}

#[test]
fn join_focus_fallback_with_headed_gives_headed() {
    assert_eq!(
        InteractionPolicy::focus_fallback().join(InteractionPolicy::headed()),
        InteractionPolicy::headed()
    );
}

/// The three named presets never have `focus_steal=false` with `cursor=true`.
/// This unnamed policy exercises that both fields are handled independently.
fn cursor_only() -> InteractionPolicy {
    InteractionPolicy {
        allow_focus_steal: false,
        allow_cursor_move: true,
    }
}

fn all_four_policies() -> [InteractionPolicy; 4] {
    [
        InteractionPolicy::headless(),
        InteractionPolicy::focus_fallback(),
        InteractionPolicy::headed(),
        cursor_only(),
    ]
}

#[test]
fn join_is_idempotent_for_all_policies_including_cursor_only() {
    for p in all_four_policies() {
        assert_eq!(p.join(p), p, "join(p, p) must equal p: {:?}", p);
    }
}

#[test]
fn join_is_commutative_over_all_policy_pairs() {
    for a in all_four_policies() {
        for b in all_four_policies() {
            assert_eq!(
                a.join(b),
                b.join(a),
                "join must be commutative: {:?} vs {:?}",
                a,
                b
            );
        }
    }
}

#[test]
fn join_never_downgrades_either_operand_on_any_field() {
    for a in all_four_policies() {
        for b in all_four_policies() {
            let result = a.join(b);
            assert!(
                result.allow_focus_steal >= a.allow_focus_steal,
                "join lowered allow_focus_steal below a: a={:?} b={:?}",
                a,
                b
            );
            assert!(
                result.allow_focus_steal >= b.allow_focus_steal,
                "join lowered allow_focus_steal below b: a={:?} b={:?}",
                a,
                b
            );
            assert!(
                result.allow_cursor_move >= a.allow_cursor_move,
                "join lowered allow_cursor_move below a: a={:?} b={:?}",
                a,
                b
            );
            assert!(
                result.allow_cursor_move >= b.allow_cursor_move,
                "join lowered allow_cursor_move below b: a={:?} b={:?}",
                a,
                b
            );
        }
    }
}

#[test]
fn each_effect_is_allowed_only_by_the_policies_that_grant_it() {
    use crate::Effect::{ClipboardWrite, CursorMove, FocusSteal};
    let cases = [
        (InteractionPolicy::headless(), [false, false, false]),
        (InteractionPolicy::focus_fallback(), [true, false, false]),
        (cursor_only(), [false, true, false]),
        (InteractionPolicy::headed(), [true, true, true]),
    ];
    for (policy, allowed) in cases {
        for (effect, allowed) in [FocusSteal, CursorMove, ClipboardWrite]
            .into_iter()
            .zip(allowed)
        {
            let result = policy.require(effect, "probe");
            assert_eq!(result.is_ok(), allowed, "{policy:?} {effect:?}");
            if let Err(error) = result {
                assert_eq!(error.code, crate::ErrorCode::PolicyDenied);
                assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
            }
        }
    }
}
