package drta;

import java.util.Objects;

/**
 * A half-unit interval over R>=0, representing a contiguous range of
 * delay values.
 *
 * All bounds are stored in half-units (long).  Standard-time integer t
 * maps to t*2 half-units.  There is never any floating-point arithmetic.
 */
public final class TimedInterval {

    public static final long POS_INF = Integer.MAX_VALUE * 10L;
    public final long lo;
    public final long hi;
    public final boolean loOpen;
    public final boolean hiInf;
    public final boolean hiOpen;

    TimedInterval(long lo, long hi, boolean loOpen, boolean hiInf, boolean hiOpen) {
        this.lo = lo;
        this.hi = hiInf ? POS_INF : hi;
        this.loOpen = loOpen;
        this.hiInf = hiInf;
        this.hiOpen = hiOpen;
    }

    public static TimedInterval closed(long lo, long hi) {
        return new TimedInterval(lo, hi, false, false, false);
    }

    public static TimedInterval closedOpen(long lo, long hi) {
        return new TimedInterval(lo, hi, false, false, true);
    }

    public static TimedInterval openClosed(long lo, long hi) {
        return new TimedInterval(lo, hi, true, false, false);
    }

    public static TimedInterval open(long lo, long hi) {
        return new TimedInterval(lo, hi, true, false, true);
    }

    public static TimedInterval upFrom(long lo) {
        return new TimedInterval(lo, 0, false, true, false);
    }

    public static TimedInterval upFromOpen(long lo) {
        return new TimedInterval(lo, 0, true, true, false);
    }

    public boolean isEmpty() {
        if (lo < 0 && !hiInf && hi < 0) return true;
        if (hiInf) return false;
        return lo > hi || (lo == hi && (loOpen || hiOpen));
    }

    public long witnessPoint() {
        if (isEmpty()) return -1;
        return lo + (loOpen ? 1 : 0);
    }

    public boolean contains(long d) {
        if (isEmpty()) return false;
        if (lo > d) return false;
        if (loOpen && lo == d) return false;
        if (hiInf) return true;
        if (hi < d) return false;
        return !(hiOpen && hi == d);
    }

    public boolean containsPoint(long d) {
        return contains(d);
    }

    public TimedIntervalSet complement() {
        return TimedIntervalSet.normalize(
            java.util.Collections.singletonList(this)).complement();
    }

    public TimedInterval intersection(TimedInterval other) {
        if (this.isEmpty() || other.isEmpty())
            return TimedInterval.closed(-1, -1);

        long newLo = Math.max(this.lo, other.lo);
        boolean newLoOpen = false;
        if (this.lo == newLo) newLoOpen |= this.loOpen;
        if (other.lo == newLo) newLoOpen |= other.loOpen;

        boolean newHiInf;
        long newHi;
        boolean newHiOpen = false;

        if (this.hiInf && other.hiInf) {
            newHiInf = true;
            newHi = POS_INF;
        } else if (this.hiInf) {
            newHiInf = false;
            newHi = other.hi;
            newHiOpen |= other.hiOpen;
        } else if (other.hiInf) {
            newHiInf = false;
            newHi = this.hi;
            newHiOpen |= this.hiOpen;
        } else if (this.hi < other.hi) {
            newHiInf = false;
            newHi = this.hi;
            newHiOpen = this.hiOpen;
        } else if (other.hi < this.hi) {
            newHiInf = false;
            newHi = other.hi;
            newHiOpen = other.hiOpen;
        } else {
            newHiInf = false;
            newHi = this.hi;
            newHiOpen = this.hiOpen || other.hiOpen;
        }

        if (newHiInf) {
            return new TimedInterval(newLo, 0, newLoOpen, true, false);
        }
        if (newLo > newHi || (newLo == newHi && (newLoOpen || newHiOpen))) {
            return TimedInterval.closed(-1, -1);
        }
        return new TimedInterval(newLo, newHi, newLoOpen, false, newHiOpen);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof TimedInterval)) return false;
        TimedInterval that = (TimedInterval) o;
        return lo == that.lo
            && hi == that.hi
            && loOpen == that.loOpen
            && hiInf == that.hiInf
            && hiOpen == that.hiOpen;
    }

    @Override
    public int hashCode() {
        return Objects.hash(lo, hi, loOpen, hiOpen, hiInf);
    }

    public static TimedInterval parse(String input) {
        if (input == null) throw new IllegalArgumentException("null input");
        String s = input.trim();
        if (s.length() < 5) {
            throw new IllegalArgumentException("malformed interval: " + input);
        }
        char loC = s.charAt(0);
        char hiC = s.charAt(s.length() - 1);
        if ((loC != '[' && loC != '(') || (hiC != ']' && hiC != ')')) {
            throw new IllegalArgumentException("malformed interval: " + input);
        }
        boolean loOpen = (loC == '(');
        boolean hiOpen = (hiC == ')');

        int comma = s.indexOf(',');
        if (comma <= 1 || comma + 1 >= s.length() - 1) {
            throw new IllegalArgumentException("malformed interval: " + input);
        }

        String loStr = s.substring(1, comma).trim();
        long rawLo = parseFiniteEndpoint(loStr, input);
        long loVal = rawLo * 2L;

        String hiStr = s.substring(comma + 1, s.length() - 1).trim();
        if (hiStr.isEmpty()) {
            throw new IllegalArgumentException("malformed interval: " + input);
        }

        long hiVal;
        boolean hiInf = hiStr.equals("+");
        if (hiInf) {
            if (!hiOpen) {
                throw new IllegalArgumentException(
                    "invalid interval bounds: + upper bound cannot be inclusive");
            }
            hiVal = POS_INF;
        } else {
            long raw = parseFiniteEndpoint(hiStr, input);
            hiVal = raw * 2L;
        }

        TimedInterval interval = new TimedInterval(loVal, hiVal, loOpen, hiInf, hiOpen);
        if (interval.isEmpty()) {
            throw new IllegalArgumentException("interval is empty");
        }
        return interval;
    }

    private static long parseFiniteEndpoint(String raw, String input) {
        if (raw.isEmpty()) {
            throw new IllegalArgumentException("malformed interval: " + input);
        }
        try {
            long value = Long.parseLong(raw);
            if (value < 0) {
                throw new IllegalArgumentException(
                    "invalid interval bounds: endpoint is negative");
            }
            if (value > Long.MAX_VALUE / 2L) {
                throw new IllegalArgumentException(
                    "invalid interval bounds: endpoint is too large");
            }
            return value;
        } catch (NumberFormatException e) {
            throw new IllegalArgumentException("malformed interval: " + input, e);
        }
    }

    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder();
        sb.append(loOpen ? '(' : '[');
        sb.append(lo == POS_INF ? "+" : lo);
        sb.append(',');
        if (hiInf) sb.append("+"); else sb.append(hi);
        sb.append(hiInf ? ')' : (hiOpen ? ')' : ']'));
        return sb.toString();
    }
}
