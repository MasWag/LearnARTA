package drta;

import org.junit.Test;
import static org.junit.Assert.*;

import java.nio.file.*;

/**
 * Tests for DrtSize using the repository's own example files.
 *
 * Example files with {"and": ...} targets are ARTA-style and produce
 * error status rows from the CLI rather than ok rows.  Only pure NRTA
 * files (with primitive, "or", or const:false targets) produce ok rows.
 */
public class DrtSizeTest {

    private static final String EXAMPLES_BASE =
        System.getProperty("repo.root", System.getProperty("user.dir") + "/../../../") + "examples/";

    // ========== atomic-small.json is the only pure-NRTA file (all targets are primitive strings) =======

    @Test
    public void testParseAtomicSmallJson() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "atomic-small.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);

        assertEquals("atomic-small.json", row.file);
        assertEquals(2, row.nrtaLocations);
        assertEquals(1, row.nrtaTransitions);
        assertEquals(1, row.alphabetSize);
        assertEquals(1, row.initialCount);
        assertEquals(1, row.acceptingCount);
        assertEquals("ok", row.status);
    }

    // ========== conjunctive files produce error rows (not crashes) =======

    @Test
    public void testConjunctiveTargetsProduceError() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "small.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);

        // must not crash; reports error for unsupported target type
        assertTrue(row.status.startsWith("error:"));
        assertTrue(row.status.toLowerCase(), 
            row.status.toLowerCase().contains("conjunctive") || 
            row.status.toLowerCase().contains("and"));
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

    // ========== malformed JSON handling =======

    @Test
    public void testRejectsMalformedJson() throws Exception {
        Path tmp = Files.createTempFile("bad", ".json");
        Files.writeString(tmp, "{ not valid json [[[");
        DrtSize.CsvRow row = DrtSize.processFile(tmp);
        Files.delete(tmp);
        assertTrue("should report error status, got: " + row.status,
            row.status.startsWith("error:"));
    }

    // ========== CsvRow formatting =======

    @Test
    public void testCsvRowFormatting() {
        DrtSize.CsvRow row = new DrtSize.CsvRow(
            "test.json", 3, 5, 2, 1, 1, "ok"
        );
        String line = row.toCsvLine();
        assertTrue(line.startsWith("test.json"));
        assertTrue(line.endsWith(",ok"));
    }

    @Test
    public void testCsvRowEscapesFilenameWithComma() {
        DrtSize.CsvRow row = new DrtSize.CsvRow(
            "a,b.json", 0, 0, 0, 0, 0, "ok"
        );
        String line = row.toCsvLine();
        assertTrue("filename should contain the comma", line.contains("\"a,b.json\""));
        assertFalse("should not split on comma", line.contains("a,\"b.json\""));
    }
}
