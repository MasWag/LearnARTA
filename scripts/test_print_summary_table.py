from __future__ import annotations

import sys
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import print_summary_table as summary


class LatexHighlightTest(unittest.TestCase):
    def test_render_latex_table_marks_superior_cells(self) -> None:
        rows = [
            {
                "benchmark_name": "(10,2,20)-1",
                "learn-arta.eq_queries": "10",
                "learn-arta.mem_queries": "100",
                "learn-arta.num_states": "4",
                "learn-arta.elapsed_time": "0:01.00",
                "nlstar-rta.eq_queries": "12",
                "nlstar-rta.mem_queries": "120",
                "nlstar-rta.num_states": "3",
                "nlstar-rta.elapsed_time": "0:02.00",
            }
        ]

        actual = summary.render_latex_table(rows)

        self.assertIn("(10,2,20)-1", actual)
        self.assertIn(r"\tbcolor{}10", actual)
        self.assertIn(r"\tbcolor{}100", actual)
        self.assertIn(r"\tbcolor{}0:01.00", actual)
        self.assertIn(r"\tbcolor{}3", actual)
        self.assertNotIn(r"\tbcolor{}12", actual)
        self.assertNotIn(r"\tbcolor{}120", actual)
        self.assertNotIn(r"\tbcolor{}4", actual)
        self.assertNotIn(r"\tbcolor{}0:02.00", actual)

    def test_build_rows_formats_benchmark_group_prefix_as_tuple(self) -> None:
        latest_entries = {
            ("10_2_20-1", "learn-arta"): summary.SummaryEntry(
                identifier="suite-10_2_20-1-learn-arta-20260410-120000",
                benchmark_name="10_2_20-1",
                algorithm="learn-arta",
                timestamp="20260410-120000",
                eq_queries=10,
                mem_queries=100,
                num_states=4,
                elapsed_time="0:01.00",
            ),
            ("10_2_20-1", "nlstar-rta"): summary.SummaryEntry(
                identifier="suite-10_2_20-1-nlstar-rta-20260410-120000",
                benchmark_name="10_2_20-1",
                algorithm="nlstar-rta",
                timestamp="20260410-120000",
                eq_queries=12,
                mem_queries=120,
                num_states=3,
                elapsed_time="0:02.00",
            ),
        }

        rows, warnings = summary.build_rows(latest_entries)

        self.assertEqual([], warnings)
        self.assertEqual("(10,2,20)-1", rows[0]["benchmark_name"])


if __name__ == "__main__":
    unittest.main()
