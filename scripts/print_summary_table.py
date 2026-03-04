#!/usr/bin/env python3
####################################################
# Name
#  print_summary_table.py
#
# Description
#  Load an experiment summary JSON file and print a benchmark comparison
#  table in plain text or LaTeX.
#
# Synopsis
#  ./scripts/print_summary_table.py SUMMARY_JSON --format {text,latex}
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
"""Print experiment summary tables in plain text or LaTeX."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from decimal import Decimal
from pathlib import Path
from typing import Any


ALGORITHMS = ("learn-arta", "nlstar-rta")
METRICS = ("eq_queries", "mem_queries", "num_states", "elapsed_time")
PLACEHOLDER = "-"
ID_PATTERN = re.compile(
    r"^(?P<suite>[A-Za-z0-9_]+)-(?P<benchmark>[A-Za-z0-9_-]+)-"
    r"(?P<algorithm>learn-arta|nlstar-rta)-(?P<date>\d{8})-(?P<time>\d{6})$"
)
ELAPSED_TIME_PATTERN = re.compile(r"^\d+(?::\d{2}){1,2}(?:\.\d{2})?$")
@dataclass(frozen=True)
class SummaryEntry:
    """Normalized subset of an experiment summary entry."""

    identifier: str
    benchmark_name: str
    algorithm: str
    timestamp: str
    eq_queries: int
    mem_queries: int
    num_states: int
    elapsed_time: str

    def value_for_metric(self, metric: str) -> str:
        return str(getattr(self, metric))


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Load an experiment summary JSON file and print a benchmark table "
            "in plain text or LaTeX."
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


def load_summary(path: Path) -> list[SummaryEntry]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"summary file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc

    if not isinstance(payload, list):
        raise ValueError("summary JSON must be a top-level array")

    return [normalize_entry(entry, index) for index, entry in enumerate(payload)]


def normalize_entry(entry: Any, index: int) -> SummaryEntry:
    if not isinstance(entry, dict):
        raise ValueError(f"entry #{index} must be an object")

    identifier = require_non_empty_string(entry, "id", index)
    benchmark_name = require_non_empty_string(entry, "benchmark_name", index)
    eq_queries = require_non_negative_int(entry, "eq_queries", index)
    mem_queries = require_non_negative_int(entry, "mem_queries", index)
    num_states = require_non_negative_int(entry, "num_states", index)
    elapsed_time = require_elapsed_time(entry, "elapsed_time", index)

    algorithm, id_benchmark_name, timestamp = extract_algorithm_and_timestamp(identifier)
    if benchmark_name != id_benchmark_name:
        raise ValueError(
            f"entry #{index} benchmark_name {benchmark_name!r} does not match id {identifier!r}"
        )

    return SummaryEntry(
        identifier=identifier,
        benchmark_name=benchmark_name,
        algorithm=algorithm,
        timestamp=timestamp,
        eq_queries=eq_queries,
        mem_queries=mem_queries,
        num_states=num_states,
        elapsed_time=elapsed_time,
    )


def require_non_empty_string(entry: dict[str, Any], field: str, index: int) -> str:
    value = entry.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"entry #{index} field {field!r} must be a non-empty string")
    return value


def require_non_negative_int(entry: dict[str, Any], field: str, index: int) -> int:
    value = entry.get(field)
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(
            f"entry #{index} field {field!r} must be a non-negative integer"
        )
    return value


def require_elapsed_time(entry: dict[str, Any], field: str, index: int) -> str:
    value = require_non_empty_string(entry, field, index)
    if not ELAPSED_TIME_PATTERN.match(value):
        raise ValueError(
            f"entry #{index} field {field!r} must match GNU time elapsed format"
        )
    return value


def extract_algorithm_and_timestamp(identifier: str) -> tuple[str, str, str]:
    match = ID_PATTERN.match(identifier)
    if match is None:
        raise ValueError(
            f"entry id {identifier!r} does not match the expected summary id pattern"
        )
    algorithm = match.group("algorithm")
    benchmark_name = match.group("benchmark")
    timestamp = f"{match.group('date')}-{match.group('time')}"
    return algorithm, benchmark_name, timestamp


def select_latest_entries(entries: list[SummaryEntry]) -> dict[tuple[str, str], SummaryEntry]:
    latest: dict[tuple[str, str], SummaryEntry] = {}
    for entry in entries:
        key = (entry.benchmark_name, entry.algorithm)
        current = latest.get(key)
        if current is None or entry.timestamp > current.timestamp:
            latest[key] = entry
    return latest


def build_rows(
    latest_entries: dict[tuple[str, str], SummaryEntry],
) -> tuple[list[dict[str, str]], list[str]]:
    benchmark_names = sorted(
        {benchmark_name for benchmark_name, _ in latest_entries},
        key=natural_sort_key,
    )
    rows: list[dict[str, str]] = []
    warnings: list[str] = []

    for benchmark_name in benchmark_names:
        row: dict[str, str] = {"benchmark_name": format_benchmark_label(benchmark_name)}
        for algorithm in ALGORITHMS:
            entry = latest_entries.get((benchmark_name, algorithm))
            if entry is None:
                warnings.append(
                    f"warning: benchmark {benchmark_name!r} is missing results for {algorithm}"
                )
                for metric in METRICS:
                    row[f"{algorithm}.{metric}"] = PLACEHOLDER
                continue
            for metric in METRICS:
                row[f"{algorithm}.{metric}"] = entry.value_for_metric(metric)
        rows.append(row)

    return rows, warnings


def natural_sort_key(value: str) -> list[int | str]:
    parts = re.split(r"(\d+)", value)
    return [int(part) if part.isdigit() else part for part in parts]


def format_group_label(group_name: str) -> str:
    parts = group_name.split("_")
    if len(parts) <= 1:
        return group_name
    return f"({','.join(parts)})"


def format_benchmark_label(benchmark_name: str) -> str:
    group_name, separator, suffix = benchmark_name.rpartition("-")
    if not separator:
        return format_group_label(benchmark_name)
    return f"{format_group_label(group_name)}-{suffix}"


def elapsed_time_to_centiseconds(value: str) -> int:
    time_part, _separator, centiseconds_part = value.partition(".")
    fields = [int(field) for field in time_part.split(":")]

    if len(fields) == 2:
        hours = 0
        minutes, seconds = fields
    elif len(fields) == 3:
        hours, minutes, seconds = fields
    else:
        raise ValueError(f"unexpected elapsed time format: {value!r}")

    centiseconds = int((centiseconds_part + "00")[:2])
    return ((hours * 60 + minutes) * 60 + seconds) * 100 + centiseconds


def comparison_value(metric: str, value: str) -> Decimal:
    if metric == "elapsed_time":
        return Decimal(elapsed_time_to_centiseconds(value))
    return Decimal(value)


def superior_cells(row: dict[str, str]) -> set[str]:
    winners: set[str] = set()

    for metric in METRICS:
        values: dict[str, Decimal] = {}
        for algorithm in ALGORITHMS:
            key = f"{algorithm}.{metric}"
            value = row[key]
            if value == PLACEHOLDER:
                continue
            values[key] = comparison_value(metric, value)

        if not values:
            continue

        best_value = min(values.values())
        for key, value in values.items():
            if value == best_value:
                winners.add(key)

    return winners


def column_names() -> list[str]:
    columns = ["benchmark_name"]
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
            if column == "benchmark_name":
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
        r"benchmark\_name & \multicolumn{4}{c}{learn-arta} & \multicolumn{4}{c}{nlstar-rta} \\",
        r"\cmidrule(lr){2-5}\cmidrule(lr){6-9}",
        r" & eq\_queries & mem\_queries & num\_states & elapsed\_time & eq\_queries & mem\_queries & num\_states & elapsed\_time \\",
        r"\midrule",
    ]

    for row in rows:
        winners = superior_cells(row)
        cells = [latex_escape(row["benchmark_name"])]
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


def latex_escape(value: str) -> str:
    replacements = {
        "\\": r"\textbackslash{}",
        "&": r"\&",
        "%": r"\%",
        "$": r"\$",
        "#": r"\#",
        "_": r"\_",
        "{": r"\{",
        "}": r"\}",
        "~": r"\textasciitilde{}",
        "^": r"\textasciicircum{}",
    }
    return "".join(replacements.get(char, char) for char in value)


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
