use crate::{
    AppError, actions::action::Action, adapter::PlatformAdapter, commands::helpers::RefArgs,
    commands::helpers_witnessed::execute_ref_action_with_context_witnessed,
    context::CommandContext,
};
use serde_json::Value;

/// Delivers a right click and reports the menu delivery it proved.
///
/// A right click is the one delivery that opens a context menu, so it is also
/// the one place the driver can say which element a later menu belongs to. The
/// witness does its work inside the delivery's own lease, and it cannot change
/// what the action reports: `menu_delivery` appears only when the delivery
/// proved which menu it produced, and its absence means the caller has no
/// provenance to act on.
pub fn execute(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let witness = crate::menu_origin::MenuWitness::new(context.session_id());
    let request = context.request_base(Action::RightClick);
    let mut value =
        execute_ref_action_with_context_witnessed(args, adapter, request, context, &witness)?;
    if let Some(receipt) = witness.receipt() {
        value["menu_delivery"] = receipt;
    }
    Ok(value)
}

#[cfg(test)]
#[path = "right_click_tests.rs"]
mod tests;
