// Copyright 2022 ComposableFi
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(unreachable_patterns)]

use async_trait::async_trait;
#[cfg(feature = "cosmos")]
use cosmos::client::{CosmosClient, CosmosClientConfig};
use derive_more::From;
use futures::Stream;
#[cfg(any(test, feature = "testing"))]
use ibc::applications::transfer::msgs::transfer::MsgTransfer;
use ibc::{
	applications::transfer::PrefixedCoin,
	core::{
		ics02_client::{
			client_state::ClientType,
			events::{CodeId, UpdateClient},
			msgs::{create_client::MsgCreateAnyClient, update_client::MsgUpdateAnyClient},
		},
		ics03_connection::msgs::{
			conn_open_ack::MsgConnectionOpenAck, conn_open_init::MsgConnectionOpenInit,
			conn_open_try::MsgConnectionOpenTry,
		},
		ics23_commitment::commitment::CommitmentPrefix,
		ics24_host::identifier::{ChannelId, ClientId, ConnectionId, PortId},
	},
	downcast,
	events::IbcEvent,
	signer::Signer,
	timestamp::Timestamp,
	tx_msg::Msg,
	Height,
};
use ibc_proto::{
	google::protobuf::Any,
	ibc::core::{
		channel::v1::{
			QueryChannelResponse, QueryChannelsResponse, QueryNextSequenceReceiveResponse,
			QueryPacketAcknowledgementResponse, QueryPacketCommitmentResponse,
			QueryPacketReceiptResponse,
		},
		client::v1::{QueryClientStateResponse, QueryConsensusStateResponse},
		connection::v1::{IdentifiedConnection, QueryConnectionResponse},
	},
};
use ics08_wasm::Bytes;
use pallet_ibc::light_clients::{AnyClientMessage, AnyClientState, AnyConsensusState};
#[cfg(any(test, feature = "testing"))]
use pallet_ibc::Timeout;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use ibc::core::ics02_client::events::UpdateClient;
use pallet_ibc::light_clients::{AnyClientState, AnyConsensusState};
use parachain::{config, ParachainClient};
use primitives::{
	mock::LocalClientTypes, Chain, IbcProvider, KeyProvider, MisbehaviourHandler, UpdateType,
};
use serde::Deserialize;
use sp_runtime::generic::Era;
use std::{pin::Pin, time::Duration};
#[cfg(feature = "dali")]
use subxt::tx::{
	SubstrateExtrinsicParams as ParachainExtrinsicParams,
	SubstrateExtrinsicParamsBuilder as ParachainExtrinsicsParamsBuilder,
};
use subxt::{tx::ExtrinsicParams, Error, OnlineClient};

#[cfg(not(feature = "dali"))]
use subxt::tx::{
	PolkadotExtrinsicParams as ParachainExtrinsicParams,
	PolkadotExtrinsicParamsBuilder as ParachainExtrinsicsParamsBuilder,
};
use tendermint_proto::Protobuf;
use thiserror::Error;

// TODO: expose extrinsic param builder
#[derive(Debug, Clone)]
pub enum DefaultConfig {}

#[async_trait]
impl config::Config for DefaultConfig {
	type AssetId = u128;
	async fn custom_extrinsic_params(
		client: &OnlineClient<Self>,
	) -> Result<
		<Self::ExtrinsicParams as ExtrinsicParams<Self::Index, Self::Hash>>::OtherParams,
		Error,
	> {
		let params =
			ParachainExtrinsicsParamsBuilder::new().era(Era::Immortal, client.genesis_hash());
		Ok(params.into())
	}
}

impl subxt::Config for DefaultConfig {
	type Index = u32;
	type BlockNumber = u32;
	type Hash = sp_core::H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = sp_runtime::AccountId32;
	type Address = sp_runtime::MultiAddress<Self::AccountId, u32>;
	type Header = sp_runtime::generic::Header<Self::BlockNumber, sp_runtime::traits::BlakeTwo256>;
	type Signature = sp_runtime::MultiSignature;
	type Extrinsic = sp_runtime::OpaqueExtrinsic;
	type ExtrinsicParams = ParachainExtrinsicParams<Self>;
}

