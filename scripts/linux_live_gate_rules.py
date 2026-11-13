"""Private desktop requirements and shard ownership for the Linux live gate."""

TIERS = ("openbox", "mutter", "wayland")
WAYLAND_MODULES = ()
MUTTER_MODULES = ()
CURSOR_WAYLAND = "system::overlay::cursor_overlay::tests::live_wayland_cursor_refusal_reports_unrendered_without_an_x11_window"
CLI_WAYLAND = "system::overlay::cursor_overlay::cli_tests::live_wayland_cursor_cli_reports_unrendered_without_x11_fallback"
EXTERNAL_WAYLAND = (CURSOR_WAYLAND, CLI_WAYLAND)
BARE_CASES = ()
OPENBOX_CASES = ()
HELPER_PARENTS = {}
# Reviewed module groups whose fixtures never acquire the global interaction lease.
# Unknown modules and all unit tests stay on one writer, preserving that lock's contract.
PARALLEL_READS = {
    "tree::live::absence_live_tests", "tree::live::chromium_live_tests",
    "tree::live::hit_live_tests::cases", "tree::live::live_read_live_tests",
    "tree::live::navigate_live_tests", "tree::live::observe_live_tests", "tree::live::qt_live_tests",
    "tree::live::registry_toolkit_live_tests", "tree::live::resolve_live_tests",
    "tree::live::live_timeout",
    "tree::live::vocabulary_live_tests", "tree::live::walker_live_tests",
}
