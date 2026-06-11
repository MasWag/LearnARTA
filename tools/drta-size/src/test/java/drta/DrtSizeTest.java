package drta;

import org.junit.Test;
import org.junit.Before;
import static org.junit.Assert.*;

import java.nio.file.*;
import java.util.*;

import org.sat4j.specs.TimeoutException;

import com.google.gson.*;

/**
 * Tests for DrtSize using the repository's own example files plus
 * hand-written NRTA examples for minimization checks.
 */
public class DrtSizeTest {

    private static final String EXAMPLES_BASE =
        System.getProperty("repo.root", System.getProperty("user.dir") + "/../../") + "examples/";

    // ============ existing tests (unchanged) ============

    @Test
    public void testParseAtomicSmallJson() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "atomic-small.json");
        NrtaToSfaConverter.ParsedNrta parsed = DrtSize.parseForTest(file);
        assertEquals("atomic-small", parsed.name);
        assertEquals(2, parsed.locationCount());
        assertEquals(1, parsed.getTransitions().size());

        // Run the full pipeline to verify SFA + minimization work
        Set<String> sigmaSet = new LinkedHashSet<>(Arrays.asList(parsed.sigma));
        TimedLetterBooleanAlgebra ba = new TimedLetterBooleanAlgebra(sigmaSet);
        NrtaToSfaConverter.MinimizationResult result =
            NrtaToSfaConverter.computeMinimumDrtaSize(parsed, ba);

        assertTrue("min_drta_states must be positive", result.minDrtaStates > 0);
        assertTrue("sfa_states must be positive", result.sfaStates > 0);

        DrtSize.CsvRow row = DrtSize.processFile(file);
        assertEquals("atomic-small.json", row.file);
        assertEquals(2, row.nrtaLocations);
        assertEquals(1, row.nrtaTransitions);
        assertEquals(1, row.alphabetSize);
        assertEquals(1, row.initialCount);
        assertEquals(1, row.acceptingCount);
        assertEquals("ok", row.status);
        assertTrue("min_drta_states must be positive for supported file", row.minDrtaStates > 0);
        assertTrue("time_ms must be non-negative", row.timeMs >= 0);
    }

    @Test
    public void testConjunctiveTargetsProduceError() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "small.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);

        assertTrue(row.status.startsWith("error:"));
        assertTrue(row.status.toLowerCase(),
            row.status.toLowerCase().contains("conjunctive") ||
            row.status.toLowerCase().contains("and"));
        assertEquals(0, row.minDrtaStates);
    }

    @Test
    public void testMiddleJsonConjunctiveProduceError() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "middle.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);
        assertTrue(row.status.startsWith("error:"));
    }

    @Test
    public void testRunningJsonConjunctiveProduceError() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "running.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);
        assertTrue(row.status.startsWith("error:"));
    }

    @Test
    public void testUntimedJsonConjunctiveProduceError() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "untimed.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);
        assertTrue(row.status.startsWith("error:"));
    }

    @Test
    public void testRejectsMalformedJson() throws Exception {
        Path tmp = Files.createTempFile("bad", ".json");
        Files.writeString(tmp, "{ not valid json [[[");
        DrtSize.CsvRow row = DrtSize.processFile(tmp);
        Files.delete(tmp);
        assertTrue("should report error status, got: " + row.status,
            row.status.startsWith("error:"));
    }

    @Test
    public void testCliSmokeTestUnsupportedProducedError() throws Exception {
        Path tmp = Files.createTempFile("conj", ".json");
        String json = "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"and\":[\"q1\"]}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        Files.writeString(tmp, json);
        DrtSize.CsvRow row = DrtSize.processFile(tmp);
        Files.delete(tmp);
        assertTrue(row.status.startsWith("error:"));
        assertEquals(0, row.minDrtaStates);
    }

    // ============ minimization tests ============

    private static Set<String> makeSigma(String... symbols) {
        return new LinkedHashSet<>(Arrays.asList(symbols));
    }

    /**
     * One-state rejecting automaton (no accepting states).
     *
     * Expected: min_drta_states = 1.
     * SFA.MkSFA with empty finalStates returns getEmptySFA(ba) which is already
     * minimal (one-state, non-accepting, self-loop on True). getMinimalOf
     * short-circuits on isEmpty and returns getEmptySFA directly.
     */
    @Test
    public void testOneStateRejectingNoAcceptingStates() throws Exception {
        String json = "{\"name\":\"reject-1\",\"l\":[\"q0\"],\"sigma\":[\"a\"]," +
                      "\"tran\":[]," +
                      "\"init\":[\"q0\"]}";
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        NrtaToSfaConverter.ParsedNrta nrta = NrtaToSfaConverter.parseNrtasJson(root, "reject-1");
        TimedLetterBooleanAlgebra ba = new TimedLetterBooleanAlgebra(makeSigma("a"));
        NrtaToSfaConverter.MinimizationResult result =
            NrtaToSfaConverter.computeMinimumDrtaSize(nrta, ba);

        assertEquals("empty-language automaton short-circuits to getEmptySFA (1 state)",
                     1, result.minDrtaStates);
    }

    /**
     * One-state accepting automaton with self-loop on full alphabet/time domain.
     *
     * Expected: min_drta_states = 1 (no sink needed because the single state
     * already covers all symbols via its self-loop guard = True).
     */
    @Test
    public void testOneStateAcceptingSelfLoopFullAlphabet() throws Exception {
        String json = "{\"name\":\"accept-1\",\"l\":[\"q0\"],\"sigma\":[\"a\",\"b\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",\"q0\"]," +
                               "\"1\":[\"q0\",\"b\",\"[0,+)\",\"q0\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q0\"]}";
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        NrtaToSfaConverter.ParsedNrta nrta = NrtaToSfaConverter.parseNrtasJson(root, "accept-1");
        TimedLetterBooleanAlgebra ba = new TimedLetterBooleanAlgebra(makeSigma("a", "b"));
        NrtaToSfaConverter.MinimizationResult result =
            NrtaToSfaConverter.computeMinimumDrtaSize(nrta, ba);

        assertEquals("fully-covering accepting automaton should minimize to 1 state",
                     1, result.minDrtaStates);
    }

    /**
     * Simple deterministic example for language b* a.
     *
     * Expected: min_drta_states = 3. The count includes q0, accepting q1,
     * and the totalization sink reached after missing transitions.
     */
    @Test
    public void testTwoStateDeterministic() throws Exception {
        String json = "{\"name\":\"det-2\",\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\",\"b\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",\"q1\"]," +
                               "\"1\":[\"q0\",\"b\",\"[0,+)\",\"q0\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        NrtaToSfaConverter.ParsedNrta nrta = NrtaToSfaConverter.parseNrtasJson(root, "det-2");
        TimedLetterBooleanAlgebra ba = new TimedLetterBooleanAlgebra(makeSigma("a", "b"));
        NrtaToSfaConverter.MinimizationResult result =
            NrtaToSfaConverter.computeMinimumDrtaSize(nrta, ba);

        assertEquals("b* a requires start, accepting state, and counted sink",
                     3, result.minDrtaStates);
    }

    /**
     * Nondeterministic "or" example for the one-letter language {a}.
     *
     * Expected: min_drta_states = 3. Determinization reaches the accepting
     * subset {q1,q2}; the totalization sink is counted.
     */
    @Test
    public void testNondeterministicOrMinimizes() throws Exception {
        String json = "{\"name\":\"nondet-or\",\"l\":[\"q0\",\"q1\",\"q2\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"or\":[\"q1\",\"q2\"]}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        NrtaToSfaConverter.ParsedNrta nrta = NrtaToSfaConverter.parseNrtasJson(root, "nondet-or");
        TimedLetterBooleanAlgebra ba = new TimedLetterBooleanAlgebra(makeSigma("a"));
        NrtaToSfaConverter.MinimizationResult result =
            NrtaToSfaConverter.computeMinimumDrtaSize(nrta, ba);

        assertEquals("single-symbol nondeterministic language counts accepting subset and sink",
                     3, result.minDrtaStates);
    }

    /**
     * Partial-transition example: q0 has no transition on symbol 'b'.
     * After totalization, a sink state is added.
     *
     * Expected: min_drta_states = 3. The count includes q0, accepting q1,
     * and the totalization sink reached from missing transitions.
     */
    @Test
    public void testPartialTransitionsSinkCounted() throws Exception {
        String json = "{\"name\":\"partial\",\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\",\"b\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",\"q1\"]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        NrtaToSfaConverter.ParsedNrta nrta = NrtaToSfaConverter.parseNrtasJson(root, "partial");
        TimedLetterBooleanAlgebra ba = new TimedLetterBooleanAlgebra(makeSigma("a", "b"));
        NrtaToSfaConverter.MinimizationResult result =
            NrtaToSfaConverter.computeMinimumDrtaSize(nrta, ba);

        assertEquals("partial automaton counts q0, q1, and totalization sink",
                     3, result.minDrtaStates);
    }

    // ============ strict validation errors ============

    private static DrtSize.CsvRow processJson(String prefix, String json) throws Exception {
        Path tmp = Files.createTempFile(prefix, ".json");
        Files.writeString(tmp, json);
        try {
            return DrtSize.processFile(tmp);
        } finally {
            Files.delete(tmp);
        }
    }

    private static void assertErrorContains(DrtSize.CsvRow row, String expected) {
        assertTrue("expected error status, got: " + row.status,
            row.status.startsWith("error:"));
        assertTrue("expected status to contain '" + expected + "', got: " + row.status,
            row.status.toLowerCase().contains(expected.toLowerCase()));
        assertEquals(0, row.minDrtaStates);
        assertEquals(-1, row.minDrtaTransitions);
    }

    @Test
    public void testRejectsUnknownSourceLocation() throws Exception {
        DrtSize.CsvRow row = processJson("bad-source",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":[\"qx\",\"a\",\"[0,+)\",\"q1\"]}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "unknown source location");
        assertEquals(2, row.nrtaLocations);
        assertEquals(1, row.nrtaTransitions);
    }

    @Test
    public void testRejectsUnknownPrimitiveTargetLocation() throws Exception {
        DrtSize.CsvRow row = processJson("bad-target",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",\"qx\"]}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "unknown target location");
    }

    @Test
    public void testRejectsUnknownTargetInsideOr() throws Exception {
        DrtSize.CsvRow row = processJson("bad-or-target",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"or\":[\"q1\",\"qx\"]}]}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "inside 'or'");
    }

    @Test
    public void testRejectsTransitionSymbolOutsideSigma() throws Exception {
        DrtSize.CsvRow row = processJson("bad-symbol",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":[\"q0\",\"b\",\"[0,+)\",\"q1\"]}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "not declared in sigma");
    }

    @Test
    public void testRejectsMalformedTransitionArray() throws Exception {
        DrtSize.CsvRow row = processJson("bad-array",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\"]}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "fewer than 4");
    }

    @Test
    public void testRejectsNonArrayTransitionEntry() throws Exception {
        DrtSize.CsvRow row = processJson("bad-entry",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":{\"source\":\"q0\"}}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "not an array");
    }

    @Test
    public void testRejectsUnknownTargetObject() throws Exception {
        DrtSize.CsvRow row = processJson("bad-object",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"foo\":[\"q1\"]}]}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "unknown target object");
    }

    @Test
    public void testRejectsMalformedGuard() throws Exception {
        DrtSize.CsvRow row = processJson("bad-guard",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":[\"q0\",\"a\",\"not-an-interval\",\"q1\"]}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "malformed guard");
    }

    @Test
    public void testRejectsSemanticallyInvalidGuards() throws Exception {
        String[] badGuards = {
            "[2,1]",
            "[0,0)",
            "[-1,1]",
            "[0,+]",
        };
        for (String guard : badGuards) {
            DrtSize.CsvRow row = processJson("bad-guard",
                "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
                "\"tran\":{\"0\":[\"q0\",\"a\",\"" + guard + "\",\"q1\"]}," +
                "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
            assertErrorContains(row, "malformed guard");
        }
    }

    @Test
    public void testRejectsEmptyOr() throws Exception {
        DrtSize.CsvRow row = processJson("bad-empty-or",
            "{\"l\":[\"q0\",\"q1\"],\"sigma\":[\"a\"]," +
            "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"or\":[]}]}," +
            "\"init\":[\"q0\"],\"accept\":[\"q1\"]}");
        assertErrorContains(row, "'or' target array is empty");
    }

    // ============ CsvRow formatting ============

    @Test
    public void testCsvRowFormattingOkStatus() throws Exception {
        DrtSize.CsvRow row = new DrtSize.CsvRow(
            "test.json", 3, 5, 2, 1, 1, 4, 6, 2, -1, 50, "ok"
        );
        String line = row.toCsvLine();
        String[] parts = line.split(",");
        assertEquals("test.json", parts[0]);
        assertEquals("ok", parts[parts.length - 1]);
        assertEquals("2", parts[8]); // min_drta_states
    }

    @Test
    public void testCsvRowFormattingErrorStatus() {
        DrtSize.CsvRow row = new DrtSize.CsvRow(
            "bad.json", 0, 0, 0, 0, 0, "error:something"
        );
        String line = row.toCsvLine();
        String[] parts = line.split(",");
        assertEquals("bad.json", parts[0]);
        assertEquals("-1", parts[9]); // min_drta_transitions for error
    }

    @Test
    public void testCsvRowEscapesFilenameWithComma() {
        DrtSize.CsvRow row = new DrtSize.CsvRow(
            "a,b.json", 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, "ok"
        );
        String line = row.toCsvLine();
        assertTrue("filename should contain the comma", line.contains("\"a,b.json\""));
        assertFalse("should not split on comma", line.contains("a,\"b.json\""));
    }
}