#[derive(Serialize, Deserialize)]
pub struct Config {
	pub chain_a: AnyConfig,
	pub chain_b: AnyConfig,
	pub core: CoreConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnyConfig {
	Parachain(parachain::ParachainClientConfig),
	#[cfg(feature = "cosmos")]
	Cosmos(CosmosClientConfig),
}

#[derive(Serialize, Deserialize)]
pub struct CoreConfig {
	pub prometheus_endpoint: Option<String>,
}

#[derive(Clone)]
pub struct WasmChain {
	pub inner: Box<AnyChain>,
	pub code_id: Bytes,
	pub client_type: ClientType,
}

#[derive(Clone)]
pub enum AnyChain {
	Parachain(ParachainClient<DefaultConfig>),
	#[cfg(feature = "cosmos")]
	Cosmos(CosmosClient<DefaultConfig>),
	Wasm(WasmChain),
}

#[derive(From)]
pub enum AnyFinalityEvent {
	Parachain(parachain::finality_protocol::FinalityEvent),
	#[cfg(feature = "cosmos")]
	Cosmos(cosmos::provider::FinalityEvent),
}

#[derive(From, Debug)]
pub enum AnyTransactionId {
	Parachain(parachain::provider::TransactionId<sp_core::H256>),
	#[cfg(feature = "cosmos")]
	Cosmos(cosmos::provider::TransactionId<cosmos::provider::Hash>),
}

#[derive(Error, Debug)]
pub enum AnyError {
	#[error("{0}")]
	Parachain(#[from] parachain::error::Error),
	#[cfg(feature = "cosmos")]
	#[error("{0}")]
	Cosmos(#[from] cosmos::error::Error),
	#[error("{0}")]
	Other(String),
}

impl From<String> for AnyError {
	fn from(s: String) -> Self {
		Self::Other(s)
	}
}

#[async_trait]
impl IbcProvider for AnyChain {
	type FinalityEvent = AnyFinalityEvent;
	type TransactionId = AnyTransactionId;
	type Error = AnyError;

	async fn query_latest_ibc_events<T>(
		&mut self,
		finality_event: Self::FinalityEvent,
		counterparty: &T,
	) -> Result<(Any, Vec<IbcEvent>, UpdateType), anyhow::Error>
	where
		T: Chain,
	{
		match self {
			AnyChain::Parachain(chain) => {
				let finality_event = downcast!(finality_event => AnyFinalityEvent::Parachain)
					.ok_or_else(|| AnyError::Other("Invalid finality event type".to_owned()))?;
				let (client_msg, events, update_type) =
					chain.query_latest_ibc_events(finality_event, counterparty).await?;
				Ok((client_msg, events, update_type))
			},
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => {
				let finality_event = downcast!(finality_event => AnyFinalityEvent::Cosmos)
					.ok_or_else(|| AnyError::Other("Invalid finality event type".to_owned()))?;
				let (client_msg, events, update_type) =
					chain.query_latest_ibc_events(finality_event, counterparty).await?;
				Ok((client_msg, events, update_type))
			},
			AnyChain::Wasm(c) =>
				c.inner.query_latest_ibc_events(finality_event, counterparty).await,
		}
	}

	async fn ibc_events(&self) -> Pin<Box<dyn Stream<Item = IbcEvent> + Send + 'static>> {
		match self {
			Self::Parachain(chain) => chain.ibc_events().await,
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.ibc_events().await,
			Self::Wasm(c) => c.inner.ibc_events().await,
		}
	}

	async fn query_client_consensus(
		&self,
		at: Height,
		client_id: ClientId,
		consensus_height: Height,
	) -> Result<QueryConsensusStateResponse, Self::Error> {
		match self {
			AnyChain::Parachain(chain) => chain
				.query_client_consensus(at, client_id, consensus_height)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain
				.query_client_consensus(at, client_id, consensus_height)
				.await
				.map_err(Into::into),
			AnyChain::Wasm(c) =>
				c.inner.query_client_consensus(at, client_id, consensus_height).await,
		}
	}

	async fn query_client_state(
		&self,
		at: Height,
		client_id: ClientId,
	) -> Result<QueryClientStateResponse, Self::Error> {
		match self {
			AnyChain::Parachain(chain) =>
				chain.query_client_state(at, client_id).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain.query_client_state(at, client_id).await.map_err(Into::into),
			AnyChain::Wasm(c) => c.inner.query_client_state(at, client_id).await,
		}
	}

	async fn query_connection_end(
		&self,
		at: Height,
		connection_id: ConnectionId,
	) -> Result<QueryConnectionResponse, Self::Error> {
		match self {
			AnyChain::Parachain(chain) =>
				chain.query_connection_end(at, connection_id).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) =>
				chain.query_connection_end(at, connection_id).await.map_err(Into::into),
			AnyChain::Wasm(c) => c.inner.query_connection_end(at, connection_id).await,
		}
	}

