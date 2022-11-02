use anyhow::anyhow;
use codec::{Decode, Encode};
use std::{
	collections::BTreeMap,
	fmt::Display,
	pin::Pin,
	time::{Duration, Instant},
};

use beefy_gadget_rpc::BeefyApiClient;
use finality_grandpa::BlockNumberOps;
use futures::{Stream, StreamExt, TryFutureExt};
use grandpa_light_client_primitives::{FinalityProof, ParachainHeaderProofs};
use ibc_proto::google::protobuf::Any;
use sp_runtime::{
	generic::Era,
	traits::{Header as HeaderT, IdentifyAccount, One, Verify},
	MultiSignature, MultiSigner,
};
use subxt::tx::{BaseExtrinsicParamsBuilder, ExtrinsicParams};
use transaction_payment_rpc::TransactionPaymentApiClient;
use transaction_payment_runtime_api::RuntimeDispatchInfo;

use primitives::{Chain, IbcProvider, MisbehaviourHandler};

use super::{error::Error, signer::ExtrinsicSigner, ParachainClient};
use crate::{
	config,
	parachain::{api, api::runtime_types::pallet_ibc::Any as RawAny, UncheckedExtrinsic},
	FinalityProtocol,
};
use finality_grandpa_rpc::GrandpaApiClient;
use ibc::{
	core::{
		ics02_client::{
			events::UpdateClient,
			msgs::{update_client::MsgUpdateAnyClient, ClientMsg},
		},
		ics26_routing::msgs::Ics26Envelope,
	},
	tx_msg::Msg,
};
use ics10_grandpa::client_message::{ClientMessage, Misbehaviour, RelayChainHeader};
use pallet_ibc::light_clients::AnyClientMessage;
use primitives::mock::LocalClientTypes;
use sp_core::H256;
use subxt::tx::{PlainTip, PolkadotExtrinsicParamsBuilder};
use tokio::time::sleep;

type GrandpaJustification = grandpa_light_client_primitives::justification::GrandpaJustification<
	polkadot_core_primitives::Header,
>;

type BeefyJustification =
	beefy_primitives::SignedCommitment<u32, beefy_primitives::crypto::Signature>;

/// An encoded justification proving that the given header has been finalized
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct JustificationNotification(sp_core::Bytes);

