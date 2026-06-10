# drta-size

`drta-size` parses NRTA JSON files, builds an SFA, minimizes it, and reports the size of the minimum deterministic runtime tree automaton (DRTA). The CLI outputs CSV rows, one per input file.

## CSV Columns

```text
file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,sfa_states,sfa_transitions,min_drta_states,min_drta_transitions,time_ms,status
```

| Column | Description |
|--------|-------------|
| `file` | Input file path |
| `nrta_locations` | Number of locations (states) in the NRTA |
| `nrta_transitions` | Number of transitions in the NRTA |
| `alphabet_size` | Number of distinct input symbols |
| `initial_count` | Number of initial states |
| `accepting_count` | Number of accepting states |
| `sfa_states` | Number of SFA states after construction (before minimization) |
| `sfa_transitions` | Number of SFA transitions after construction (before minimization) |
| `min_drta_states` | Number of states in the minimized DRTA (the primary reported value; positive integer when `status=ok`) |
| `min_drta_transitions` | Number of transitions in the minimized DRTA (currently `-1`; only the state count is meaningful) |
| `time_ms` | Elapsed time in milliseconds for the full pipeline (parse + SFA + minimize) |
| `status` | `"ok"` when parsing, SFA construction, and minimization all succeeded; `"error:<reason>"` otherwise |

## How `min_drta_states` Is Computed

For each supported (non-ARTA-style) input file, the CLI runs this pipeline:

```
NRTA JSON --> ParsedNrta --> SFA (via toSfa) --> SFA.minimize (via getMinimalOf) --> min_drta_states
```

The `SFA.getMinimalOf` method performs:

1. **`removeEpsilon`** -- removes epsilon moves
2. **`determinize`** -- subset construction from NFA to DFA
3. **`mkTotal`** -- adds a global sink/totalization state to make the DFA complete
4. **Partition refinement** -- Hopcroft-style minimization of the total DFA

### Sink-State Convention

`min_drta_states` **includes** the totalization sink state. The sink is added by `mkTotal` **before** minimization and is preserved in the minimized automaton when it is not equivalent to any other state. Do not subtract or exclude the sink state without an explicit convention.

### `min_drta_transitions`

Currently set to `-1`. The symbolicautomata API does not expose a meaningful post-minimization transition count in this workflow. Only `min_drta_states` is guaranteed to be meaningful.

## Rejected Targets

Files containing:

- `{"and": ["q1","q2"]}` -- ARTA-style conjunctive targets are **not supported**. The file produces a CSV row with `status=error:<reason>` and the batch continues.
- `{"const": true}` -- Tautologous target has no meaningful interpretation in NRTA and is rejected with `UnsupportedTargetException`.

## Unsupported ARTA-Style Conjunctive Targets

Files that contain `{"and": ...}` targets in their transitions are ARTA (Alternating Regular Tree Automata) constructs and are **not** converted to SFA. They produce a CSV row with `status=error` and the CLI continues processing subsequent files. This is the expected behavior.

## Strict Input Validation

The parser is strict because `status=ok` rows are intended for paper numbers.
Malformed transitions and undeclared names are rejected instead of skipped.

- Every transition entry under `tran` must be an array with at least four elements: source location, symbol, guard, and target.
- Every transition source and every primitive target location must be declared in `l`.
- Every target listed inside `{"or": [...]}` must be declared in `l`; empty `or` arrays are rejected.
- Every transition symbol must be declared in `sigma`. The Boolean-algebra alphabet is exactly the declared `sigma`; transitions do not expand it.
- Guards must parse as timed intervals.
- Unknown target objects, such as `{"foo": ...}`, produce `status=error:<reason>`.
- `{"and": ...}` targets remain unsupported and produce `status=error:<reason>`.

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
file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,sfa_states,sfa_transitions,min_drta_states,min_drta_transitions,time_ms,status
atomic-small.json,2,1,1,1,1,2,1,3,-1,14,ok
middle.json,3,6,2,1,1,0,0,0,-1,0,error:middle.json: transition '1': has unsupported ARTA-style conjunctive target ('and')
small.json,3,4,2,2,1,0,0,0,-1,0,error:small.json: transition '1': has unsupported ARTA-style conjunctive target ('and')
```

## Usage

```sh
# Default: process all JSON files under examples/.
./scripts/min-drta-sizes.sh