	async fn query_channel_end(
		&self,
		at: Height,
		channel_id: ChannelId,
		port_id: PortId,
	) -> Result<QueryChannelResponse, Self::Error> {
		match self {
			AnyChain::Parachain(chain) =>
				chain.query_channel_end(at, channel_id, port_id).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) =>
				chain.query_channel_end(at, channel_id, port_id).await.map_err(Into::into),
			AnyChain::Wasm(c) => c.inner.query_channel_end(at, channel_id, port_id).await,
		}
	}

	async fn query_proof(&self, at: Height, keys: Vec<Vec<u8>>) -> Result<Vec<u8>, Self::Error> {
		match self {
			AnyChain::Parachain(chain) => chain.query_proof(at, keys).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain.query_proof(at, keys).await.map_err(Into::into),
			AnyChain::Wasm(c) => c.inner.query_proof(at, keys).await,
		}
	}

	async fn query_packet_commitment(
		&self,
		at: Height,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: u64,
	) -> Result<QueryPacketCommitmentResponse, Self::Error> {
		match self {
			AnyChain::Parachain(chain) => chain
				.query_packet_commitment(at, port_id, channel_id, seq)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain
				.query_packet_commitment(at, port_id, channel_id, seq)
				.await
				.map_err(Into::into),
			AnyChain::Wasm(c) =>
				c.inner.query_packet_commitment(at, port_id, channel_id, seq).await,
		}
	}

	async fn query_packet_acknowledgement(
		&self,
		at: Height,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: u64,
	) -> Result<QueryPacketAcknowledgementResponse, Self::Error> {
		match self {
			AnyChain::Parachain(chain) => chain
				.query_packet_acknowledgement(at, port_id, channel_id, seq)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain
				.query_packet_acknowledgement(at, port_id, channel_id, seq)
				.await
				.map_err(Into::into),
			AnyChain::Wasm(c) =>
				c.inner.query_packet_acknowledgement(at, port_id, channel_id, seq).await,
		}
	}

	async fn query_next_sequence_recv(
		&self,
		at: Height,
		port_id: &PortId,
		channel_id: &ChannelId,
	) -> Result<QueryNextSequenceReceiveResponse, Self::Error> {
		match self {
			AnyChain::Parachain(chain) => chain
				.query_next_sequence_recv(at, port_id, channel_id)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain
				.query_next_sequence_recv(at, port_id, channel_id)
				.await
				.map_err(Into::into),
			AnyChain::Wasm(c) => c.inner.query_next_sequence_recv(at, port_id, channel_id).await,
		}
	}

