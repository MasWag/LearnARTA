package drta;

import com.google.gson.*;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.Arrays;
import java.util.stream.*;

/**
 * DrtSize -- parses NRTA JSON files and produces CSV output.
 *
 * The CLI now uses {@link NrtaToSfaConverter} for parsing, which:
 * <ul>
 *   <li>Supports NRTA targets: primitive location strings, {@code {"or": [...]}} (nondeterminism), and {@code {"const": false}}.</li>
 *   <li><b>Rejects</b> ARTA-style conjunctive target {@code {"and": [...]}} with status {@code "error"}.
 *       See {@link NrtaToSfaConverter.UnsupportedTargetException}.</li>
 *   <li><b>Rejects</b> {@code {"const": true}} targets as they are meaningless in NRTA.</li>
 * </ul>
 *
 * <h3>Minimum DRTA size</h3>
 * For supported inputs the CLI builds an SFA, minimizes it, and reports
 * the count of all states in the minimized SFA as {@code min_drta_states}.
 * The minimize pipeline in {@code SFA.getMinimalOf} is
 * {@code removeEpsilon -> determinize -> mkTotal -> partition refinement}.
 * Since {@code mkTotal} adds a totalization sink state <em>before</em> minimization,
 * the reported {@code min_drta_states} <b>includes</b> that sink state.
 * Do not subtract or exclude the sink state without an explicit convention.
 *
 * <h3>CSV columns</h3>
 * {@code file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,sfa_states,sfa_transitions,min_drta_states,min_drta_transitions,time_ms,status}
 *
 * {@code min_drta_transitions} is -1 when the underlying symbolicautomata API
 * does not expose a meaningful post-minimization transition count.
 * Only the state count is currently guaranteed to be meaningful.
 */
public class DrtSize {

    public static void main(String[] args) throws Exception {
        System.out.println(
            "file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,sfa_states,sfa_transitions,min_drta_states,min_drta_transitions,time_ms,status");

        for (Path file : collectFiles(args)) {
            CsvRow row = processFile(file);
            System.out.println(row.toCsvLine());
        }
    }

    static List<Path> collectFiles(String[] args) throws IOException {
        String[] inputs = (args == null || args.length == 0) ? new String[] {"examples"} : args;
        List<Path> files = new ArrayList<>();

        for (String input : inputs) {
            Path path = Paths.get(input);
            if (Files.isRegularFile(path)) {
                files.add(path);
            } else if (Files.isDirectory(path)) {
                try (Stream<Path> stream = Files.list(path)) {
                    stream
                        .filter(p -> p.getFileName().toString().endsWith(".json"))
                        .sorted()
                        .forEach(files::add);
                }
            } else if (input.contains("*") || input.contains("?")) {
                Path parent = path.getParent() == null ? Paths.get(".") : path.getParent();
                PathMatcher matcher = FileSystems.getDefault().getPathMatcher("glob:" + input);
                try (Stream<Path> stream = Files.list(parent)) {
                    stream
                        .filter(matcher::matches)
                        .sorted()
                        .forEach(files::add);
                }
            } else {
                throw new NoSuchFileException(input);
            }
        }

        return files;
    }

    static CsvRow processFile(Path file) throws IOException {
        String json = new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
        String filename = file.getFileName().toString();
        CsvRow.Counts counts = CsvRow.Counts.empty();

        try {
            JsonObject root = JsonParser.parseString(json).getAsJsonObject();
            counts = CsvRow.Counts.fromRoot(root);

            // Parse using the NRTA parser for full context and error messages
            NrtaToSfaConverter.ParsedNrta parsed = NrtaToSfaConverter.parseNrtasJson(root, filename);

            // Build SFA and minimize
            Set<String> sigmaSet = new LinkedHashSet<>(Arrays.asList(parsed.sigma));
            TimedLetterBooleanAlgebra ba = new TimedLetterBooleanAlgebra(sigmaSet);
            NrtaToSfaConverter.MinimizationResult result =
                NrtaToSfaConverter.computeMinimumDrtaSize(parsed, ba);

            return new CsvRow(
                filename,
                parsed.locationCount(),
                parsed.getTransitions().size(),
                parsed.sigma.length,
                parsed.initialLocations.length,
                parsed.acceptingLocations.length,
                result.sfaStates,
                result.sfaTransitions,
                result.minDrtaStates,
                result.minDrtaTransitions,
                result.timeMs,
                "ok"
            );

        } catch (NrtaToSfaConverter.UnsupportedTargetException e) {
            return CsvRow.error(filename, counts, e.getMessage());
        } catch (Exception e) {
            return CsvRow.error(filename, counts, e.getMessage());
        }
    }

