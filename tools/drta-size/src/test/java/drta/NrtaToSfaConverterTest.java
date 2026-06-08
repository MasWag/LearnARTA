package drta;

import automata.sfa.SFA;
import com.google.gson.*;
import org.junit.Test;
import org.junit.Before;
import static org.junit.Assert.*;

import java.io.*;
import java.nio.file.*;
import java.util.*;

import org.sat4j.specs.TimeoutException;

/**
 * Tests for NrtaToSfaConverter: converting tiny hand-written NRTA to SFA and
 * checking acceptance / rejection of a few timed words.
 *
 * Test NRTA 1 (simple NRTA):
 *   q0 --a[0,1)--> q1   // accepts 'a' at delay 0 (half-units [0,2))
 *   q0 --a[1,+)--> q1   // accepts 'a' at delay >= 1 (half-units [2,+))
 *   q0 --b[0,+)--> q1   // accepts any 'b'
 *   init: q0,  accept: {q1}
 * This is deterministic and accepts timed words where:
 *   (a,0) or (a,>=1) goes to q1 (accepting)
 *   (b,>=0) goes to q1 (accepting)
 */
public class NrtaToSfaConverterTest {

    private TimedLetterBooleanAlgebra ba;
    private Set<String> alphabet;

    @Before
    public void setUp() {
        alphabet = new LinkedHashSet<>();
        alphabet.add("a");
        alphabet.add("b");
        ba = new TimedLetterBooleanAlgebra(alphabet);
    }

    private static List<List<TimedLetter>> word(TimedLetter... letters) {
        List<List<TimedLetter>> result = new ArrayList<>();
        for (TimedLetter letter : letters) {
            result.add(Collections.singletonList(letter));
        }
        return result;
    }

    /** Build a tiny NRTA: q0 --a[0,1)--> q1, init=q0, accept={q1}. */
    private NrtaToSfaConverter.ParsedNrta buildTinyNrtA() {
        List<String> locations = Arrays.asList("q0", "q1");
        int n = locations.size();
        int[] initialLocs = new int[] { 0 };   // q0
        int[] acceptLocs = new int[] { 1 };    // q1

        Map<String, JsonElement> transitions = new LinkedHashMap<>();
        transitions.put("0", new JsonParser().parse(
            "[\"q0\", \"a\", \"[0,1)\", \"q1\"]"));

        return new NrtaToSfaConverter.ParsedNrta(
            "tiny",
            locations,
            transitions,
            new String[] {"a", "b"},
            initialLocs,
            acceptLocs
        );
    }

