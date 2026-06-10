package drta;

import org.junit.Test;
import org.junit.Before;
import static org.junit.Assert.*;

import java.util.*;

/**
 * Tests for TimedInterval covering parsing, containment, intersection,
 * and complement operations.
 *
 * All bounds use half-units (long) to avoid floating-point equality pitfalls.
 * Standard time integer t maps to t*2 half-units.
 */
public class TimedIntervalTest {

    private TimedInterval iv;

    // ---------- parsing tests ----------

    @Test
    public void testParseClosedInterval() {
        TimedInterval iv = TimedInterval.parse("[0,1]");
        assertFalse(iv.loOpen);
        assertFalse(iv.hiOpen);
        assertFalse(iv.hiInf);
        assertEquals(0L, iv.lo);          // 0*2 = 0 half-units
        assertEquals(2L, iv.hi);          // 1*2 = 2 half-units
    }

    @Test
    public void testParseHalfOpenInterval() {
        TimedInterval iv = TimedInterval.parse("[0,1)");
        assertFalse(iv.loOpen);
        assertTrue(iv.hiOpen);
        assertFalse(iv.hiInf);
        assertEquals(0L, iv.lo);
        assertEquals(2L, iv.hi);
    }

    @Test
    public void testParseOpenInterval() {
        TimedInterval iv = TimedInterval.parse("(0,2]");
        assertTrue(iv.loOpen);
        assertFalse(iv.hiOpen);
        assertFalse(iv.hiInf);
        assertEquals(0L, iv.lo);
        assertEquals(4L, iv.hi);
    }

    @Test
    public void testParseHalfOpenUpperInterval() {
        TimedInterval iv = TimedInterval.parse("[1,+)");
        assertFalse(iv.loOpen);
        assertTrue(iv.hiInf);
        assertEquals(2L, iv.lo);
    }

    @Test
    public void testParseOpenUpFromInterval() {
        TimedInterval iv = TimedInterval.parse("(3,+)");
        assertTrue(iv.loOpen);
        assertTrue(iv.hiInf);
        assertEquals(6L, iv.lo);
    }

    @Test
    public void testParseSinglePoint() {
        TimedInterval iv = TimedInterval.parse("[0,0]");
        assertFalse(iv.loOpen);
        assertFalse(iv.hiOpen);
        assertFalse(iv.hiInf);
        assertEquals(0L, iv.lo);
        assertEquals(0L, iv.hi);
    }

    @Test
    public void testInvalidIntervalThrows() {
        try {
            TimedInterval.parse(null);
            fail("Should throw for null");
        } catch (IllegalArgumentException e) {
            // expected
        }
        TimedInterval iv = TimedInterval.parse("[2,1]");
        assertNotNull(iv);
        // [2,1] in string time units => [4,2] half-units => empty
        assertTrue(iv.isEmpty());
    }

    @Test
    public void testToStringForClosedInterval() {
        TimedInterval iv = TimedInterval.closed(0, 2);
        assertEquals("[0,2]", iv.toString());
    }

    @Test
    public void testToStringForOpenInterval() {
        TimedInterval iv = TimedInterval.open(0, 2);
        assertEquals("(0,2)", iv.toString());
    }

    // ---------- containment tests ----------

    @Test
    public void testContainsPointInClosedInterval() {
        TimedInterval iv = TimedInterval.closed(0, 2);
        assertTrue(iv.containsPoint(0));
        assertTrue(iv.containsPoint(1));
        assertTrue(iv.containsPoint(2));
        assertFalse(iv.containsPoint(3));
        assertFalse(iv.containsPoint(-1));
    }

    @Test
    public void testContainsPointInOpenInterval() {
        TimedInterval iv = TimedInterval.open(0, 2);
        assertFalse(iv.containsPoint(0));
        assertTrue(iv.containsPoint(1));
        assertFalse(iv.containsPoint(2));
    }

    @Test
    public void testContainsPointInHalfOpenInterval() {
        TimedInterval iv = TimedInterval.closedOpen(0, 2);
        assertTrue(iv.containsPoint(0));
        assertTrue(iv.containsPoint(1));
        assertFalse(iv.containsPoint(2));
    }