#[async_trait::async_trait]
impl<T: config::Config + Send + Sync> Chain for ParachainClient<T>
where
	u32: From<<<T as subxt::Config>::Header as HeaderT>::Number>,
	u32: From<<T as subxt::Config>::BlockNumber>,
	<T::Signature as Verify>::Signer: From<MultiSigner> + IdentifyAccount<AccountId = T::AccountId>,
	MultiSigner: From<MultiSigner>,
	<T as subxt::Config>::Address: From<<T as subxt::Config>::AccountId>,
	T::Signature: From<MultiSignature>,
	T::BlockNumber: BlockNumberOps + From<u32> + Display + Ord + sp_runtime::traits::Zero + One,
	T::Hash: From<sp_core::H256> + From<[u8; 32]>,
	FinalityProof<sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>>:
		From<FinalityProof<T::Header>>,
	BTreeMap<sp_core::H256, ParachainHeaderProofs>:
		From<BTreeMap<<T as subxt::Config>::Hash, ParachainHeaderProofs>>,
	sp_core::H256: From<T::Hash>,
	<T::ExtrinsicParams as ExtrinsicParams<T::Index, T::Hash>>::OtherParams:
		From<BaseExtrinsicParamsBuilder<T, PlainTip>> + Send + Sync,
{
	fn name(&self) -> &str {
		&*self.name
	}

	fn block_max_weight(&self) -> u64 {
		self.max_extrinsic_weight
	}

	async fn estimate_weight(&self, messages: Vec<Any>) -> Result<u64, Self::Error> {
		let extrinsic = {
			// todo: put this in utils
			let signer = ExtrinsicSigner::<T, Self>::new(
				self.key_store.clone(),
				self.key_type_id.clone(),
				self.public_key.clone(),
			);

			let messages = messages
				.into_iter()
				.map(|msg| RawAny { type_url: msg.type_url.as_bytes().to_vec(), value: msg.value })
				.collect::<Vec<_>>();

			let tx_params = PolkadotExtrinsicParamsBuilder::new()
				.tip(PlainTip::new(100_000))
				.era(Era::Immortal, self.para_client.genesis_hash());
			let call = api::tx().ibc().deliver(messages);
			self.para_client.tx().create_signed(&call, &signer, tx_params.into()).await?
		};
		let dispatch_info =
			TransactionPaymentApiClient::<sp_core::H256, RuntimeDispatchInfo<u128>>::query_info(
				&*self.para_ws_client,
				extrinsic.encoded().to_vec().into(),
				None,
			)
			.await
			.map_err(|e| Error::from(format!("Rpc Error {:?}", e)))?;
		Ok(dispatch_info.weight)
	}

	async fn finality_notifications(
		&self,
	) -> Pin<Box<dyn Stream<Item = <Self as IbcProvider>::FinalityEvent> + Send + Sync>> {
		match self.finality_protocol {
			FinalityProtocol::Grandpa => {
				let subscription =
					GrandpaApiClient::<JustificationNotification, sp_core::H256, u32>::subscribe_justifications(
						&*self.relay_ws_client,
					)
						.await
						.expect("Failed to subscribe to grandpa justifications")
						.chunks(6)
						.map(|mut notifs| notifs.remove(notifs.len() - 1)); // skip every 4 finality notifications

				let stream = subscription.filter_map(|justification_notif| {
					let encoded_justification = match justification_notif {
						Ok(JustificationNotification(sp_core::Bytes(justification))) =>
							justification,
						Err(err) => {
							log::error!("Failed to fetch Justification: {}", err);
							return futures::future::ready(None)
						},
					};

					let justification =
						match GrandpaJustification::decode(&mut &*encoded_justification) {
							Ok(j) => j,
							Err(err) => {
								log::error!("Grandpa Justification scale decode error: {}", err);
								return futures::future::ready(None)
							},
						};
					futures::future::ready(Some(Self::FinalityEvent::Grandpa(justification)))
				});

				Box::pin(Box::new(stream))
			},
			FinalityProtocol::Beefy => {
				let subscription =
					BeefyApiClient::<JustificationNotification, sp_core::H256>::subscribe_justifications(
						&*self.relay_ws_client,
					)
						.await
						.expect("Failed to subscribe to beefy justifications");

				let stream = subscription.filter_map(|commitment_notification| {
					let encoded_commitment = match commitment_notification {
						Ok(JustificationNotification(sp_core::Bytes(commitment))) => commitment,
						Err(err) => {
							log::error!("Failed to fetch Commitment: {}", err);
							return futures::future::ready(None)
						},
					};

					let signed_commitment =
						match BeefyJustification::decode(&mut &*encoded_commitment) {
							Ok(c) => c,
							Err(err) => {
								log::error!("SignedCommitment scale decode error: {}", err);
								return futures::future::ready(None)
							},
						};
					futures::future::ready(Some(Self::FinalityEvent::Beefy(signed_commitment)))
				});

				Box::pin(Box::new(stream))
			},
		}
	}

	async fn submit(
		&self,
		messages: Vec<Any>,
	) -> Result<(sp_core::H256, Option<sp_core::H256>), Error> {
		let messages = messages
			.into_iter()
			.map(|msg| RawAny { type_url: msg.type_url.as_bytes().to_vec(), value: msg.value })
			.collect::<Vec<_>>();

		let call = api::tx().ibc().deliver(messages);
		let (ext_hash, block_hash) = self.submit_call(call).await?;

		Ok((ext_hash.into(), Some(block_hash.into())))
	}

	async fn query_client_message(&self, update: UpdateClient) -> Result<AnyClientMessage, Error> {
		use api::runtime_types::{
			pallet_ibc::pallet::Call as IbcCall, parachain_runtime::Call as RuntimeCall,
		};

		let host_height = update.height();
		let light_client_height = update.consensus_height();

		// todo:
		// first query block events at host_height.
		// next find the event that matches update
		// get extrinsic that emitted event.
		// profit.
	}
}

