--------------------------- MODULE UltraDAGConsensus ---------------------------
(***************************************************************************
 * TLA+ Formal Specification of UltraDAG DAG-BFT Consensus
 *
 * Models the core safety and liveness properties of UltraDAG's leaderless
 * DAG-BFT protocol. Derived from the Rust implementation in:
 *   - crates/ultradag-coin/src/consensus/dag.rs       (BlockDag, equivocation)
 *   - crates/ultradag-coin/src/consensus/finality.rs   (FinalityTracker, BFT finality)
 *   - crates/ultradag-coin/src/consensus/validator_set.rs (quorum_threshold = ceil(2N/3))
 *   - crates/ultradag-coin/src/constants.rs            (parameters)
 *
 * Scope: vertex production, parent referencing, BFT finality, equivocation
 * detection. Does NOT model: staking, pruning, checkpoints, state engine,
 * transactions, or network transport.
 *
 * Equivocation model: Byzantine validators CAN produce multiple vertices in
 * the same round (equivocating). However, honest validators' DAGs only contain
 * ONE vertex per (validator, round) — the first seen. This is enforced by
 * try_insert() in dag.rs (line 236-264). In this spec, honest vertices
 * reference at most one id per validator from the previous round, modeling
 * the equivocation detection. Byzantine equivocating vertices exist in the
 * global vertex set but honest validators' parent sets only see one per equivocator.
 *
 * Safety: No two finalized vertices from the same validator in the same round.
 * Liveness: Every vertex produced by an honest validator is eventually finalized
 *           (under fair scheduling).
 ***************************************************************************)

EXTENDS Integers, FiniteSets, TLC

\* =========================================================================
\* Constants — set via the .cfg file for bounded model checking.
\* =========================================================================

CONSTANTS
    VALIDATORS,       \* Set of all validator identifiers, e.g. {"v1","v2","v3","v4"}
    MAX_ROUNDS,       \* Maximum round number to explore
    MAX_BYZANTINE     \* Upper bound on number of Byzantine validators: floor((|VALIDATORS|-1)/3)

\* =========================================================================
\* Derived constants
\* =========================================================================

N == Cardinality(VALIDATORS)

\* Quorum threshold: ceil(2N/3), matching validator_set.rs line 94:
\*   (2 * effective_count + 2) / 3
QuorumThreshold == (2 * N + 2) \div 3

\* Upper bound on vertex IDs to keep state space finite for TLC.
\* Honest: N * MAX_ROUNDS. Byzantine equivocation: up to N * MAX_ROUNDS more.
MaxId == N * MAX_ROUNDS * 3

\* =========================================================================
\* State variables
\* =========================================================================

VARIABLES
    round,        \* Current global round number (monotonically increasing)
    vertices,     \* Set of vertex records: [validator, round, parents, id]
                  \*   - id distinguishes equivocating vertices (same validator+round)
    finalized,    \* Set of vertex records that have been finalized
    byzantine,    \* Subset of VALIDATORS that are Byzantine (fixed at init)
    active,       \* Subset of honest validators currently active (not crashed)
    nextId        \* Counter to generate unique vertex ids

vars == <<round, vertices, finalized, byzantine, active, nextId>>

\* =========================================================================
\* Type invariant — structural well-formedness
\* =========================================================================

TypeOK ==
    /\ round \in 0..MAX_ROUNDS
    /\ \A vtx \in vertices :
        /\ vtx.validator \in VALIDATORS
        /\ vtx.round \in 1..MAX_ROUNDS
        /\ vtx.id \in 1..(MaxId + 1)
        /\ vtx.parents \subseteq 1..MaxId
    /\ finalized \subseteq vertices
    /\ byzantine \subseteq VALIDATORS
    /\ active \subseteq (VALIDATORS \ byzantine)
    /\ nextId \in 1..(MaxId + 1)

\* =========================================================================
\* Helper operators
\* =========================================================================

\* All vertices produced by validator v in round r
VerticesOf(v, r) == {vtx \in vertices : vtx.validator = v /\ vtx.round = r}

\* All vertices in round r
VerticesInRound(r) == {vtx \in vertices : vtx.round = r}