	async fn query_packet_receipt(
		&self,
		at: Height,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: u64,
	) -> Result<QueryPacketReceiptResponse, Self::Error> {
		match self {
			AnyChain::Parachain(chain) => chain
				.query_packet_receipt(at, port_id, channel_id, seq)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain
				.query_packet_receipt(at, port_id, channel_id, seq)
				.await
				.map_err(Into::into),
			AnyChain::Wasm(c) => c.inner.query_packet_receipt(at, port_id, channel_id, seq).await,
		}
	}

	async fn latest_height_and_timestamp(&self) -> Result<(Height, Timestamp), Self::Error> {
		match self {
			AnyChain::Parachain(chain) =>
				chain.latest_height_and_timestamp().await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain.latest_height_and_timestamp().await.map_err(Into::into),
			AnyChain::Wasm(c) => c.inner.latest_height_and_timestamp().await,
		}
	}

	async fn query_packet_commitments(
		&self,
		at: Height,
		channel_id: ChannelId,
		port_id: PortId,
	) -> Result<Vec<u64>, Self::Error> {
		match self {
			Self::Parachain(chain) => chain
				.query_packet_commitments(at, channel_id, port_id)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain
				.query_packet_commitments(at, channel_id, port_id)
				.await
				.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_packet_commitments(at, channel_id, port_id).await,
		}
	}

	async fn query_packet_acknowledgements(
		&self,
		at: Height,
		channel_id: ChannelId,
		port_id: PortId,
	) -> Result<Vec<u64>, Self::Error> {
		match self {
			Self::Parachain(chain) => chain
				.query_packet_acknowledgements(at, channel_id, port_id)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain
				.query_packet_acknowledgements(at, channel_id, port_id)
				.await
				.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_packet_acknowledgements(at, channel_id, port_id).await,
		}
	}

	async fn query_unreceived_packets(
		&self,
		at: Height,
		channel_id: ChannelId,
		port_id: PortId,
		seqs: Vec<u64>,
	) -> Result<Vec<u64>, Self::Error> {
		match self {
			Self::Parachain(chain) => chain
				.query_unreceived_packets(at, channel_id, port_id, seqs)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain
				.query_unreceived_packets(at, channel_id, port_id, seqs)
				.await
				.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_unreceived_packets(at, channel_id, port_id, seqs).await,
		}
	}

	async fn query_unreceived_acknowledgements(
		&self,
		at: Height,
		channel_id: ChannelId,
		port_id: PortId,
		seqs: Vec<u64>,
	) -> Result<Vec<u64>, Self::Error> {
		match self {
			Self::Parachain(chain) => chain
				.query_unreceived_acknowledgements(at, channel_id, port_id, seqs)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain
				.query_unreceived_acknowledgements(at, channel_id, port_id, seqs)
				.await
				.map_err(Into::into),
			Self::Wasm(c) =>
				c.inner.query_unreceived_acknowledgements(at, channel_id, port_id, seqs).await,
		}
	}

	fn channel_whitelist(&self) -> Vec<(ChannelId, PortId)> {
		match self {
			Self::Parachain(chain) => chain.channel_whitelist(),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.channel_whitelist(),
			Self::Wasm(c) => c.inner.channel_whitelist(),
		}
	}