#[async_trait::async_trait]
impl<T: config::Config + Send + Sync> MisbehaviourHandler for ParachainClient<T>
where
	u32: From<<<T as subxt::Config>::Header as HeaderT>::Number>,
	u32: From<<T as subxt::Config>::BlockNumber>,
	<T::Signature as Verify>::Signer: From<MultiSigner> + IdentifyAccount<AccountId = T::AccountId>,
	MultiSigner: From<MultiSigner>,
	<T as subxt::Config>::Address: From<<T as subxt::Config>::AccountId>,
	T::Signature: From<MultiSignature>,
	T::BlockNumber: BlockNumberOps + From<u32> + Display + Ord + sp_runtime::traits::Zero + One,
	T::Hash: From<sp_core::H256> + From<[u8; 32]>,
	FinalityProof<sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>>:
		From<FinalityProof<T::Header>>,
	BTreeMap<sp_core::H256, ParachainHeaderProofs>:
		From<BTreeMap<<T as subxt::Config>::Hash, ParachainHeaderProofs>>,
	sp_core::H256: From<T::Hash>,
	<T::ExtrinsicParams as ExtrinsicParams<T::Index, T::Hash>>::OtherParams:
		From<BaseExtrinsicParamsBuilder<T, PlainTip>> + Send + Sync,
{
	async fn check_for_misbehaviour<C: Chain>(
		&self,
		counterparty: &C,
		client_message: AnyClientMessage,
	) -> Result<(), anyhow::Error> {
		match client_message {
			AnyClientMessage::Grandpa(ClientMessage::Header(header)) => {
				let target_block_number = header
					.finality_proof
					.unknown_headers
					.iter()
					.max_by_key(|h| h.number)
					.expect("unknown_headers always contain at least one header; qed")
					.number;
				let finalized_block_number =
					*self.relay_client.rpc().block(None).await?.unwrap().block.header.number();
				// We require a proof for the block number that may not exist on the relay chain.
				// So, if it's greater than the latest block block the relay chain, we use the
				// latter.
				let encoded =
					GrandpaApiClient::<JustificationNotification, H256, u32>::prove_finality(
						&*self.relay_ws_client,
						target_block_number
							.min(u32::from(finalized_block_number))
							.saturating_sub(1),
					)
					.await?
					.ok_or_else(|| {
						anyhow!(
							"No justification found for block: {:?}",
							header.finality_proof.block
						)
					})?
					.0;

				// TODO: sometimes `unknown_blocks` don't contain any blocks. Investigate why
				let trusted_finality_proof =
					FinalityProof::<RelayChainHeader>::decode(&mut &encoded[..])?;

				let justification =
					GrandpaJustification::decode(&mut &*header.finality_proof.justification)?;
				let trusted_justification =
					GrandpaJustification::decode(&mut &*trusted_finality_proof.justification)?;
				if justification.commit.target_hash != trusted_justification.commit.target_hash {
					log::warn!(
						"Found misbehaviour on client {}: {:?} != {:?}",
						self.client_id
							.as_ref()
							.map(|x| x.as_str().to_owned())
							.unwrap_or_else(|| "{unknown}".to_owned()),
						header.finality_proof.block,
						trusted_finality_proof.block
					);

					let misbehaviour = ClientMessage::Misbehaviour(Misbehaviour {
						first_finality_proof: header.finality_proof,
						second_finality_proof: trusted_finality_proof,
					});

					counterparty
						.submit(vec![MsgUpdateAnyClient::<LocalClientTypes>::new(
							self.client_id(),
							AnyClientMessage::Grandpa(misbehaviour.clone()),
							counterparty.account_id(),
						)
						.to_any()])
						.map_err(|e| anyhow!("Failed to submit misbehaviour report: {:?}", e))
						.await?;
				}
			},
			_ => {},
		}
		Ok(())
	}
}