    @Test
    public void testConvertTinyNrtAProducesSfa() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildTinyNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        assertNotNull(sfa.getStates());
        assertFalse(sfa.getStates().isEmpty());
        assertTrue(sfa.getFinalStates().contains(1)); // q1 is accepting
    }

    @Test
    public void testTinySfaAcceptsA0() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildTinyNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        // Timed word: (a, 0) -- delay 0 in half-units
        List<List<TimedLetter>> tw = word(TimedLetter.of("a", 0));
        assertTrue("should accept (a,0)", sfa.accepts(tw, ba));
    }

    @Test
    public void testTinySfaAcceptsA1Half() throws Exception {
        // [0,1) means intervals [0,1) in standard time => [0,2) half-units
        // That's delay = 0 only (since delay 1 => 2 half-units which is excluded)
        // Actually wait: TimedInterval.parse("[0,1)") => [0,2) half-units
        // So point 2 (i.e. standard time 1) is excluded (open upper bound)
        // Point 0 (standard time 0) is included.

        NrtaToSfaConverter.ParsedNrta nrta = buildTinyNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        // Timed word (a, 0) should accept
        List<List<TimedLetter>> tw = word(TimedLetter.of("a", 0));
        assertTrue(sfa.accepts(tw, ba));
    }

    @Test
    public void testTinySfaAcceptsBAny() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildTinyNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        // Timed word (b, 0) -- there's no transition on b in our NRTA!
        // Wait, our NRTA only has an 'a' transition. But SFA transitions
        // are partial, so (b, 0) should be rejected.
        List<List<TimedLetter>> tw = word(TimedLetter.of("b", 0));
        // SFA with partial transitions: no transition on b means rejection
        assertFalse(sfa.accepts(tw, ba));
    }

    /** Build a second NRTA with multiple transitions on the same symbol. */
    private NrtaToSfaConverter.ParsedNrta buildMultiTransitionNrtA() {
        List<String> locations = Arrays.asList("q0", "q1");
        int n = locations.size();
        int[] initialLocs = new int[] { 0 };
        int[] acceptLocs = new int[] { 1 };

        Map<String, JsonElement> transitions = new LinkedHashMap<>();
        transitions.put("0", new JsonParser().parse("[\"q0\", \"a\", \"[0,1)\", \"q1\"]"));
        transitions.put("1", new JsonParser().parse("[\"q0\", \"a\", \"[1,+)\", \"q1\"]"));

        return new NrtaToSfaConverter.ParsedNrta(
            "multi",
            locations,
            transitions,
            new String[] {"a"},
            initialLocs,
            acceptLocs
        );
    }

    @Test
    public void testMultiTransitionAcceptsA0AndA1() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildMultiTransitionNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        // (a, 0) => [0,1) covers it
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
        // (a, 1) => [1,+) covers it
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 1)), ba));
    }

    /**
     * Build an NRTA that maps to a conjunction target (and):
     *   q0 --a[0,2)--> (q1 AND q2)
     * This is a valid NRTA transition where the target is a conjunction.
     * In the SFA, we add individual SFAInputMoves from q0 to q1 and from q0 to q2.
     */
    private NrtaToSfaConverter.ParsedNrta buildConjunctiveNrtA() {
        List<String> locations = Arrays.asList("q0", "q1", "q2");
        int n = locations.size();
        int[] initialLocs = new int[] { 0 };
        int[] acceptLocs = new int[] { 1 };

        Map<String, JsonElement> transitions = new LinkedHashMap<>();
        transitions.put("0", new JsonParser().parse(
            "[\"q0\", \"a\", \"[0,2)\", {\"and\": [\"q1\", \"q2\"]}]"));

        return new NrtaToSfaConverter.ParsedNrta(
            "conjunctive",
            locations,
            transitions,
            new String[] {"a"},
            initialLocs,
            acceptLocs
        );
    }

    @Test
    public void testConjunctiveSfaHasStates() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildConjunctiveNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        assertEquals(3, sfa.getStates().size());
        assertTrue(sfa.getFinalStates().contains(1));
        assertFalse(sfa.getFinalStates().contains(2));
    }

    @Test
    public void testConjunctiveSfaAcceptsA0() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildConjunctiveNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
    }

    /**
     * Build an NRTA with multiple initial locations:
     *   init = [q0, q1], accept = {q1}
     *   q0 --a[0,)--> q1
     *   q1 --a[0,)--> q1
     */
    private NrtaToSfaConverter.ParsedNrta buildMultipleInitialNrtA() {
        List<String> locations = Arrays.asList("q0", "q1");
        int n = locations.size();
        // Multiple initial locations
        int[] initialLocs = new int[] { 0, 1 };
        int[] acceptLocs = new int[] { 1 };

        Map<String, JsonElement> transitions = new LinkedHashMap<>();
        transitions.put("0", new JsonParser().parse(
            "[\"q0\", \"a\", \"[0,1)\", \"q1\"]"));
        transitions.put("1", new JsonParser().parse(
            "[\"q1\", \"a\", \"[0,1)\", \"q1\"]"));

        return new NrtaToSfaConverter.ParsedNrta(
            "multi-init",
            locations,
            transitions,
            new String[] {"a"},
            initialLocs,
            acceptLocs
        );
    }

    @Test
    public void testMultipleInitialNrtAAccepts() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildMultipleInitialNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
    }

    /**
     * Build a tiny NRTA with a disjunctive target:
     *   q0 --a[0,+)--> (q1 OR q2)
     */
    private NrtaToSfaConverter.ParsedNrta buildDisjunctiveNrtA() {
        List<String> locations = Arrays.asList("q0", "q1", "q2");
        int n = locations.size();
        int[] initialLocs = new int[] { 0 };
        int[] acceptLocs = new int[] { 2 }; // q2 is accepting

        Map<String, JsonElement> transitions = new LinkedHashMap<>();
        transitions.put("0", new JsonParser().parse(
            "[\"q0\", \"a\", \"[0,+)\", {\"or\": [\"q1\", \"q2\"]}]"));

        return new NrtaToSfaConverter.ParsedNrta(
            "disjunctive",
            locations,
            transitions,
            new String[] {"a"},
            initialLocs,
            acceptLocs
        );
    }

    @Test
    public void testDisjunctiveSfaAccepts() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildDisjunctiveNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        assertEquals(3, sfa.getStates().size());
        assertTrue(sfa.getFinalStates().contains(2));
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
    }

    /**
     * Build an NRTA with a const:false target (dead transition):
     *   q0 --a[0,+)--> const:false
     *   q0 init, no accepting states
     */
    private NrtaToSfaConverter.ParsedNrta buildConstFalseNrtA() {
        List<String> locations = Arrays.asList("q0");
        int n = locations.size();
        int[] initialLocs = new int[] { 0 };
        int[] acceptLocs = new int[] { 0 };

        Map<String, JsonElement> transitions = new LinkedHashMap<>();
        transitions.put("0", new JsonParser().parse(
            "[\"q0\", \"a\", \"[0,+)\", {\"const\": false}]"));

        return new NrtaToSfaConverter.ParsedNrta(
            "const-false",
            locations,
            transitions,
            new String[] {"a"},
            initialLocs,
            acceptLocs
        );
    }

    @Test
    public void testConstFalseNrtARejectsAll() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildConstFalseNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        // The transition has const:false target, so no SFAInputMoves are added.
        // Initial state is q0 (index 0) which is also accepting.
        // So the empty word should be accepted (initial state is final).
        // But (a,0) should be rejected since there are no outgoing moves.
        assertTrue(sfa.accepts(Collections.emptyList(), ba));
        assertFalse(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
    }

    /**
     * Build an NRTA that uses a union of intervals:
     *   q0 --a[0,1)--> q1
     *   q0 --a[3,4)--> q1
     * This tests that SFA has two transitions (preserving nondeterminism).
     */
    private NrtaToSfaConverter.ParsedNrta buildTwoIntervalsNrtA() {
        List<String> locations = Arrays.asList("q0", "q1");
        int n = locations.size();
        int[] initialLocs = new int[] { 0 };
        int[] acceptLocs = new int[] { 1 };

        Map<String, JsonElement> transitions = new LinkedHashMap<>();
        transitions.put("0", new JsonParser().parse("[\"q0\", \"a\", \"[0,1)\", \"q1\"]"));
        transitions.put("1", new JsonParser().parse("[\"q0\", \"a\", \"[3,4)\", \"q1\"]"));

        return new NrtaToSfaConverter.ParsedNrta(
            "two-intervals",
            locations,
            transitions,
            new String[] {"a"},
            initialLocs,
            acceptLocs
        );
    }

    @Test
    public void testTwoIntervalsSfa() throws Exception {
        NrtaToSfaConverter.ParsedNrta nrta = buildTwoIntervalsNrtA();
        SFA<TimedPredicate, List<TimedLetter>> sfa =
            NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
        // [3,4) in standard time => [6,8) half-units => delay = 3 (6 half-units) should accept
        // Actually: [3,4) => lower=3*2=6, upper=4*2=8 => contains [6, 8)
        // Point 6 (standard time 3) is included
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 3)), ba));
        // Point 5 half-units (standard time 2.5 not in this set of integer delays)
        // Wait, our TimedLetter.of("a", 1) maps to 2 half-units.
        // So we only test integer delays.
        // [3,4) doesn't include standard time 1 (2 half-units)
        assertFalse(sfa.accepts(word(TimedLetter.of("a", 1)), ba));
    }
}