	async fn query_connection_channels(
		&self,
		at: Height,
		connection_id: &ConnectionId,
	) -> Result<QueryChannelsResponse, Self::Error> {
		match self {
			Self::Parachain(chain) =>
				chain.query_connection_channels(at, connection_id).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) =>
				chain.query_connection_channels(at, connection_id).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_connection_channels(at, connection_id).await,
		}
	}

	async fn query_send_packets(
		&self,
		channel_id: ChannelId,
		port_id: PortId,
		seqs: Vec<u64>,
	) -> Result<Vec<ibc_rpc::PacketInfo>, Self::Error> {
		match self {
			Self::Parachain(chain) =>
				chain.query_send_packets(channel_id, port_id, seqs).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) =>
				chain.query_send_packets(channel_id, port_id, seqs).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_send_packets(channel_id, port_id, seqs).await,
		}
	}

	async fn query_recv_packets(
		&self,
		channel_id: ChannelId,
		port_id: PortId,
		seqs: Vec<u64>,
	) -> Result<Vec<ibc_rpc::PacketInfo>, Self::Error> {
		match self {
			Self::Parachain(chain) =>
				chain.query_recv_packets(channel_id, port_id, seqs).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) =>
				chain.query_recv_packets(channel_id, port_id, seqs).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_recv_packets(channel_id, port_id, seqs).await,
		}
	}

	fn expected_block_time(&self) -> Duration {
		match self {
			Self::Parachain(chain) => chain.expected_block_time(),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.expected_block_time(),
			Self::Wasm(c) => c.inner.expected_block_time(),
		}
	}

	async fn query_client_update_time_and_height(
		&self,
		client_id: ClientId,
		client_height: Height,
	) -> Result<(Height, Timestamp), Self::Error> {
		match self {
			Self::Parachain(chain) => chain
				.query_client_update_time_and_height(client_id, client_height)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain
				.query_client_update_time_and_height(client_id, client_height)
				.await
				.map_err(Into::into),
			Self::Wasm(c) =>
				c.inner.query_client_update_time_and_height(client_id, client_height).await,
		}
	}

	async fn query_host_consensus_state_proof(
		&self,
		height: Height,
	) -> Result<Option<Vec<u8>>, Self::Error> {
		match self {
			AnyChain::Parachain(chain) =>
				chain.query_host_consensus_state_proof(height).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) =>
				chain.query_host_consensus_state_proof(height).await.map_err(Into::into),
			AnyChain::Wasm(c) => c.inner.query_host_consensus_state_proof(height).await,
		}
	}

	async fn query_ibc_balance(&self) -> Result<Vec<PrefixedCoin>, Self::Error> {
		match self {
			Self::Parachain(chain) => chain.query_ibc_balance().await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.query_ibc_balance().await.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_ibc_balance().await,
		}
	}

	fn connection_prefix(&self) -> CommitmentPrefix {
		match self {
			AnyChain::Parachain(chain) => chain.connection_prefix(),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain.connection_prefix(),
			AnyChain::Wasm(c) => c.inner.connection_prefix(),
		}
	}

	fn client_id(&self) -> ClientId {
		match self {
			AnyChain::Parachain(chain) => chain.client_id(),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain.client_id(),
			AnyChain::Wasm(c) => c.inner.client_id(),
		}
	}

	fn connection_id(&self) -> ConnectionId {
		match self {
			AnyChain::Parachain(chain) => chain.connection_id(),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain.connection_id(),
			AnyChain::Wasm(c) => c.inner.connection_id(),
		}
	}

	fn client_type(&self) -> ClientType {
		match self {
			AnyChain::Parachain(chain) => chain.client_type(),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(chain) => chain.client_type(),
			AnyChain::Wasm(c) => c.inner.client_type(),
		}
	}

	async fn query_timestamp_at(&self, block_number: u64) -> Result<u64, Self::Error> {
		match self {
			Self::Parachain(chain) =>
				chain.query_timestamp_at(block_number).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.query_timestamp_at(block_number).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_timestamp_at(block_number).await,
		}
	}

	async fn query_clients(&self) -> Result<Vec<ClientId>, Self::Error> {
		match self {
			Self::Parachain(chain) => chain.query_clients().await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.query_clients().await.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_clients().await,
		}
	}

	async fn query_channels(&self) -> Result<Vec<(ChannelId, PortId)>, Self::Error> {
		match self {
			Self::Parachain(chain) => chain.query_channels().await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.query_channels().await.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_channels().await,
		}
	}

	async fn query_connection_using_client(
		&self,
		height: u32,
		client_id: String,
	) -> Result<Vec<IdentifiedConnection>, Self::Error> {
		match self {
			Self::Parachain(chain) =>
				chain.query_connection_using_client(height, client_id).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) =>
				chain.query_connection_using_client(height, client_id).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_connection_using_client(height, client_id).await,
		}
	}

	fn is_update_required(
		&self,
		latest_height: u64,
		latest_client_height_on_counterparty: u64,
	) -> bool {
		match self {
			Self::Parachain(chain) =>
				chain.is_update_required(latest_height, latest_client_height_on_counterparty),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) =>
				chain.is_update_required(latest_height, latest_client_height_on_counterparty),
			Self::Wasm(c) =>
				c.inner.is_update_required(latest_height, latest_client_height_on_counterparty),
		}
	}
	async fn initialize_client_state(
		&self,
	) -> Result<(AnyClientState, AnyConsensusState), Self::Error> {
		match self {
			Self::Parachain(chain) => chain.initialize_client_state().await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.initialize_client_state().await.map_err(Into::into),
			Self::Wasm(c) => c.inner.initialize_client_state().await,
		}
	}

	async fn query_client_id_from_tx_hash(
		&self,
		tx_id: Self::TransactionId,
	) -> Result<ClientId, Self::Error> {
		match self {
			Self::Parachain(chain) => chain
				.query_client_id_from_tx_hash(
					downcast!(tx_id => AnyTransactionId::Parachain)
						.expect("Should be parachain transaction id"),
				)
				.await
				.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain
				.query_client_id_from_tx_hash(
					downcast!(tx_id => AnyTransactionId::Cosmos)
						.expect("Should be cosmos transaction id"),
				)
				.await
				.map_err(Into::into),
			Self::Wasm(c) => c.inner.query_client_id_from_tx_hash(tx_id).await,
		}
	}

	async fn upload_wasm(&self, wasm: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
		match self {
			Self::Parachain(chain) => chain.upload_wasm(wasm).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.upload_wasm(wasm).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.upload_wasm(wasm).await,
		}
	}
}

