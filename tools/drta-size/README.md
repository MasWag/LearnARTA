# drta-size

`drta-size` computes raw NRTA JSON statistics (via a strong parser) and prepares for minimum-DRTA-size work through SFA conversion. The CLI outputs CSV rows, one per input file.

## CSV Columns

```text
file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,status
```

`status` is `"ok"` when the file parses cleanly and `"error:<reason>"` otherwise (e.g., ARTA-style conjunctive targets).

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
middle.json,3,6,2,1,1,error:contains ARTA-style conjunctive targets (...)
small.json,3,4,2,2,1,error:contains ARTA-style conjunctive targets (...)
...
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

The script builds `tools/drta-size` with Maven and then runs the shaded JAR. If `vendor/symbolicautomata/models/target` is missing, it runs `scripts/bootstrap-symbolicautomata.sh` first.

## Build And Test

```sh
mvn -f tools/drta-size/pom.xml test
mvn -f tools/drta-size/pom.xml clean package
```

## Supported NRTA Targets

The CLI and converter currently support these NRTA-style target types:

| Target format       | Meaning                                         | Status            |
|---------------------|--------------------------------------------------|-------------------|
| `"q1"`              | Primitive location target -- one transition to q1 | **Supported**     |
| `{"or": ["q1","q2"]}` | Disjunctive / nondeterministic branching        | **Supported**     |
| `{"const": false}`  | Dead-end edge (no successor transition)          | **Supported**     |

## Rejected Targets

The following target types are rejected by the NRTA-to-SFA conversion path:

- `{"and": ["q1","q2"]}` — This is an ARTA-style conjunctive (alternating) target. It belongs to the ARTA formalism, not NRTA. Files containing such targets will produce a CSV row with `status=error:...` and the CLI continues processing subsequent files.
- `{"const": true}` — Tautologous target has no meaningful interpretation in the NRTA location-target model and is rejected during parsing.

## SFA Alphabet Element

The SFA alphabet element is a **timed letter** `(symbol, delay)`, represented by `TimedLetter` (not `List<TimedLetter>`). A timed word is `List<TimedLetter>`. The underlying `BooleanAlgebra<TimedPredicate, TimedLetter>` domain uses single `TimedLetter` elements; each transition guard is a `TimedPredicate` over that domain.

## Minimum DRTA Minimization

Minimum DRTA minimization (`SFA.minimize`) is **not yet added** to the CLI. The current scope ends at conversion from NRTA JSON to SFA, with conjunctive-target rejection and parse-time error reporting fully implemented. The `SFA.minimize` call is reserved for a future milestone.

## Directory Layout

```text
tools/drta-size/
  pom.xml
  README.md
  src/main/java/drta/DrtSize.java
  src/main/java/drta/NrtaToSfaConverter.java       (parser + SFA converter)
  src/main/java/drta/Timed*.java                    (TimedLetter, TimedPredicate, etc.)
  src/test/java/drta/*.java
  src/test/resources/drta/example.json
```

Related scripts:

```text
scripts/bootstrap-symbolicautomata.sh
scripts/min-drta-sizes.sh
```

## Architecture

The data flow is:

```
NRTA JSON file  -->  DrtSize.processFile()  -->  CSV row
                          |
                    NrtaToSfaConverter.parseNrtasJson()
                          |   <-- detects {"and":...} and {const:true} errors
                  ParsedNrta + raw transitions
                          |
                    NrtaToSfaConverter.toSfa()
                          |   <-- rejects conjunctive targets here too
                  SFA<TimedPredicate, TimedLetter>
```

The parser (`parseNrtasJson`) separates parsing from conversion so that:
1. Files with conjunctive targets can produce error CSV rows without crashing the whole batch.
2. The converter gets a strongly-typed `ParsedNrta` structure rather than raw JSON manipulation.
3. Parse/conversion errors include file name and transition context for debugging.
