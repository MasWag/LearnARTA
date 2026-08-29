#!/usr/bin/env python3
####################################################
# Name
#  print_group_table_size.py
#
# Description
#  Load an experiment summary JSON file and print a benchmark-group
#  table of average final observation-table sizes (rows, columns)
#  in plain text or LaTeX. Each benchmark group is the prefix of
#  benchmark_name before the last "-".
#
# Synopsis
#  ./scripts/print_group_table_size.py SUMMARY_JSON --format {text,latex}
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
"""Print benchmark-group observation-table size tables in plain text or LaTeX."""

from __future__ import annotations

import argparse
import sys
from decimal import Decimal
from pathlib import Path

from print_group_summary_table import (
    benchmark_group_name,
    format_decimal_average,
)
from print_summary_table import (
    ALGORITHMS,
    PLACEHOLDER,
    SummaryEntry,
    format_group_label,
    latex_escape,
    load_summary,
    natural_sort_key,
    select_latest_entries,
)


TABLE_METRICS = ("rows", "columns")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Load an experiment summary JSON file and print a benchmark-group "
            "table of average observation-table sizes in plain text or LaTeX."
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
                for metric in TABLE_METRICS:
                    row[f"{algorithm}.{metric}"] = PLACEHOLDER
                continue

            if len(entries) != expected_count:
                warnings.append(
                    f"warning: benchmark group {group_name!r} has "
                    f"{len(entries)}/{expected_count} results for {algorithm}"
                )

            for metric in TABLE_METRICS:
                row[f"{algorithm}.{metric}"] = format_decimal_average(
                    sum(getattr(entry, metric) for entry in entries), len(entries)
                )
        rows.append(row)

    return rows, warnings


def superior_cells(row: dict[str, str]) -> set[str]:
    winners: set[str] = set()

    for metric in TABLE_METRICS:
        values: dict[str, Decimal] = {}
        for algorithm in ALGORITHMS:
            key = f"{algorithm}.{metric}"
            value = row[key]
            if value == PLACEHOLDER:
                continue
            values[key] = Decimal(value)

        if not values:
            continue

        best_value = min(values.values())
        for key, value in values.items():
            if value == best_value:
                winners.add(key)

    return winners


def column_names() -> list[str]:
    columns = ["benchmark_group"]
    for algorithm in ALGORITHMS:
        for metric in TABLE_METRICS:
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
        r"\begin{tabular}{lrrrr}",
        r"\toprule",
        r"benchmark\_group & \multicolumn{2}{c}{learn-arta} & \multicolumn{2}{c}{nlstar-rta} \\",
        r"\cmidrule(lr){2-3}\cmidrule(lr){4-5}",
        r" & rows & columns & rows & columns \\",
        r"\midrule",
    ]

    for row in rows:
        winners = superior_cells(row)
        cells = [latex_escape(row["benchmark_group"])]
        for algorithm in ALGORITHMS:
            for metric in TABLE_METRICS:
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
