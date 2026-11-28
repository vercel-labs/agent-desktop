import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from runner import legs, verdict
from runner.product import Envelope
from runner.verdict import LegContext
from scenarios import launch


class LaunchScenarioTests(unittest.TestCase):
    def test_launch_teardowns_force_the_close_because_headless_refuses_a_graceful_one(self):
        for leg_id, name, leg in (
            ("launch.launch_calculator_attributed", "Calculator", launch.launch_calculator_attributed),
            ("launch.launch_terminal_owner", "Terminal", launch.launch_terminal_owner),
        ):
            with self.subTest(leg=leg_id):
                tier = SimpleNamespace(runtime_dir=Path("/private/test"))
                ctx = LegContext(tier, "headless", legs.leg_by_id(leg_id),
                                 member_reader=lambda _: frozenset())
                state, closed = {"members": []}, []
                def product(tier, argv, **kw):
                    if argv[0] == "launch":
                        state["members"] = [(123, 1, "gnome-terminal-server")]
                        return Envelope(True, {"pid": 123}, None, "{}", 0, argv)
                    if argv[0] == "close-app":
                        closed.append(argv)
                        state["members"] = []
                        return Envelope(True, {}, None, "{}", 0, argv)
                    return Envelope(True, [{"id": "w-1"}], None, "{}", 0, argv)
                with patch.object(launch, "members", side_effect=lambda _: state["members"]), \
                     patch.object(launch, "app_root", return_value=SimpleNamespace(name=name)), \
                     patch.object(launch, "changed", side_effect=lambda read, *a, **kw: read()), \
                     patch("runner.verdict.run_product", side_effect=product):
                    leg(ctx)
                self.assertEqual(closed, [["close-app", name, "--force"]])

    def test_alias_queries_require_the_independently_launched_calculator(self):
        calculator = {"name": "gnome-calculator", "pid": 123}
        for bad in (None, [], [{"name": "gnome-text-editor", "pid": 456}],
                    [{"name": "gnome-calculator", "pid": 456}],
                    [{"name": "gnome-text-editor", "pid": 123}],
                    [calculator, {"name": "gnome-text-editor", "pid": 456}]):
            for query in (0, 1, "both"):
                with self.subTest(bad=bad, query=query):
                    tier = SimpleNamespace(runtime_dir=Path("/private/test"))
                    ctx = LegContext(tier, "headless", legs.leg_by_id("launch.list_apps_alias"),
                                     member_reader=lambda _: frozenset())
                    calls = []
                    def product(tier, argv, **kw):
                        rows = bad if bad is not None and query in (len(calls), "both") else [calculator]
                        calls.append(argv)
                        return Envelope(True, {"apps": rows}, None, str(rows), 0, argv)
                    root = SimpleNamespace(name="gnome-calculator")
                    with patch.object(launch, "launch", return_value=SimpleNamespace(pid=123)), \
                         patch.object(launch, "stop"), \
                         patch.object(launch, "registry", return_value=[root]), \
                         patch.object(launch, "app_root", return_value=root), \
                         patch("runner.verdict.run_product", side_effect=product):
                        if bad is None:
                            launch.list_apps_alias(ctx)
                            self.assertEqual(verdict.evaluate(ctx), [])
                            self.assertEqual([argv[-1] for argv in calls], ["Calculator", "calculator"])
                        else:
                            with self.assertRaises(AssertionError):
                                launch.list_apps_alias(ctx)
