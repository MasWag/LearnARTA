package drta;

import automata.sfa.SFA;
import automata.sfa.SFAEpsilon;
import automata.sfa.SFAInputMove;
import automata.sfa.SFAMove;
import com.google.gson.*;
import org.sat4j.specs.TimeoutException;

import java.util.*;

/**
 * Converts a parsed NRTA JSON document into a symbolic finite automaton (SFA).
 *
 * <h3>NRTA target support</h3>
 * <ul>
 *   <li><b>Primitive target</b> (e.g. {@code "q1"}) &rarr; one transition to q1.</li>
 *   <li>{@code {"or": ["q1", "q2"]}} (disjunctive) &rarr; nondeterministic branching:
 *       one transition per listed target.</li>
 *   <li>{@code {"const": false}} &rarr; no successor transition (dead-end edge).</li>
 *   <li>{@code {"const": true}} &rarr; rejected with UnsupportedTargetException.
 *       Tautologous targets have no meaningful interpretation in NRTA location targeting.</li>
 * </ul>
 *
 * <h3>ARTA-style conjunctive targets</h3>
 * {@code {"and": [...]}} targets are ARTA (Alternating Regular Tree Automata) constructs
 * and are <b>not supported</b> in the NRTA-to-SFA path. Parsing an NRTA with
 * conjunctive targets will raise {@link UnsupportedOperationException}.
 */
public class NrtaToSfaConverter {

    /** One parsed NRTA transition with structural info. */
    public static class ParsedTransition {
        public final String sourceName, symbol, guardStr;
        public TimedInterval guardInterval;
        public boolean isDeadEnd;

        public ParsedTransition(String sourceName, String symbol, String guardStr) {
            this.sourceName = sourceName;
            this.symbol = symbol;
            this.guardStr = guardStr;
        }
    }

    /** Parsed data extracted from NRTA JSON (produced by parseNrtasJson). */
    public static class ParsedNrta {
        public final String name;
        public final List<String> locations;       // l
        public final String[] sigma;               // alphabet
        public final int[] initialLocations;
        public final int[] acceptingLocations;

        /** Parsed transitions corresponding to each "tran" entry. */
        private List<ParsedTransition> allTransitions;

        /** Raw transition JSON elements, keyed by tran-key, for re-scan during SFA conversion. */
        private Map<String, JsonElement> rawTransitions;

        public ParsedNrta(String name, List<String> locations,
                           String[] sigma,
                           int[] initialLocations,
                           int[] acceptingLocations) {
            this.name = name;
            this.locations = Collections.unmodifiableList(new ArrayList<>(locations));
            this.sigma = Objects.requireNonNull(sigma);
            this.initialLocations = initialLocations;
            this.acceptingLocations = acceptingLocations;
        }

        public void setTransitions(List<ParsedTransition> allTransitions) {
            this.allTransitions = allTransitions;
        }

        public List<ParsedTransition> getTransitions() {
            return allTransitions != null ? allTransitions : Collections.emptyList();
        }

        public void setRawTransitions(Map<String, JsonElement> raw) {
            this.rawTransitions = raw;
        }

        public int locationCount() {
            return locations.size();
        }


        /** Target info extracted from the JSON target field. */
        public static class TargetInfo {
            public final List<Integer> ids;
            public final String kind; // "primitive", "or", "const_false", "conjunctive"
            public boolean isConjunctive;

            public TargetInfo(List<Integer> ids, String kind) {
                this.ids = Collections.unmodifiableList(new ArrayList<>(ids));
                this.kind = kind;
                this.isConjunctive = "conjunctive".equals(kind);
            }

            public static TargetInfo empty() { return new TargetInfo(Collections.emptyList(), "none"); }
            public static TargetInfo conjunctive(List<Integer> ids) {
                return new TargetInfo(ids, "conjunctive");
            }
        }
    }

    /** Exception thrown when the NRTA contains unsupported targets (ARTA-style). */
    public static class UnsupportedTargetException extends UnsupportedOperationException {
        private final String fileName;
        private final int transitionIndex;

        public UnsupportedTargetException(String message, String fileName, int transitionIndex) {
            super(message);
            this.fileName = fileName;
            this.transitionIndex = transitionIndex;
        }

