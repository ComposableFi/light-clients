use crate::{
	config::EthereumClientConfig,
	utils::{DeployYuiIbc, ProviderImpl},
};
use async_trait::async_trait;
use cast::revm::db;
use ethers::{
	abi::{AbiEncode, Address, ParamType, Token},
	prelude::{
		coins_bip39::English, signer::SignerMiddlewareError, Authorization, BlockId, BlockNumber,
		EIP1186ProofResponse, Filter, LocalWallet, Log, MnemonicBuilder, NameOrAddress, H256,
	},
	providers::{Http, Middleware, Provider, ProviderError, ProviderExt, Ws},
	signers::Signer,
	types::U256,
	utils::keccak256,
};
// use ethers_providers::
use crate::jwt::{JwtAuth, JwtKey};
use futures::{Stream, TryFutureExt};
use ibc::{
	applications::transfer::{msgs::transfer::MsgTransfer, PrefixedCoin},
	core::ics24_host::{
		error::ValidationError,
		identifier::{ChannelId, ClientId, PortId},
	},
	Height,
};
use ibc_primitives::Timeout;
use once_cell::sync::Lazy;
use primitives::CommonClientState;
use std::{future::Future, ops::Add, pin::Pin, str::FromStr, sync::Arc};
use thiserror::Error;

pub type EthRpcClient = ethers::prelude::SignerMiddleware<
	ethers::providers::Provider<Http>,
	ethers::signers::Wallet<ethers::prelude::k256::ecdsa::SigningKey>,
>;
pub(crate) type WsEth = Provider<Ws>;

pub static IBC_STORAGE_SLOT: Lazy<U256> =
	Lazy::new(|| U256::from_big_endian(&keccak256(b"ibc.core")[..]));

// TODO: generate this from the contract automatically
pub const COMMITMENTS_STORAGE_INDEX: u32 = 0;
pub const CLIENT_IMPLS_STORAGE_INDEX: u32 = 3;
pub const CONNECTIONS_STORAGE_INDEX: u32 = 4;
pub const CHANNELS_STORAGE_INDEX: u32 = 5;

#[derive(Debug, Clone)]
pub struct EthereumClient {
	http_rpc: Arc<EthRpcClient>,
	pub(crate) ws_uri: http::Uri,
	pub config: EthereumClientConfig,
	/// Common relayer data
	pub common_state: CommonClientState,
	pub yui: DeployYuiIbc<Arc<ProviderImpl>, ProviderImpl>,
	pub prev_state: Arc<std::sync::Mutex<(Vec<u8>, Vec<u8>)>>,
}

pub type MiddlewareErrorType = SignerMiddlewareError<
	Provider<Http>,
	ethers::signers::Wallet<ethers::prelude::k256::ecdsa::SigningKey>,
>;

#[derive(Debug, Error)]
pub enum ClientError {
	#[error("uri-parse-error: {0} {0}")]
	UriParseError(http::Uri),
	#[error("provider-error: {0}: {0}")]
	ProviderError(http::Uri, ProviderError),
	#[error("Ethereum error: {0}")]
	Ethers(#[from] ethers::providers::ProviderError),
	#[error("middleware-error: {0}")]
	MiddlewareError(MiddlewareErrorType),
	#[error("no-storage-proof: there was no storage proof for the given storage index")]
	NoStorageProof,
	#[error("{0}")]
	Other(String),
}

impl From<ValidationError> for ClientError {
	fn from(value: ValidationError) -> Self {
		Self::Other(value.to_string())
	}
}

impl From<String> for ClientError {
	fn from(value: String) -> Self {
		Self::Other(value)
	}
}

pub struct AckPacket {
	pub sequence: u64,
	pub source_port: String,
	pub source_channel: String,
	pub dest_port: String,
	pub dest_channel: String,
	pub data: Vec<u8>,
	pub timeout_height: (u64, u64),
	pub timeout_timestamp: u64,
	pub acknowledgement: Vec<u8>,
}

impl EthereumClient {
	pub async fn new(mut config: EthereumClientConfig) -> Result<Self, ClientError> {
		let client = Provider::<Http>::try_from(config.http_rpc_url.to_string())
			.map_err(|_| ClientError::UriParseError(config.http_rpc_url.clone()))?;

		let chain_id = client.get_chainid().await.unwrap();

		let wallet: LocalWallet = if let Some(mnemonic) = &config.mnemonic {
			MnemonicBuilder::<English>::default()
				.phrase(mnemonic.as_str())
				.build()
				.unwrap()
				.with_chain_id(chain_id.as_u64())
		} else if let Some(path) = config.private_key_path.take() {
			LocalWallet::decrypt_keystore(
				path,
				option_env!("KEY_PASS").expect("KEY_PASS is not set"),
			)
			.unwrap()
			.into()
		} else if let Some(private_key) = config.private_key.take() {
			let key = elliptic_curve::SecretKey::<ethers::prelude::k256::Secp256k1>::from_sec1_pem(
				private_key.as_str(),
			)
			.unwrap();
			key.into()
		} else {
			panic!("no private key or mnemonic provided")
		};

		let client = ethers::middleware::SignerMiddleware::new(client, wallet);

		let yui = config.yui.take().unwrap();
		Ok(Self {
			http_rpc: Arc::new(client),
			ws_uri: config.ws_rpc_url.clone(),
			config,
			common_state: Default::default(),
			yui,
			prev_state: Arc::new(std::sync::Mutex::new((vec![], vec![]))),
		})
	}

