# drta-size

`drta-size` is a small Java utility for inspecting NRTA JSON examples and
exercising the Symbolic Automata conversion used for minimum-DRTA-size work.

The command-line output is CSV.  Each row reports the raw NRTA size fields that
are currently available from the input format:

```text
file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,status
```

## Prerequisites

- `git` with submodule support
- Java 8 or later
- Apache Maven 3.2 or later

## Quick Start

From the repository root:

```sh
# Bootstrap the Symbolic Automata submodule and install its models library.
./scripts/bootstrap-symbolicautomata.sh

# Build the utility and process every JSON file under examples/.
./scripts/min-drta-sizes.sh
```

Example output:

```text
file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,status
atomic-small.json,2,1,1,1,1,ok
middle.json,3,6,2,1,1,ok
running.json,3,6,2,1,1,ok
small.json,3,4,2,2,1,ok
untimed.json,3,5,2,1,2,ok
```

## Usage

```sh
# Default: process all JSON files under examples/.
./scripts/min-drta-sizes.sh

# Process specific files.
./scripts/min-drta-sizes.sh examples/small.json examples/running.json

# Process a directory.
./scripts/min-drta-sizes.sh examples

# Process anything matching a shell glob.
./scripts/min-drta-sizes.sh examples/*.json
```

The script builds `tools/drta-size` with Maven and then runs the shaded JAR.
If `vendor/symbolicautomata/models/target` is missing, it runs
`scripts/bootstrap-symbolicautomata.sh` first.

## Build And Test

```sh
mvn -f tools/drta-size/pom.xml test
mvn -f tools/drta-size/pom.xml clean package
```

The package step runs the same unit tests before producing the runnable JAR.

## What The Utility Does

1. Parses each NRTA JSON file directly with Gson, preserving the raw structure
   of fields such as `l`, `sigma`, `tran`, `init`, and `accept`.
2. Counts locations, transitions, alphabet entries, initial locations, and
   accepting locations.
3. Provides a tested NRTA-to-SFA converter in `NrtaToSfaConverter` that builds
   `automata.sfa.SFA<TimedPredicate, List<TimedLetter>>` values through the
   vendored Symbolic Automata API.

The parser intentionally does not reuse the `learn-arta-core` JSON importer,
because that importer canonicalizes overlapping guards during import.  This
utility keeps overlapping guards and Boolean target formulas visible so they
can be studied independently.

## Current Scope

The CLI currently reports raw NRTA CSV statistics.  The SFA conversion layer is
implemented and covered by tests, including overlapping intervals, Boolean
targets, multiple initial locations, and open/closed timed-interval boundaries.

The next step for minimum DRTA size reporting is to run determinization and
minimization on the converted SFA and add the resulting size to the CSV output.

## Directory Layout

```text
tools/drta-size/
  pom.xml
  README.md
  src/main/java/drta/DrtSize.java
  src/main/java/drta/NrtaToSfaConverter.java
  src/main/java/drta/Timed*.java
  src/test/java/drta/*.java
  src/test/resources/drta/example.json
```

Related scripts:

```text
scripts/bootstrap-symbolicautomata.sh
scripts/min-drta-sizes.sh
```

## Adding NRTA Format Support

The parser reads `l`, `sigma`, `tran`, `init`, and `accept` directly from the
raw JSON tree.  To support another field, update `DrtSize.processFile`, extend
`CsvRow` if the field should appear in the CSV, and add a focused test under
`tools/drta-size/src/test/java/drta`.