    @Test
    public void testContainsPointInUpFromInterval() {
        TimedInterval iv = TimedInterval.upFrom(2);
        assertFalse(iv.containsPoint(0));
        assertFalse(iv.containsPoint(1));
        assertTrue(iv.containsPoint(2));
        assertTrue(iv.containsPoint(10));
    }

    @Test
    public void testWitnessPoint() {
        TimedInterval iv = TimedInterval.closed(0, 2);
        assertEquals(0L, iv.witnessPoint());

        TimedInterval ivOpen = TimedInterval.open(0, 2);
        assertEquals(1L, ivOpen.witnessPoint());

        TimedInterval empty = TimedInterval.closed(-1, -1);
        assertEquals(-1L, empty.witnessPoint());
    }

    @Test
    public void testContainsPointInSingleton() {
        TimedInterval iv = TimedInterval.closed(4, 4);
        assertFalse(iv.containsPoint(0));
        assertTrue(iv.containsPoint(4));
        assertFalse(iv.containsPoint(5));
    }

    // ---------- intersection tests ----------

    @Test
    public void testIntersectionOverlapping() {
        TimedInterval a = TimedInterval.closed(0, 4);
        TimedInterval b = TimedInterval.closedOpen(2, 6);
        TimedInterval inter = a.intersection(b);
        assertEquals(2L, inter.lo);
        assertEquals(false, inter.loOpen);
        assertEquals(4L, inter.hi);
        assertEquals(false, inter.hiOpen);
    }

    @Test
    public void testIntersectionDisjoint() {
        TimedInterval a = TimedInterval.closed(0, 2);
        TimedInterval b = TimedInterval.upFrom(4);
        TimedInterval inter = a.intersection(b);
        assertTrue(inter.isEmpty());
    }

    @Test
    public void testIntersectionAdjacentIntervals() {
        // [0,2] and (2,4] should be disjoint (point 2 included in first, excluded in second)
        TimedInterval a = TimedInterval.closed(0, 2);
        TimedInterval b = TimedInterval.openClosed(2, 4);
        TimedInterval inter = a.intersection(b);
        assertTrue(inter.isEmpty());
    }

    @Test
    public void testIntersectionSameInterval() {
        TimedInterval a = TimedInterval.closed(0, 4);
        TimedInterval b = TimedInterval.closed(0, 4);
        TimedInterval inter = a.intersection(b);
        assertEquals(a, inter);
    }

    @Test
    public void testIntersectionWithItself() {
        TimedInterval a = TimedInterval.closed(5, 10);
        TimedInterval inter = a.intersection(a);
        assertEquals(a, inter);
    }

    @Test
    public void testIntersectionEmptyInterval() {
        TimedInterval empty = TimedInterval.closed(-1, -1);
        TimedInterval a = TimedInterval.closed(0, 4);
        assertEquals(empty, a.intersection(empty));
        assertEquals(empty, empty.intersection(a));
    }

    @Test
    public void testIntersectionPartialOverlap() {
        TimedInterval a = TimedInterval.closed(0, 4);
        TimedInterval b = TimedInterval.closed(2, 6);
        TimedInterval inter = a.intersection(b);
        assertEquals(2L, inter.lo);
        assertEquals(4L, inter.hi);
        assertFalse(inter.loOpen);
        assertFalse(inter.hiOpen);
    }

    @Test
    public void testIntersectionInfinity() {
        TimedInterval a = TimedInterval.upFrom(3);
        TimedInterval b = TimedInterval.closed(0, 5);
        TimedInterval inter = a.intersection(b);
        assertEquals(3L, inter.lo);
        assertEquals(5L, inter.hi);
        assertFalse(inter.loOpen);
        assertFalse(inter.hiOpen);
    }

    // ---------- complement tests ----------

    @Test
    public void testComplementBasic() {
        TimedInterval iv = TimedInterval.closed(2, 6); // [2,6] in half-units
        TimedIntervalSet comp = iv.complement();
        assertFalse(comp.isEmpty());
        // Complement should have gap points: [0,1] and (6,+inf)
        assertTrue(comp.containsPoint(0));
        assertTrue(comp.containsPoint(1));
        assertFalse(comp.containsPoint(2));
        assertFalse(comp.containsPoint(4));
        assertFalse(comp.containsPoint(6));
        assertTrue(comp.containsPoint(7));
        assertTrue(comp.containsPoint(100));
    }