	pub fn client(&self) -> Arc<EthRpcClient> {
		self.http_rpc.clone()
	}

	pub async fn websocket_provider(&self) -> Result<Provider<Ws>, ClientError> {
		let secret = std::fs::read_to_string(format!(
			"{}/.lighthouse/local-testnet/geth_datadir1/geth/jwtsecret",
			env!("HOME"),
		))
		.unwrap();
		println!("secret = {secret}");
		let secret = JwtKey::from_slice(&hex::decode(&secret[2..]).unwrap()).expect("oops");
		let jwt_auth = JwtAuth::new(secret, None, None);
		let token = jwt_auth.generate_token().unwrap();

		let auth = Authorization::bearer(dbg!(token));
		Provider::<Ws>::connect_with_auth(self.ws_uri.to_string(), auth)
			.await
			.map_err(|e| ClientError::ProviderError(self.ws_uri.clone(), ProviderError::from(e)))
	}

	pub async fn generated_channel_identifiers(
		&self,
		from_block: BlockNumber,
	) -> Result<Vec<(String, String)>, ClientError> {
		let filter = Filter::new()
			.from_block(BlockNumber::Earliest)
			// .from_block(from_block)
			.to_block(BlockNumber::Latest)
			.address(self.config.ibc_handler_address)
			.event("OpenInitChannel(string,string)");

		let logs = self.client().get_logs(&filter).await.unwrap();

		let v = logs
			.into_iter()
			.map(|log| {
				let toks =
					ethers::abi::decode(&[ParamType::String, ParamType::String], &log.data.0)
						.unwrap();
				(toks[0].to_string(), toks[1].to_string())
			})
			.collect();

		Ok(v)
	}

	pub async fn generated_client_identifiers(&self, from_block: BlockNumber) -> Vec<String> {
		let filter = Filter::new()
			.from_block(from_block)
			.to_block(BlockNumber::Latest)
			.address(self.config.ibc_handler_address)
			.event("GeneratedClientIdentifier(string)");

		let logs = self.client().get_logs(&filter).await.unwrap();

		logs.into_iter()
			.map(|log| {
				ethers::abi::decode(&[ParamType::String], &log.data.0)
					.unwrap()
					.into_iter()
					.next()
					.unwrap()
					.to_string()
			})
			.collect()
	}

	pub async fn generated_connection_identifiers(&self, from_block: BlockNumber) -> Vec<String> {
		let filter = Filter::new()
			.from_block(from_block)
			.to_block(BlockNumber::Latest)
			.address(self.config.ibc_handler_address)
			.event("GeneratedConnectionIdentifier(string)");

		let logs = self.client().get_logs(&filter).await.unwrap();

		logs.into_iter()
			.map(|log| {
				ethers::abi::decode(&[ParamType::String], &log.data.0)
					.unwrap()
					.into_iter()
					.next()
					.unwrap()
					.to_string()
			})
			.collect()
	}

