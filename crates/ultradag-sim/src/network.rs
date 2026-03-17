use ultradag_coin::DagVertex;
use rand::seq::SliceRandom;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;
use std::collections::VecDeque;

/// Delivery policy for the virtual network.
#[derive(Clone)]
pub enum DeliveryPolicy {
    /// All messages arrive immediately in send order.
    Perfect,
    /// Messages arrive in random order (shuffled per round).
    RandomOrder,
    /// Messages are dropped with the given probability (0.0–1.0).
    Drop { probability: f64 },
    /// Partition: messages between the two groups are dropped.
    /// Validators split into [0..split) and [split..n).
    /// Healing happens after `heal_after_rounds` rounds.
    Partition { split: usize, heal_after_rounds: u64 },
    /// Combined: reorder + drop.
    Lossy { drop_probability: f64 },
}

struct InFlightMessage {
    vertex: DagVertex,
    from: usize,
    to: usize,
}

pub struct VirtualNetwork {
    inboxes: Vec<VecDeque<DagVertex>>,
    pending: Vec<InFlightMessage>,
    num_validators: usize,
    policy: DeliveryPolicy,
    rng: ChaCha8Rng,
    current_round: u64,
    pub messages_sent: u64,
    pub messages_dropped: u64,
}

impl VirtualNetwork {
    pub fn new(num_validators: usize, policy: DeliveryPolicy, seed: u64) -> Self {
        Self {
            inboxes: (0..num_validators).map(|_| VecDeque::new()).collect(),
            pending: Vec::new(),
            num_validators,
            policy,
            rng: ChaCha8Rng::seed_from_u64(seed),
            current_round: 0,
            messages_sent: 0,
            messages_dropped: 0,
        }
    }

    /// Broadcast a vertex from `from` to all other validators.
    pub fn broadcast(&mut self, from: usize, vertex: DagVertex) {
        for to in 0..self.num_validators {
            if to != from {
                self.pending.push(InFlightMessage {
                    vertex: vertex.clone(),
                    from,
                    to,
                });
                self.messages_sent += 1;
            }
        }
    }

    /// Send a vertex to specific targets only.
    pub fn send_to(&mut self, from: usize, vertex: DagVertex, targets: &[usize]) {
        for &to in targets {
            if to != from && to < self.num_validators {
                self.pending.push(InFlightMessage {
                    vertex: vertex.clone(),
                    from,
                    to,
                });
                self.messages_sent += 1;
            }
        }
    }

    /// Process pending messages according to the delivery policy.
    pub fn deliver(&mut self, round: u64) {
        self.current_round = round;

        match self.policy.clone() {
            DeliveryPolicy::Perfect => {
                for msg in self.pending.drain(..) {
                    self.inboxes[msg.to].push_back(msg.vertex);
                }
            }
            DeliveryPolicy::RandomOrder => {
                self.pending.shuffle(&mut self.rng);
                for msg in self.pending.drain(..) {
                    self.inboxes[msg.to].push_back(msg.vertex);
                }
            }
            DeliveryPolicy::Drop { probability } => {
                let mut kept = Vec::new();
                for msg in self.pending.drain(..) {
                    if self.rng.gen::<f64>() >= probability {
                        kept.push(msg);
                    } else {
                        self.messages_dropped += 1;
                    }
                }
                for msg in kept {
                    self.inboxes[msg.to].push_back(msg.vertex);
                }
            }
            DeliveryPolicy::Partition { split, heal_after_rounds } => {
                let healed = round >= heal_after_rounds;
                let mut kept = Vec::new();
                for msg in self.pending.drain(..) {
                    let from_group = msg.from < split;
                    let to_group = msg.to < split;
                    if healed || from_group == to_group {
                        kept.push(msg);
                    } else {
                        self.messages_dropped += 1;
                    }
                }
                for msg in kept {
                    self.inboxes[msg.to].push_back(msg.vertex);
                }
            }
            DeliveryPolicy::Lossy { drop_probability } => {
                self.pending.shuffle(&mut self.rng);
                let mut kept = Vec::new();
                for msg in self.pending.drain(..) {
                    if self.rng.gen::<f64>() >= drop_probability {
                        kept.push(msg);
                    } else {
                        self.messages_dropped += 1;
                    }
                }
                for msg in kept {
                    self.inboxes[msg.to].push_back(msg.vertex);
                }
            }
        }
    }

    /// Take all messages from a validator's inbox.
    pub fn drain_inbox(&mut self, validator_idx: usize) -> Vec<DagVertex> {
        self.inboxes[validator_idx].drain(..).collect()
    }

    /// Change policy mid-simulation.
    pub fn set_policy(&mut self, policy: DeliveryPolicy) {
        self.policy = policy;
    }
}
