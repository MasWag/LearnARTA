from __future__ import annotations

import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import print_group_summary_table as grouped
import print_group_table_size as table_size
import print_summary_table as summary
from print_summary_table import SummaryEntry


def make_entry(
    benchmark_name: str,
    algorithm: str,
    *,
    eq_queries: int,
    mem_queries: int,
    rows: int,
    columns: int,
    num_states: int,
    elapsed_time: str,
    timestamp: str = "20260409-120000",
) -> SummaryEntry:
    date_part, time_part = timestamp.split("-")
    return SummaryEntry(
        identifier=f"suite-{benchmark_name}-{algorithm}-{date_part}-{time_part}",
        benchmark_name=benchmark_name,
        algorithm=algorithm,
        timestamp=timestamp,
        eq_queries=eq_queries,
        mem_queries=mem_queries,
        rows=rows,
        columns=columns,
        num_states=num_states,
        elapsed_time=elapsed_time,
    )


class BuildRowsTest(unittest.TestCase):
    def test_build_rows_groups_by_prefix_and_averages_rows_and_columns(self) -> None:
        latest_entries = {
            ("10_2_20-1", "learn-arta"): make_entry(
                "10_2_20-1",
                "learn-arta",
                eq_queries=10,
                mem_queries=100,
                rows=200,
                columns=40,
                num_states=3,
                elapsed_time="0:01.00",
            ),
            ("10_2_20-2", "learn-arta"): make_entry(
                "10_2_20-2",
                "learn-arta",
                eq_queries=14,
                mem_queries=140,
                rows=300,
                columns=44,
                num_states=5,
                elapsed_time="0:03.00",
            ),
            ("10_2_20-1", "nlstar-rta"): make_entry(
                "10_2_20-1",
                "nlstar-rta",
                eq_queries=8,
                mem_queries=80,
                rows=50,
                columns=10,
                num_states=4,
                elapsed_time="0:02.00",
            ),
            ("10_2_20-2", "nlstar-rta"): make_entry(
                "10_2_20-2",
                "nlstar-rta",
                eq_queries=10,
                mem_queries=120,
                rows=80,
                columns=20,
                num_states=6,
                elapsed_time="0:04.00",
            ),
        }

        rows, warnings = table_size.build_rows(latest_entries)

        self.assertEqual([], warnings)
        self.assertEqual(
            [
                {
                    "benchmark_group": "(10,2,20)",
                    "learn-arta.rows": "250",
                    "learn-arta.columns": "42",
                    "nlstar-rta.rows": "65",
                    "nlstar-rta.columns": "15",
                }
            ],
            rows,
        )

    def test_build_rows_orders_multiple_groups_naturally(self) -> None:
        latest_entries = {
            ("10_2_100-1", "learn-arta"): make_entry(
                "10_2_100-1",
                "learn-arta",
                eq_queries=10,
                mem_queries=100,
                rows=100,
                columns=10,
                num_states=3,
                elapsed_time="0:01.00",
            ),
            ("10_2_100-1", "nlstar-rta"): make_entry(
                "10_2_100-1",
                "nlstar-rta",
                eq_queries=10,
                mem_queries=100,
                rows=90,
                columns=9,
                num_states=3,
                elapsed_time="0:01.00",
            ),
            ("10_2_30-1", "learn-arta"): make_entry(
                "10_2_30-1",
                "learn-arta",
                eq_queries=10,
                mem_queries=100,
                rows=20,
                columns=2,
                num_states=3,
                elapsed_time="0:01.00",
            ),
            ("10_2_30-1", "nlstar-rta"): make_entry(
                "10_2_30-1",
                "nlstar-rta",
                eq_queries=10,
                mem_queries=100,
                rows=10,
                columns=1,
                num_states=3,
                elapsed_time="0:01.00",
            ),
        }

        rows, warnings = table_size.build_rows(latest_entries)

        self.assertEqual([], warnings)
        self.assertEqual(["(10,2,30)", "(10,2,100)"], [row["benchmark_group"] for row in rows])
        self.assertEqual("20", rows[0]["learn-arta.rows"])
        self.assertEqual("100", rows[1]["learn-arta.rows"])

    def test_build_rows_warns_for_partial_group_results(self) -> None:
        latest_entries = {
            ("10_2_20-1", "learn-arta"): make_entry(
                "10_2_20-1",
                "learn-arta",
                eq_queries=10,
                mem_queries=100,
                rows=200,
                columns=40,
                num_states=3,
                elapsed_time="0:01.00",
            ),
            ("10_2_20-2", "learn-arta"): make_entry(
                "10_2_20-2",
                "learn-arta",
                eq_queries=12,
                mem_queries=120,
                rows=250,
                columns=42,
                num_states=5,
                elapsed_time="0:03.00",
            ),
            ("10_2_20-1", "nlstar-rta"): make_entry(
                "10_2_20-1",
                "nlstar-rta",
                eq_queries=7,
                mem_queries=70,
                rows=60,
                columns=12,
                num_states=4,
                elapsed_time="0:02.00",
            ),
        }

        rows, warnings = table_size.build_rows(latest_entries)

        self.assertEqual(
            ["warning: benchmark group '10_2_20' has 1/2 results for nlstar-rta"],
            warnings,
        )
        self.assertEqual("60", rows[0]["nlstar-rta.rows"])
        self.assertEqual("12", rows[0]["nlstar-rta.columns"])

    def test_build_rows_warns_for_missing_algorithm_results(self) -> None:
        latest_entries = {
            ("10_2_20-1", "nlstar-rta"): make_entry(
                "10_2_20-1",
                "nlstar-rta",
                eq_queries=7,
                mem_queries=70,
                rows=60,
                columns=12,
                num_states=4,
                elapsed_time="0:02.00",
            ),
        }

        rows, warnings = table_size.build_rows(latest_entries)

        self.assertEqual(
            ["warning: benchmark group '10_2_20' is missing results for learn-arta"],
            warnings,
        )
        self.assertEqual("-", rows[0]["learn-arta.rows"])
        self.assertEqual("-", rows[0]["learn-arta.columns"])
        self.assertEqual("60", rows[0]["nlstar-rta.rows"])


