use crate::{
	client_state::ClientState, consensus_state::ConsensusState, error::Error, header::Header,
};
use ibc::core::ics02_client::{
	client_consensus::ConsensusState as _, client_state::ClientState as _,
};

use crate::header::RelayChainHeader;
use alloc::vec::Vec;
use core::marker::PhantomData;
use grandpa_client_primitives::ParachainHeadersWithFinalityProof;
use ibc::{
	core::{
		ics02_client::{
			client_def::{ClientDef, ConsensusUpdateResult},
			error::Error as Ics02Error,
		},
		ics03_connection::connection::ConnectionEnd,
		ics04_channel::{
			channel::ChannelEnd,
			commitment::{AcknowledgementCommitment, PacketCommitment},
			packet::Sequence,
		},
		ics23_commitment::commitment::{CommitmentPrefix, CommitmentProofBytes, CommitmentRoot},
		ics24_host::{
			identifier::{ChannelId, ClientId, ConnectionId, PortId},
			path::{
				AcksPath, ChannelEndsPath, ClientConsensusStatePath, ClientStatePath,
				CommitmentsPath, ConnectionsPath, ReceiptsPath, SeqRecvsPath,
			},
		},
		ics26_routing::context::ReaderContext,
	},
	Height,
};
use light_client_common::{verify_membership, verify_non_membership};
use sp_runtime::{generic, OpaqueExtrinsic};
use tendermint_proto::Protobuf;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GrandpaClient<T>(PhantomData<T>);

type Block = generic::Block<RelayChainHeader, OpaqueExtrinsic>;

