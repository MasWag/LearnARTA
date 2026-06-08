package drta;

import automata.sfa.SFA;
import automata.sfa.SFAEpsilon;
import automata.sfa.SFAInputMove;
import automata.sfa.SFAMove;
import com.google.gson.*;
import org.sat4j.specs.TimeoutException;

import java.nio.file.*;
import java.util.*;

/**
 * NrtaToSfaConverter converts a parsed NRTA JSON document into a symbolic
 * finite automaton (SFA) using the TimedLetterBooleanAlgebra.
 *
 * Conversion rules:
 *   - Each NRTA location `l[i]` maps to state `i`.
 *   - Each NRTA transition
 *       [source, symbol, guard, targetFormula]
 *     maps to an SFAInputMove from state(source) to state(targetFormulaStates)
 *     with predicate guard = {symbol -> TimedIntervalSet(parsed from guard)}.
 *     If targetFormula is a Boolean formula (and/or/const), the target states
 *     are extracted and individual SFAInputMoves are added for each.
 *   - If the init field is a list, the first element becomes the initial state.
 *     If it is a formula, a fresh initial state with epsilon transitions is added.
 *   - All NRTA accepting states are passed to SFA.MkSFA as final states.
 *
 * Nondeterminism is preserved: overlapping guards produce multiple SFAInputMoves
 * because we do NOT canonicalize the guards before building the SFA.
 */
public class NrtaToSfaConverter {

    /** Parsed data extracted from NRTA JSON (milestone-1 parser output). */
    public static class ParsedNrta {
        public final String name;
        public final List<String> locations;       // l
        public final Map<String, JsonElement> transitions;   // tran
        public final String[] sigma;               // alphabet
        public final boolean[] isInitial;
        public final int[] initialLocations;        // -1 if formula init
        public final boolean[] isAccepting;
        public final int[] acceptingLocations;      // -1 if formula accept (not supported yet)

        public ParsedNrta(String name, List<String> locations,
                          Map<String, JsonElement> transitions,
                          String[] sigma,
                          int[] initialLocations,
                          int[] acceptingLocations) {
            this.name = name;
            this.locations = locations;
            this.transitions = Collections.unmodifiableMap(new LinkedHashMap<>(transitions));
            this.sigma = sigma;
            this.isInitial = initToBoolArray(initialLocations, locations.size());
            this.isAccepting = acceptToBoolArray(acceptingLocations, locations.size());
            this.initialLocations = initialLocations;
            this.acceptingLocations = acceptingLocations;
        }

        private static boolean[] initToBoolArray(int[] locs, int n) {
            boolean[] result = new boolean[n];
            for (int i : locs) {
                if (i >= 0 && i < n) result[i] = true;
            }
            return result;
        }

        private static boolean[] acceptToBoolArray(int[] locs, int n) {
            boolean[] result = new boolean[n];
            for (int i : locs) {
                if (i >= 0 && i < n) result[i] = true;
            }
            return result;
        }
    }

    /** Convert ParsedNrta to an SFA<TimedPredicate, List<TimedLetter>>. */
    public static SFA<TimedPredicate, List<TimedLetter>> toSfa(ParsedNrta nrta,
                                                                 TimedLetterBooleanAlgebra ba)
            throws TimeoutException {
        return toSfaInternal(nrta, ba);
    }