        public String getFileName() { return fileName; }
        public int getTransitionIndex() { return transitionIndex; }
    }

    /**
     * Result of running SFA minimization to compute minimum DRTA size.
     * <p>
     * {@code minDrtaTransitions} is -1 when the underlying SFA API does not
     * expose a meaningful transition count after minimization.  The state count
     * ({@code minDrtaStates}) is always meaningful.
     * </p>
     */
    public static class MinimizationResult {
        public final int sfaStates;
        public final int sfaTransitions;
        public final int minDrtaStates;
        public final int minDrtaTransitions;
        public final long timeMs;

        public MinimizationResult(int sfaStates, int sfaTransitions,
                                  int minDrtaStates, int minDrtaTransitions,
                                  long timeMs) {
            this.sfaStates = sfaStates;
            this.sfaTransitions = sfaTransitions;
            this.minDrtaStates = minDrtaStates;
            this.minDrtaTransitions = minDrtaTransitions;
            this.timeMs = timeMs;
        }
    }

    /**
     * Build an SFA from a parsed NRTA, minimize it, and return the result.
     *
     * <p>
     * The SFA minimization pipeline in {@code getMinimalOf} is:
     * {@code removeEpsilon -> determinize -> mkTotal (adds global sink state) -> partition refinement}.
     * The sink state is added by {@code mkTotal} before minimization, so
     * {@code minDrtaStates} counts ALL states in the minimized SFA, including
     * the totalization sink state required for complete deterministic semantics.
     * Do not subtract or exclude the sink state without an explicit convention.
     * </p>
     *
     * @throws TimeoutException if the underlying SAT solver times out
     */
    public static MinimizationResult computeMinimumDrtaSize(ParsedNrta nrta,
                                                            TimedLetterBooleanAlgebra ba)
            throws TimeoutException {
        long t0 = System.currentTimeMillis();

        SFA<TimedPredicate, TimedLetter> sfa = toSfa(nrta, ba);

        int sfaStates = sfa.stateCount();
        int sfaTransitions = sfa.getTransitions().size();

        SFA<TimedPredicate, TimedLetter> minimized = SFA.getMinimalOf(sfa, ba);

        int minStates = minimized.stateCount();
        int minTransitions = -1;

        long elapsed = System.currentTimeMillis() - t0;

        return new MinimizationResult(sfaStates, sfaTransitions,
                                      minStates, minTransitions, elapsed);
    }