	pub async fn acknowledge_packets(&self, from_block: BlockNumber) -> Vec<AckPacket> {
		let filter = Filter::new()
			.from_block(from_block)
			.to_block(BlockNumber::Latest)
			.address(self.config.ibc_handler_address)
			.event("AcknowledgePacket((uint64,string,string,string,string,bytes,(uint64,uint64),uint64),bytes)");

		let logs = self.client().get_logs(&filter).await.unwrap();

		logs.into_iter()
			.map(|log| {
				let decoded = ethers::abi::decode(
					&[
						ParamType::Tuple(vec![
							ParamType::Uint(64),
							ParamType::String,
							ParamType::String,
							ParamType::String,
							ParamType::String,
							ParamType::Bytes,
							ParamType::Tuple(vec![ParamType::Uint(64), ParamType::Uint(64)]),
							ParamType::Uint(64),
						]),
						ParamType::Bytes,
					],
					&log.data.0,
				)
				.unwrap();

				let Token::Tuple(packet) = decoded[0].clone() else {
					panic!("expected tuple, got {:?}", decoded[0])
				};

				// use a match statement to destructure the `packet` into the fields
				// for the `AckPacket` struct
				let (sequence, source_port, source_channel, dest_port, dest_channel, data, timeout_height, timeout_timestamp) = match packet.as_slice() {
					[Token::Uint(sequence),
					Token::String(source_port), Token::String(source_channel), Token::String(dest_port), Token::String(dest_channel), Token::Bytes(data), Token::Tuple(timeout_height), Token::Uint(timeout_timestamp)] => {
						let [Token::Uint(rev), Token::Uint(height)] = timeout_height.as_slice() else {
							panic!("need timeout height to be a tuple of two uints, revision and height");
						};

						(sequence.as_u64(), source_port.clone(), source_channel.clone(), dest_port.clone(), dest_channel.clone(), data.clone(), (
							rev.as_u64(),
							height.as_u64(),
						), timeout_timestamp.as_u64())
					},
					_ => panic!("expected tuple, got {:?}", packet),
				};

				let Token::Bytes(acknowledgement) = decoded[1].clone() else {
					panic!("expected bytes, got {:?}", decoded[1])
				};

				let packet = AckPacket {
					sequence,
					source_port,
					source_channel,
					dest_port,
					dest_channel,
					data,
					timeout_height,
					timeout_timestamp,
					acknowledgement,
				};

				packet
			})
			.collect()
	}

	pub async fn address_of_client_id(&self, client_id: &str) -> Address {
		let proof = self.eth_query_proof(dbg!(client_id), None, 3).await.unwrap();

		match proof.storage_proof.last() {
			Some(proof) => todo!("{:?}", proof.value),
			None => Address::zero(),
		}
	}

	pub fn _query_packet_commitment(
		&self,
		at: Height,
		port_id: &PortId,
		channel_id: &ChannelId,
		seq: u64,
	) -> impl Future<
		Output = Result<
			ibc_proto::ibc::core::channel::v1::QueryPacketCommitmentResponse,
			ClientError,
		>,
	> {
		async move { todo!() }
	}

	/// produce a stream of events emitted from the contract address for the given block range
	pub fn query_events(
		&self,
		event_name: &str,
		from: BlockNumber,
		to: BlockNumber,
	) -> impl Stream<Item = Log> {
		let filter = Filter::new()
			.from_block(from)
			.to_block(to)
			.address(self.config.ibc_handler_address)
			.event(event_name);
		let client = self.client().clone();

		async_stream::stream! {
			let logs = client.get_logs(&filter).await.unwrap();
			for log in logs {
				yield log;
			}
		}
	}

	pub fn eth_query_proof(
		&self,
		key: &str,
		block_height: Option<u64>,
		storage_index: u32,
	) -> impl Future<Output = Result<EIP1186ProofResponse, ClientError>> {
		let key = keccak256(key.as_bytes());
		let var_name = format!("0x{}", hex::encode(key));

		let index = cast::SimpleCast::index(
			"bytes32",
			&var_name,
			&format!("0x{}", hex::encode(IBC_STORAGE_SLOT.add(U256::from(storage_index)).encode())),
		)
		.unwrap();

		let client = self.client().clone();
		let address = self.config.ibc_handler_address.clone();

		async move {
			Ok(client
				.get_proof(
					NameOrAddress::Address(address),
					vec![H256::from_str(&index).unwrap()],
					block_height.map(|i| BlockId::from(i)),
				)
				.await
				.unwrap())
		}
	}

	pub fn eth_query_proof_tokens(
		&self,
		tokens: &[Token],
		block_height: Option<u64>,
		storage_index: u32,
	) -> impl Future<Output = Result<EIP1186ProofResponse, ClientError>> {
		let vec1 = ethers::abi::encode_packed(tokens).unwrap();
		let key = ethers::utils::keccak256(&vec1);
		let key = hex::encode(key);

		let var_name = format!("0x{key}");
		let storage_index = format!("{storage_index}");
		let index =
			cast::SimpleCast::index("bytes32", dbg!(&var_name), dbg!(&storage_index)).unwrap();

		let client = self.client().clone();
		let address = self.config.ibc_handler_address.clone();

		dbg!(&address);
		dbg!(&H256::from_str(&index).unwrap());
		dbg!(&block_height);

		async move {
			Ok(client
				.get_proof(
					NameOrAddress::Address(address),
					vec![H256::from_str(&index).unwrap()],
					block_height.map(|i| BlockId::from(i)),
				)
				.await
				.unwrap())
		}
	}

