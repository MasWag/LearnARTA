package drta;

import java.util.*;

/**
 * A timed predicate: a finite map from symbols to TimedIntervalSet.
 *
 * Represents a predicate over the domain of timed letters (symbol, delay).
 * It is True iff it maps every declared symbol to the full interval set [0,+).
 * It is False iff it maps every declared symbol to the empty set.
 * An atom predicate for TimedLetter(s, d) maps s to [d,d] and every other
 * symbol to the empty set.
 *
 * Boolean operations produce new TimedPredicate objects without mutating state.
 */
public final class TimedPredicate {

    /** The map from symbol to interval set. */
    public final Map<String, TimedIntervalSet> domain;

    /** The full domain of declared symbols. */
    private final Set<String> fullDomain;

    TimedPredicate(Map<String, TimedIntervalSet> domain, Set<String> fullDomain) {
        this.domain = Collections.unmodifiableMap(new HashMap<>(domain));
        this.fullDomain = Collections.unmodifiableSet(new HashSet<>(fullDomain));
    }

    public static TimedPredicate truePredicate(Set<String> symbols) {
        Map<String, TimedIntervalSet> m = new HashMap<>();
        for (String s : symbols) {
            m.put(s, TimedIntervalSet.FULL);
        }
        return new TimedPredicate(m, new HashSet<>(symbols));
    }

    public static TimedPredicate falsePredicate(Set<String> symbols) {
        Map<String, TimedIntervalSet> m = new HashMap<>();
        for (String s : symbols) {
            m.put(s, TimedIntervalSet.EMPTY);
        }
        return new TimedPredicate(m, new HashSet<>(symbols));
    }

    /** Atom predicate: True for a specific TimedLetter, False for all others. */
    public static TimedPredicate atom(TimedLetter tl, Set<String> symbols) {
        Map<String, TimedIntervalSet> m = new HashMap<>();
        TimedIntervalSet pointSet = TimedIntervalSet.normalize(
            Collections.singletonList(
                TimedInterval.closed(tl.delayHalfUnits, tl.delayHalfUnits)));
        for (String s : symbols) {
            m.put(s, s.equals(tl.symbol) ? pointSet : TimedIntervalSet.EMPTY);
        }
        TimedPredicate pred = new TimedPredicate(m, symbols);
        return pred;
    }

    /**
     * Predicate for one transition guard: symbol == sym AND delay in interval.
     * All other symbols get the empty set.
     */
    public static TimedPredicate fromGuard(String sym, TimedInterval interval,
                                            Set<String> symbols) {
        if (interval.isEmpty()) return falsePredicate(symbols);
        List<TimedInterval> list = new ArrayList<>();
        if (!interval.isEmpty()) list.add(interval);
        TimedIntervalSet ivSet = TimedIntervalSet.normalize(list);
        TimedIntervalSet emptySet = TimedIntervalSet.EMPTY;
        Map<String, TimedIntervalSet> m = new HashMap<>();
        for (String s : symbols) {
            m.put(s, s.equals(sym) ? ivSet : emptySet);
        }
        return new TimedPredicate(m, symbols);
    }

    /** Boolean AND: pointwise intersection. */
    public TimedPredicate and(TimedPredicate other) {
        HashMap<String, TimedIntervalSet> m = new HashMap<>();
        Set<String> keys = new HashSet<>(this.domain.keySet());
        keys.addAll(other.domain.keySet());
        TimedIntervalSet emptySet = TimedIntervalSet.EMPTY;
        for (String k : keys) {
            TimedIntervalSet a = this.domain.getOrDefault(k, emptySet);
            TimedIntervalSet b = other.domain.getOrDefault(k, emptySet);
            m.put(k, a.intersection(b));
        }
        return new TimedPredicate(m, new HashSet<>(keys));
    }

    /** Boolean OR: pointwise union. */
    public TimedPredicate or(TimedPredicate other) {
        HashMap<String, TimedIntervalSet> m = new HashMap<>();
        Set<String> keys = new HashSet<>(this.domain.keySet());
        keys.addAll(other.domain.keySet());
        TimedIntervalSet emptySet = TimedIntervalSet.EMPTY;
        for (String k : keys) {
            TimedIntervalSet a = this.domain.getOrDefault(k, emptySet);
            TimedIntervalSet b = other.domain.getOrDefault(k, emptySet);
            m.put(k, a.union(b));
        }
        return new TimedPredicate(m, new HashSet<>(keys));
    }

    /** Boolean NOT: pointwise complement. */
    public TimedPredicate notFullDomain() {
        HashMap<String, TimedIntervalSet> m = new HashMap<>();
        TimedIntervalSet fullSet = TimedIntervalSet.FULL;
        for (String k : this.fullDomain) {
            TimedIntervalSet iv = this.domain.getOrDefault(k, fullSet);
            m.put(k, iv.complement());
        }
        return new TimedPredicate(m, new HashSet<>(this.fullDomain));
    }

    /** Two predicates are equivalent iff their domain mappings are pairwise equal. */
    public boolean areEquivalent(TimedPredicate other) {
        if (this.fullDomain.size() != other.fullDomain.size()) return false;
        for (String k : this.fullDomain) {
            if (!this.domain.containsKey(k) || !other.domain.containsKey(k)) return false;
            if (!this.domain.get(k).equals(other.domain.get(k))) return false;
        }
        return true;
    }

    public boolean isSatisfiable() {
        TimedIntervalSet emptySet = TimedIntervalSet.EMPTY;
        for (String k : this.domain.keySet()) {
            TimedIntervalSet iv = this.domain.get(k);
            if (!iv.isEmpty()) return true;
        }
        return false;
    }

    /**
     * Check whether a single TimedLetter satisfies this predicate.
     */
    public boolean hasModel(TimedLetter letter) {
        if (letter == null) return false;
        TimedIntervalSet ivs = this.domain.getOrDefault(letter.symbol, TimedIntervalSet.EMPTY);
        return !ivs.isEmpty() && ivs.containsPoint(letter.delayHalfUnits);
    }

    /** Generate a witnessing TimedLetter, or null if unsatisfiable. */
    public TimedLetter generateWitness() {
        for (Map.Entry<String, TimedIntervalSet> entry : domain.entrySet()) {
            TimedIntervalSet ivs = entry.getValue();
            if (!ivs.isEmpty()) {
                long w = ivs.witness();
                if (w >= 0) {
                    return TimedLetter.ofHalf(entry.getKey(), w);
                }
            }
        }
        return null;
    }

    /** Format for debugging. */
    public String toDotString() {
        StringBuilder sb = new StringBuilder();
        sb.append("{");
        for (Map.Entry<String, TimedIntervalSet> entry : domain.entrySet()) {
            if (sb.length() > 1) sb.append(", ");
            sb.append(entry.getKey());
            sb.append("->");
            sb.append(entry.getValue().toDotString());
        }
        sb.append("}");
        return sb.toString();
    }

    @Override
    public String toString() {
        return toDotString();
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof TimedPredicate)) return false;
        TimedPredicate that = (TimedPredicate) o;
        return this.fullDomain.equals(that.fullDomain) && this.domain.equals(that.domain);
    }

    @Override
    public int hashCode() {
        return Objects.hash(domain, fullDomain);
    }
}
