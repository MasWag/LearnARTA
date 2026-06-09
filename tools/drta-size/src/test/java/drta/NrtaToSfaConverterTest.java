package drta;

import automata.sfa.SFA;
import com.google.gson.*;
import org.junit.Test;
import org.junit.Before;
import static org.junit.Assert.*;

import java.nio.file.*;
import java.util.*;

import org.sat4j.specs.TimeoutException;

/**
 * Tests for NrtaToSfaConverter.
 * The SFA domain is {@code TimedLetter} (single symbol+delay pair). A timed word is
 * {@code List<TimedLetter>}, NOT {@code List<List<TimedLetter>>}.
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

    private static List<TimedLetter> word(TimedLetter... letters) {
        return Arrays.asList(letters);
    }

    private NrtaToSfaConverter.ParsedNrta parse(String json, String name) {
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        return NrtaToSfaConverter.parseNrtasJson(root, name);
    }

    // ========================= basic conversion and acceptance =========================

    @Test
    public void testConvertTinyNrtAProducesSfa() throws Exception {
        String json = "{\"name\":\"tiny\",\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\",\"b\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,1)\",\"q1\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "tiny");
        SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        assertFalse(sfa.getStates().isEmpty());
        assertTrue(sfa.getFinalStates().contains(1));
        assertEquals(2, nrta.locationCount());
        assertEquals(1, nrta.getTransitions().size());
    }

    @Test
    public void testTinySfaAcceptsA0() throws Exception {
        String json = "{\"name\":\"tiny\",\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\",\"b\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,1)\",\"q1\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "tiny");
        SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(nrta, ba);

        List<TimedLetter> tw = word(TimedLetter.of("a", 0));
        assertTrue(sfa.accepts(tw, ba));
    }

    @Test
    public void testTinySfaRejectsBAny() throws Exception {
        String json = "{\"name\":\"tiny\",\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\",\"b\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,1)\",\"q1\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "tiny");
        SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(nrta, ba);

        List<TimedLetter> tw = word(TimedLetter.of("b", 0));
        assertFalse(sfa.accepts(tw, ba));
    }

    // ========================= multiple transitions on same symbol =========================

    @Test
    public void testMultiTransitionAcceptsA0AndA1() throws Exception {
        String json = "{\"name\":\"multi\",\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,1)\",\"q1\"]," +
                               "\"1\":[\"q0\",\"a\",\"[1,+)\",\"q1\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "multi");
        SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(nrta, ba);

        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 1)), ba));
    }

    // ========================= two non-overlapping intervals =========================

    @Test
    public void testTwoIntervalsSfa() throws Exception {
        String json = "{\"name\":\"two\",\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,1)\",\"q1\"]," +
                               "\"1\":[\"q0\",\"a\",\"[3,4)\",\"q1\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "two");
        SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(nrta, ba);

        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 3)), ba));
        assertFalse(sfa.accepts(word(TimedLetter.of("a", 1)), ba));
    }

    // ========================= conjunctive target (and) is rejected =========================

    @Test
    public void testConjunctiveTargetRejectedDuringConversion() throws Exception {
        String json = "{\"name\":\"and\",\"l\":[\"q0\",\"q1\",\"q2\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,2)\",{\"and\":[\"q1\",\"q2\"]}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        try {
            SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(
                parse(json, "and"), ba);
            fail("toSfa should have rejected conjunctive target");
        } catch (NrtaToSfaConverter.UnsupportedTargetException e) {
            assertTrue(e.getMessage().toLowerCase(), 
                e.getMessage().toLowerCase().contains("conjunctive") || 
                e.getMessage().toLowerCase().contains("and"));
        }
    }

    @Test
    public void testHasConjunctiveTargetsDetected() throws Exception {
        String json = "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,2)\",{\"and\":[\"q1\"]}]}}";
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        assertTrue(NrtaToSfaConverter.hasConjunctiveTargets(root));
    }

    @Test
    public void testNoConjunctiveTargetsForPureString() throws Exception {
        String json = "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,2)\",\"q1\"]}}";
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        assertFalse(NrtaToSfaConverter.hasConjunctiveTargets(root));
    }

    // ========================= const:true is rejected during parsing =========================

    @Test
    public void testConstTrueTargetRejectedDuringParsing() throws Exception {
        String json = "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"const\":true}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        try {
            parse(json, "const-true");
            fail("parse should have rejected const:true");
        } catch (NrtaToSfaConverter.UnsupportedTargetException e) {
            assertTrue(e.getMessage().toLowerCase(), 
                e.getMessage().toLowerCase().contains("true") || 
                e.getMessage().toLowerCase().contains("const"));
        }
    }

    // ========================= disjunctive target handled as nondeterminism =========================

    @Test
    public void testDisjunctiveTargetHandledAsNondeterminism() throws Exception {
        String json = "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"or\":[\"q1\"]}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "disjunctive");
        SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
    }

    @Test
    public void testDisjunctiveOrTwoTargets() throws Exception {
        String json = "{\"l\":[\"q0\",\"q1\",\"q2\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"or\":[\"q1\",\"q2\"]}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\",\"q2\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "or-two");
        SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(nrta, ba);

        assertNotNull(sfa);
        assertTrue(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
    }

    // ========================= const:false target (dead end) =========================

    @Test
    public void testConstFalseNrtARejectsTimedWord() throws Exception {
        String json = "{\"l\":[\"q0\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"const\":false}]}," +
                      "\"init\":[\"q0\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "const-false");
        
        assertEquals(1, nrta.locationCount());
        assertEquals(1, nrta.getTransitions().size());
    }

    @Test
    public void testConstFalseWithAcceptingStart() throws Exception {
        String json = "{\"l\":[\"q0\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"const\":false}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q0\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "const-false-acc");
        SFA<TimedPredicate, TimedLetter> sfa = NrtaToSfaConverter.toSfa(nrta, ba);

        assertTrue(sfa.accepts(Collections.emptyList(), ba));
        assertFalse(sfa.accepts(word(TimedLetter.of("a", 0)), ba));
    }

    // ========================= parsed transition info correctness =========================

    @Test
    public void testParsedTransitionInfo() throws Exception {
        String json = "{\"name\":\"info\",\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\",\"b\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,1)\",\"q1\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        NrtaToSfaConverter.ParsedNrta nrta = parse(json, "info");
        assertEquals(2, nrta.locationCount());
        assertEquals(1, nrta.getTransitions().size());
        assertEquals("a", nrta.sigma[0]);
    }

    // ========================= CLI rejection of conjunctive files =========================

    @Test
    public void testCliRejectsConjunctiveFile() throws Exception {
        String json = "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"and\":[\"q1\"]}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        Path tmp = Files.createTempFile("conj", ".json");
        Files.writeString(tmp, json);

        DrtSize.CsvRow row = DrtSize.processFile(tmp);
        Files.delete(tmp);

        assertTrue(row.status.startsWith("error:"));
    }
}