    @Test
    public void testComplementAtBound() {
        // [0,2] over R>=0 => complement is (2,+inf)
        TimedInterval iv = TimedInterval.closed(0, 2);
        TimedIntervalSet comp = iv.complement();
        assertFalse(comp.containsPoint(0));
        assertFalse(comp.containsPoint(1));
        assertFalse(comp.containsPoint(2));
        assertTrue(comp.containsPoint(3));
        assertTrue(comp.containsPoint(100));
    }

    @Test
    public void testComplementFullDomain() {
        TimedInterval iv = TimedInterval.upFrom(0);
        TimedIntervalSet comp = iv.complement();
        assertTrue(comp.isEmpty());
    }

    @Test
    public void testComplementOpenAtLowerBound() {
        TimedInterval iv = TimedInterval.openClosed(2, 6);
        TimedIntervalSet comp = iv.complement();
        assertTrue(comp.containsPoint(0));
        assertTrue(comp.containsPoint(1));
        // 2 is included in complement (iv excludes 2)
        assertTrue(comp.containsPoint(2));
        assertFalse(comp.containsPoint(4));
        assertFalse(comp.containsPoint(6));
        assertTrue(comp.containsPoint(7));
    }

    // ---------- union normalization tests ----------

    @Test
    public void testUnionOfAdjacentIntervals() {
        TimedIntervalSet a = TimedIntervalSet.normalize(
            Collections.singletonList(TimedInterval.closed(0, 2)));
        TimedIntervalSet b = TimedIntervalSet.normalize(
            Collections.singletonList(TimedInterval.closed(3, 5)));
        TimedIntervalSet union = a.union(b);
        // These are adjacent: [0,2] and [3,5] -> [0,5] since 2+1=3
        assertFalse(union.isEmpty());
        // After normalization, the union should merge adjacent intervals
        // [0,2] and [3,5] are NOT adjacent in half-units (gap between 2 and 3)
        // Actually in half-units, 2 and 3 are consecutive integers
        assertTrue(union.containsPoint(0));
        assertTrue(union.containsPoint(2));
        assertTrue(union.containsPoint(3));
        assertTrue(union.containsPoint(5));
    }

    @Test
    public void testUnionOfOverlappingIntervals() {
        TimedIntervalSet a = TimedIntervalSet.normalize(
            Collections.singletonList(TimedInterval.closed(0, 4)));
        TimedIntervalSet b = TimedIntervalSet.normalize(
            Collections.singletonList(TimedInterval.closed(2, 6)));
        TimedIntervalSet union = a.union(b);
        assertFalse(union.isEmpty());
        assertTrue(union.containsPoint(0));
        assertTrue(union.containsPoint(4));
        assertTrue(union.containsPoint(6));
    }

    @Test
    public void testIntersectionOfSets() {
        TimedIntervalSet a = TimedIntervalSet.normalize(
            Arrays.asList(
                TimedInterval.closed(0, 4),
                TimedInterval.closed(10, 14)
            ));
        TimedIntervalSet b = TimedIntervalSet.normalize(
            Collections.singletonList(TimedInterval.closed(2, 12)));
        TimedIntervalSet inter = a.intersection(b);
        assertFalse(inter.isEmpty());
        assertTrue(inter.containsPoint(2));
        assertTrue(inter.containsPoint(4));
        assertTrue(inter.containsPoint(10));
        assertTrue(inter.containsPoint(12));
    }

    @Test
    public void testIntersectingSetsAreDisjoint() {
        TimedIntervalSet a = TimedIntervalSet.normalize(
            Collections.singletonList(TimedInterval.closed(0, 2)));
        TimedIntervalSet b = TimedIntervalSet.normalize(
            Collections.singletonList(TimedInterval.closed(4, 6)));
        TimedIntervalSet inter = a.intersection(b);
        assertTrue(inter.isEmpty());
    }
}
