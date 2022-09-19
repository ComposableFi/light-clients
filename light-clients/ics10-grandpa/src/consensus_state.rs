use alloc::{format, vec, vec::Vec};
use anyhow::anyhow;
use codec::Decode;
use core::{convert::Infallible, fmt::Debug};
use serde::Serialize;
use tendermint::time::Time;
use tendermint_proto::{google::protobuf as tpb, Protobuf};

use crate::proto::ConsensusState as RawConsensusState;

use crate::error::Error;
use grandpa_client_primitives::{parachain_header_storage_key, ParachainHeaderProofs};
use ibc::{core::ics23_commitment::commitment::CommitmentRoot, timestamp::Timestamp};
use light_client_common::decode_timestamp_extrinsic;
use primitive_types::H256;
use sp_runtime::{generic, traits::BlakeTwo256, SaturatedConversion};
use sp_trie::StorageProof;

/// Protobuf type url for GRANDPA Consensus State
pub const GRANDPA_CONSENSUS_STATE_TYPE_URL: &str = "/ibc.lightclients.grandpa.v1.ConsensusState";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConsensusState {
	pub timestamp: Time,
	pub root: CommitmentRoot,
}

impl ConsensusState {
	pub fn new(root: Vec<u8>, timestamp: Time) -> Self {
		Self { timestamp, root: root.into() }
	}

	pub fn from_header<H>(
		parachain_header_proof: ParachainHeaderProofs,
		para_id: u32,
		relay_state_root: H256,
	) -> Result<Self, Error>
	where
		H: grandpa_client_primitives::HostFunctions,
	{
		let key = parachain_header_storage_key(para_id);
		let proof = StorageProof::new(parachain_header_proof.state_proof);
		let parachain_header_bytes = H::read_proof_check(
			relay_state_root.as_fixed_bytes(),
			proof,
			vec![parachain_header_storage_key(para_id)],
		)?
		.remove(key.as_ref())
		.flatten()
		.ok_or_else(|| anyhow!("Invalid state proof for parachain header"))?;

		let parachain_header =
			generic::Header::<u32, BlakeTwo256>::decode(&mut &parachain_header_bytes[..])?;
		let root = parachain_header.state_root.0.to_vec();

		let timestamp = decode_timestamp_extrinsic(&parachain_header_proof.extrinsic)?;
		let duration = core::time::Duration::from_millis(timestamp);
		let timestamp = Timestamp::from_nanoseconds(duration.as_nanos().saturated_into::<u64>())?
			.into_tm_time()
			.ok_or_else(|| anyhow!("Error decoding Timestamp, timestamp cannot be zero"))?;

		Ok(Self { root: root.into(), timestamp })
	}
}

impl ibc::core::ics02_client::client_consensus::ConsensusState for ConsensusState {
	type Error = Infallible;

	fn root(&self) -> &CommitmentRoot {
		&self.root
	}

	fn timestamp(&self) -> Timestamp {
		self.timestamp.into()
	}

	fn encode_to_vec(&self) -> Vec<u8> {
		self.encode_vec()
	}
}

impl Protobuf<RawConsensusState> for ConsensusState {}

impl TryFrom<RawConsensusState> for ConsensusState {
	type Error = Error;

	fn try_from(raw: RawConsensusState) -> Result<Self, Self::Error> {
		let prost_types::Timestamp { seconds, nanos } = raw
			.timestamp
			.ok_or_else(|| Error::Custom(format!("Invalid consensus state: missing timestamp")))?;
		let proto_timestamp = tpb::Timestamp { seconds, nanos };
		let timestamp = proto_timestamp.try_into().map_err(|e| {
			Error::Custom(format!("Invalid consensus state: invalid timestamp {e}"))
		})?;

		Ok(Self { root: raw.root.into(), timestamp })
	}
}

impl From<ConsensusState> for RawConsensusState {
	fn from(value: ConsensusState) -> Self {
		let tpb::Timestamp { seconds, nanos } = value.timestamp.into();
		let timestamp = prost_types::Timestamp { seconds, nanos };

		RawConsensusState { timestamp: Some(timestamp), root: value.root.into_vec() }
	}
}

#[cfg(any(test, feature = "mocks"))]
pub mod test_util {
	use super::*;
	use crate::mock::AnyConsensusState;

	pub fn get_dummy_beefy_consensus_state() -> AnyConsensusState {
		AnyConsensusState::Grandpa(ConsensusState {
			timestamp: Time::now(),
			root: vec![0; 32].into(),
		})
	}
}
