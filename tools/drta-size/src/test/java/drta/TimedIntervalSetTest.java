package drta;

import org.junit.Test;
import static org.junit.Assert.*;

import java.util.*;

/**
 * Tests for canonical finite unions of exact half-unit intervals.
 *
 * Bounds in this file are internal half-units. Standard-time 1 is half-unit 2.
 */
public class TimedIntervalSetTest {

    private static TimedIntervalSet set(TimedInterval... intervals) {
        return TimedIntervalSet.normalize(Arrays.asList(intervals));
    }

    private static void assertSingleInterval(
            TimedIntervalSet set,
            long lo,
            boolean loOpen,
            long hi,
            boolean hiInf,
            boolean hiOpen) {
        assertEquals(1, set.size());
        TimedInterval iv = set.intervals().get(0);
        assertEquals(lo, iv.lo);
        assertEquals(loOpen, iv.loOpen);
        assertEquals(hiInf, iv.hiInf);
        if (!hiInf) {
            assertEquals(hi, iv.hi);
            assertEquals(hiOpen, iv.hiOpen);
        }
    }

    @Test
    public void normalizeRemovesEmptyIntervals() {
        TimedIntervalSet s = set(TimedInterval.closed(-1, -1), TimedInterval.closed(0, 2));
        assertSingleInterval(s, 0, false, 2, false, false);
    }

    @Test
    public void normalizeMergesOverlappingIntervals() {
        TimedIntervalSet s = set(TimedInterval.closed(0, 4), TimedInterval.closed(2, 6));
        assertSingleInterval(s, 0, false, 6, false, false);
        assertTrue(s.containsPoint(0));
        assertTrue(s.containsPoint(6));
    }

    @Test
    public void normalizeMergesBoundaryTouchWithIncludedEndpoint() {
        TimedIntervalSet s = set(TimedInterval.closedOpen(0, 2), TimedInterval.closedOpen(2, 4));
        assertSingleInterval(s, 0, false, 4, false, true);
        assertTrue(s.containsPoint(2));
        assertFalse(s.containsPoint(4));
    }

    @Test
    public void normalizeDoesNotMergeOpenOpenBoundary() {
        TimedIntervalSet s = set(TimedInterval.closedOpen(0, 2), TimedInterval.openClosed(2, 4));
        assertEquals(2, s.size());
        assertFalse(s.containsPoint(2));
    }

    @Test
    public void normalizeDoesNotMergeRealGap() {
        TimedIntervalSet s = set(TimedInterval.closed(0, 0), TimedInterval.closed(2, 4));
        assertEquals(2, s.size());
        assertFalse(s.containsPoint(1));
    }

    @Test
    public void unionFiniteOverlappingIntervals() {
        TimedIntervalSet u = set(TimedInterval.closed(0, 4))
            .union(set(TimedInterval.closed(3, 6)));
        assertSingleInterval(u, 0, false, 6, false, false);
    }

    @Test
    public void unionFiniteAdjacentIntervals() {
        TimedIntervalSet u = set(TimedInterval.closedOpen(0, 2))
            .union(set(TimedInterval.closedOpen(2, 4)));
        assertSingleInterval(u, 0, false, 4, false, true);
    }

    @Test
    public void unionWithUnboundedRemainsUnbounded() {
        TimedIntervalSet u = set(TimedInterval.closed(0, 4))
            .union(set(TimedInterval.upFrom(2)));
        assertSingleInterval(u, 0, false, 0, true, false);
    }

    @Test
    public void unionOfTwoUnboundedIntervalsKeepsWidestLowerBound() {
        TimedIntervalSet u = set(TimedInterval.upFromOpen(2))
            .union(set(TimedInterval.upFrom(4)));
        assertSingleInterval(u, 2, true, 0, true, false);
    }

    @Test
    public void unionContainedIntervalIsOuterInterval() {
        TimedIntervalSet u = set(TimedInterval.closed(0, 6))
            .union(set(TimedInterval.closed(2, 4)));
        assertSingleInterval(u, 0, false, 6, false, false);
    }