    /**
     * Parse raw NRTA JSON into a ParsedNrta.
     */
    public static ParsedNrta parseNrtasJson(JsonObject root, String fileName) {
        List<String> locations = readLocations(root);
        int n = locations.size();

        String[] sigma;
        if (root.has("sigma") && root.get("sigma").isJsonArray()) {
            JsonArray sigmaArr = root.getAsJsonArray("sigma");
            sigma = new String[sigmaArr.size()];
            for (int i = 0; i < sigmaArr.size(); i++) {
                sigma[i] = sigmaArr.get(i).getAsString();
            }
        } else {
            sigma = new String[0];
        }

        // Parse init locations (required; matches Rust convention where "init" is a required field)
        if (!root.has("init") || !root.get("init").isJsonArray()) {
            String msg = "missing required field 'init'";
            throw new IllegalArgumentException(makeHelp(fileName, msg, "add an \"init\" array with at least one location name (e.g., \"init\":[\"q0\"])"));
        }
        JsonArray initArr = root.getAsJsonArray("init");
        if (initArr == null || initArr.size() == 0) {
            String msg = "'init' array is empty";
            throw new IllegalArgumentException(makeHelp(fileName, msg, "add at least one initial location name to the \"init\" array (e.g., \"init\":[\"q0\"])"));
        }
        List<Integer> initIds = new ArrayList<>();
        for (int i = 0; i < initArr.size(); i++) {
            String initLoc = initArr.get(i).getAsString();
            int locId = -1;
            for (int j = 0; j < n; j++) {
                if (locations.get(j).equals(initLoc)) {
                    locId = j;
                    break;
                }
            }
            if (locId < 0) {
                throw new IllegalArgumentException(fileName != null ? fileName + ": unknown initial location '" + initLoc + "' not in locations" : "unknown initial location '" + initLoc + "' not in locations");
            }
            initIds.add(locId);
        }
        int[] initialLocations = new int[initIds.size()];
        for (int i = 0; i < initIds.size(); i++) {
            initialLocations[i] = initIds.get(i);
        }

        // Parse accepting locations (unknown names are parse errors, matching Rust convention)
        List<Integer> acceptIds = new ArrayList<>();
        if (root.has("accept") && root.get("accept").isJsonArray()) {
            JsonArray accArr = root.getAsJsonArray("accept");
            for (int i = 0; i < accArr.size(); i++) {
                String loc = accArr.get(i).getAsString();
                int locId = -1;
                for (int j = 0; j < n; j++) {
                    if (locations.get(j).equals(loc)) {
                        locId = j;
                        break;
                    }
                }
                if (locId < 0) {
                    throw new IllegalArgumentException(fileName != null ? fileName + ": unknown accepting location '" + loc + "' not in locations" : "unknown accepting location '" + loc + "' not in locations");
                }
                acceptIds.add(locId);
            }
        }
        // Missing 'accept' -> empty accepting set (valid NRTA default)
        int[] acceptingLocations = new int[acceptIds.size()];
        for (int i = 0; i < acceptIds.size(); i++) {
            acceptingLocations[i] = acceptIds.get(i);
        }

        String name;
        if (root.has("name") && root.get("name").isJsonPrimitive()) {
            name = root.get("name").getAsString();
        } else {
            name = fileName != null ? fileName : "unknown";
        }

        ParsedNrta parsed = new ParsedNrta(name, locations, sigma, initialLocations, acceptingLocations);
        Map<String, JsonElement> rawTransitionsMap = new LinkedHashMap<>();

        // Parse transitions
        List<ParsedTransition> allTransitionList = new ArrayList<>();

        if (root.has("tran") && root.get("tran").isJsonObject()) {
            JsonObject tranObj = root.getAsJsonObject("tran");
            List<String> keys = new ArrayList<>(tranObj.keySet());
            Collections.sort(keys);
            for (String key : keys) {
                JsonElement transitionElem = tranObj.get(key);
                rawTransitionsMap.put(key, transitionElem);
                if (!transitionElem.isJsonArray()) {
                    throw validationError(fileName, key, "transition entry is not an array");
                }
                JsonArray arr = transitionElem.getAsJsonArray();
                if (arr.size() < 4) {
                    throw validationError(fileName, key,
                        "transition array has fewer than 4 elements");
                }

                String sourceName = requireString(arr.get(0), fileName, key, "source location");
                String symbol = requireString(arr.get(1), fileName, key, "symbol");
                String guardStr = requireString(arr.get(2), fileName, key, "guard");
                JsonElement targetElem = arr.get(3);

                if (nameToId(sourceName, locations) < 0) {
                    throw validationError(fileName, key,
                        "unknown source location '" + sourceName + "' not in locations");
                }
                if (!Arrays.asList(sigma).contains(symbol)) {
                    throw validationError(fileName, key,
                        "transition symbol '" + symbol + "' not declared in sigma");
                }

                TimedInterval guardInterval;
                try {
                    guardInterval = TimedInterval.parse(guardStr);
                } catch (RuntimeException e) {
                    throw validationError(fileName, key,
                        "malformed guard '" + guardStr + "': " + e.getMessage());
                }

                ParsedTransition trans = new ParsedTransition(sourceName, symbol, guardStr);
                trans.guardInterval = guardInterval;

                ParsedNrta.TargetInfo targets = extractTargets(targetElem, locations, n, key, fileName);
                trans.isDeadEnd = "const_false".equals(targets.kind);
                allTransitionList.add(trans);
            }
        } else if (root.has("tran") && root.get("tran").isJsonArray()
                && root.getAsJsonArray("tran").size() > 0) {
            throw new IllegalArgumentException(
                (fileName != null ? fileName + ": " : "")
                + "field 'tran' must be an object keyed by transition id, or an empty array");
        } else if (root.has("tran") && !root.get("tran").isJsonArray()) {
            throw new IllegalArgumentException(
                (fileName != null ? fileName + ": " : "")
                + "field 'tran' must be an object keyed by transition id");
        }

        parsed.setTransitions(allTransitionList);
        parsed.setRawTransitions(rawTransitionsMap);
        return parsed;
    }

