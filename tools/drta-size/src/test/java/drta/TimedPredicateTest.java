package drta;

import org.junit.Test;
import org.junit.Before;
import static org.junit.Assert.*;

import java.util.*;

/**
 * Tests for TimedPredicate Boolean operations over multiple symbols.
 *
 * The domain alphabet is {a, b} in most tests.  Predicates are finite maps
 * from symbol names to TimedIntervalSets.  Boolean operations work pointwise
 * on domain entries.
 */
public class TimedPredicateTest {

    private TimedPredicate truePred, falsePred;
    private Set<String> alphabet;

    @Before
    public void setUp() {
        alphabet = new LinkedHashSet<>();
        alphabet.add("a");
        alphabet.add("b");
        truePred = TimedPredicate.truePredicate(alphabet);
        falsePred = TimedPredicate.falsePredicate(alphabet);
    }

    // ---------- True / False tests ----------

    @Test
    public void testTruePredicateIsSatisfiable() {
        assertTrue(truePred.isSatisfiable());
    }

    @Test
    public void testFalsePredicateIsNotSatisfiable() {
        assertFalse(falsePred.isSatisfiable());
    }

    @Test
    public void testTrueAndTrueIsTrue() {
        TimedPredicate result = truePred.and(truePred);
        assertTrue(result.areEquivalent(truePred));
    }

    @Test
    public void testTrueOrFalseIsTrue() {
        TimedPredicate result = truePred.or(falsePred);
        assertTrue(result.areEquivalent(truePred));
    }

    @Test
    public void testFalseAndTrueIsFalse() {
        TimedPredicate result = falsePred.and(truePred);
        assertTrue(result.areEquivalent(falsePred));
    }

    @Test
    public void testFalseOrFalseIsFalse() {
        TimedPredicate result = falsePred.or(falsePred);
        assertTrue(result.areEquivalent(falsePred));
    }

    @Test
    public void testTrueNotIsFalse() {
        TimedPredicate result = truePred.notFullDomain();
        assertTrue(result.areEquivalent(falsePred));
    }

    @Test
    public void testFalseNotIsTrue() {
        TimedPredicate result = falsePred.notFullDomain();
        assertTrue(result.areEquivalent(truePred));
    }

    // ---------- Atom predicate tests ----------

    @Test
    public void testAtomPredicate() {
        TimedLetter letter = TimedLetter.of("a", 0);
        TimedPredicate atom = TimedPredicate.atom(letter, alphabet);

        // atom should be satisfiable (contains exactly the letter 'a' at delay 0)
        assertTrue(atom.isSatisfiable());

        // Should have model for this letter
        assertTrue(atom.hasModel(Collections.singletonList(letter)));

        // Should not have model for a different letter
        assertFalse(atom.hasModel(Collections.singletonList(
            TimedLetter.of("b", 0))));

        // Should not have model for a different delay
        assertFalse(atom.hasModel(Collections.singletonList(
            TimedLetter.of("a", 1))));
    }

    // ---------- Guard predicate tests ----------

    @Test
    public void testFromGuard() {
        TimedPredicate pred = TimedPredicate.fromGuard(
            "a", TimedInterval.parse("[0,2]"), alphabet);

        assertTrue(pred.isSatisfiable());

        // Should have model for (a, 0), (a, 1), (a, 2)
        // These are in half-units: 0, 2, 4
        // Actually guard is [0,2] in string time => [0,4] half-units
        assertTrue(pred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 0))));
        assertTrue(pred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 4))));
        assertTrue(pred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 2))));

        // Should not have model for (a, 3) => 6 half-units
        assertFalse(pred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 6))));

        // Should not have model for symbol 'b' at any delay
        assertFalse(pred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("b", 0))));
    }

    @Test
    public void testFromGuardWithEmptyInterval() {
        TimedInterval empty = TimedInterval.closed(5, 3); // empty
        TimedPredicate pred = TimedPredicate.fromGuard("a", empty, alphabet);
        assertFalse(pred.isSatisfiable());
    }

    @Test
    public void testFromGuardWithInfinity() {
        TimedPredicate pred = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(1), alphabet);

        // Should have model for (a, 1) => 2 half-units
        assertTrue(pred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 2))));
        assertTrue(pred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 10))));

        // Should not have model for (a, 0) => 0 half-units
        assertFalse(pred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 0))));
    }

    // ---------- Two-symbol Boolean operations ----------

    @Test
    public void testAndOverTwoDifferentSymbols() {
        TimedPredicate a1 = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(0), alphabet);
        TimedPredicate b1 = TimedPredicate.fromGuard(
            "b", TimedInterval.upFrom(0), alphabet);

        TimedPredicate andPred = a1.and(b1);

        assertFalse(andPred.isSatisfiable());
        assertFalse(andPred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 0))));
        assertFalse(andPred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("b", 0))));
    }

    @Test
    public void testOrOverTwoDifferentSymbols() {
        TimedPredicate a1 = TimedPredicate.fromGuard(
            "a", TimedInterval.closed(0, 0), alphabet);
        TimedPredicate b1 = TimedPredicate.fromGuard(
            "b", TimedInterval.parse("[5,5]"), alphabet);

        TimedPredicate orPred = a1.or(b1);

        assertTrue(orPred.isSatisfiable());
        assertTrue(orPred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 0))));
        assertTrue(orPred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("b", 10))));
        assertFalse(orPred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 1))));
    }

    @Test
    public void testNotOverTwoSymbols() {
        TimedPredicate a1 = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(0), alphabet);

        TimedPredicate notPred = a1.notFullDomain();

        // 'a' part: [0,+inf)' => empty
        // 'b' part: empty' => full
        assertTrue(notPred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("b", 0))));
        assertFalse(notPred.hasModel(Collections.singletonList(
            TimedLetter.ofHalf("a", 0))));
    }

    // ---- equivalence tests ----

    @Test
    public void testEquivalentPredicates() {
        TimedPredicate a1 = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(0), alphabet);
        TimedPredicate a2 = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(0), alphabet);

        assertTrue(a1.areEquivalent(a2));
    }

    @Test
    public void testInequivalentPredicates() {
        TimedPredicate a1 = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(0), alphabet);
        TimedPredicate a3 = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(5), alphabet);

        assertFalse(a1.areEquivalent(a3));
    }

    // ---- witness generation ----

    @Test
    public void testGenerateWitness() {
        TimedPredicate pred = TimedPredicate.fromGuard(
            "a", TimedInterval.closed(2, 6), alphabet);

        TimedLetter witness = pred.generateWitness();
        assertNotNull(witness);
        assertEquals("a", witness.symbol);
        assertTrue(witness.delayHalfUnits >= 2L);
    }

    @Test
    public void testGenerateWitnessNoSatisfiable() {
        TimedPredicate pred = TimedPredicate.falsePredicate(alphabet);
        assertNull(pred.generateWitness());
    }
}
