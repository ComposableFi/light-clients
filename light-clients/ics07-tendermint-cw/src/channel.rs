use crate::context::Context;
use ibc::{
	core::{
		ics04_channel::{
			channel::ChannelEnd,
			commitment::{AcknowledgementCommitment, PacketCommitment as PacketCommitmentType},
			context::ChannelReader,
			error::Error,
			packet::{Receipt, Sequence},
		},
		ics24_host::identifier::{ChannelId, ClientId, ConnectionId, PortId},
	},
	timestamp::Timestamp,
	Height,
};
use ics07_tendermint::HostFunctionsProvider;
use sha2::{Digest, Sha256};
use std::time::Duration;

impl<'a, H: HostFunctionsProvider> ChannelReader for Context<'a, H> {
	fn channel_end(&self, _port_channel_id: &(PortId, ChannelId)) -> Result<ChannelEnd, Error> {
		Err(Error::implementation_specific(
			"'channel_end' is unavailable from the client".to_string(),
		))
	}

	fn connection_channels(
		&self,
		_conn_id: &ConnectionId,
	) -> Result<Vec<(PortId, ChannelId)>, Error> {
		Err(Error::implementation_specific(
			"'connection_channels' is unavailable from the client".to_string(),
		))
	}

	fn get_next_sequence_send(
		&self,
		_port_channel_id: &(PortId, ChannelId),
	) -> Result<Sequence, Error> {
		Err(Error::implementation_specific(
			"'get_next_sequence_send' is unavailable from the client".to_string(),
		))
	}

	fn get_next_sequence_recv(
		&self,
		_port_channel_id: &(PortId, ChannelId),
	) -> Result<Sequence, Error> {
		Err(Error::implementation_specific(
			"'get_next_sequence_recv' is unavailable from the client".to_string(),
		))
	}

	fn get_next_sequence_ack(
		&self,
		_port_channel_id: &(PortId, ChannelId),
	) -> Result<Sequence, Error> {
		Err(Error::implementation_specific(
			"'get_next_sequence_ack' is unavailable from the client".to_string(),
		))
	}

	fn get_packet_commitment(
		&self,
		_key: &(PortId, ChannelId, Sequence),
	) -> Result<PacketCommitmentType, Error> {
		Err(Error::implementation_specific(
			"'get_packet_commitment' is unavailable from the client".to_string(),
		))
	}

	fn get_packet_receipt(&self, _key: &(PortId, ChannelId, Sequence)) -> Result<Receipt, Error> {
		Err(Error::implementation_specific(
			"'get_packet_receipt' is unavailable from the client".to_string(),
		))
	}

	fn get_packet_acknowledgement(
		&self,
		_key: &(PortId, ChannelId, Sequence),
	) -> Result<AcknowledgementCommitment, Error> {
		Err(Error::implementation_specific(
			"'get_packet_acknowledgement' is unavailable from the client".to_string(),
		))
	}

	/// A hashing function for packet commitments
	fn hash(&self, value: Vec<u8>) -> Vec<u8> {
		let mut hasher = Sha256::default();
		hasher.update(value);
		hasher.finalize().to_vec()
	}

	fn client_update_time(
		&self,
		_client_id: &ClientId,
		_height: Height,
	) -> Result<Timestamp, Error> {
		Err(Error::implementation_specific(
			"'client_update_time' is unavailable from the client".to_string(),
		))
	}

	fn client_update_height(
		&self,
		_client_id: &ClientId,
		_height: Height,
	) -> Result<Height, Error> {
		Err(Error::implementation_specific(
			"'client_update_height' is unavailable from the client".to_string(),
		))
	}

	/// Returns a counter on the number of channel ids have been created thus far.
	/// The value of this counter should increase only via method
	/// `ChannelKeeper::increase_channel_counter`.
	fn channel_counter(&self) -> Result<u64, Error> {
		Err(Error::implementation_specific(
			"'channel_counter' is unavailable from the client".to_string(),
		))
	}

	fn max_expected_time_per_block(&self) -> Duration {
		unimplemented!("'max_expected_time_per_block' is unavailable from the client")
	}
}
