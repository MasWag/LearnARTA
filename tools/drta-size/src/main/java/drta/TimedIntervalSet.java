package drta;

import java.util.*;

/**
 * A finite union of disjoint TimedIntervals over R>=0, stored in normalized
 * form.
 *
 * Canonical form invariants:
 *   1) Intervals are sorted by lower bound, ascending.
 *   2) No two intervals overlap or touch at an included boundary.
 *   3) Empty set is represented by an empty list (not a list with one
 *      empty interval).
 */
public final class TimedIntervalSet implements Iterable<TimedInterval> {

    private final List<TimedInterval> intervals;
    public static final TimedIntervalSet EMPTY = new TimedIntervalSet();
    public static final TimedIntervalSet EMPTY_INTERVAL
        = new TimedIntervalSet(Collections.singletonList(
            TimedInterval.closed(-1, -1)));
    public static final TimedIntervalSet FULL_INTERVAL;

    static {
        FULL_INTERVAL = new TimedIntervalSet(Collections.singletonList(
            TimedInterval.upFrom(0)));
    }

    private static final long INF = TimedInterval.POS_INF;
    public static TimedIntervalSet FULL = FULL_INTERVAL;

    private TimedIntervalSet(List<TimedInterval> intervals) {
        this.intervals = Collections.unmodifiableList(
            intervals.isEmpty() ? new ArrayList<>() : new ArrayList<>(intervals));
    }

    private TimedIntervalSet() {
        this.intervals = Collections.unmodifiableList(new ArrayList<>());
    }

    /**
     * Parse a list of interval strings from JSON (e.g. arrays of guard strings).
     * Each string is interpreted as an interval; if the JSON uses a single
     * interval string, that is treated as a singleton union.
     */
    public static TimedIntervalSet parse(String... inputs) {
        List<TimedInterval> parts = new ArrayList<>();
        for (String s : inputs) {
            if (s == null || s.isEmpty()) continue;
            TimedInterval iv = TimedInterval.parse(s);
            if (!iv.isEmpty()) parts.add(iv);
        }
        return normalize(parts);
    }

    /**
     * Normalize a list of intervals into a canonical finite union of disjoint
     * TimedIntervals over R>=0.
     *
     * <p>Invariants of the normalized form:
     * <ol>
     *   <li>All intervals are disjoint (no point is in more than one).</li>
     *   <li>No two intervals overlap or touch at an included boundary.</li>
     *   <li>Empty set is represented by an empty list.</li>
     *   <li>All intervals use exact half-unit integers (no floating-point).</li>
     * </ol>
     */
    public static TimedIntervalSet normalize(List<TimedInterval> intervals) {
        if (intervals == null || intervals.isEmpty()) return EMPTY;

        // Step 1: Remove empty intervals
        List<TimedInterval> nonEmpty = new ArrayList<>();
        for (TimedInterval iv : intervals) {
            if (!iv.isEmpty()) nonEmpty.add(iv);
        }
        if (nonEmpty.isEmpty()) return EMPTY;

        // Step 2: Deduplicate identical intervals (same structural signature)
        Set<List<Long>> seen = new HashSet<>();
        List<TimedInterval> deduped = new ArrayList<>();
        for (TimedInterval iv : nonEmpty) {
            List<Long> key = Arrays.asList(iv.lo, iv.hi, iv.loOpen ? 1L : 0L,
                                           iv.hiInf ? 1L : 0L, iv.hiOpen ? 1L : 0L);
            if (!seen.contains(key)) {
                seen.add(key);
                deduped.add(iv);
            }
        }
        if (deduped.isEmpty()) return EMPTY;

        // If only one unique interval remains, return it directly
        if (deduped.size() == 1) {
            return new TimedIntervalSet(deduped);
        }

        // Step 3: Sort by lo ascending, wider lower/upper endpoints first.
        deduped.sort((a, b) -> {
            int cmp = Long.compare(a.lo, b.lo);
            if (cmp != 0) return cmp;

            cmp = Boolean.compare(a.loOpen, b.loOpen);
            if (cmp != 0) return cmp;

            if (a.hiInf && !b.hiInf) return -1;
            if (!a.hiInf && b.hiInf) return 1;

            cmp = Long.compare(b.hi, a.hi);
            if (cmp != 0) return cmp;

            return Boolean.compare(a.hiOpen, b.hiOpen);
        });

        // Step 4: Scan and merge overlapping / touching intervals.
        List<TimedInterval> merged = new ArrayList<>();
        TimedInterval current = deduped.get(0);

        for (int i = 1; i < deduped.size(); i++) {
            TimedInterval next = deduped.get(i);

            if (!canMerge(current, next)) {
                merged.add(current);
                current = next;
                continue;
            }

            // Merge: union of `current` and `next`.
            // Lower bound: current.lo (sorted), openness depends.
            long newLo = current.lo;
            boolean newLoOpen;
            if (current.lo == next.lo) {
                // Both start at the same point → excluded only if BOTH exclude it
                newLoOpen = current.loOpen && next.loOpen;
            } else {
                // current.lo < next.lo → only `current` can include newLo.
                newLoOpen = current.loOpen;
            }

            // Upper bound: max of both finite highs, or infinity if either infinite.
            long newHi;
            boolean newHiInf;
            boolean newHiOpen;

            if (current.hiInf || next.hiInf) {
                // At least one is unbounded → union is unbounded above
                newHiInf = true;
                newHi = TimedInterval.POS_INF;
                newHiOpen = false; // Unbounded above has no meaningful open endpoint
            } else {
                newHiInf = false;
                long maxHi = Math.max(current.hi, next.hi);
                if (maxHi == current.hi && maxHi == next.hi) {
                    // Both reach the same hi → excluded only if BOTH exclude it
                    newHiOpen = current.hiOpen && next.hiOpen;
                } else if (maxHi == current.hi) {
                    // Only current has this hi
                    newHiOpen = current.hiOpen;
                } else {
                    // Only next has this hi
                    newHiOpen = next.hiOpen;
                }
                newHi = maxHi;
            }

            current = new TimedInterval(newLo, newHi, newLoOpen, newHiInf, newHiOpen);
        }
        merged.add(current);

        return new TimedIntervalSet(merged);
    }

