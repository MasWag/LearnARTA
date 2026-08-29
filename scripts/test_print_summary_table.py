from __future__ import annotations

import sys
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import generate_drta_size_map as drta_map
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
                rows=10,
                columns=5,
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
                rows=12,
                columns=6,
                num_states=3,
                elapsed_time="0:02.00",
            ),
        }

        rows, warnings = summary.build_rows(latest_entries)

        self.assertEqual([], warnings)
        self.assertEqual("(10,2,20)-1", rows[0]["benchmark_name"])

    def test_build_rows_adds_drta_size_column_from_raw_benchmark_name(self) -> None:
        latest_entries = {
            ("10_2_20-1", "learn-arta"): summary.SummaryEntry(
                identifier="suite-10_2_20-1-learn-arta-20260410-120000",
                benchmark_name="10_2_20-1",
                algorithm="learn-arta",
                timestamp="20260410-120000",
                eq_queries=10,
                mem_queries=100,
                rows=10,
                columns=5,
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
                rows=12,
                columns=6,
                num_states=3,
                elapsed_time="0:02.00",
            ),
        }
        drta_sizes = {
            "10_2_20-1": summary.DrtaSizeEntry(
                benchmark_name="10_2_20-1",
                drta_size="7",
                status="ok",
            )
        }

        rows, warnings = summary.build_rows(latest_entries, drta_sizes)

        self.assertEqual([], warnings)
        self.assertEqual("7", rows[0]["DRTA-size"])

    def test_build_rows_warns_for_missing_and_error_drta_sizes(self) -> None:
        latest_entries = {
            ("10_2_20-1", "learn-arta"): summary.SummaryEntry(
                identifier="suite-10_2_20-1-learn-arta-20260410-120000",
                benchmark_name="10_2_20-1",
                algorithm="learn-arta",
                timestamp="20260410-120000",
                eq_queries=10,
                mem_queries=100,
                rows=10,
                columns=5,
                num_states=4,
                elapsed_time="0:01.00",
            ),
            ("10_2_20-2", "learn-arta"): summary.SummaryEntry(
                identifier="suite-10_2_20-2-learn-arta-20260410-120000",
                benchmark_name="10_2_20-2",
                algorithm="learn-arta",
                timestamp="20260410-120000",
                eq_queries=11,
                mem_queries=110,
                rows=11,
                columns=7,
                num_states=5,
                elapsed_time="0:01.50",
            ),
        }
        drta_sizes = {
            "10_2_20-2": summary.DrtaSizeEntry(
                benchmark_name="10_2_20-2",
                drta_size="",
                status="error:unsupported target",
            )
        }

        rows, warnings = summary.build_rows(latest_entries, drta_sizes)

        self.assertEqual("-", rows[0]["DRTA-size"])
        self.assertEqual("-", rows[1]["DRTA-size"])
        self.assertIn(
            "warning: benchmark '10_2_20-1' is missing DRTA-size data",
            warnings,
        )
        self.assertIn(
            "warning: benchmark '10_2_20-2' has DRTA-size status "
            "'error:unsupported target'",
            warnings,
        )

    def test_render_latex_table_with_drta_size_does_not_highlight_it(self) -> None:
        rows = [
            {
                "benchmark_name": "(10,2,20)-1",
                "DRTA-size": "1",
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

        self.assertIn(r"\begin{tabular}{lrrrrrrrrr}", actual)
        self.assertIn("benchmark\\_name & DRTA-size", actual)
        self.assertIn(r"\cmidrule(lr){3-6}\cmidrule(lr){7-10}", actual)
        self.assertIn("(10,2,20)-1 & 1 &", actual)
        self.assertNotIn(r"\tbcolor{}1 &", actual)


class DrtaSizeMapTest(unittest.TestCase):
    def test_parse_drta_size_output_maps_ok_and_error_rows(self) -> None:
        output = "\n".join(
            [
                "Building drta-size ...",
                "file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,sfa_states,sfa_transitions,min_drta_states,min_drta_transitions,time_ms,status",
                "10_2_20-1.json,2,1,1,1,1,2,1,7,-1,14,ok",
                "10_2_20-2.json,3,2,1,1,1,0,0,0,-1,0,error:unsupported",
            ]
        )

        rows = drta_map.parse_drta_size_output(output)

        self.assertEqual(
            [
                drta_map.DrtaSizeRow(
                    benchmark_name="10_2_20-1",
                    file="10_2_20-1.json",
                    drta_size="7",
                    status="ok",
                ),
                drta_map.DrtaSizeRow(
                    benchmark_name="10_2_20-2",
                    file="10_2_20-2.json",
                    drta_size="",
                    status="error:unsupported",
                ),
            ],
            rows,
        )

    def test_parse_drta_size_output_rejects_missing_header(self) -> None:
        with self.assertRaisesRegex(ValueError, "expected CSV header"):
            drta_map.parse_drta_size_output("Building drta-size ...")


if __name__ == "__main__":
    unittest.main()
