#!/usr/bin/env python3
####################################################
# Name
#  generate_drta_size_map.py
#
# Description
#  Generate a benchmark_name-to-minimum-DRTA-size CSV map by invoking the
#  existing Java drta-size utility through scripts/min-drta-sizes.sh.
#
# Synopsis
#  ./scripts/generate_drta_size_map.py --output logs/drta-sizes.csv
#
# Requirements
#  * Python 3
#  * Java and Maven prerequisites required by scripts/min-drta-sizes.sh
#
# Author
#  Masaki Waga
#
# License
#  Apache License, Version 2.0
####################################################
"""Generate a CSV map from benchmark names to minimum DRTA sizes."""

from __future__ import annotations

import argparse
import csv
import io
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


JAVA_CSV_HEADER = (
    "file",
    "nrta_locations",
    "nrta_transitions",
    "alphabet_size",
    "initial_count",
    "accepting_count",
    "sfa_states",
    "sfa_transitions",
    "min_drta_states",
    "min_drta_transitions",
    "time_ms",
    "status",
)
OUTPUT_COLUMNS = ("benchmark_name", "file", "drta_size", "status")


@dataclass(frozen=True)
class DrtaSizeRow:
    benchmark_name: str
    file: str
    drta_size: str
    status: str


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    repo_root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(
        description=(
            "Run the drta-size utility over benchmark JSON files and write a "
            "benchmark_name,file,drta_size,status CSV map."
        )
    )
    parser.add_argument(
        "inputs",
        nargs="*",
        default=[str(repo_root / "baselines" / "NLStarRTA" / "test")],
        help="Benchmark JSON files or directories to scan recursively",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Destination CSV path",
    )
    return parser.parse_args(argv)


def collect_json_files(inputs: list[str]) -> list[Path]:
    files: list[Path] = []
    for raw_input in inputs:
        path = Path(raw_input)
        if path.is_file():
            if path.suffix == ".json":
                files.append(path)
            continue
        if path.is_dir():
            files.extend(sorted(path.rglob("*.json"), key=lambda item: str(item)))
            continue
        raise ValueError(f"input path does not exist: {path}")
    return sorted(files, key=lambda item: natural_sort_key(str(item)))


def natural_sort_key(value: str) -> list[int | str]:
    import re

    parts = re.split(r"(\d+)", value)
    return [int(part) if part.isdigit() else part for part in parts]


def run_drta_size(repo_root: Path, files: list[Path]) -> str:
    if not files:
        raise ValueError("no benchmark JSON files found")

    command = [
        str(repo_root / "scripts" / "min-drta-sizes.sh"),
        *[str(path) for path in files],
    ]
    completed = subprocess.run(
        command,
        cwd=repo_root,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise ValueError(
            "drta-size command failed with exit code "
            f"{completed.returncode}: {completed.stderr.strip()}"
        )
    return completed.stdout


def parse_drta_size_output(output: str) -> list[DrtaSizeRow]:
    lines = output.splitlines()
    header_line = ",".join(JAVA_CSV_HEADER)
    try:
        header_index = lines.index(header_line)
    except ValueError as exc:
        raise ValueError("drta-size output did not contain the expected CSV header") from exc

    csv_text = "\n".join(lines[header_index:])
    reader = csv.DictReader(io.StringIO(csv_text))
    rows: list[DrtaSizeRow] = []
    seen: set[str] = set()

    for row_number, row in enumerate(reader, start=2):
        filename = (row.get("file") or "").strip()
        status = (row.get("status") or "").strip()
        benchmark_name = Path(filename).stem
        min_drta_states = (row.get("min_drta_states") or "").strip()
        drta_size = min_drta_states if status == "ok" else ""

        if not filename:
            raise ValueError(f"drta-size CSV row {row_number} has empty file")
        if not benchmark_name:
            raise ValueError(f"drta-size CSV row {row_number} has empty benchmark name")
        if benchmark_name in seen:
            raise ValueError(f"duplicate benchmark in drta-size output: {benchmark_name}")
        if status == "ok" and not drta_size.isdecimal():
            raise ValueError(
                f"drta-size CSV row {row_number} has invalid min_drta_states"
            )

        seen.add(benchmark_name)
        rows.append(
            DrtaSizeRow(
                benchmark_name=benchmark_name,
                file=filename,
                drta_size=drta_size,
                status=status,
            )
        )

    return rows


def write_output_csv(path: Path, rows: list[DrtaSizeRow]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=OUTPUT_COLUMNS)
        writer.writeheader()
        for row in sorted(rows, key=lambda item: natural_sort_key(item.benchmark_name)):
            writer.writerow(
                {
                    "benchmark_name": row.benchmark_name,
                    "file": row.file,
                    "drta_size": row.drta_size,
                    "status": row.status,
                }
            )


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo_root = Path(__file__).resolve().parents[1]

    try:
        files = collect_json_files(args.inputs)
        output = run_drta_size(repo_root, files)
        rows = parse_drta_size_output(output)
        write_output_csv(Path(args.output), rows)
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
