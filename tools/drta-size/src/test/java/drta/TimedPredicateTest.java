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
 * on domain entries. hasModel() now takes a single TimedLetter (not List).
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
        assertTrue(atom.hasModel(letter));

        // Should not have model for a different letter
        assertFalse(atom.hasModel(TimedLetter.of("b", 0)));

        // Should not have model for a different delay
        assertFalse(atom.hasModel(TimedLetter.of("a", 1)));
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
        assertTrue(pred.hasModel(TimedLetter.ofHalf("a", 0)));
        assertTrue(pred.hasModel(TimedLetter.ofHalf("a", 4)));
        assertTrue(pred.hasModel(TimedLetter.ofHalf("a", 2)));

        // Should not have model for (a, 3) => 6 half-units
        assertFalse(pred.hasModel(TimedLetter.ofHalf("a", 6)));

        // Should not have model for symbol 'b' at any delay
        assertFalse(pred.hasModel(TimedLetter.ofHalf("b", 0)));
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
        assertTrue(pred.hasModel(TimedLetter.ofHalf("a", 2)));
        assertTrue(pred.hasModel(TimedLetter.ofHalf("a", 10)));

        // Should not have model for (a, 0) => 0 half-units
        assertFalse(pred.hasModel(TimedLetter.ofHalf("a", 0)));
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
        assertFalse(andPred.hasModel(TimedLetter.ofHalf("a", 0)));
        assertFalse(andPred.hasModel(TimedLetter.ofHalf("b", 0)));
    }

    @Test
    public void testOrOverTwoDifferentSymbols() {
        TimedPredicate a1 = TimedPredicate.fromGuard(
            "a", TimedInterval.closed(0, 0), alphabet);
        TimedPredicate b1 = TimedPredicate.fromGuard(
            "b", TimedInterval.parse("[5,5]"), alphabet);

        TimedPredicate orPred = a1.or(b1);

        assertTrue(orPred.isSatisfiable());
        assertTrue(orPred.hasModel(TimedLetter.ofHalf("a", 0)));
        assertTrue(orPred.hasModel(TimedLetter.ofHalf("b", 10)));
        assertFalse(orPred.hasModel(TimedLetter.ofHalf("a", 1)));
    }

    @Test
    public void testNotOverTwoSymbols() {
        TimedPredicate a1 = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(0), alphabet);

        TimedPredicate notPred = a1.notFullDomain();

        // 'a' part: [0,+inf)' => empty
        // 'b' part: empty' => full
        assertTrue(notPred.hasModel(TimedLetter.ofHalf("b", 0)));
        assertFalse(notPred.hasModel(TimedLetter.ofHalf("a", 0)));
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

    // ---- canonicalization equivalence tests ----

    @Test
    public void testAdjacentIntervalsMergeToSingleInterval() {
        // [0,4) U [4,+inf) should normalize to [0,+inf) (FULL)
        TimedPredicate p1 = TimedPredicate.fromGuard(
            "a", TimedInterval.closedOpen(0, 4), alphabet);
        TimedPredicate p2 = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(4), alphabet);
        TimedPredicate merged = p1.or(p2);

        // Both symbols should map to FULL
        assertTrue(merged.domain.get("a").isFull());
    }

    @Test
    public void testUnionOfHalfAndUpFromOpensFull() {
        // [0,1) in string-time ⇒ [0,2) in half-units; [1,+inf) ⇒ upFrom(2)
        TimedPredicate a = TimedPredicate.fromGuard(
            "a", TimedInterval.closedOpen(0, 2), alphabet);
        TimedPredicate b = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(2), alphabet);
        TimedPredicate result = a.or(b);

        assertTrue(result.domain.get("a").isFull());
    }

    @Test
    public void testComplementBoundaryCorrect() {
        // complement of [0,+) should be empty
        TimedPredicate fullPred = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(0), alphabet);
        TimedPredicate comp = fullPred.notFullDomain();

        // 'a' part should be EMPTY
        assertFalse(comp.domain.get("a").satisfies());
    }

    @Test
    public void testCanonicalEqualityAfterNormalize() {
        // Two ways to build the same interval: via individual unions or directly
        TimedIntervalSet s1 = TimedIntervalSet.normalize(Arrays.asList(
            TimedInterval.closedOpen(0, 4),
            TimedInterval.upFrom(4)
        ));
        TimedIntervalSet s2 = TimedIntervalSet.FULL;

        assertEquals(s1, s2); // structural equality after canonicalization
    }

    @Test
    public void testSplitFiniteIntervalsEquivalentToSingleInterval() {
        TimedPredicate left = TimedPredicate.fromGuard(
            "a", TimedInterval.closedOpen(0, 2), alphabet);
        TimedPredicate right = TimedPredicate.fromGuard(
            "a", TimedInterval.closedOpen(2, 4), alphabet);
        TimedPredicate split = left.or(right);
        TimedPredicate direct = TimedPredicate.fromGuard(
            "a", TimedInterval.closedOpen(0, 4), alphabet);

        assertTrue(split.areEquivalent(direct));
        assertEquals(direct.domain.get("a"), split.domain.get("a"));
    }

    @Test
    public void testSplitFullIntervalsEquivalentToFullInterval() {
        TimedPredicate left = TimedPredicate.fromGuard(
            "a", TimedInterval.closedOpen(0, 2), alphabet);
        TimedPredicate right = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(2), alphabet);
        TimedPredicate split = left.or(right);
        TimedPredicate direct = TimedPredicate.fromGuard(
            "a", TimedInterval.upFrom(0), alphabet);

        assertTrue(split.areEquivalent(direct));
        assertTrue(split.domain.get("a").isFull());
    }

    @Test
    public void testPredicateComplementOfEmptyIsTrue() {
        TimedPredicate emptyPred = TimedPredicate.falsePredicate(alphabet);
        TimedPredicate comp = emptyPred.notFullDomain();

        assertTrue(comp.isSatisfiable());
        assertTrue(comp.domain.get("a").isFull());
        assertTrue(comp.domain.get("b").isFull());
    }

    @Test
    public void testIntersectionProducesCanonicalBoundaryResult() {
        TimedPredicate p1 = TimedPredicate.fromGuard(
            "a", TimedInterval.openClosed(0, 4), alphabet);
        TimedPredicate p2 = TimedPredicate.fromGuard(
            "a", TimedInterval.closed(4, 6), alphabet);

        TimedPredicate inter = p1.and(p2);
        assertTrue(inter.hasModel(TimedLetter.ofHalf("a", 4)));
        assertFalse(inter.hasModel(TimedLetter.ofHalf("a", 3)));
        assertFalse(inter.hasModel(TimedLetter.ofHalf("a", 5)));
    }
}
