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

use core::time::Duration;
use futures::StreamExt;
use hyperspace_core::{
	chain::{AnyAssetId, AnyChain, AnyConfig},
	logging,
	substrate::DefaultConfig,
};
use hyperspace_cosmos::client::{CosmosClient, CosmosClientConfig};
use hyperspace_ethereum::{
	config::EthereumClientConfig,
	mock::sanity_checks::{
		deploy_yui_ibc_and_mock_client_fixture, hyperspace_ethereum_client_fixture,
		DeployYuiIbcMockClient,
	},
};
use hyperspace_parachain::{finality_protocol::FinalityProtocol, ParachainClientConfig};
use hyperspace_primitives::{utils::create_clients, CommonClientConfig, IbcProvider};
use hyperspace_testsuite::{
	ibc_channel_close, ibc_messaging_packet_height_timeout_with_connection_delay,
	ibc_messaging_packet_timeout_on_channel_close,
	ibc_messaging_packet_timestamp_timeout_with_connection_delay,
	ibc_messaging_with_connection_delay, misbehaviour::ibc_messaging_submit_misbehaviour,
	setup_connection_and_channel,
};
use ibc::core::ics24_host::identifier::PortId;
use sp_core::hashing::sha2_256;

#[derive(Debug, Clone)]
pub struct Args {
	pub chain_a: String,
	pub chain_b: String,
	pub connection_prefix_a: String,
	pub connection_prefix_b: String,
	pub cosmos_grpc: String,
	pub cosmos_ws: String,
	pub ethereum_rpc: String,
	pub wasm_path: String,
}

