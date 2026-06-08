package drta;

import org.junit.Test;
import static org.junit.Assert.*;

import java.nio.file.*;

/**
 * Tests for DrtSize using the repository's own example files.
 */
public class DrtSizeTest {

    private static final String EXAMPLES_BASE =
        System.getProperty("repo.root", System.getProperty("user.dir") + "/../../../") + "examples/";

    @Test
    public void testParseSmallJson() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "small.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);

        assertEquals("small.json", row.file);
        assertEquals(3, row.nrtaLocations);
        assertEquals(4, row.nrtaTransitions);
        assertEquals(2, row.alphabetSize);
        assertEquals(2, row.initialCount);
        assertEquals(1, row.acceptingCount);
        assertEquals("ok", row.status);
    }

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

    @Test
    public void testParseMiddleJson() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "middle.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);

        assertEquals("middle.json", row.file);
        assertEquals(3, row.nrtaLocations);
        assertEquals(6, row.nrtaTransitions);
        assertEquals(2, row.alphabetSize);
        assertEquals(1, row.initialCount);
        assertEquals(1, row.acceptingCount);
        assertEquals("ok", row.status);
    }

    @Test
    public void testParseRunningJson() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "running.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);

        assertEquals("running.json", row.file);
        assertEquals(3, row.nrtaLocations);
        assertEquals(6, row.nrtaTransitions);
        assertEquals(2, row.alphabetSize);
        assertEquals(1, row.initialCount);
        assertEquals(1, row.acceptingCount);
        assertEquals("ok", row.status);
    }

    @Test
    public void testParseUntimedJson() throws Exception {
        Path file = Paths.get(EXAMPLES_BASE + "untimed.json");
        DrtSize.CsvRow row = DrtSize.processFile(file);

        assertEquals("untimed.json", row.file);
        assertEquals(3, row.nrtaLocations);
        assertEquals(5, row.nrtaTransitions);
        assertEquals(2, row.alphabetSize);
        assertEquals(1, row.initialCount);
        assertEquals(2, row.acceptingCount);
        assertEquals("ok", row.status);
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
        // The comma in the filename should be preserved (not split at the comma)
        assertTrue("filename should contain the comma", line.contains("\"a,b.json\""));
        // The raw CSV should not have "a","b.json" (no splitting)
        assertFalse("should not split on comma", line.contains("a,\"b.json\""));
    }
}
