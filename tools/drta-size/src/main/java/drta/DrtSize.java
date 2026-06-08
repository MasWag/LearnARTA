package drta;

import com.google.gson.*;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.stream.*;

/**
 * DrtSize -- raw NRTA JSON parser and CSV reporter.
 *
 * This tool parses NRTA JSON files and outputs statistics about each automaton
 * as CSV rows.  It preserves the raw structure of the input JSON (including
 * nondeterministic Boolean targets and overlapping guards) and does not
 * canonicalize or rewrite the automaton.
 *
 * CSV columns:
 *   file               -- input file path
 *   nrta_locations     -- number of locations (states)
 *   nrta_transitions   -- number of transitions
 *   alphabet_size      -- number of distinct input symbols (from sigma field)
 *   initial_count      -- number of initial states
 *   accepting_count    -- number of accepting states
 *   status             -- "ok" if parsing succeeded, "error" with reason
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

            int locations = 0;
            if (root.has("l") && root.get("l").isJsonArray()) {
                locations = root.getAsJsonArray("l").size();
            }

            int transitions = 0;
            if (root.has("tran") && root.get("tran").isJsonObject()) {
                transitions = root.getAsJsonObject("tran").size();
            }

            int alphabetSize = 0;
            if (root.has("sigma") && root.get("sigma").isJsonArray()) {
                alphabetSize = root.getAsJsonArray("sigma").size();
            }

            int initialCount = 0;
            if (root.has("init")) {
                JsonElement initEl = root.get("init");
                if (initEl.isJsonArray()) {
                    initialCount = initEl.getAsJsonArray().size();
                } else if (initEl.isJsonObject()) {
                    initialCount = -1;
                }
            }

            int acceptingCount = 0;
            if (root.has("accept") && root.get("accept").isJsonArray()) {
                acceptingCount = root.getAsJsonArray("accept").size();
            }

            return new CsvRow(
                filename,
                locations,
                transitions,
                alphabetSize,
                initialCount,
                acceptingCount,
                "ok"
            );

        } catch (Exception e) {
            return new CsvRow(
                filename,
                0,
                0,
                0,
                0,
                0,
                "error:" + e.getMessage()
            );
        }
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