impl Default for Args {
	fn default() -> Self {
		let eth = std::env::var("ETHEREUM_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
		let cosmos = std::env::var("COSMOS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
		let wasm_path = std::env::var("WASM_PATH").unwrap_or_else(|_| {
			"../../target/wasm32-unknown-unknown/release/icsxx_ethereum_cw.wasm".to_string()
		});

		Args {
			chain_a: format!("ws://{eth}:5001"),
			chain_b: format!("http://{cosmos}:26657"),
			connection_prefix_a: "ibc/".to_string(),
			connection_prefix_b: "ibc".to_string(),
			cosmos_grpc: format!("http://{cosmos}:9090"),
			cosmos_ws: format!("ws://{cosmos}:26657/websocket"),
			ethereum_rpc: format!("http://{eth}:6001"),
			wasm_path,
		}
	}
}

async fn setup_clients() -> (AnyChain, AnyChain) {
	log::info!(target: "hyperspace", "=========================== Starting Test ===========================");
	let args = Args::default();

	// Create client configurations
	// let config_a = EthereumClientConfig {
	// 	http_rpc_url: Default::default(),
	// 	ws_rpc_url: Default::default(),
	// 	ibc_handler_address: Default::default(),
	// 	name: "ethereum".to_string(),
	// 	// para_id: args.para_id,
	// 	// parachain_rpc_url: args.chain_a,
	// 	// relay_chain_rpc_url: args.relay_chain.clone(),
	// 	client_id: None,
	// 	connection_id: None,
	// 	commitment_prefix: args.connection_prefix_a.as_bytes().to_vec().into(),
	// 	// ss58_version: 42,
	// 	channel_whitelist: vec![],
	// 	// finality_protocol: FinalityProtocol::Grandpa,
	// 	private_key: None,
	// 	// key_type: "sr25519".to_string(),
	// 	// wasm_code_id: None,
	// 	private_key_path: Some(
	// 		"../ethereum/keys/0x73db010c3275eb7a92e5c38770316248f4c644ee".to_string(),
	// 	),
	// 	mnemonic: None,
	// 	max_block_weight: 1,
	// };

	let DeployYuiIbcMockClient { anvil, yui_ibc, .. } =
		deploy_yui_ibc_and_mock_client_fixture().await;
	let config_a = hyperspace_ethereum_client_fixture(&anvil, yui_ibc).await;

	let mut config_b = CosmosClientConfig {
		name: "centauri".to_string(),
		rpc_url: args.chain_b.clone().parse().unwrap(),
		grpc_url: args.cosmos_grpc.clone().parse().unwrap(),
		websocket_url: args.cosmos_ws.clone().parse().unwrap(),
		chain_id: "centauri-testnet-1".to_string(),
		client_id: None,
		connection_id: None,
		account_prefix: "centauri".to_string(),
		fee_denom: "stake".to_string(),
		fee_amount: "4000".to_string(),
		gas_limit: (i64::MAX - 1) as u64,
		store_prefix: args.connection_prefix_b,
		max_tx_size: 200000,
		mnemonic: // 26657
			"sense state fringe stool behind explain area quit ugly affair develop thumb clinic weasel choice atom gesture spare sea renew penalty second upon peace"
				.to_string(),
		wasm_code_id: None,
		channel_whitelist: vec![],
		common: CommonClientConfig {
			skip_optional_client_updates: true,
			max_packets_to_process: 200,
		},
	};

	let chain_b = CosmosClient::<DefaultConfig>::new(config_b.clone()).await.unwrap();

	let wasm_data = tokio::fs::read(&args.wasm_path).await.expect("Failed to read wasm file");
	let code_id = match chain_b.upload_wasm(wasm_data.clone()).await {
		Ok(code_id) => code_id,
		Err(e) => {
			let e_str = format!("{e:?}");
			if !e_str.contains("wasm code already exists") {
				panic!("Failed to upload wasm: {e_str}");
			}
			sha2_256(&wasm_data).to_vec()
		},
	};
	let code_id_str = hex::encode(code_id);
	config_b.wasm_code_id = Some(code_id_str);

	let mut chain_a_wrapped = AnyConfig::Ethereum(config_a).into_client().await.unwrap();
	let mut chain_b_wrapped = AnyConfig::Cosmos(config_b).into_client().await.unwrap();

	let clients_on_a = chain_a_wrapped.query_clients().await.unwrap();
	let clients_on_b = chain_b_wrapped.query_clients().await.unwrap();

	if !clients_on_a.is_empty() && !clients_on_b.is_empty() {
		chain_a_wrapped.set_client_id(clients_on_b[0].clone());
		chain_b_wrapped.set_client_id(clients_on_a[0].clone());
		return (chain_a_wrapped, chain_b_wrapped)
	}

	let (client_b, client_a) =
		create_clients(&mut chain_b_wrapped, &mut chain_a_wrapped).await.unwrap();
	chain_a_wrapped.set_client_id(client_a);
	chain_b_wrapped.set_client_id(client_b);
	(chain_a_wrapped, chain_b_wrapped)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn ethereum_to_cosmos_ibc_messaging_full_integration_test() {
	logging::setup_logging();

	let asset_id_a = AnyAssetId::Ethereum(());
	let asset_id_b = AnyAssetId::Cosmos(
		"ibc/47B97D8FF01DA03FCB2F4B1FFEC931645F254E21EF465FA95CBA6888CB964DC4".to_string(),
	);
	let (mut chain_a, mut chain_b) = setup_clients().await;
	// let (handle, channel_a, channel_b, connection_id_a, connection_id_b) =
	// 	setup_connection_and_channel(&mut chain_a, &mut chain_b, Duration::from_secs(60 * 2)).await;
	// handle.abort();
	//
	// // Set connections and channel whitelist
	// chain_a.set_connection_id(connection_id_a);
	// chain_b.set_connection_id(connection_id_b);
	//
	// chain_a.set_channel_whitelist(vec![(channel_a, PortId::transfer())].into_iter().collect());
	// chain_b.set_channel_whitelist(vec![(channel_b, PortId::transfer())].into_iter().collect());
	//
	// // Run tests sequentially
	//
	// // no timeouts + connection delay
	//
	// ibc_messaging_with_connection_delay(
	// 	&mut chain_a,
	// 	&mut chain_b,
	// 	asset_id_a.clone(),
	// 	asset_id_b.clone(),
	// 	channel_a,
	// 	channel_b,
	// )
	// .await;
	//
	// // timeouts + connection delay
	// ibc_messaging_packet_height_timeout_with_connection_delay(
	// 	&mut chain_a,
	// 	&mut chain_b,
	// 	asset_id_a.clone(),
	// 	channel_a,
	// 	channel_b,
	// )
	// .await;
	// ibc_messaging_packet_timestamp_timeout_with_connection_delay(
	// 	&mut chain_a,
	// 	&mut chain_b,
	// 	asset_id_a.clone(),
	// 	channel_a,
	// 	channel_b,
	// )
	// .await;
	//
	// // channel closing semantics
	// ibc_messaging_packet_timeout_on_channel_close(
	// 	&mut chain_a,
	// 	&mut chain_b,
	// 	asset_id_a.clone(),
	// 	channel_a,
	// )
	// .await;
	// ibc_channel_close(&mut chain_a, &mut chain_b).await;

	// TODO: tendermint misbehaviour?
	// ibc_messaging_submit_misbehaviour(&mut chain_a, &mut chain_b).await;
}

#[tokio::test]
#[ignore]
async fn cosmos_to_ethereum_ibc_messaging_full_integration_test() {
	logging::setup_logging();

	let (chain_a, chain_b) = setup_clients().await;
	let (mut chain_b, mut chain_a) = (chain_a, chain_b);

	let (handle, channel_a, channel_b, connection_id_a, connection_id_b) =
		setup_connection_and_channel(&mut chain_a, &mut chain_b, Duration::from_secs(60 * 2)).await;
	handle.abort();

	// Set connections and channel whitelist
	chain_a.set_connection_id(connection_id_a);
	chain_b.set_connection_id(connection_id_b);

	chain_a.set_channel_whitelist(vec![(channel_a, PortId::transfer())].into_iter().collect());
	chain_b.set_channel_whitelist(vec![(channel_b, PortId::transfer())].into_iter().collect());

	let asset_id_a = AnyAssetId::Cosmos("stake".to_string());
	let asset_id_b = AnyAssetId::Parachain(2);

	// Run tests sequentially

	// no timeouts + connection delay
	ibc_messaging_with_connection_delay(
		&mut chain_a,
		&mut chain_b,
		asset_id_a.clone(),
		asset_id_b.clone(),
		channel_a,
		channel_b,
	)
	.await;

	// timeouts + connection delay
	ibc_messaging_packet_height_timeout_with_connection_delay(
		&mut chain_a,
		&mut chain_b,
		asset_id_a.clone(),
		channel_a,
		channel_b,
	)
	.await;
	ibc_messaging_packet_timestamp_timeout_with_connection_delay(
		&mut chain_a,
		&mut chain_b,
		asset_id_a.clone(),
		channel_a,
		channel_b,
	)
	.await;

	// channel closing semantics (doesn't work on cosmos)
	// ibc_messaging_packet_timeout_on_channel_close(&mut chain_a, &mut chain_b, asset_id_a.clone())
	// 	.await;
	// ibc_channel_close(&mut chain_a, &mut chain_b).await;

	ibc_messaging_submit_misbehaviour(&mut chain_a, &mut chain_b).await;
}