    /** Detect any conjunctive targets in the raw JSON before full parsing. */
    public static boolean hasConjunctiveTargets(JsonObject root) {
        if (!root.has("tran") || !root.get("tran").isJsonObject()) return false;
        JsonObject tran = root.getAsJsonObject("tran");
        List<String> keys = new ArrayList<>(tran.keySet());
        Collections.sort(keys);
        for (String key : keys) {
            JsonElement elem = tran.get(key);
            JsonArray arr = elem.getAsJsonArray();
            if (arr == null || arr.size() < 4) continue;
            JsonElement targetElem = arr.get(3);
            if (targetElem.isJsonObject()) {
                JsonObject obj = targetElem.getAsJsonObject();
                if (obj.has("and")) return true;
            }
        }
        return false;
    }

    /** Convert to an SFA<TimedPredicate, TimedLetter>. */
    public static SFA<TimedPredicate, TimedLetter> toSfa(ParsedNrta nrta,
                                                         TimedLetterBooleanAlgebra ba)
            throws TimeoutException {
        return toSfaInternal(nrta, ba);
    }

    private static SFA<TimedPredicate, TimedLetter> toSfaInternal(ParsedNrta nrta,
                                                                  TimedLetterBooleanAlgebra ba)
            throws TimeoutException {
        int n = nrta.locationCount();
        if (n == 0) return SFA.MkSFA(Collections.emptyList(), 0, Collections.emptyList(), ba);

        Collection<SFAMove<TimedPredicate, TimedLetter>> moves = new ArrayList<>();
        int initialState;
        if (nrta.initialLocations.length == 1 && nrta.initialLocations[0] >= 0) {
            initialState = nrta.initialLocations[0];
        } else {
            initialState = n;
            for (int i : nrta.initialLocations) {
                if (i >= 0 && i < n) {
                    moves.add(new SFAEpsilon<TimedPredicate, TimedLetter>(initialState, i));
                }
            }
        }

        Set<Integer> finalStates = new LinkedHashSet<>();
        for (int i : nrta.acceptingLocations) {
            if (i >= 0 && i < n) {
                finalStates.add(i);
            }
        }

        // Collect full sigma for predicates
        Set<String> sigmaSet = new LinkedHashSet<>(ba.alphabet());

        // Build SFA from raw JSON transitions (they're stored in sorted key order)
        int transIndex = 0;
        for (Map.Entry<String, JsonElement> entry : nrta.rawTransitions.entrySet()) {
            String key = entry.getKey();
            JsonArray arr = entry.getValue().getAsJsonArray();

            String sourceName = arr.get(0).getAsString();
            String sym = arr.get(1).getAsString();
            String guardStr = arr.get(2).getAsString();
            JsonElement targetElem = arr.get(3);

            int sourceId = nameToId(sourceName, nrta.locations);

            ParsedNrta.TargetInfo targets = extractTargets(targetElem, nrta.locations, n, key, nrta.name);
            
            // Detect conjunctive targets and reject
            if (targets.isConjunctive) {
                throw new UnsupportedTargetException(
                    "transition '" + key + "' has conjunctive target ('and'): " + arr.get(3).toString(),
                    nrta.name, transIndex);
            }

            TimedPredicate pred = TimedPredicate.fromGuard(sym, TimedInterval.parse(guardStr), sigmaSet);

            if ("const_false".equals(targets.kind)) {
                continue; // dead-end
            } else if ("or".equals(targets.kind) || "primitive".equals(targets.kind)) {
                for (int target : targets.ids) {
                    if (target >= 0 && target < n) {
                        moves.add(new SFAInputMove<>(sourceId, target, pred));
                    }
                }
            }

            transIndex++;
        }

        return SFA.MkSFA(moves, initialState, finalStates, ba, false, false, true);
    }

    private static int nameToId(String name, List<String> locs) {
        if (name != null && locs != null) {
            for (int i = 0; i < locs.size(); i++) {
                if (locs.get(i).equals(name)) return i;
            }
        }
        return -1; // Not found sentinel
    }