class RenderTest(unittest.TestCase):
    def make_single_row(self) -> list[dict[str, str]]:
        return [
            {
                "benchmark_group": "(10,2,20)",
                "learn-arta.rows": "250",
                "learn-arta.columns": "42",
                "nlstar-rta.rows": "65",
                "nlstar-rta.columns": "15",
            }
        ]

    def test_render_text_table(self) -> None:
        actual = table_size.render_text_table(self.make_single_row())

        lines = actual.splitlines()
        self.assertEqual(
            [
                "benchmark_group  learn-arta.rows  learn-arta.columns"
                "  nlstar-rta.rows  nlstar-rta.columns",
                "  ".join(["-" * 15, "-" * 15, "-" * 18, "-" * 15, "-" * 18]),
            ],
            lines[:2],
        )
        self.assertEqual(
            ["(10,2,20)", "250", "42", "65", "15"],
            lines[2].split(),
        )

    def test_render_latex_table_marks_superior_cells(self) -> None:
        rows = [
            {
                "benchmark_group": "(10,2,20)",
                "learn-arta.rows": "250",
                "learn-arta.columns": "42",
                "nlstar-rta.rows": "65",
                "nlstar-rta.columns": "15",
            }
        ]

        actual = table_size.render_latex_table(rows)

        self.assertIn(r"\begin{tabular}{lrrrr}", actual)
        self.assertIn(
            "benchmark\\_group"
            " & \\multicolumn{2}{c}{learn-arta}"
            " & \\multicolumn{2}{c}{nlstar-rta}"
            " \\\\",
            actual,
        )
        self.assertIn(r"\cmidrule(lr){2-3}\cmidrule(lr){4-5}", actual)
        self.assertIn(" & rows & columns & rows & columns \\", actual)
        # Only the smaller values of each metric are highlighted.
        self.assertIn(
            "(10,2,20) & 250 & 42 & \\tbcolor{}65 & \\tbcolor{}15 \\\\",
            actual,
        )
        self.assertNotIn("\\tbcolor{}250", actual)
        self.assertNotIn("\\tbcolor{}42", actual)

    def test_render_latex_table_marks_tied_minima_for_both_learners(self) -> None:
        rows = [
            {
                "benchmark_group": "(10,2,20)",
                "learn-arta.rows": "50",
                "learn-arta.columns": "10",
                "nlstar-rta.rows": "50",
                "nlstar-rta.columns": "20",
            }
        ]

        actual = table_size.render_latex_table(rows)

        # Tied rows: both learners highlighted. Distinct columns: only the smaller.
        self.assertIn(
            "(10,2,20) & \\tbcolor{}50 & \\tbcolor{}10 & \\tbcolor{}50 & 20 \\\\",
            actual,
        )
        self.assertNotIn("\\tbcolor{}20", actual)

    def test_render_latex_table_with_placeholders_does_not_highlight(self) -> None:
        rows = [
            {
                "benchmark_group": "(10,2,20)",
                "learn-arta.rows": "-",
                "learn-arta.columns": "-",
                "nlstar-rta.rows": "65",
                "nlstar-rta.columns": "15",
            }
        ]

        actual = table_size.render_latex_table(rows)

        self.assertIn("- & - & \\tbcolor{}65 & \\tbcolor{}15 \\\\", actual)

    def test_render_text_table_with_placeholders(self) -> None:
        rows = [
            {
                "benchmark_group": "(10,2,20)",
                "learn-arta.rows": "-",
                "learn-arta.columns": "-",
                "nlstar-rta.rows": "65",
                "nlstar-rta.columns": "15",
            }
        ]

        actual = table_size.render_text_table(rows)

        self.assertIn("-", actual)
        self.assertIn("65", actual)


