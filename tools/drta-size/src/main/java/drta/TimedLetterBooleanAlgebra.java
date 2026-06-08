package drta;

import org.sat4j.specs.TimeoutException;
import theory.BooleanAlgebra;
import utilities.Pair;

import java.util.*;

/**
 * TimedLetterBooleanAlgebra
 *
 * Extends symbolicautomata's BooleanAlgebra<TimedPredicate, List<TimedLetter>>.
 * This is a finite Boolean algebra where the domain S is the set of all
 * TimedLetters (symbol, delay) and P is the set of predicates over that domain.
 *
 * The domain alphabet must be supplied at construction time.  All predicates
 * are over this fixed alphabet.
 */
public class TimedLetterBooleanAlgebra extends BooleanAlgebra<TimedPredicate, List<TimedLetter>> {

    private final Set<String> alphabet;
    private final TimedPredicate truePred;
    private final TimedPredicate falsePred;

    public TimedLetterBooleanAlgebra(Set<String> alphabet) {
        if (alphabet == null || alphabet.isEmpty()) {
            throw new IllegalArgumentException("alphabet must be non-empty");
        }
        this.alphabet = Collections.unmodifiableSet(new HashSet<>(alphabet));
        this.truePred = TimedPredicate.truePredicate(this.alphabet);
        this.falsePred = TimedPredicate.falsePredicate(this.alphabet);
    }

    public Set<String> alphabet() {
        return alphabet;
    }

    @Override
    public TimedPredicate MkAtom(List<TimedLetter> s) {
        if (s == null || s.size() != 1) {
            return falsePred;
        }
        return TimedPredicate.atom(s.get(0), alphabet);
    }

    @Override
    public TimedPredicate MkNot(TimedPredicate p) throws TimeoutException {
        return p.notFullDomain();
    }

    @Override
    public TimedPredicate MkOr(Collection<TimedPredicate> pset) throws TimeoutException {
        if (pset.isEmpty()) return falsePred;
        TimedPredicate result = new TimedPredicate(new HashMap<>(), new HashSet<>());
        boolean first = true;
        for (TimedPredicate p : pset) {
            if (first) {
                result = p;
                first = false;
            } else {
                result = result.or(p);
            }
        }
        return result;
    }

    @Override
    public TimedPredicate MkOr(TimedPredicate p1, TimedPredicate p2) throws TimeoutException {
        return p1.or(p2);
    }

    @Override
    public TimedPredicate MkAnd(Collection<TimedPredicate> pset) throws TimeoutException {
        if (pset.isEmpty()) return truePred;
        TimedPredicate result = new TimedPredicate(new HashMap<>(), new HashSet<>());
        boolean first = true;
        for (TimedPredicate p : pset) {
            if (first) {
                result = p;
                first = false;
            } else {
                result = result.and(p);
            }
        }
        return result;
    }

    @Override
    public TimedPredicate MkAnd(TimedPredicate p1, TimedPredicate p2) throws TimeoutException {
        return p1.and(p2);
    }

    @Override
    public TimedPredicate True() {
        return truePred;
    }

    @Override
    public TimedPredicate False() {
        return falsePred;
    }

    @Override
    public boolean AreEquivalent(TimedPredicate p1, TimedPredicate p2) throws TimeoutException {
        return p1.areEquivalent(p2);
    }

    @Override
    public boolean IsSatisfiable(TimedPredicate p1) throws TimeoutException {
        return p1.isSatisfiable();
    }

    @Override
    public boolean HasModel(TimedPredicate p1, List<TimedLetter> el) throws TimeoutException {
        return p1.hasModel(el);
    }

    @Override
    public boolean HasModel(TimedPredicate p1, List<TimedLetter> el1, List<TimedLetter> el2)
            throws TimeoutException {
        throw new UnsupportedOperationException("TimedLetterBooleanAlgebra is unary");
    }

    @Override
    public List<TimedLetter> generateWitness(TimedPredicate p1) throws TimeoutException {
        TimedLetter w = p1.generateWitness();
        if (w == null) return Collections.emptyList();
        return Collections.singletonList(w);
    }

    @Override
    public Pair<List<TimedLetter>, List<TimedLetter>> generateWitnesses(TimedPredicate p1)
            throws TimeoutException {
        throw new UnsupportedOperationException("TimedLetterBooleanAlgebra is unary");
    }

    public TimedPredicate MkNotSingleton(TimedPredicate p1, List<TimedLetter> el) throws TimeoutException {
        TimedPredicate atomPred = this.MkAtom(el);
        return this.MkAnd(this.MkNot(atomPred), p1);
    }
}
