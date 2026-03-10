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

\* Upper bound on vertex IDs. Honest validators produce at most N * MAX_ROUNDS.
\* Byzantine validators can equivocate, but we bound total vertices to keep
\* the state space finite for TLC.
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
\* Checks constraints on individual vertex fields without defining an
\* enumerable VertexType set (which would be infinite or combinatorially
\* explosive due to the parents subset field).
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

\* Set of ids of vertices in round r (used as parent references)
IdsInRound(r) == {vtx.id : vtx \in VerticesInRound(r)}

\* Direct children: vertices that reference vtx.id as a parent
DirectChildren(vtx) == {v \in vertices : vtx.id \in v.parents}

\* All descendants of vtx (transitive closure through children).
\* Since MAX_ROUNDS is small, we compute this via fixed-point iteration.
RECURSIVE DescendantsAcc(_, _)
DescendantsAcc(frontier, acc) ==
    LET newChildren == UNION {DirectChildren(v) : v \in frontier} \ acc
    IN  IF newChildren = {} THEN acc
        ELSE DescendantsAcc(newChildren, acc \union newChildren)

Descendants(vtx) == DescendantsAcc({vtx}, {})

\* Distinct validators among a set of descendants
DescendantValidators(vtx) ==
    {d.validator : d \in Descendants(vtx)}

\* Count of distinct validators with descendants of vtx
DescendantValidatorCount(vtx) == Cardinality(DescendantValidators(vtx))

\* An honest validator has not been designated Byzantine
IsHonest(v) == v \notin byzantine

\* Parent quorum check: does round r-1 have vertices from >= QuorumThreshold
\* distinct validators? This is the "2f+1 gate" from validator.rs.
PrevRoundHasQuorum(r) ==
    IF r = 1 THEN TRUE \* Round 1 has no prior round requirement
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
 *   - r <= MAX_ROUNDS (bounded model)
 *   - nextId <= MaxId (bounded model)
 *
 * The vertex references all vertex ids from round r-1 as parents
 * (matching the "vertices_in_round(prev_round)" parent selection from validator.rs).
 *)
ProduceVertex(v, r) ==
    /\ IsHonest(v)
    /\ v \in active
    /\ r >= 1
    /\ r <= MAX_ROUNDS
    /\ nextId <= MaxId
    \* Validator produces for current frontier: round must be <= round + 1
    /\ r <= round + 1
    \* Equivocation prevention: honest validator produces at most one vertex per round
    /\ VerticesOf(v, r) = {}
    \* 2f+1 gate: previous round must have quorum
    /\ PrevRoundHasQuorum(r)
    \* Create vertex referencing all vertices from round r-1
    /\ LET parentIds == IF r = 1 THEN {} ELSE IdsInRound(r - 1)
           newVertex == [validator |-> v, round |-> r, parents |-> parentIds, id |-> nextId]
       IN  /\ vertices' = vertices \union {newVertex}
           /\ nextId' = nextId + 1
    \* Advance global round if this vertex is in a new round
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
 *   - All parents must be finalized first (or be the genesis sentinel)
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
 * This is a projection of FinalizeVertex onto the finite (VALIDATORS x rounds) domain,
 * allowing TLC to express fairness constraints without enumerating an infinite type.
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
 *   (a) Produce an equivocating vertex (same round, different content/id)
 *   (b) Stay silent (do nothing) — modeled by not taking this action
 *
 * Byzantine vertices have unique ids (different content), creating equivocation.
 * The DAG in dag.rs try_insert() detects this and marks the validator as Byzantine,
 * but the equivocating vertex may still be referenced by other vertices before detection.
 *)
ByzantineAction(v, r) ==
    /\ v \in byzantine
    /\ r >= 1
    /\ r <= MAX_ROUNDS
    /\ r <= round + 1
    /\ nextId <= MaxId
    \* Byzantine validator produces a vertex (possibly equivocating — no uniqueness check)
    /\ LET parentIds == IF r = 1 THEN {} ELSE IdsInRound(r - 1)
           newVertex == [validator |-> v, round |-> r, parents |-> parentIds, id |-> nextId]
       IN  /\ vertices' = vertices \union {newVertex}
           /\ nextId' = nextId + 1
    /\ round' = IF r > round THEN r ELSE round
    /\ UNCHANGED <<finalized, byzantine, active>>

(*
 * AdvanceRound: The system can advance to the next round.
 * Models the round timer from validator.rs (tokio::interval).
 * Only advances if current round has at least one vertex.
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
\* We quantify over the finite domains VALIDATORS x 1..MAX_ROUNDS rather than
\* the infinite VertexType. WF on ProduceVertex ensures honest validators
\* produce when enabled. SF on FinalizeForValidatorRound ensures that if a
\* vertex from (v,r) becomes repeatedly eligible for finalization, it is
\* eventually finalized. WF on AdvanceRound ensures rounds progress.
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
 * In the Rust code, equivocation is prevented for honest validators at
 * production time (has_vertex_from_validator_in_round check in validator.rs)
 * and detected for Byzantine validators at insertion time (try_insert in dag.rs).
 * Finality requires 2/3+ descendants, so with f <= floor((N-1)/3) Byzantine
 * validators, conflicting vertices cannot both reach finality.
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
\* Quantified over the finite VALIDATORS x 1..MAX_ROUNDS domain so TLC
\* can evaluate it (avoids enumerating the infinite VertexType set).
\* =========================================================================

(*
 * Liveness: If an honest validator produces a vertex in round r, eventually
 * some vertex from that validator in that round is finalized.
 *)
Liveness == \A v \in VALIDATORS : \A r \in 1..MAX_ROUNDS :
    (IsHonest(v) /\ (\E vtx \in vertices : vtx.validator = v /\ vtx.round = r))
    ~> (\E vtx \in finalized : vtx.validator = v /\ vtx.round = r)

\* =========================================================================
\* Auxiliary invariants (help TLC prove properties faster)
\* =========================================================================

\* Round monotonicity: finalized vertices have round <= current round
RoundMonotonicity == \A vtx \in finalized : vtx.round <= round

\* Byzantine bound: at most MAX_BYZANTINE Byzantine validators
ByzantineBound == Cardinality(byzantine) <= MAX_BYZANTINE

\* Quorum threshold is correct for the given validator set size
QuorumCorrectness == QuorumThreshold = (2 * N + 2) \div 3

=============================================================================
