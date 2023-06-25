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

#[cfg(any(test, feature = "testing"))]
use crate::TestProvider;
use crate::{mock::LocalClientTypes, Chain};
use futures::{future, StreamExt};
use ibc::{
	core::{
		ics02_client::msgs::create_client::MsgCreateAnyClient,
		ics03_connection::{connection::Counterparty, msgs::conn_open_init::MsgConnectionOpenInit},
		ics04_channel,
		ics04_channel::{
			channel,
			channel::{ChannelEnd, Order, State},
			msgs::chan_open_init::MsgChannelOpenInit,
		},
		ics24_host::identifier::{ChannelId, ClientId, ConnectionId, PortId},
	},
	events::IbcEvent,
	protobuf::Protobuf,
	tx_msg::Msg,
};
use ibc_proto::google::protobuf::Any;
use std::{future::Future, time::Duration};

pub async fn timeout_future<T: Future>(future: T, secs: u64, reason: String) -> T::Output {
	let duration = Duration::from_secs(secs);
	match tokio::time::timeout(duration.clone(), future).await {
		Ok(output) => output,
		Err(_) => panic!("Future didn't finish within {duration:?}, {reason}"),
	}
}

#[cfg(any(test, feature = "testing"))]
pub async fn timeout_after<C: TestProvider, T: Future + Send + 'static>(
	chain: &C,
	future: T,
	blocks: u64,
	reason: String,
) where
	T::Output: Send + 'static,
{
	let task = tokio::spawn(future);
	let task_2 =
		tokio::spawn(chain.subscribe_blocks().await.take(blocks as usize).collect::<Vec<_>>());
	tokio::select! {
		_output = task => {}
		_blocks = task_2 => {
			panic!("Future didn't finish after {blocks:?} produced, {reason}")
		}
	}
}

pub async fn create_clients(
	chain_a: &mut impl Chain,
	chain_b: &mut impl Chain,
) -> Result<(ClientId, ClientId), anyhow::Error> {
	let (client_state_a, cs_state_a) = chain_a.initialize_client_state().await?;
	let (client_state_b, cs_state_b) = chain_b.initialize_client_state().await?;

	let msg = MsgCreateAnyClient::<LocalClientTypes> {
		client_state: client_state_b,
		consensus_state: cs_state_b,
		signer: chain_a.account_id(),
	};

	let msg = Any { type_url: msg.type_url(), value: msg.encode_vec()? };

	let tx_id = chain_a.submit(vec![msg]).await?;
	let client_id_b_on_a = chain_a.query_client_id_from_tx_hash(tx_id).await?;
	chain_a.set_client_id(client_id_b_on_a.clone());

	let msg = MsgCreateAnyClient::<LocalClientTypes> {
		client_state: client_state_a,
		consensus_state: cs_state_a,
		signer: chain_b.account_id(),
	};

	let msg = Any { type_url: msg.type_url(), value: msg.encode_vec()? };

	let tx_id = chain_b.submit(vec![msg]).await?;
	let client_id_a_on_b = chain_b.query_client_id_from_tx_hash(tx_id).await?;
	chain_a.set_client_id(client_id_b_on_a.clone());

	Ok((client_id_a_on_b, client_id_b_on_a))
}

