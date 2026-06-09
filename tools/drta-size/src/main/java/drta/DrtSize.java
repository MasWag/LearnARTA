package drta;

import com.google.gson.*;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.stream.*;

/**
 * DrtSize -- parses NRTA JSON files and produces CSV output.
 *
 * The CLI now uses {@link NrtaToSfaConverter} for parsing, which:
 * <ul>
 *   <li>Supports NRTA targets: primitive location strings, {@code {"or": [...]}} (nondeterminism), and {@code {"const": false}}.</li>
 *   <li><b>Rejects</b> ARTA-style conjunctive target {@code {"and": [...]}} with status {@code "error" + reason}.</li>
 *   <li><b>Rejects</b> {@code {"const": true}} targets as they are meaningless in NRTA.</li>
 * </ul>
 *
 * CSV columns:
 *   file               -- input file path
 *   nrta_locations     -- number of locations (states)
 *   nrta_transitions   -- number of transitions
 *   alphabet_size      -- number of distinct input symbols (from sigma field)
 *   initial_count      -- number of initial states
 *   accepting_count    -- number of accepting states
 *   status             -- "ok" if parsing succeeded, "error:<reason>" otherwise
 */
public class DrtSize {

    public static void main(String[] args) throws Exception {
        System.out.println(
            "file,nrta_locations,nrta_transitions,alphabet_size,initial_count,accepting_count,status");

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

        try {
            JsonObject root = JsonParser.parseString(json).getAsJsonObject();

            // First check for conjunctive targets (ARTA-style) before parsing
            if (NrtaToSfaConverter.hasConjunctiveTargets(root)) {
                return new CsvRow(
                    filename,
                    0, 0, 0, 0, 0,
                    "error:contains ARTA-style conjunctive targets ({\"and\": [...]})"
                );
            }

            // Parse using the NRTA parser for full context and error messages
            NrtaToSfaConverter.ParsedNrta parsed = NrtaToSfaConverter.parseNrtasJson(root, filename);

            return new CsvRow(
                filename,
                parsed.locationCount(),
                parsed.getTransitions().size(),
                parsed.sigma.length,
                parsed.initialLocations.length,
                parsed.acceptingLocations.length,
                "ok"
            );

        } catch (NrtaToSfaConverter.UnsupportedTargetException e) {
            return new CsvRow(
                filename,
                0, 0, 0, 0, 0,
                "error:" + e.getMessage()
            );
        } catch (Exception e) {
            return new CsvRow(
                filename,
                0, 0, 0, 0, 0,
                "error:" + e.getMessage()
            );
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
        final String status;

        CsvRow(String file, int nrtaLocations, int nrtaTransitions,
               int alphabetSize, int initialCount, int acceptingCount,
               String status) {
            this.file = file;
            this.nrtaLocations = nrtaLocations;
            this.nrtaTransitions = nrtaTransitions;
            this.alphabetSize = alphabetSize;
            this.initialCount = initialCount;
            this.acceptingCount = acceptingCount;
            this.status = status;
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
                csvEscape(status)
            );
        }
    }
}