\* Set of distinct validators that produced at least one vertex in round r
ValidatorsInRound(r) == {vtx.validator : vtx \in VerticesInRound(r)}

\* Set of ids of vertices in round r
IdsInRound(r) == {vtx.id : vtx \in VerticesInRound(r)}

\* Ids of vertices from validator val in round r
IdsOfValidatorInRound(val, r) == {vtx.id : vtx \in VerticesOf(val, r)}

(*
 * ValidParentSets(r): Set of all valid parent selections for round r.
 *
 * Models equivocation detection (dag.rs try_insert):
 * Each honest node's DAG contains at most ONE vertex per (validator, round).
 * When constructing parents from round r-1, an honest validator picks exactly
 * one vertex id per distinct validator that produced in round r-1.
 *
 * Non-deterministic choice across equivocating vertices models the fact that
 * different honest nodes may have received different versions from a Byzantine
 * equivocator (each node accepts whichever arrived first).
 *)
ValidParentSets(r) ==
    IF r = 1 THEN {{}}
    ELSE LET prevVals == ValidatorsInRound(r - 1)
         IN  IF prevVals = {} THEN {{}}
             ELSE {ps \in SUBSET IdsInRound(r - 1) :
                    \* Exactly one id per validator that produced in round r-1
                    /\ \A val \in prevVals :
                        Cardinality({pid \in ps : pid \in IdsOfValidatorInRound(val, r - 1)}) = 1
                    \* No extra ids
                    /\ Cardinality(ps) = Cardinality(prevVals)
                  }

\* Direct children: vertices that reference vtx.id as a parent
DirectChildren(vtx) == {v \in vertices : vtx.id \in v.parents}

\* All descendants of vtx (transitive closure through children).
RECURSIVE DescendantsAcc(_, _)
DescendantsAcc(frontier, acc) ==
    LET newChildren == UNION {DirectChildren(v) : v \in frontier} \ acc
    IN  IF newChildren = {} THEN acc
        ELSE DescendantsAcc(newChildren, acc \union newChildren)

Descendants(vtx) == DescendantsAcc({vtx}, {})

\* Distinct validators among descendants of vtx
DescendantValidators(vtx) ==
    {d.validator : d \in Descendants(vtx)}

\* Count of distinct validators with descendants of vtx
DescendantValidatorCount(vtx) == Cardinality(DescendantValidators(vtx))

\* An honest validator has not been designated Byzantine
IsHonest(v) == v \notin byzantine

\* Parent quorum check: does round r-1 have vertices from >= QuorumThreshold
\* distinct validators? This is the "2f+1 gate" from validator.rs.
PrevRoundHasQuorum(r) ==
    IF r = 1 THEN TRUE
    ELSE Cardinality(ValidatorsInRound(r - 1)) >= QuorumThreshold

\* =========================================================================
\* Initial state
\* =========================================================================

Init ==
    /\ round = 0
    /\ vertices = {}
    /\ finalized = {}
    \* Non-deterministically pick a set of Byzantine validators, up to MAX_BYZANTINE
    /\ byzantine \in {B \in SUBSET VALIDATORS : Cardinality(B) <= MAX_BYZANTINE}
    \* All honest validators start active
    /\ active = VALIDATORS \ byzantine
    /\ nextId = 1

\* =========================================================================
\* Actions
\* =========================================================================

(*
 * ProduceVertex(v, r): An honest, active validator v produces a vertex in round r.
 *
 * Preconditions (matching validator.rs):
 *   - v is honest and active
 *   - r is the next round to produce (round + 1, or current round if not yet produced)
 *   - v has NOT already produced a vertex in round r (equivocation prevention)
 *   - Round r-1 has >= QuorumThreshold distinct validators (2f+1 gate)
 *   - r <= MAX_ROUNDS, nextId <= MaxId (bounded model)
 *
 * Parent selection: picks exactly one vertex id per distinct validator from
 * round r-1 (models dag.rs equivocation detection — each honest DAG has at
 * most one vertex per validator per round).
 *)
