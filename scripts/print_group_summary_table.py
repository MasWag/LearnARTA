#!/usr/bin/env python3
####################################################
# Name
#  print_group_summary_table.py
#
# Description
#  Load an experiment summary JSON file and print a benchmark-group
#  comparison table in plain text or LaTeX. Each benchmark group is the
#  prefix of benchmark_name before the last "-".
#
# Synopsis
#  ./scripts/print_group_summary_table.py SUMMARY_JSON --format {text,latex}
#
# Requirements
#  * Python 3
#
# Portability
#  This script should work with modern Python 3.
#
# Author
#  Masaki Waga
#
# License
#  Apache License, Version 2.0
####################################################
"""Print benchmark-group summary tables in plain text or LaTeX."""

from __future__ import annotations

import argparse
import sys
from decimal import Decimal, ROUND_HALF_UP
from pathlib import Path

from print_summary_table import (
    ALGORITHMS,
    METRICS,
    PLACEHOLDER,
    SummaryEntry,
    elapsed_time_to_centiseconds,
    format_group_label,
    latex_escape,
    load_summary,
    natural_sort_key,
    select_latest_entries,
    superior_cells,
)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Load an experiment summary JSON file and print a benchmark-group "
            "table in plain text or LaTeX."
        )
    )
    parser.add_argument(
        "summary_json",
        help="Path to the experiment summary JSON file",
    )
    parser.add_argument(
        "--format",
        choices=("text", "latex"),
        default="text",
        help="Output format (default: text)",
    )
    return parser.parse_args(argv)


def benchmark_group_name(benchmark_name: str) -> str:
    group_name, separator, _suffix = benchmark_name.rpartition("-")
    return group_name if separator else benchmark_name


def format_decimal_average(total: int, count: int) -> str:
    average = (Decimal(total) / Decimal(count)).quantize(
        Decimal("0.01"), rounding=ROUND_HALF_UP
    )
    rendered = format(average, "f").rstrip("0").rstrip(".")
    return rendered or "0"


def format_elapsed_average(total_centiseconds: int, count: int) -> str:
    average_centiseconds = (
        Decimal(total_centiseconds) / Decimal(count)
    ).quantize(Decimal("1"), rounding=ROUND_HALF_UP)
    return render_centiseconds(int(average_centiseconds))


def render_centiseconds(total_centiseconds: int) -> str:
    hours, remainder = divmod(total_centiseconds, 60 * 60 * 100)
    minutes, remainder = divmod(remainder, 60 * 100)
    seconds, centiseconds = divmod(remainder, 100)

    if hours > 0:
        return f"{hours}:{minutes:02d}:{seconds:02d}.{centiseconds:02d}"
    return f"{minutes}:{seconds:02d}.{centiseconds:02d}"


def build_rows(
    latest_entries: dict[tuple[str, str], SummaryEntry],
) -> tuple[list[dict[str, str]], list[str]]:
    group_to_benchmarks: dict[str, set[str]] = {}
    grouped_entries: dict[tuple[str, str], list[SummaryEntry]] = {}

    for (benchmark_name, algorithm), entry in latest_entries.items():
        group_name = benchmark_group_name(benchmark_name)
        group_to_benchmarks.setdefault(group_name, set()).add(benchmark_name)
        grouped_entries.setdefault((group_name, algorithm), []).append(entry)

    group_names = sorted(group_to_benchmarks, key=natural_sort_key)
    rows: list[dict[str, str]] = []
    warnings: list[str] = []

    for group_name in group_names:
        expected_count = len(group_to_benchmarks[group_name])
        row: dict[str, str] = {"benchmark_group": format_group_label(group_name)}
        for algorithm in ALGORITHMS:
            entries = grouped_entries.get((group_name, algorithm), [])
            if not entries:
                warnings.append(
                    f"warning: benchmark group {group_name!r} is missing results for {algorithm}"
                )
                for metric in METRICS:
                    row[f"{algorithm}.{metric}"] = PLACEHOLDER
                continue

            if len(entries) != expected_count:
                warnings.append(
                    f"warning: benchmark group {group_name!r} has "
                    f"{len(entries)}/{expected_count} results for {algorithm}"
                )

            row[f"{algorithm}.eq_queries"] = format_decimal_average(
                sum(entry.eq_queries for entry in entries), len(entries)
            )
            row[f"{algorithm}.mem_queries"] = format_decimal_average(
                sum(entry.mem_queries for entry in entries), len(entries)
            )
            row[f"{algorithm}.num_states"] = format_decimal_average(
                sum(entry.num_states for entry in entries), len(entries)
            )
            row[f"{algorithm}.elapsed_time"] = format_elapsed_average(
                sum(elapsed_time_to_centiseconds(entry.elapsed_time) for entry in entries),
                len(entries),
            )
        rows.append(row)

    return rows, warnings


def column_names() -> list[str]:
    columns = ["benchmark_group"]
    for algorithm in ALGORITHMS:
        for metric in METRICS:
            columns.append(f"{algorithm}.{metric}")
    return columns


def render_text_table(rows: list[dict[str, str]]) -> str:
    columns = column_names()
    widths = {
        column: max(len(column), *(len(row[column]) for row in rows))
        if rows
        else len(column)
        for column in columns
    }

    def render_row(row: dict[str, str]) -> str:
        rendered_cells: list[str] = []
        for column in columns:
            value = row[column]
            if column == "benchmark_group":
                rendered_cells.append(value.ljust(widths[column]))
            else:
                rendered_cells.append(value.rjust(widths[column]))
        return "  ".join(rendered_cells)

    header = render_row({column: column for column in columns})
    separator = "  ".join("-" * widths[column] for column in columns)
    body = [render_row(row) for row in rows]
    return "\n".join([header, separator, *body])


def render_latex_table(rows: list[dict[str, str]]) -> str:
    lines = [
        r"\begin{tabular}{lrrrrrrrr}",
        r"\toprule",
        r"benchmark\_group & \multicolumn{4}{c}{learn-arta} & \multicolumn{4}{c}{nlstar-rta} \\",
        r"\cmidrule(lr){2-5}\cmidrule(lr){6-9}",
        r" & eq\_queries & mem\_queries & num\_states & elapsed\_time & eq\_queries & mem\_queries & num\_states & elapsed\_time \\",
        r"\midrule",
    ]

    for row in rows:
        winners = superior_cells(row)
        cells = [latex_escape(row["benchmark_group"])]
        for algorithm in ALGORITHMS:
            for metric in METRICS:
                key = f"{algorithm}.{metric}"
                cell = latex_escape(row[key])
                if key in winners:
                    cell = r"\tbcolor{}" + cell
                cells.append(cell)
        lines.append(" & ".join(cells) + r" \\")

    lines.extend([r"\bottomrule", r"\end{tabular}"])
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    try:
        entries = load_summary(Path(args.summary_json))
        latest_entries = select_latest_entries(entries)
        rows, warnings = build_rows(latest_entries)
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    for warning in warnings:
        print(warning, file=sys.stderr)

    if args.format == "latex":
        output = render_latex_table(rows)
    else:
        output = render_text_table(rows)

    print(output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
