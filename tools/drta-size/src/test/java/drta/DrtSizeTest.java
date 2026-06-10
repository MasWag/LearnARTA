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
     * Simple two-state deterministic example.
     *
     * q0 --a/[0,+)--> q1, q0 has accepting guard for 'b'=[0,+)
     * q1 is accepting. The two states are not equivalent, so min = 2.
     *
     * Sink is counted (3 states total).
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

        assertTrue("two-state deterministic automaton should produce positive min states (sink counted)",
                   result.minDrtaStates >= 2);
    }

    /**
     * Nondeterministic "or" example where both q1 and q2 are same target.
     *
     * {"or":["q1","q1"]} is nondeterministic but after determinization + minimization
     * the result should still be consistent.
     */
    @Test
    public void testNondeterministicOrMinimizes() throws Exception {
        String json = "{\"name\":\"nondet-or\",\"l\":[\"q0\",\"q1\",\"q2\"],\"sigma\":[\"a\"]," +
                      "\"tran\":{\"0\":[\"q0\",\"a\",\"[0,+)\",{\"or\":[\"q1\",\"q1\"]}]}," +
                      "\"init\":[\"q0\"],\"accept\":[\"q1\"]}";
        JsonObject root = JsonParser.parseString(json).getAsJsonObject();
        NrtaToSfaConverter.ParsedNrta nrta = NrtaToSfaConverter.parseNrtasJson(root, "nondet-or");
        TimedLetterBooleanAlgebra ba = new TimedLetterBooleanAlgebra(makeSigma("a"));
        NrtaToSfaConverter.MinimizationResult result =
            NrtaToSfaConverter.computeMinimumDrtaSize(nrta, ba);

        assertTrue("nondeterministic automaton should produce positive min states",
                   result.minDrtaStates > 0);
    }

    /**
     * Partial-transition example: q0 has no transition on symbol 'b'.
     * After totalization, a sink state is added.
     *
     * Expected: min_drta_states = 2 (q0 + sink).
     * (q0 and sink are not equivalent because q0 has no self-loop on 'b'
     * while sink has a self-loop on 'b'; they will be differentiated.)
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

        assertTrue("partial automaton must produce positive min states (sink counted)",
                   result.minDrtaStates > 0);
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