#[async_trait]
impl MisbehaviourHandler for AnyChain {
	async fn check_for_misbehaviour<C: Chain>(
		&self,
		counterparty: &C,
		client_message: AnyClientMessage,
	) -> Result<(), anyhow::Error> {
		match self {
			AnyChain::Parachain(parachain) =>
				parachain.check_for_misbehaviour(counterparty, client_message).await,
			_ => unreachable!(),
		}
	}
}

impl KeyProvider for AnyChain {
	fn account_id(&self) -> Signer {
		match self {
			AnyChain::Parachain(parachain) => parachain.account_id(),
			#[cfg(feature = "cosmos")]
			AnyChain::Cosmos(cosmos) => cosmos.account_id(),
			AnyChain::Wasm(c) => c.inner.account_id(),
		}
	}
}

#[async_trait]
impl Chain for AnyChain {
	fn name(&self) -> &str {
		match self {
			Self::Parachain(chain) => chain.name(),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.name(),
			Self::Wasm(c) => c.inner.name(),
		}
	}

	fn block_max_weight(&self) -> u64 {
		match self {
			Self::Parachain(chain) => chain.block_max_weight(),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.block_max_weight(),
			Self::Wasm(c) => c.inner.block_max_weight(),
		}
	}

	async fn estimate_weight(&self, msg: Vec<Any>) -> Result<u64, Self::Error> {
		match self {
			Self::Parachain(chain) => chain.estimate_weight(msg).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.estimate_weight(msg).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.estimate_weight(msg).await,
		}
	}

	async fn finality_notifications(
		&self,
	) -> Pin<Box<dyn Stream<Item = Self::FinalityEvent> + Send + Sync>> {
		match self {
			Self::Parachain(chain) => {
				use futures::StreamExt;
				Box::pin(chain.finality_notifications().await.map(|x| x.into()))
			},
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => {
				use futures::StreamExt;
				Box::pin(chain.finality_notifications().await.map(|x| x.into()))
			},
			Self::Wasm(c) => c.inner.finality_notifications().await,
		}
	}