    @Test
    public void unionClosedOpenAndUpFromBecomesFull() {
        TimedIntervalSet u = set(TimedInterval.closedOpen(0, 2))
            .union(set(TimedInterval.upFrom(2)));
        assertTrue(u.isFull());
        assertEquals(TimedIntervalSet.FULL, u);
    }

    @Test
    public void unionOpenFromZeroAndZeroPointBecomesFull() {
        TimedIntervalSet u = set(TimedInterval.upFromOpen(0))
            .union(set(TimedInterval.closed(0, 0)));
        assertTrue(u.isFull());
        assertEquals(TimedIntervalSet.FULL, u);
    }

    @Test
    public void complementOfEmptyIsFull() {
        assertTrue(TimedIntervalSet.EMPTY.complement().isFull());
    }

    @Test
    public void complementOfFullIsEmpty() {
        assertTrue(TimedIntervalSet.FULL.complement().isEmpty());
    }

    @Test
    public void complementOfOpenFromZeroContainsExactlyZero() {
        TimedIntervalSet comp = set(TimedInterval.upFromOpen(0)).complement();
        assertSingleInterval(comp, 0, false, 0, false, false);
        assertTrue(comp.containsPoint(0));
        assertFalse(comp.containsPoint(1));
        assertFalse(comp.containsPoint(2));
    }

    @Test
    public void complementOfClosedZeroToOneStartsAtOne() {
        TimedIntervalSet comp = set(TimedInterval.closedOpen(0, 2)).complement();
        assertSingleInterval(comp, 2, false, 0, true, false);
        assertFalse(comp.containsPoint(0));
        assertFalse(comp.containsPoint(1));
        assertTrue(comp.containsPoint(2));
    }

    @Test
    public void complementOfOpenZeroToOneContainsBothEnds() {
        TimedIntervalSet comp = set(TimedInterval.open(0, 2)).complement();
        assertEquals(2, comp.size());
        assertTrue(comp.containsPoint(0));
        assertFalse(comp.containsPoint(1));
        assertTrue(comp.containsPoint(2));
    }

    @Test
    public void complementOfClosedOneToInfinityIsClosedZeroToOneOpen() {
        TimedIntervalSet comp = set(TimedInterval.upFrom(2)).complement();
        assertSingleInterval(comp, 0, false, 2, false, true);
        assertTrue(comp.containsPoint(0));
        assertFalse(comp.containsPoint(2));
    }

    @Test
    public void complementOfOpenOneToInfinityIsClosedZeroToOneClosed() {
        TimedIntervalSet comp = set(TimedInterval.upFromOpen(2)).complement();
        assertSingleInterval(comp, 0, false, 2, false, false);
        assertTrue(comp.containsPoint(0));
        assertTrue(comp.containsPoint(2));
        assertFalse(comp.containsPoint(3));
    }

    @Test
    public void complementOfTwoIntervalsIsMiddleGap() {
        TimedIntervalSet comp = set(
            TimedInterval.closedOpen(0, 2),
            TimedInterval.upFrom(4)
        ).complement();
        assertSingleInterval(comp, 2, false, 4, false, true);
        assertTrue(comp.containsPoint(2));
        assertFalse(comp.containsPoint(4));
    }

    @Test
    public void complementOfZeroExcludesZeroAndIncludesLaterPoints() {
        TimedIntervalSet comp = set(TimedInterval.closed(0, 0)).complement();
        assertSingleInterval(comp, 0, true, 0, true, false);
        assertFalse(comp.containsPoint(0));
        assertTrue(comp.containsPoint(1));
    }

    @Test
    public void intersectionBoundedAndBounded() {
        TimedIntervalSet inter = set(TimedInterval.closed(0, 4))
            .intersection(set(TimedInterval.closedOpen(2, 6)));
        assertSingleInterval(inter, 2, false, 4, false, false);
    }