class MainTest(unittest.TestCase):
    def write_summary_json(self, entries: list[dict]) -> Path:
        handle = tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False, encoding="utf-8"
        )
        with handle:
            json.dump(entries, handle)
        return Path(handle.name)

    def make_json_entry(
        self,
        benchmark_name: str,
        algorithm: str,
        *,
        rows: int,
        columns: int,
        timestamp: str,
    ) -> dict:
        return {
            "id": f"suite-{benchmark_name}-{algorithm}-{timestamp}",
            "benchmark_name": benchmark_name,
            "eq_queries": 10,
            "mem_queries": 100,
            "rows": rows,
            "columns": columns,
            "num_states": 3,
            "elapsed_time": "0:01.00",
        }

    def test_main_averages_only_latest_entries(self) -> None:
        path = self.write_summary_json(
            [
                self.make_json_entry(
                    "10_2_20-1",
                    "learn-arta",
                    rows=10,
                    columns=1,
                    timestamp="20260409-120000",
                ),
                self.make_json_entry(
                    "10_2_20-1",
                    "learn-arta",
                    rows=200,
                    columns=40,
                    timestamp="20260410-120000",
                ),
                self.make_json_entry(
                    "10_2_20-1",
                    "nlstar-rta",
                    rows=65,
                    columns=15,
                    timestamp="20260409-120000",
                ),
            ]
        )

        entries = summary.load_summary(path)
        latest_entries = summary.select_latest_entries(entries)
        rows, warnings = table_size.build_rows(latest_entries)

        self.assertEqual([], warnings)
        self.assertEqual("200", rows[0]["learn-arta.rows"])
        self.assertEqual("40", rows[0]["learn-arta.columns"])
        self.assertEqual("65", rows[0]["nlstar-rta.rows"])
        self.assertEqual("15", rows[0]["nlstar-rta.columns"])

        stdout = io.StringIO()
        with contextlib.redirect_stdout(stdout):
            status = table_size.main([str(path), "--format", "text"])

        self.assertEqual(0, status)
        self.assertIn("benchmark_group", stdout.getvalue())
        self.assertIn("200", stdout.getvalue())
        self.assertIn("40", stdout.getvalue())

    def test_main_prints_missing_algorithm_warning_to_stderr(self) -> None:
        path = self.write_summary_json(
            [
                self.make_json_entry(
                    "10_2_20-1",
                    "nlstar-rta",
                    rows=65,
                    columns=15,
                    timestamp="20260409-120000",
                ),
            ]
        )

        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = table_size.main([str(path), "--format", "latex"])

        self.assertEqual(0, status)
        self.assertIn(
            "warning: benchmark group '10_2_20' is missing results for learn-arta",
            stderr.getvalue(),
        )
        self.assertIn(r"\end{tabular}", stdout.getvalue())