	async fn submit(&self, messages: Vec<Any>) -> Result<Self::TransactionId, Self::Error> {
		match self {
			Self::Parachain(chain) => chain
				.submit(messages)
				.await
				.map_err(Into::into)
				.map(|id| AnyTransactionId::Parachain(id)),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain
				.submit(messages)
				.await
				.map_err(Into::into)
				.map(|id| AnyTransactionId::Cosmos(id)),
			Self::Wasm(chain) => {
				println!("start converting");
				let messages = messages
					.into_iter()
					.map(|msg| wrap_any_msg_into_wasm(msg, chain.code_id.clone()))
					.collect();
				println!("stop converting, submitting to {}", chain.inner.name());
				chain.inner.submit(messages).await.map_err(Into::into)
			},
		}
	}

	async fn query_client_message(
		&self,
		update: UpdateClient,
	) -> Result<AnyClientMessage, Self::Error> {
		match self {
			Self::Parachain(chain) => chain.query_client_message(update).await.map_err(Into::into),
			_ => unreachable!(),
		}
	}
}

fn wrap_any_msg_into_wasm(msg: Any, code_id: Bytes) -> Any {
	// TODO: consider rewriting with Ics26Envelope
	use ibc::core::{
		ics02_client::msgs::{
			create_client::TYPE_URL as CREATE_CLIENT_TYPE_URL,
			update_client::TYPE_URL as UPDATE_CLIENT_TYPE_URL,
		},
		ics03_connection::msgs::{
			conn_open_ack::TYPE_URL as CONN_OPEN_ACK_TYPE_URL,
			conn_open_try::TYPE_URL as CONN_OPEN_TRY_TYPE_URL,
		},
	};

	println!("converting: {}", msg.type_url);
	match msg.type_url.as_str() {
		CREATE_CLIENT_TYPE_URL => {
			let mut msg_decoded =
				MsgCreateAnyClient::<LocalClientTypes>::decode_vec(&msg.value).unwrap();
			msg_decoded.consensus_state =
				AnyConsensusState::wasm(msg_decoded.consensus_state, code_id.clone(), 1);
			msg_decoded.client_state = AnyClientState::wasm(msg_decoded.client_state, code_id);
			msg_decoded.to_any()
		},
		CONN_OPEN_TRY_TYPE_URL => {
			let mut msg_decoded =
				MsgConnectionOpenTry::<LocalClientTypes>::decode_vec(&msg.value).unwrap();
			// println!("decoded: {:?}", msg_decoded);
			// msg_decoded.client_state = msg_decoded
			// 	.client_state
			// 	.map(|client_state| AnyClientState::wasm(client_state, code_id));
			msg_decoded.to_any()
		},
		CONN_OPEN_ACK_TYPE_URL => {
			let mut msg_decoded =
				MsgConnectionOpenAck::<LocalClientTypes>::decode_vec(&msg.value).unwrap();
			msg_decoded.client_state = msg_decoded
				.client_state
				.map(|client_state| AnyClientState::wasm(client_state, code_id));
			msg_decoded.to_any()
		},
		UPDATE_CLIENT_TYPE_URL => {
			let mut msg_decoded =
				MsgUpdateAnyClient::<LocalClientTypes>::decode_vec(&msg.value).unwrap();
			msg_decoded.client_message = AnyClientMessage::wasm(msg_decoded.client_message);
			// println!("decoded {}: {:?}", UPDATE_CLIENT_TYPE_URL, msg_decoded);
			let any = msg_decoded.to_any();
			// println!("converted {}: {}", any.type_url, hex::encode(&any.value));
			any
		},
		_ => msg,
	}
}