    @Test
    public void intersectionBoundedAndUnbounded() {
        TimedIntervalSet inter = set(TimedInterval.closed(0, 4))
            .intersection(set(TimedInterval.upFrom(2)));
        assertSingleInterval(inter, 2, false, 4, false, false);
    }

    @Test
    public void intersectionClosedBoundarySingleton() {
        TimedIntervalSet inter = set(TimedInterval.openClosed(0, 4))
            .intersection(set(TimedInterval.closed(4, 6)));
        assertSingleInterval(inter, 4, false, 4, false, false);
    }

    @Test
    public void intersectionOpenBoundaryIsEmpty() {
        TimedIntervalSet inter = set(TimedInterval.closedOpen(0, 4))
            .intersection(set(TimedInterval.openClosed(4, 6)));
        assertTrue(inter.isEmpty());
    }

    @Test
    public void intersectionTwoUnboundedIntervals() {
        TimedIntervalSet inter = set(TimedInterval.upFrom(2))
            .intersection(set(TimedInterval.upFromOpen(4)));
        assertSingleInterval(inter, 4, true, 0, true, false);
    }

    @Test
    public void intersectionDisjointSetsIsEmpty() {
        TimedIntervalSet inter = set(TimedInterval.closed(0, 2))
            .intersection(set(TimedInterval.upFrom(4)));
        assertTrue(inter.isEmpty());
    }

    @Test
    public void isFullOnlyForClosedZeroToInfinity() {
        assertTrue(TimedIntervalSet.FULL.isFull());
        assertFalse(set(TimedInterval.upFromOpen(0)).isFull());
        assertFalse(set(TimedInterval.upFrom(1)).isFull());
        assertFalse(set(TimedInterval.closedOpen(0, 2), TimedInterval.upFromOpen(2)).isFull());
    }

    @Test
    public void witnessForFullSetIsZero() {
        assertEquals(0L, TimedIntervalSet.FULL.witness());
    }

    @Test
    public void witnessForOpenLowerBoundSkipsExcludedPoint() {
        TimedIntervalSet s = set(TimedInterval.upFromOpen(0));
        long w = s.witness();
        assertTrue(w > 0);
        assertTrue(s.containsPoint(w));
    }

    @Test
    public void witnessForClosedUnboundedIsLowerBound() {
        TimedIntervalSet s = set(TimedInterval.upFrom(2));
        assertEquals(2L, s.witness());
        assertTrue(s.containsPoint(s.witness()));
    }

    @Test
    public void witnessForOpenUnboundedIsGreaterThanLowerBound() {
        TimedIntervalSet s = set(TimedInterval.upFromOpen(2));
        assertTrue(s.witness() > 2);
        assertTrue(s.containsPoint(s.witness()));
    }

    @Test
    public void witnessForEmptySetIsMinusOne() {
        assertEquals(-1L, TimedIntervalSet.EMPTY.witness());
    }

    @Test
    public void equivalentCanonicalizationsCompareEqual() {
        TimedIntervalSet split = set(
            TimedInterval.closedOpen(0, 2),
            TimedInterval.closedOpen(2, 4)
        );
        TimedIntervalSet direct = set(TimedInterval.closedOpen(0, 4));
        assertEquals(direct, split);
        assertEquals(direct.hashCode(), split.hashCode());
    }

    @Test
    public void fullCanonicalizationsCompareEqual() {
        TimedIntervalSet split = set(TimedInterval.closedOpen(0, 2), TimedInterval.upFrom(2));
        assertEquals(TimedIntervalSet.FULL, split);
        assertEquals(TimedIntervalSet.FULL.hashCode(), split.hashCode());
    }

    @Test
    public void complementOfComplementReturnsCanonicalOriginal() {
        TimedIntervalSet s = set(TimedInterval.closedOpen(0, 2), TimedInterval.upFrom(4));
        assertEquals(s, s.complement().complement());
    }
}