impl<H> ClientDef for GrandpaClient<H>
where
	H: light_client_common::HostFunctions + grandpa_client_primitives::HostFunctions,
{
	type Header = Header;
	type ClientState = ClientState<H>;
	type ConsensusState = ConsensusState;

	fn verify_header<Ctx: ReaderContext>(
		&self,
		_ctx: &Ctx,
		_client_id: ClientId,
		client_state: Self::ClientState,
		header: Self::Header,
	) -> Result<(), Ics02Error> {
		let headers_with_finality_proof = ParachainHeadersWithFinalityProof {
			finality_proof: header.finality_proof,
			parachain_headers: header.parachain_headers,
		};
		let client_state = grandpa_client_primitives::ClientState {
			current_authorities: client_state.current_authorities,
			current_set_id: client_state.current_set_id,
			latest_relay_hash: client_state.latest_relay_hash,
			para_id: client_state.para_id,
		};
		grandpa_client::verify_parachain_headers_with_grandpa_finality_proof::<Block, H>(
			client_state,
			headers_with_finality_proof,
		)
		.map_err(Error::GrandpaPrimitives)?;

		Ok(())
	}

	fn update_state<Ctx: ReaderContext>(
		&self,
		_ctx: &Ctx,
		_client_id: ClientId,
		_client_state: Self::ClientState,
		_header: Self::Header,
	) -> Result<(Self::ClientState, ConsensusUpdateResult<Ctx>), Ics02Error> {
		todo!()
	}

	fn update_state_on_misbehaviour(
		&self,
		_client_state: Self::ClientState,
		_header: Self::Header,
	) -> Result<Self::ClientState, Ics02Error> {
		todo!()
	}

	fn check_for_misbehaviour<Ctx: ReaderContext>(
		&self,
		_ctx: &Ctx,
		_client_id: ClientId,
		_client_state: Self::ClientState,
		_header: Self::Header,
	) -> Result<bool, Ics02Error> {
		todo!()
	}

	fn verify_upgrade_and_update_state<Ctx: ReaderContext>(
		&self,
		_client_state: &Self::ClientState,
		_consensus_state: &Self::ConsensusState,
		_proof_upgrade_client: Vec<u8>,
		_proof_upgrade_consensus_state: Vec<u8>,
	) -> Result<(Self::ClientState, ConsensusUpdateResult<Ctx>), Ics02Error> {
		todo!()
	}

	fn verify_client_consensus_state<Ctx: ReaderContext>(
		&self,
		_ctx: &Ctx,
		client_state: &Self::ClientState,
		height: Height,
		prefix: &CommitmentPrefix,
		proof: &CommitmentProofBytes,
		root: &CommitmentRoot,
		client_id: &ClientId,
		consensus_height: Height,
		expected_consensus_state: &Ctx::AnyConsensusState,
	) -> Result<(), Ics02Error> {
		client_state.verify_height(height)?;
		let path = ClientConsensusStatePath {
			client_id: client_id.clone(),
			epoch: consensus_height.revision_number,
			height: consensus_height.revision_height,
		};
		let value = expected_consensus_state.encode_to_vec();
		verify_membership::<H, _>(prefix, proof, root, path, value).map_err(Error::Anyhow)?;
		Ok(())
	}

	fn verify_connection_state<Ctx: ReaderContext>(
		&self,
		_ctx: &Ctx,
		_client_id: &ClientId,
		client_state: &Self::ClientState,
		height: Height,
		prefix: &CommitmentPrefix,
		proof: &CommitmentProofBytes,
		root: &CommitmentRoot,
		connection_id: &ConnectionId,
		expected_connection_end: &ConnectionEnd,
	) -> Result<(), Ics02Error> {
		client_state.verify_height(height)?;
		let path = ConnectionsPath(connection_id.clone());
		let value = expected_connection_end.encode_vec();
		verify_membership::<H, _>(prefix, proof, root, path, value).map_err(Error::Anyhow)?;
		Ok(())
	}

	fn verify_channel_state<Ctx: ReaderContext>(
		&self,
		_ctx: &Ctx,
		_client_id: &ClientId,
		client_state: &Self::ClientState,
		height: Height,
		prefix: &CommitmentPrefix,
		proof: &CommitmentProofBytes,
		root: &CommitmentRoot,
		port_id: &PortId,
		channel_id: &ChannelId,
		expected_channel_end: &ChannelEnd,
	) -> Result<(), Ics02Error> {
		client_state.verify_height(height)?;
		let path = ChannelEndsPath(port_id.clone(), *channel_id);
		let value = expected_channel_end.encode_vec();
		verify_membership::<H, _>(prefix, proof, root, path, value).map_err(Error::Anyhow)?;
		Ok(())
	}

	fn verify_client_full_state<Ctx: ReaderContext>(
		&self,
		_ctx: &Ctx,
		client_state: &Self::ClientState,
		height: Height,
		prefix: &CommitmentPrefix,
		proof: &CommitmentProofBytes,
		root: &CommitmentRoot,
		client_id: &ClientId,
		expected_client_state: &Ctx::AnyClientState,
	) -> Result<(), Ics02Error> {
		client_state.verify_height(height)?;
		let path = ClientStatePath(client_id.clone());
		let value = expected_client_state.encode_to_vec();
		verify_membership::<H, _>(prefix, proof, root, path, value).map_err(Error::Anyhow)?;
		Ok(())
	}

	fn verify_packet_data<Ctx: ReaderContext>(
		&self,
		ctx: &Ctx,
		_client_id: &ClientId,
		client_state: &Self::ClientState,
		height: Height,
		connection_end: &ConnectionEnd,
		proof: &CommitmentProofBytes,
		root: &CommitmentRoot,
		port_id: &PortId,
		channel_id: &ChannelId,
		sequence: Sequence,
		commitment: PacketCommitment,
	) -> Result<(), Ics02Error> {
		client_state.verify_height(height)?;
		verify_delay_passed::<H, _>(ctx, height, connection_end)?;

		let commitment_path =
			CommitmentsPath { port_id: port_id.clone(), channel_id: *channel_id, sequence };

		verify_membership::<H, _>(
			connection_end.counterparty().prefix(),
			proof,
			root,
			commitment_path,
			commitment.into_vec(),
		)
		.map_err(Error::Anyhow)?;
		Ok(())
	}

	fn verify_packet_acknowledgement<Ctx: ReaderContext>(
		&self,
		ctx: &Ctx,
		_client_id: &ClientId,
		client_state: &Self::ClientState,
		height: Height,
		connection_end: &ConnectionEnd,
		proof: &CommitmentProofBytes,
		root: &CommitmentRoot,
		port_id: &PortId,
		channel_id: &ChannelId,
		sequence: Sequence,
		ack: AcknowledgementCommitment,
	) -> Result<(), Ics02Error> {
		client_state.verify_height(height)?;
		verify_delay_passed::<H, _>(ctx, height, connection_end)?;

		let ack_path = AcksPath { port_id: port_id.clone(), channel_id: *channel_id, sequence };
		verify_membership::<H, _>(
			connection_end.counterparty().prefix(),
			proof,
			root,
			ack_path,
			ack.into_vec(),
		)
		.map_err(Error::Anyhow)?;
		Ok(())
	}

	fn verify_next_sequence_recv<Ctx: ReaderContext>(
		&self,
		ctx: &Ctx,
		_client_id: &ClientId,
		client_state: &Self::ClientState,
		height: Height,
		connection_end: &ConnectionEnd,
		proof: &CommitmentProofBytes,
		root: &CommitmentRoot,
		port_id: &PortId,
		channel_id: &ChannelId,
		sequence: Sequence,
	) -> Result<(), Ics02Error> {
		client_state.verify_height(height)?;
		verify_delay_passed::<H, _>(ctx, height, connection_end)?;

		let seq_bytes = codec::Encode::encode(&u64::from(sequence));

		let seq_path = SeqRecvsPath(port_id.clone(), *channel_id);
		verify_membership::<H, _>(
			connection_end.counterparty().prefix(),
			proof,
			root,
			seq_path,
			seq_bytes,
		)
		.map_err(Error::Anyhow)?;
		Ok(())
	}

	fn verify_packet_receipt_absence<Ctx: ReaderContext>(
		&self,
		ctx: &Ctx,
		_client_id: &ClientId,
		client_state: &Self::ClientState,
		height: Height,
		connection_end: &ConnectionEnd,
		proof: &CommitmentProofBytes,
		root: &CommitmentRoot,
		port_id: &PortId,
		channel_id: &ChannelId,
		sequence: Sequence,
	) -> Result<(), Ics02Error> {
		client_state.verify_height(height)?;
		verify_delay_passed::<H, _>(ctx, height, connection_end)?;

		let receipt_path =
			ReceiptsPath { port_id: port_id.clone(), channel_id: *channel_id, sequence };
		verify_non_membership::<H, _>(
			connection_end.counterparty().prefix(),
			proof,
			root,
			receipt_path,
		)
		.map_err(Error::Anyhow)?;
		Ok(())
	}
}

fn verify_delay_passed<H, C>(
	ctx: &C,
	height: Height,
	connection_end: &ConnectionEnd,
) -> Result<(), Error>
where
	H: Clone,
	C: ReaderContext,
{
	let current_timestamp = ctx.host_timestamp();
	let current_height = ctx.host_height();

	let client_id = connection_end.client_id();
	let processed_time = ctx.client_update_time(client_id, height).map_err(Error::from)?;
	let processed_height = ctx.client_update_height(client_id, height).map_err(Error::from)?;

	let delay_period_time = connection_end.delay_period();
	let delay_period_height = ctx.block_delay(delay_period_time);

	ClientState::<()>::verify_delay_passed(
		current_timestamp,
		current_height,
		processed_time,
		processed_height,
		delay_period_time,
		delay_period_height,
	)
}