	pub fn eth_query_proof_2d(
		&self,
		key1: &str,
		key2: &str,
		block_height: Option<u64>,
		storage_index: u32,
	) -> impl Future<Output = Result<EIP1186ProofResponse, ClientError>> {
		let key1 = ethers::utils::keccak256(key1.as_bytes());

		let combined_key1 = [key1.as_slice(), storage_index.to_be_bytes().as_ref()].concat();
		let key1_hashed = ethers::utils::keccak256(&combined_key1);
		let key1_hashed_hex = hex::encode(&key1_hashed);

		let key2 = ethers::utils::keccak256(key2.as_bytes());

		let combined_key2 = [key2.as_slice(), key1_hashed_hex.as_bytes()].concat();
		let key2_hashed = ethers::utils::keccak256(&combined_key2);
		let key2_hashed_hex = hex::encode(&key2_hashed);

		let index = cast::SimpleCast::index("bytes32", &key2_hashed_hex, &key2_hashed_hex).unwrap();

		let client = self.client().clone();
		let address = self.config.ibc_handler_address.clone();

		async move {
			client
				.get_proof(
					NameOrAddress::Address(address),
					vec![H256::from_str(&index).unwrap()],
					block_height.map(|i| BlockId::from(i)),
				)
				.map_err(|err| panic!("{err}"))
				.await
		}
	}

	pub fn query_client_impl_address(
		&self,
		client_id: ClientId,
		at: Height,
	) -> impl Future<Output = Result<(Vec<u8>, bool), ClientError>> + '_ {
		let fut = self.eth_query_proof(
			client_id.as_str(),
			Some(at.revision_height),
			CLIENT_IMPLS_STORAGE_INDEX,
		);

		async move {
			let proof = fut.await?;

			if let Some(storage_proof) = proof.storage_proof.first() {
				if !storage_proof.value.is_zero() {
					let binding = self
						.yui
						.method("getClientState", (client_id.as_str().to_owned(),))
						.expect("contract is missing getClientState");

					let get_client_state_fut = binding.call();
					let client_state: (Vec<u8>, bool) =
						get_client_state_fut.await.map_err(|err| todo!()).unwrap();

					Ok(client_state)
				} else {
					todo!("error: client address is zero")
				}
			} else {
				todo!("error: no storage proof")
			}
		}
	}

	#[track_caller]
	pub fn has_packet_receipt(
		&self,
		at: Height,
		port_id: String,
		channel_id: String,
		sequence: u64,
	) -> impl Future<Output = Result<bool, ClientError>> + '_ {
		async move {
			let binding = self
				.yui
				.method("hasPacketReceipt", (port_id, channel_id, sequence))
				.expect("contract is missing hasPacketReceipt");

			let receipt: bool = binding
				.block(BlockId::Number(BlockNumber::Number(at.revision_height.into())))
				.call()
				.await
				.map_err(|err| todo!())
				.unwrap();

			Ok(receipt)
		}
	}

	#[track_caller]
	pub fn has_acknowledgement(
		&self,
		at: Height,
		port_id: String,
		channel_id: String,
		sequence: u64,
	) -> impl Future<Output = Result<bool, ClientError>> + '_ {
		async move {
			let binding = self
				.yui
				.method("hasAcknowledgement", (port_id, channel_id, sequence))
				.expect("contract is missing hasAcknowledgement");

			// let receipt_fut = ;
			let receipt: bool = binding
				.block(BlockId::Number(BlockNumber::Number(at.revision_height.into())))
				.call()
				.await
				.map_err(|err| todo!())
				.unwrap();

			Ok(receipt)
		}
	}
}

// #[cfg(any(test, feature = "testing"))]
#[async_trait]
impl primitives::TestProvider for EthereumClient {
	async fn send_transfer(&self, params: MsgTransfer<PrefixedCoin>) -> Result<(), Self::Error> {
		todo!()
	}

	async fn send_ordered_packet(
		&self,
		channel_id: ChannelId,
		timeout: Timeout,
	) -> Result<(), Self::Error> {
		todo!()
	}

	async fn subscribe_blocks(&self) -> Pin<Box<dyn Stream<Item = u64> + Send + Sync>> {
		todo!()
	}

	async fn increase_counters(&mut self) -> Result<(), Self::Error> {
		todo!()
	}
}