#[cfg(any(test, feature = "testing"))]
#[async_trait]
impl primitives::TestProvider for AnyChain {
	async fn send_transfer(&self, params: MsgTransfer<PrefixedCoin>) -> Result<(), Self::Error> {
		match self {
			Self::Parachain(chain) => chain.send_transfer(params).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.send_transfer(params).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.send_transfer(params).await,
		}
	}

	async fn send_ordered_packet(
		&self,
		channel_id: ChannelId,
		timeout: Timeout,
	) -> Result<(), Self::Error> {
		match self {
			Self::Parachain(chain) =>
				chain.send_ordered_packet(channel_id, timeout).await.map_err(Into::into),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.send_ordered_packet(channel_id, timeout).await.map_err(Into::into),
			Self::Wasm(c) => c.inner.send_ordered_packet(channel_id, timeout).await,
		}
	}

	async fn subscribe_blocks(&self) -> Pin<Box<dyn Stream<Item = u64> + Send + Sync>> {
		match self {
			Self::Parachain(chain) => chain.subscribe_blocks().await,
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.subscribe_blocks().await,
			Self::Wasm(c) => c.inner.subscribe_blocks().await,
		}
	}

	fn set_channel_whitelist(&mut self, channel_whitelist: Vec<(ChannelId, PortId)>) {
		match self {
			Self::Parachain(chain) => chain.set_channel_whitelist(channel_whitelist),
			#[cfg(feature = "cosmos")]
			Self::Cosmos(chain) => chain.set_channel_whitelist(channel_whitelist),
			Self::Wasm(c) => c.inner.set_channel_whitelist(channel_whitelist),
		}
	}
}

impl AnyConfig {
	pub fn wasm_code_id(&self) -> Option<(CodeId, ClientType)> {
		let (maybe_code_id, maybe_client_type) = match self {
			AnyConfig::Parachain(config) =>
				(config.wasm_code_id.as_ref(), config.wasm_client_type.as_ref()),
			#[cfg(feature = "cosmos")]
			AnyConfig::Cosmos(config) => (config.wasm_code_id.as_ref(), config.wasm_client_type.as_ref()),
		};
		if maybe_code_id.is_some() != maybe_client_type.is_some() {
			panic!("Wasm code id and client type must be both set or both unset");
		}

		let maybe_code_id =
			maybe_code_id.map(|s| hex::decode(s).expect("Wasm code id is hex-encoded"));

		maybe_code_id.map(|code_id| (code_id, maybe_client_type.unwrap().clone()))
	}

	pub async fn into_client(self) -> anyhow::Result<AnyChain> {
		let maybe_wasm_code_id = self.wasm_code_id();
		let chain = match self {
			AnyConfig::Parachain(config) =>
				AnyChain::Parachain(ParachainClient::new(config).await?),
			#[cfg(feature = "cosmos")]
			AnyConfig::Cosmos(config) => AnyChain::Cosmos(CosmosClient::new(config).await?),
		};
		if let Some((code_id, client_type)) = maybe_wasm_code_id {
			// println!("inserting wasm client {}", client_type);
			ics08_wasm::add_wasm_client_type(code_id.clone(), client_type.clone());
			Ok(AnyChain::Wasm(WasmChain { inner: Box::new(chain), code_id, client_type }))
		} else {
			Ok(chain)
		}
	}

	pub fn set_client_id(&mut self, client_id: ClientId) {
		match self {
			Self::Parachain(chain) => {
				chain.client_id.replace(client_id);
			},
		}
	}

	pub fn set_connection_id(&mut self, connection_id: ConnectionId) {
		match self {
			Self::Parachain(chain) => {
				chain.connection_id.replace(connection_id);
			},
		}
	}

	pub fn set_channel_whitelist(&mut self, channel_id: ChannelId, port_id: PortId) {
		match self {
			Self::Parachain(chain) => {
				chain.channel_whitelist.push((channel_id, port_id));
			},
		}
	}
}
