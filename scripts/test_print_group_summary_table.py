from __future__ import annotations

import sys
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import print_group_summary_table as grouped
from print_summary_table import SummaryEntry


def make_entry(
    benchmark_name: str,
    algorithm: str,
    *,
    eq_queries: int,
    mem_queries: int,
    num_states: int,
    elapsed_time: str,
) -> SummaryEntry:
    return SummaryEntry(
        identifier=f"suite-{benchmark_name}-{algorithm}-20260409-120000",
        benchmark_name=benchmark_name,
        algorithm=algorithm,
        timestamp="20260409-120000",
        eq_queries=eq_queries,
        mem_queries=mem_queries,
        num_states=num_states,
        elapsed_time=elapsed_time,
    )


class BuildRowsTest(unittest.TestCase):
    def test_build_rows_groups_by_prefix_and_averages_metrics(self) -> None:
        latest_entries = {
            ("10_2_20-1", "learn-arta"): make_entry(
                "10_2_20-1",
                "learn-arta",
                eq_queries=10,
                mem_queries=100,
                num_states=3,
                elapsed_time="0:01.00",
            ),
            ("10_2_20-2", "learn-arta"): make_entry(
                "10_2_20-2",
                "learn-arta",
                eq_queries=14,
                mem_queries=140,
                num_states=5,
                elapsed_time="0:03.00",
            ),
            ("10_2_20-1", "nlstar-rta"): make_entry(
                "10_2_20-1",
                "nlstar-rta",
                eq_queries=8,
                mem_queries=80,
                num_states=4,
                elapsed_time="0:02.00",
            ),
            ("10_2_20-2", "nlstar-rta"): make_entry(
                "10_2_20-2",
                "nlstar-rta",
                eq_queries=10,
                mem_queries=120,
                num_states=6,
                elapsed_time="0:04.00",
            ),
        }

        rows, warnings = grouped.build_rows(latest_entries)

        self.assertEqual([], warnings)
        self.assertEqual(
            [
                {
                    "benchmark_group": "(10,2,20)",
                    "learn-arta.eq_queries": "12",
                    "learn-arta.mem_queries": "120",
                    "learn-arta.num_states": "4",
                    "learn-arta.elapsed_time": "0:02.00",
                    "nlstar-rta.eq_queries": "9",
                    "nlstar-rta.mem_queries": "100",
                    "nlstar-rta.num_states": "5",
                    "nlstar-rta.elapsed_time": "0:03.00",
                }
            ],
            rows,
        )

    def test_build_rows_warns_for_partial_group_results(self) -> None:
        latest_entries = {
            ("10_2_20-1", "learn-arta"): make_entry(
                "10_2_20-1",
                "learn-arta",
                eq_queries=10,
                mem_queries=100,
                num_states=3,
                elapsed_time="0:01.00",
            ),
            ("10_2_20-2", "learn-arta"): make_entry(
                "10_2_20-2",
                "learn-arta",
                eq_queries=12,
                mem_queries=120,
                num_states=5,
                elapsed_time="0:03.00",
            ),
            ("10_2_20-1", "nlstar-rta"): make_entry(
                "10_2_20-1",
                "nlstar-rta",
                eq_queries=7,
                mem_queries=70,
                num_states=4,
                elapsed_time="0:02.00",
            ),
        }

        rows, warnings = grouped.build_rows(latest_entries)

        self.assertEqual(
            ["warning: benchmark group '10_2_20' has 1/2 results for nlstar-rta"],
            warnings,
        )
        self.assertEqual("7", rows[0]["nlstar-rta.eq_queries"])
        self.assertEqual("0:02.00", rows[0]["nlstar-rta.elapsed_time"])

    def test_render_latex_table_marks_superior_cells_and_uses_tuple_labels(self) -> None:
        rows = [
            {
                "benchmark_group": "(10,2,20)",
                "learn-arta.eq_queries": "12",
                "learn-arta.mem_queries": "120",
                "learn-arta.num_states": "4",
                "learn-arta.elapsed_time": "0:02.00",
                "nlstar-rta.eq_queries": "13",
                "nlstar-rta.mem_queries": "150",
                "nlstar-rta.num_states": "3",
                "nlstar-rta.elapsed_time": "0:03.00",
            }
        ]

        actual = grouped.render_latex_table(rows)

        self.assertIn("(10,2,20)", actual)
        self.assertIn(r"\tbcolor{}12", actual)
        self.assertIn(r"\tbcolor{}120", actual)
        self.assertIn(r"\tbcolor{}0:02.00", actual)
        self.assertIn(r"\tbcolor{}3", actual)
        self.assertNotIn(r"\tbcolor{}13", actual)
        self.assertNotIn(r"\tbcolor{}150", actual)
        self.assertNotIn(r"\tbcolor{}4", actual)
        self.assertNotIn(r"\tbcolor{}0:03.00", actual)


if __name__ == "__main__":
    unittest.main()
