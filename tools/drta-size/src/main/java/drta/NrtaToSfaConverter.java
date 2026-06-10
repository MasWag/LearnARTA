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
 * and are <b>not supported</b> in the NRTA-to-SFA path. Attempting to convert an NRTA
 * with conjunctive targets will raise {@link UnsupportedOperationException}.
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
                JsonArray arr = transitionElem.getAsJsonArray();
                if (arr == null || arr.size() < 4) continue;

                String sourceName = arr.get(0).getAsString();
                String symbol = arr.get(1).getAsString();
                String guardStr = arr.get(2).getAsString();
                JsonElement targetElem = arr.get(3);

                // Check for const:true early — throw immediately with context
                if (targetElem.isJsonObject()) {
                    JsonObject obj = targetElem.getAsJsonObject();
                    if (obj.has("const") && obj.get("const").getAsBoolean()) {
                        throw new UnsupportedTargetException(
                            fileName + ": transition '" + key + "' uses {\"const\": true}, which is unsupported in NRTA",
                            fileName, Integer.parseInt(key));
                    }
                }

                ParsedTransition trans = new ParsedTransition(sourceName, symbol, guardStr);
                
                boolean isDeadEnd;
                if (targetElem.isJsonObject()) {
                    JsonObject obj = targetElem.getAsJsonObject();
                    if (obj.has("const") && !obj.get("const").getAsBoolean()) {
                        isDeadEnd = true; // const:false
                    } else if (obj.has("and")) {
                        isDeadEnd = false; // conjunctive — caller will reject during conversion
                    } else {
                        isDeadEnd = false;
                    }
                } else {
                    // primitive string target
                    String loc = targetElem.getAsString();
                    Integer id = nameToId(loc, locations);
                    isDeadEnd = (id == -1);
                }

                trans.isDeadEnd = isDeadEnd;
                allTransitionList.add(trans);
            }
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
            if (arr == null || arr.size() < 4) { transIndex++; continue; }

            String sourceName = arr.get(0).getAsString();
            String sym = arr.get(1).getAsString();
            String guardStr = arr.get(2).getAsString();
            JsonElement targetElem = arr.get(3);

            int sourceId = nameToId(sourceName, nrta.locations);
            if (sourceId == -1) { transIndex++; continue; }

            sigmaSet.add(sym);

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
        if (element == null || element.isJsonNull()) return ParsedNrta.TargetInfo.empty();
        if (element.isJsonPrimitive()) {
            String loc = element.getAsString();
            Integer id = nameToId(loc, locs);
            if (id == -1) return ParsedNrta.TargetInfo.empty();
            return new ParsedNrta.TargetInfo(Collections.singletonList(id), "primitive");
        }
        if (!element.isJsonObject()) {
            return ParsedNrta.TargetInfo.empty();
        }

        JsonObject obj = element.getAsJsonObject();
        if (obj.has("and")) {
            List<Integer> ids = new ArrayList<>();
            for (JsonElement e : obj.get("and").getAsJsonArray()) {
                if (e.isJsonPrimitive()) {
                    Integer id = nameToId(e.getAsString(), locs);
                    if (id != -1) ids.add(id);
                }
            }
            return ParsedNrta.TargetInfo.conjunctive(ids);
        }
        if (obj.has("or")) {
            List<Integer> ids = new ArrayList<>();
            for (JsonElement e : obj.get("or").getAsJsonArray()) {
                if (e.isJsonPrimitive()) {
                    Integer id = nameToId(e.getAsString(), locs);
                    if (id != -1) ids.add(id);
                }
            }
            return new ParsedNrta.TargetInfo(ids, "or");
        }
        if (obj.has("const")) {
            boolean val = obj.get("const").getAsBoolean();
            if (val) {
                throw new UnsupportedTargetException(
                    fileName + ": transition '" + transitionKey + "' uses {\"const\": true}, which is unsupported in NRTA",
                    fileName, Integer.parseInt(transitionKey));
            } else {
                return new ParsedNrta.TargetInfo(Collections.emptyList(), "const_false");
            }
        }
        return ParsedNrta.TargetInfo.empty();
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
}
