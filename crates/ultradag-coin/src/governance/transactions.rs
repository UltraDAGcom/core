use serde::{Deserialize, Serialize};
use crate::address::{Address, Signature};
use crate::governance::ProposalType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProposalTx {
    pub from: Address,
    pub proposal_id: u64,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl CreateProposalTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"proposal");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.proposal_id.to_le_bytes());
        // Length-delimit variable-length fields to prevent concatenation ambiguity
        // ("AB"+"CD" must differ from "ABC"+"D")
        buf.extend_from_slice(&(self.title.len() as u32).to_le_bytes());
        buf.extend_from_slice(self.title.as_bytes());
        buf.extend_from_slice(&(self.description.len() as u32).to_le_bytes());
        buf.extend_from_slice(self.description.as_bytes());

        match &self.proposal_type {
            ProposalType::TextProposal => {
                buf.push(0);
            }
            ProposalType::ParameterChange { param, new_value } => {
                buf.push(1);
                buf.extend_from_slice(&(param.len() as u32).to_le_bytes());
                buf.extend_from_slice(param.as_bytes());
                buf.extend_from_slice(&(new_value.len() as u32).to_le_bytes());
                buf.extend_from_slice(new_value.as_bytes());
            }
        }
        
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.from.0);
        hasher.update(&self.proposal_id.to_le_bytes());
        hasher.update(self.title.as_bytes());
        hasher.update(self.description.as_bytes());
        match &self.proposal_type {
            ProposalType::TextProposal => {
                hasher.update(&[0]);
            }
            ProposalType::ParameterChange { param, new_value } => {
                hasher.update(&[1]);
                hasher.update(param.as_bytes());
                hasher.update(new_value.as_bytes());
            }
        }
        hasher.update(&self.fee.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify_signature(&self) -> bool {
        let expected_addr = Address(*blake3::hash(&self.pub_key).as_bytes());
        if expected_addr != self.from {
            return false;
        }

        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };

        self.signature.verify(&vk, &self.signable_bytes())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteTx {
    pub from: Address,
    pub proposal_id: u64,
    pub vote: bool,
    pub fee: u64,
    pub nonce: u64,
    pub pub_key: [u8; 32],
    pub signature: Signature,
}

impl VoteTx {
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(100);
        buf.extend_from_slice(crate::constants::NETWORK_ID);
        buf.extend_from_slice(b"vote");
        buf.extend_from_slice(&self.from.0);
        buf.extend_from_slice(&self.proposal_id.to_le_bytes());
        buf.push(if self.vote { 1 } else { 0 });
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.from.0);
        hasher.update(&self.proposal_id.to_le_bytes());
        hasher.update(&[if self.vote { 1 } else { 0 }]);
        hasher.update(&self.fee.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify_signature(&self) -> bool {
        let expected_addr = Address(*blake3::hash(&self.pub_key).as_bytes());
        if expected_addr != self.from {
            return false;
        }

        let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&self.pub_key) else {
            return false;
        };

        self.signature.verify(&vk, &self.signable_bytes())
    }
}