# Process specific files.
./scripts/min-drta-sizes.sh examples/atomic-small.json

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
|---------------------|-------------------------------------------------|-------------------|
| `"q1"`              | Primitive location target -- one transition to q1 | **Supported**     |
| `{"or": ["q1","q2"]}` | Disjunctive / nondeterministic branching      | **Supported**     |
| `{"const": false}`  | Dead-end edge (no successor transition)         | **Supported**     |

## SFA Alphabet Element

The SFA alphabet element is a **timed letter** `(symbol, delay)`, represented by `TimedLetter` (not `List<TimedLetter>`). A timed word is `List<TimedLetter>`. The underlying `BooleanAlgebra<TimedPredicate, TimedLetter>` domain uses single `TimedLetter` elements; each transition guard is a `TimedPredicate` over that domain.

## Interval Operations

Timed interval operations are exact over the internal half-unit representation: standard-time integer `t` is stored as `2*t`, and open/closed endpoints are tracked explicitly. `TimedIntervalSet` canonicalizes finite unions before Boolean-algebra operations so equivalent representations such as `[0,1) U [1,+)` and `[0,+)` compare equal.

## Tests

Tests for the minimization pipeline are in `src/test/java/drta/DrtSizeTest.java`:

- `testOneStateRejectingNoAcceptingStates` -- one-state rejecting, `min_drta_states = 1` (getEmptySFA short-circuit)
- `testOneStateAcceptingSelfLoopFullAlphabet` -- one-state accepting with full self-loops, `min_drta_states = 1`
- `testTwoStateDeterministic` -- deterministic language `b* a`, `min_drta_states = 3` (sink counted)
- `testNondeterministicOrMinimizes` -- nondeterministic one-letter language, `min_drta_states = 3` (sink counted)
- `testPartialTransitionsSinkCounted` -- partial transitions, `min_drta_states = 3` (sink counted)
- `testCliSmokeTestSupported` -- CLI on supported file produces `status=ok`, `min_drta_states > 0`
- `testCliSmokeTestUnsupportedProducedError` -- CLI on ARTA-style file produces `status=error`
- strict validation tests cover unknown locations, undeclared symbols, malformed transitions, malformed guards, unknown target objects, and empty `or`

## Directory Layout

```text
tools/drta-size/
  pom.xml
  README.md
  src/main/java/drta/DrtSize.java
  src/main/java/drta/NrtaToSfaConverter.java       (parser + SFA converter + MinimizationResult)
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
                           |   <-- strict transition validation
                   ParsedNrta + raw transitions
                           |
                     NrtaToSfaConverter.toSfa()
                           |   <-- uses exactly the declared sigma alphabet
                   SFA<TimedPredicate, TimedLetter>
                           |
                     NrtaToSfaConverter.computeMinimumDrtaSize(minimize)
                           |
                   MinimizationResult (sfa_states, min_drta_states, time_ms, ...)
```

The parser (`parseNrtasJson`) separates parsing from conversion so that:
1. Malformed files can produce error CSV rows without crashing the whole batch.
2. The converter gets a strongly-typed `ParsedNrta` structure rather than raw JSON manipulation.
3. Parse/conversion errors include file name and transition context for debugging.

`computeMinimumDrtaSize` runs the minimization pipeline (`toSfa` + `getMinimalOf`) and returns a `MinimizationResult` containing pre-minimization SFA statistics, the minimized state count, elapsed time, and the status string.