    /** Extract target state IDs and kind from a JSON element. */
    private static ParsedNrta.TargetInfo extractTargets(JsonElement element, List<String> locs, int n,
                                                         String transitionKey, String fileName) {
        if (element == null || element.isJsonNull()) {
            throw validationError(fileName, transitionKey, "missing target");
        }
        if (element.isJsonPrimitive()) {
            String loc = element.getAsString();
            Integer id = nameToId(loc, locs);
            if (id == -1) {
                throw validationError(fileName, transitionKey,
                    "unknown target location '" + loc + "' not in locations");
            }
            return new ParsedNrta.TargetInfo(Collections.singletonList(id), "primitive");
        }
        if (!element.isJsonObject()) {
            throw validationError(fileName, transitionKey, "target is neither a location nor an object");
        }

        JsonObject obj = element.getAsJsonObject();
        if (obj.has("and")) {
            throw new UnsupportedTargetException(
                formatTransitionMessage(fileName, transitionKey,
                    "has unsupported ARTA-style conjunctive target ('and')"),
                fileName, transitionIndexFromKey(transitionKey));
        }
        if (obj.has("or")) {
            if (!obj.get("or").isJsonArray()) {
                throw validationError(fileName, transitionKey, "'or' target is not an array");
            }
            JsonArray arr = obj.getAsJsonArray("or");
            if (arr.size() == 0) {
                throw validationError(fileName, transitionKey, "'or' target array is empty");
            }
            List<Integer> ids = new ArrayList<>();
            for (JsonElement e : arr) {
                if (!e.isJsonPrimitive() || !e.getAsJsonPrimitive().isString()) {
                    throw validationError(fileName, transitionKey,
                        "'or' target contains a non-string location");
                }
                String loc = e.getAsString();
                Integer id = nameToId(loc, locs);
                if (id == -1) {
                    throw validationError(fileName, transitionKey,
                        "unknown target location '" + loc + "' inside 'or' not in locations");
                }
                ids.add(id);
            }
            return new ParsedNrta.TargetInfo(ids, "or");
        }
        if (obj.has("const")) {
            if (!obj.get("const").isJsonPrimitive()
                    || !obj.getAsJsonPrimitive("const").isBoolean()) {
                throw validationError(fileName, transitionKey, "'const' target is not boolean");
            }
            boolean val = obj.get("const").getAsBoolean();
            if (val) {
                throw new UnsupportedTargetException(
                    formatTransitionMessage(fileName, transitionKey,
                        "uses {\"const\": true}, which is unsupported in NRTA"),
                    fileName, transitionIndexFromKey(transitionKey));
            } else {
                return new ParsedNrta.TargetInfo(Collections.emptyList(), "const_false");
            }
        }
        throw validationError(fileName, transitionKey,
            "unknown target object " + obj.toString());
    }

    private static List<String> readLocations(JsonObject root) {
        List<String> locs = new ArrayList<>();
        if (root.has("l") && root.get("l").isJsonArray()) {
            for (JsonElement e : root.getAsJsonArray("l")) {
                if (e.isJsonPrimitive()) {
                    locs.add(e.getAsString());
                }
            }
        }
        return locs.isEmpty() ? Collections.emptyList() : locs;
    }

    private static String makeHelp(String fileName, String msg, String hint) {
        if (fileName != null) {
            return fileName + ": " + msg + "\n  Hint: " + hint;
        } else {
            return msg + "\n  Hint: " + hint;
        }
    }

    private static String requireString(JsonElement element, String fileName,
                                        String transitionKey, String fieldName) {
        if (element == null || !element.isJsonPrimitive()
                || !element.getAsJsonPrimitive().isString()) {
            throw validationError(fileName, transitionKey, fieldName + " is not a string");
        }
        return element.getAsString();
    }

    private static IllegalArgumentException validationError(String fileName,
                                                           String transitionKey,
                                                           String message) {
        return new IllegalArgumentException(
            formatTransitionMessage(fileName, transitionKey, message));
    }

    private static String formatTransitionMessage(String fileName,
                                                  String transitionKey,
                                                  String message) {
        String prefix = fileName != null ? fileName + ": " : "";
        return prefix + "transition '" + transitionKey + "': " + message;
    }

    private static int transitionIndexFromKey(String key) {
        try {
            return Integer.parseInt(key);
        } catch (NumberFormatException e) {
            return -1;
        }
    }
}
