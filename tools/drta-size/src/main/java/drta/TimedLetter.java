package drta;

import java.util.Objects;

/**
 * A timed letter: (symbol, delay).
 *
 * Delay is stored in half-units (integer arithmetic, no floating-point).
 * Standard time t maps to t * 2 half-units.  This matches the DelayRep
 * convention used in learn-arta-core.
 */
public final class TimedLetter {
    public final String symbol;
    public final long delayHalfUnits;

    public TimedLetter(String symbol, long delayHalfUnits) {
        this.symbol = symbol;
        this.delayHalfUnits = delayHalfUnits;
    }

    public static TimedLetter of(String symbol, int delay) {
        return new TimedLetter(symbol, delay * 2L);
    }

    public static TimedLetter ofHalf(String symbol, long halfUnits) {
        return new TimedLetter(symbol, halfUnits);
    }

    public int delayInteger() {
        return (int) (delayHalfUnits / 2L);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof TimedLetter)) return false;
        TimedLetter that = (TimedLetter) o;
        return delayHalfUnits == that.delayHalfUnits
            && symbol.equals(that.symbol);
    }

    @Override
    public int hashCode() {
        return Objects.hash(symbol, delayHalfUnits);
    }

    @Override
    public String toString() {
        return symbol + "@" + delayHalfUnits;
    }
}