/// Completes the connection handshake process
/// The relayer process must be running before this function is executed
pub async fn create_connection(
	chain_a: &mut impl Chain,
	chain_b: &mut impl Chain,
	delay_period: Duration,
) -> Result<(ConnectionId, ConnectionId), anyhow::Error> {
	let msg = MsgConnectionOpenInit {
		client_id: chain_b.client_id(),
		counterparty: Counterparty::new(chain_a.client_id(), None, chain_b.connection_prefix()),
		version: Some(Default::default()),
		delay_period,
		signer: chain_a.account_id(),
	};

	let msg = Any { type_url: msg.type_url(), value: msg.encode_vec()? };

	let tx_id = chain_a.submit(vec![msg]).await?;
	let connection_id_a = chain_a.query_connection_id_from_tx_hash(tx_id).await?;
	chain_a.set_connection_id(connection_id_a.clone());

	log::info!(target: "hyperspace", "============= Wait till both chains have completed connection handshake =============");

	// wait till both chains have completed connection handshake
	let future = chain_b
		.ibc_events()
		.await
		.skip_while(|ev| {
			future::ready(!matches!(ev, IbcEvent::OpenTryConnection(e) if
					e.0.counterparty_connection_id == connection_id_a
			))
		})
		.take(1)
		.collect::<Vec<_>>();

	let mut events = timeout_future(
		future,
		5 * 60,
		format!("Didn't see OpenTryConnection on {}", chain_b.name()),
	)
	.await;

	let connection_id_b = match events.pop() {
		Some(IbcEvent::OpenTryConnection(conn)) => (conn.connection_id().unwrap().clone()),
		got => panic!("Last event should be OpenTryConnection: {got:?}"),
	};
	chain_b.set_connection_id(connection_id_b.clone());

	// wait till both chains have completed connection handshake
	let future = chain_b
		.ibc_events()
		.await
		.skip_while(|ev| {
			future::ready(!matches!(ev,
				IbcEvent::OpenConfirmConnection(e) if
					e.0.connection_id == connection_id_b &&
					e.0.counterparty_connection_id == connection_id_a
			))
		})
		.take(1)
		.collect::<Vec<_>>();

	let mut _events = timeout_future(
		future,
		10 * 60,
		format!("Didn't see OpenConfirmConnection on {}", chain_b.name()),
	)
	.await;

	Ok((connection_id_a, connection_id_b))
}

/// Completes the chanel handshake process
/// The relayer process must be running before this function is executed
pub async fn create_channel(
	chain_a: &mut impl Chain,
	chain_b: &mut impl Chain,
	connection_id: ConnectionId,
	port_id: PortId,
	version: String,
	order: Order,
) -> Result<(ChannelId, ChannelId), anyhow::Error> {
	let channel = ChannelEnd::new(
		State::Init,
		order,
		channel::Counterparty::new(port_id.clone(), None),
		vec![connection_id],
		ics04_channel::Version::new(version),
	);

	let msg = MsgChannelOpenInit::new(port_id, channel, chain_a.account_id());

	let msg = Any { type_url: msg.type_url(), value: msg.encode_vec()? };

	let tx_id = chain_a.submit(vec![msg]).await?;
	let channel_and_port_id_a = chain_a.query_channel_id_from_tx_hash(tx_id).await?;
	chain_a.add_channel_to_whitelist(channel_and_port_id_a.clone());

	let (channel_id_a, port_id_a) = channel_and_port_id_a;

	log::info!(target: "hyperspace", "============= Wait till both chains have completed channel handshake =============");

	let future = chain_b
		.ibc_events()
		.await
		.skip_while(|ev| {
			future::ready(!matches!(ev, IbcEvent::OpenTryChannel(e) if
			e.counterparty_channel_id == channel_id_a && e.counterparty_port_id == port_id_a))
		})
		.take(1)
		.collect::<Vec<_>>();

	let mut events =
		timeout_future(future, 10 * 60, format!("Didn't see OpenTryChannel on {}", chain_b.name()))
			.await;

	let channel_and_port_id_b = match events.pop() {
		Some(IbcEvent::OpenTryChannel(chan)) =>
			(chan.channel_id().unwrap().clone(), chan.port_id().clone()),
		got => panic!("Last event should be OpenTryChannel: {got:?}"),
	};
	chain_b.add_channel_to_whitelist(channel_and_port_id_b.clone());

	let (channel_id_b, port_id_b) = channel_and_port_id_b;

	let future = chain_b
		.ibc_events()
		.await
		.skip_while(|ev| {
			future::ready(!matches!(ev, IbcEvent::OpenConfirmChannel(e) if
				e.channel_id == channel_id_b && e.port_id == port_id_b &&
				e.counterparty_channel_id == channel_id_a && e.counterparty_port_id == port_id_a
			))
		})
		.take(1)
		.collect::<Vec<_>>();

	let mut _events = timeout_future(
		future,
		20 * 60,
		format!("Didn't see OpenConfirmChannel on {}", chain_b.name()),
	)
	.await;

	Ok((channel_id_a, channel_id_b))
}