    /**
     * Returns true if `a` and `b` can be unified into a single interval.
     * Assumes a.lo <= b.lo (i.e., they appear in sorted order).
     */
    private static boolean canMerge(TimedInterval a, TimedInterval b) {
        if (a.hiInf) {
            return true; // Unbounded above always overlaps anything that starts >= its lo
        }
        if (b.lo < a.hi) {
            return true;
        }
        if (b.lo > a.hi) return false;
        return !a.hiOpen || !b.loOpen;
    }

    /** Union with another set. */
    public TimedIntervalSet union(TimedIntervalSet other) {
        List<TimedInterval> all = new ArrayList<>(intervals);
        all.addAll(other.intervals);
        return normalize(all);
    }

    /** Intersection with another set. */
    public TimedIntervalSet intersection(TimedIntervalSet other) {
        List<TimedInterval> result = new ArrayList<>();
        for (TimedInterval a : this.intervals) {
            for (TimedInterval b : other.intervals) {
                TimedInterval inter = a.intersection(b);
                if (!inter.isEmpty()) result.add(inter);
            }
        }
        return normalize(result);
    }

    /** Complement over R>=0. */
    public TimedIntervalSet complement() {
        if (intervals.isEmpty()) return FULL;
        List<TimedInterval> result = new ArrayList<>();
        long gapStart = 0L;
        boolean gapStartOpen = false;

        for (TimedInterval iv : intervals) {
            TimedInterval leftGap = new TimedInterval(
                gapStart,
                iv.lo,
                gapStartOpen,
                false,
                !iv.loOpen);
            if (!leftGap.isEmpty()) {
                result.add(leftGap);
            }

            if (iv.hiInf) {
                return normalize(result);
            }

            gapStart = iv.hi;
            gapStartOpen = !iv.hiOpen;
        }

        // Gap after last interval
        result.add(gapStartOpen
            ? TimedInterval.upFromOpen(gapStart)
            : TimedInterval.upFrom(gapStart));

        return normalize(result);
    }
    public boolean isEmpty() {
        return intervals.isEmpty();
    }

    public boolean isFull() {
        if (intervals.size() != 1) return false;
        TimedInterval iv = intervals.get(0);
        return iv.lo == 0 && !iv.loOpen && iv.hiInf;
    }

    public boolean isEmptySet() {
        return isEmpty();
    }

    public boolean satisfies() {
        return !isEmpty();
    }

    /** Does this set contain the given half-unit delay? */
    public boolean containsPoint(long d) {
        for (TimedInterval iv : intervals) {
            if (iv.contains(d)) return true;
        }
        return false;
    }

    /** Yield a witness delay (in half-units) or -1 if empty. */
    public long witness() {
        for (TimedInterval iv : intervals) {
            long w = iv.witnessPoint();
            if (w >= 0) return w;
        }
        return -1;
    }

    public int size() {
        return intervals.size();
    }

    public List<TimedInterval> intervals() {
        return intervals;
    }

    /** Format for debugging / Dotty output. */
    public String toDotString() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < intervals.size(); i++) {
            if (i > 0) sb.append(" u ");
            sb.append(intervals.get(i).toString());
        }
        return sb.toString();
    }

    /** Convert to a single interval. If empty return EMPTY_INTERVAL,
     *  if single element return that, otherwise throw. */
    public TimedInterval toSingleInterval() {
        if (intervals.size() == 1) return intervals.get(0);
        if (intervals.isEmpty()) return TimedInterval.closed(-1, -1);
        throw new UnsupportedOperationException(
            "cannot convert multi-interval set to single interval");
    }

    @Override
    public String toString() {
        return toDotString();
    }

    @Override
    public Iterator<TimedInterval> iterator() {
        return intervals.iterator();
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof TimedIntervalSet)) return false;
        TimedIntervalSet that = (TimedIntervalSet) o;
        return intervals.equals(that.intervals);
    }

    @Override
    public int hashCode() {
        return Objects.hash(intervals);
    }
}