class NormalizeEntryTest(unittest.TestCase):
    def make_entry_dict(self) -> dict:
        return {
            "id": "suite-10_2_20-1-learn-arta-20260409-120000",
            "benchmark_name": "10_2_20-1",
            "eq_queries": 10,
            "mem_queries": 100,
            "num_states": 3,
            "elapsed_time": "0:01.00",
        }

    def test_normalize_entry_requires_rows(self) -> None:
        with self.assertRaisesRegex(ValueError, "'rows'"):
            summary.normalize_entry(self.make_entry_dict(), 0)

    def test_normalize_entry_rejects_negative_rows(self) -> None:
        entry = self.make_entry_dict()
        entry["rows"] = -1
        entry["columns"] = 0
        with self.assertRaisesRegex(ValueError, "'rows'"):
            summary.normalize_entry(entry, 0)

    def test_normalize_entry_rejects_missing_columns(self) -> None:
        entry = self.make_entry_dict()
        entry["rows"] = 10
        with self.assertRaisesRegex(ValueError, "'columns'"):
            summary.normalize_entry(entry, 0)


class ExistingScriptsUnchangedTest(unittest.TestCase):
    def make_latest_entries(self) -> dict[tuple[str, str], SummaryEntry]:
        return {
            ("10_2_20-1", "learn-arta"): make_entry(
                "10_2_20-1",
                "learn-arta",
                eq_queries=10,
                mem_queries=100,
                rows=200,
                columns=40,
                num_states=4,
                elapsed_time="0:01.00",
            ),
            ("10_2_20-1", "nlstar-rta"): make_entry(
                "10_2_20-1",
                "nlstar-rta",
                eq_queries=12,
                mem_queries=120,
                rows=50,
                columns=10,
                num_states=3,
                elapsed_time="0:02.00",
            ),
        }

    def test_print_summary_table_build_rows_is_unchanged(self) -> None:
        rows, warnings = summary.build_rows(self.make_latest_entries())

        self.assertEqual([], warnings)
        self.assertEqual("(10,2,20)-1", rows[0]["benchmark_name"])
        self.assertEqual("10", rows[0]["learn-arta.eq_queries"])
        self.assertEqual("100", rows[0]["learn-arta.mem_queries"])
        self.assertEqual("4", rows[0]["learn-arta.num_states"])
        self.assertEqual("0:01.00", rows[0]["learn-arta.elapsed_time"])
        self.assertNotIn("learn-arta.rows", rows[0])
        self.assertNotIn("learn-arta.columns", rows[0])

    def test_print_group_summary_table_build_rows_is_unchanged(self) -> None:
        rows, warnings = grouped.build_rows(self.make_latest_entries())

        self.assertEqual([], warnings)
        self.assertEqual(
            [
                {
                    "benchmark_group": "(10,2,20)",
                    "learn-arta.eq_queries": "10",
                    "learn-arta.mem_queries": "100",
                    "learn-arta.num_states": "4",
                    "learn-arta.elapsed_time": "0:01.00",
                    "nlstar-rta.eq_queries": "12",
                    "nlstar-rta.mem_queries": "120",
                    "nlstar-rta.num_states": "3",
                    "nlstar-rta.elapsed_time": "0:02.00",
                }
            ],
            rows,
        )


if __name__ == "__main__":
    unittest.main()