    /** Exposes parsed NRTA for test use */
    static NrtaToSfaConverter.ParsedNrta parseForTest(Path file) throws IOException {
        String json = new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        return NrtaToSfaConverter.parseNrtasJson(root, file.getFileName().toString());
    }

    static class CsvRow {
        final String file;
        final int nrtaLocations;
        final int nrtaTransitions;
        final int alphabetSize;
        final int initialCount;
        final int acceptingCount;
        // SFA stats (before minimization)
        final int sfaStates;
        final int sfaTransitions;
        // Minimum DRTA size
        final int minDrtaStates;
        final int minDrtaTransitions;
        final long timeMs;
        final String status;

        static class Counts {
            final int nrtaLocations;
            final int nrtaTransitions;
            final int alphabetSize;
            final int initialCount;
            final int acceptingCount;

            Counts(int nrtaLocations, int nrtaTransitions, int alphabetSize,
                   int initialCount, int acceptingCount) {
                this.nrtaLocations = nrtaLocations;
                this.nrtaTransitions = nrtaTransitions;
                this.alphabetSize = alphabetSize;
                this.initialCount = initialCount;
                this.acceptingCount = acceptingCount;
            }

            static Counts empty() {
                return new Counts(0, 0, 0, 0, 0);
            }

            static Counts fromRoot(JsonObject root) {
                return new Counts(
                    arraySize(root, "l"),
                    transitionCount(root),
                    arraySize(root, "sigma"),
                    arraySize(root, "init"),
                    arraySize(root, "accept")
                );
            }

            private static int arraySize(JsonObject root, String field) {
                return root.has(field) && root.get(field).isJsonArray()
                    ? root.getAsJsonArray(field).size()
                    : 0;
            }

            private static int transitionCount(JsonObject root) {
                if (!root.has("tran")) {
                    return 0;
                }
                JsonElement tran = root.get("tran");
                if (tran.isJsonObject()) {
                    return tran.getAsJsonObject().size();
                }
                if (tran.isJsonArray()) {
                    return tran.getAsJsonArray().size();
                }
                return 0;
            }
        }

        CsvRow(String file, int nrtaLocations, int nrtaTransitions,
                int alphabetSize, int initialCount, int acceptingCount,
                int sfaStates, int sfaTransitions,
                int minDrtaStates, int minDrtaTransitions,
                long timeMs, String status) {
            this.file = file;
            this.nrtaLocations = nrtaLocations;
            this.nrtaTransitions = nrtaTransitions;
            this.alphabetSize = alphabetSize;
            this.initialCount = initialCount;
            this.acceptingCount = acceptingCount;
            this.sfaStates = sfaStates;
            this.sfaTransitions = sfaTransitions;
            this.minDrtaStates = minDrtaStates;
            this.minDrtaTransitions = minDrtaTransitions;
            this.timeMs = timeMs;
            this.status = status;
        }

        // Legacy constructor for errors
        CsvRow(String file, int nrtaLocations, int nrtaTransitions,
                int alphabetSize, int initialCount, int acceptingCount,
                String status) {
            this(file, nrtaLocations, nrtaTransitions, alphabetSize,
                 initialCount, acceptingCount,
                 0, 0, 0, -1, 0, status);
        }

        static CsvRow error(String file, Counts counts, String message) {
            String reason = message == null || message.trim().isEmpty()
                ? "unknown error"
                : message.replace('\n', ' ').replace('\r', ' ').trim();
            return new CsvRow(
                file,
                counts.nrtaLocations,
                counts.nrtaTransitions,
                counts.alphabetSize,
                counts.initialCount,
                counts.acceptingCount,
                "error:" + reason
            );
        }

        static String csvEscape(String s) {
            if (s == null || s.contains(",") || s.contains("\"") || s.contains("\n")) {
                return "\"" + s.replace("\"", "\"\"") + "\"";
            }
            return s;
        }

        String toCsvLine() {
            int ic = (initialCount >= 0) ? initialCount : -1;
            return String.join(",",
                csvEscape(file),
                String.valueOf(nrtaLocations),
                String.valueOf(nrtaTransitions),
                String.valueOf(alphabetSize),
                String.valueOf(ic),
                String.valueOf(acceptingCount),
                String.valueOf(sfaStates),
                String.valueOf(sfaTransitions),
                String.valueOf(minDrtaStates),
                String.valueOf(minDrtaTransitions),
                String.valueOf(timeMs),
                csvEscape(status)
            );
        }
    }
}