    private static SFA<TimedPredicate, List<TimedLetter>> toSfaInternal(ParsedNrta nrta,
                                                                          TimedLetterBooleanAlgebra ba)
            throws TimeoutException {
        int n = nrta.locations.size();
        Collection<SFAMove<TimedPredicate, List<TimedLetter>>> moves = new ArrayList<>();

        int initialState;
        if (nrta.initialLocations.length == 1 && nrta.initialLocations[0] >= 0) {
            initialState = nrta.initialLocations[0];
        } else {
            initialState = n;
            for (int i : nrta.initialLocations) {
                if (i >= 0 && i < n) {
                    moves.add(new SFAEpsilon<TimedPredicate, List<TimedLetter>>(initialState, i));
                }
            }
        }

        Set<Integer> finalStates = new LinkedHashSet<>();
        for (int i : nrta.acceptingLocations) {
            if (i >= 0 && i < n) {
                finalStates.add(i);
            }
        }

        Set<String> sigmaSet = new LinkedHashSet<>(ba.alphabet());

        for (Map.Entry<String, JsonElement> entry : nrta.transitions.entrySet()) {
            JsonArray arr = entry.getValue().getAsJsonArray();
            if (arr == null || arr.size() < 4) continue;

            String sourceName = arr.get(0).getAsString();
            String sym = arr.get(1).getAsString();
            String guardStr = arr.get(2).getAsString();

            Integer sourceId = nameToId(sourceName, nrta.locations, n);
            if (sourceId == null) continue;

            sigmaSet.add(sym);

            TargetInfo targets = extractTargets(arr.get(3), nrta);
            if (targets == null || targets.ids.isEmpty()) continue;

            TimedPredicate pred = TimedPredicate.fromGuard(sym, TimedInterval.parse(guardStr), sigmaSet);

            if (targets.isConjunctive) {
                // Conjunctive target (and): add one SFAInputMove per target state
                for (int target : targets.ids) {
                    moves.add(new SFAInputMove<TimedPredicate, List<TimedLetter>>(sourceId, target, pred));
                }
            } else if (targets.isDisjunctive) {
                // Disjunctive target (or): add one SFAInputMove per target state
                for (int target : targets.ids) {
                    moves.add(new SFAInputMove<TimedPredicate, List<TimedLetter>>(sourceId, target, pred));
                }
            } else if (targets.isConstFalse) {
                // Const false: no transitions (this transition is a dead-end)
                continue;
            } else {
                // Atomic target (single string location)
                for (int target : targets.ids) {
                    moves.add(new SFAInputMove<TimedPredicate, List<TimedLetter>>(sourceId, target, pred));
                }
            }
        }

        return SFA.MkSFA(moves, initialState, finalStates, ba, false, false, true);
    }

    private static Integer nameToId(String name, List<String> locs, int expectedN) {
        for (int i = 0; i < locs.size(); i++) {
            if (locs.get(i).equals(name)) return i;
        }
        return null;
    }

    /** Extract target state IDs from a JSON element that may be a string or Boolean formula. */
    private static TargetInfo extractTargets(JsonElement element, ParsedNrta nrta) {
        if (element == null || element.isJsonNull()) return new TargetInfo();
        if (element.isJsonPrimitive()) {
            String loc = element.getAsString();
            Integer id = nameToId(loc, nrta.locations, nrta.locations.size());
            if (id == null) return new TargetInfo();
            List<Integer> ids = new ArrayList<>();
            ids.add(id);
            return new TargetInfo(ids, false, false, false);
        }
        if (element.isJsonObject()) {
            JsonObject obj = element.getAsJsonObject();
            if (obj.has("and")) {
                List<Integer> ids = new ArrayList<>();
                for (JsonElement e : obj.get("and").getAsJsonArray()) {
                    if (e.isJsonPrimitive()) {
                        Integer id = nameToId(e.getAsString(), nrta.locations, nrta.locations.size());
                        if (id != null) ids.add(id);
                    }
                }
                return new TargetInfo(ids, true, false, false);
            }
            if (obj.has("or")) {
                List<Integer> ids = new ArrayList<>();
                for (JsonElement e : obj.get("or").getAsJsonArray()) {
                    if (e.isJsonPrimitive()) {
                        Integer id = nameToId(e.getAsString(), nrta.locations, nrta.locations.size());
                        if (id != null) ids.add(id);
                    }
                }
                return new TargetInfo(ids, false, true, false);
            }
            if (obj.has("const")) {
                boolean val = obj.get("const").getAsBoolean();
                return new TargetInfo(new ArrayList<>(), false, false, !val); // isConstFalse
            }
            if (obj.has("or")) return new TargetInfo(); // empty
        }
        return new TargetInfo();
    }

    private static class TargetInfo {
        final List<Integer> ids;
        final boolean isConjunctive;
        final boolean isDisjunctive;
        final boolean isConstFalse;

        TargetInfo() { this.ids = new ArrayList<>(); this.isConjunctive = false; this.isDisjunctive = false; this.isConstFalse = false; }
        TargetInfo(List<Integer> ids, boolean conj, boolean disj, boolean constFalse) {
            this.ids = ids; this.isConjunctive = conj; this.isDisjunctive = disj; this.isConstFalse = constFalse;
        }
    }
}