ProduceVertex(v, r) ==
    /\ IsHonest(v)
    /\ v \in active
    /\ r >= 1
    /\ r <= MAX_ROUNDS
    /\ nextId <= MaxId
    /\ r <= round + 1
    \* Equivocation prevention: honest validator produces at most one vertex per round
    /\ VerticesOf(v, r) = {}
    \* 2f+1 gate: previous round must have quorum
    /\ PrevRoundHasQuorum(r)
    \* Create vertex with valid parent selection (one per validator from round r-1)
    /\ \E parentIds \in ValidParentSets(r) :
        LET newVertex == [validator |-> v, round |-> r, parents |-> parentIds, id |-> nextId]
        IN  /\ vertices' = vertices \union {newVertex}
            /\ nextId' = nextId + 1
    /\ round' = IF r > round THEN r ELSE round
    /\ UNCHANGED <<finalized, byzantine, active>>

(*
 * FinalizeVertex(vtx): A vertex becomes finalized when >= QuorumThreshold
 * distinct validators have produced descendants of it.
 *
 * Matches finality.rs check_finality():
 *   - descendant_validator_count(hash) >= quorum_threshold
 *
 * Additionally enforces the parent finality guarantee from find_newly_finalized():
 *   - All parents must be finalized first (or vertex has no parents — round 1)
 *)
FinalizeVertex(vtx) ==
    /\ vtx \in vertices
    /\ vtx \notin finalized
    \* BFT finality: >= ceil(2N/3) distinct validators have descendants
    /\ DescendantValidatorCount(vtx) >= QuorumThreshold
    \* Parent finality guarantee: all parents must be finalized
    /\ \A pid \in vtx.parents :
        \E fv \in finalized : fv.id = pid
    /\ finalized' = finalized \union {vtx}
    /\ UNCHANGED <<round, vertices, byzantine, active, nextId>>

(*
 * FinalizeForValidatorRound(v, r): Finalize some vertex from validator v in round r.
 * Projection onto finite (VALIDATORS x rounds) domain for TLC fairness/liveness.
 *)
FinalizeForValidatorRound(v, r) ==
    \E vtx \in vertices :
        /\ vtx.validator = v
        /\ vtx.round = r
        /\ vtx \notin finalized
        /\ DescendantValidatorCount(vtx) >= QuorumThreshold
        /\ \A pid \in vtx.parents :
            \E fv \in finalized : fv.id = pid
        /\ finalized' = finalized \union {vtx}
        /\ UNCHANGED <<round, vertices, byzantine, active, nextId>>

(*
 * ByzantineAction(v, r): A Byzantine validator can:
 *   (a) Produce an equivocating vertex (same round as an existing vertex — different id)
 *   (b) Produce a normal vertex in a new round
 *   (c) Stay silent (do nothing) — modeled by not taking this action
 *
 * Unlike honest validators, Byzantine validators:
 *   - Have NO equivocation prevention check (can produce multiple vertices per round)
 *   - Can choose arbitrary parent subsets (strategic parent selection)
 *   - Are NOT constrained by the 2f+1 gate
 *
 * In the actual protocol, equivocating vertices are detected by try_insert()
 * and rejected from honest nodes' DAGs. But they exist in the "global" view
 * and the safety property must hold despite their existence.
 *)
ByzantineAction(v, r) ==
    /\ v \in byzantine
    /\ r >= 1
    /\ r <= MAX_ROUNDS
    /\ r <= round + 1
    /\ nextId <= MaxId
    \* Byzantine can choose ANY subset of round r-1 ids as parents (strategic)
    /\ \E parentIds \in SUBSET (IF r = 1 THEN {} ELSE IdsInRound(r - 1)) :
        LET newVertex == [validator |-> v, round |-> r, parents |-> parentIds, id |-> nextId]
        IN  /\ vertices' = vertices \union {newVertex}
            /\ nextId' = nextId + 1
    /\ round' = IF r > round THEN r ELSE round
    /\ UNCHANGED <<finalized, byzantine, active>>

(*
 * AdvanceRound: The system can advance to the next round.
 * Models the round timer from validator.rs (tokio::interval).
 *)
AdvanceRound ==
    /\ round < MAX_ROUNDS
    /\ VerticesInRound(round) /= {} \/ round = 0
    /\ round' = round + 1
    /\ UNCHANGED <<vertices, finalized, byzantine, active, nextId>>

\* =========================================================================
\* Next-state relation
\* =========================================================================

Next ==
    \/ \E v \in VALIDATORS, r \in 1..MAX_ROUNDS : ProduceVertex(v, r)
    \/ \E vtx \in vertices : FinalizeVertex(vtx)
    \/ \E v \in VALIDATORS, r \in 1..MAX_ROUNDS : ByzantineAction(v, r)
    \/ AdvanceRound

\* =========================================================================
\* Fairness (for liveness)
\*
\* Quantified over finite VALIDATORS x 1..MAX_ROUNDS domain.
\* WF on ProduceVertex: honest validators produce when enabled.
\* SF on FinalizeForValidatorRound: eligible vertices eventually finalized.
\* WF on AdvanceRound: rounds progress.
\* =========================================================================

Fairness ==
    /\ \A v \in VALIDATORS : \A r \in 1..MAX_ROUNDS :
        WF_vars(ProduceVertex(v, r))
    /\ \A v \in VALIDATORS : \A r \in 1..MAX_ROUNDS :
        SF_vars(FinalizeForValidatorRound(v, r))
    /\ WF_vars(AdvanceRound)

\* =========================================================================
\* Specification
\* =========================================================================

Spec == Init /\ [][Next]_vars /\ Fairness

\* =========================================================================
\* Safety Properties
\* =========================================================================

(*
 * Safety: No equivocation among finalized vertices.
 * Two finalized vertices from the same validator in the same round
 * must be identical (same id). This is the core BFT safety guarantee.
 *
 * With f <= floor((N-1)/3) Byzantine validators and honest validators
 * selecting at most one vertex per (validator, round) as parent:
 * - At most one of any equivocating pair can accumulate >= ceil(2N/3) descendants
 * - Because honest validators (>= N-f >= ceil(2N/3)) each reference only ONE
 *   version, and finality requires ceil(2N/3) validator descendants
 *)
Conflicting(v1, v2) ==
    /\ v1.validator = v2.validator
    /\ v1.round = v2.round
    /\ v1.id /= v2.id

Safety == \A v1, v2 \in finalized : ~Conflicting(v1, v2)

(*
 * No equivocation from honest validators (stronger than Safety):
 * Honest validators never produce two vertices in the same round.
 *)
HonestNoEquivocation ==
    \A v \in VALIDATORS : \A r \in 1..MAX_ROUNDS :
        IsHonest(v) => Cardinality(VerticesOf(v, r)) <= 1

(*
 * Finalized vertices form a consistent history:
 * If a vertex is finalized, all its referenced parents are also finalized.
 *)
FinalizedParentsConsistency ==
    \A vtx \in finalized :
        \A pid \in vtx.parents :
            \E fv \in finalized : fv.id = pid

\* =========================================================================
\* Liveness Properties (temporal)
\*
\* Quantified over finite VALIDATORS x 1..MAX_ROUNDS domain.
\* =========================================================================

(*
 * Liveness: If an honest validator produces a vertex in round r, eventually
 * some vertex from that validator in that round is finalized.
 *)
Liveness == \A v \in VALIDATORS : \A r \in 1..MAX_ROUNDS :
    (IsHonest(v) /\ (\E vtx \in vertices : vtx.validator = v /\ vtx.round = r))
    ~> (\E vtx \in finalized : vtx.validator = v /\ vtx.round = r)

\* =========================================================================
\* Auxiliary invariants
\* =========================================================================

\* Round monotonicity: finalized vertices have round <= current round
RoundMonotonicity == \A vtx \in finalized : vtx.round <= round

\* Byzantine bound: at most MAX_BYZANTINE Byzantine validators
ByzantineBound == Cardinality(byzantine) <= MAX_BYZANTINE

\* Quorum threshold is correct for the given validator set size
QuorumCorrectness == QuorumThreshold = (2 * N + 2) \div 3

=============================================================================
