#[macro_export]
macro_rules! define_id {
	(
		$name: ident,
		$id_type: path
	) => {
		#[derive(Decode)]
		pub struct $name(pub $id_type);

		impl From<u32> for $name {
			fn from(value: u32) -> Self {
				$name($id_type(value))
			}
		}

		impl From<$name> for u32 {
			fn from(value: $name) -> Self {
				value.0 .0
			}
		}
	};
}

#[macro_export]
macro_rules! define_head_data {
	(
		$name: ident,
		$head_data_type: ty,
	) => {
		#[derive(Decode)]
		pub struct $name(pub $head_data_type);

		impl AsRef<[u8]> for $name {
			fn as_ref(&self) -> &[u8] {
				self.0 .0.as_ref()
			}
		}

		impl Into<Vec<u8>> for $name {
			fn into(self) -> Vec<u8> {
				self.0 .0
			}
		}
	};
}

#[macro_export]
macro_rules! define_para_lifecycle {
	(
		$name: ident,
		$ty: ty
	) => {
		#[derive(Decode)]
		pub struct $name(pub $ty);

		impl ParaLifecycleT for $name {
			fn is_parachain(&self) -> bool {
				matches!(self.0, <$ty>::Parachain)
			}
		}
	};
}

#[macro_export]
macro_rules! define_beefy_authority_set {
	(
		$name: ident,
		$ty: ty
	) => {
		#[derive(Decode, Encode)]
		pub struct $name<T>(pub $ty);

		impl BeefyAuthoritySetT for $name<H256> {
			fn root(&self) -> H256 {
				self.0.root
			}

			fn len(&self) -> u32 {
				self.0.len
			}
		}
	};
}

#[macro_export]
macro_rules! define_pallet_params {
	(
		$name: ident,
		$pallet_params_type: ty,
		$raw_pallet_params_type: ty
	) => {
		pub struct $name(pub $pallet_params_type);

		impl From<$name> for $raw_pallet_params_type {
			fn from(value: $name) -> Self {
				let params = value.0;
				Self { send_enabled: params.send_enabled, receive_enabled: params.receive_enabled }
			}
		}
	};
}

#[macro_export]
macro_rules! define_any_wrapper {
	(
		$name: ident,
		$raw_any_type: ty
	) => {
		pub struct $name(pub $raw_any_type);

		impl From<$name> for Any {
			fn from(value: $name) -> Self {
				Any {
					type_url: String::from_utf8(value.0.type_url.into()).unwrap(),
					value: value.0.value,
				}
			}
		}
	};
}

#[macro_export]
macro_rules! define_ibc_event_wrapper {
	(
		$name: ident,
		$meta_ibc_event_type: ty
	) => {
		/// Allows to implement traits for the subxt generated code
		pub struct $name(pub $meta_ibc_event_type);

		impl From<$name> for RawIbcEvent {
			fn from(event: $name) -> Self {
				let event = event.0;
				match event {
					MetadataIbcEvent::NewBlock { revision_height, revision_number } =>
						RawIbcEvent::NewBlock { revision_height, revision_number },
					MetadataIbcEvent::OpenInitConnection {
						revision_height,
						revision_number,
						connection_id,
						counterparty_connection_id,
						client_id,
						counterparty_client_id,
					} => RawIbcEvent::OpenInitConnection {
						revision_height,
						revision_number,
						connection_id,
						counterparty_connection_id,
						client_id,
						counterparty_client_id,
					},
					MetadataIbcEvent::OpenTryConnection {
						revision_height,
						revision_number,
						connection_id,
						counterparty_connection_id,
						client_id,
						counterparty_client_id,
					} => RawIbcEvent::OpenTryConnection {
						revision_height,
						revision_number,
						connection_id,
						counterparty_connection_id,
						client_id,
						counterparty_client_id,
					},
					MetadataIbcEvent::OpenAckConnection {
						revision_height,
						revision_number,
						connection_id,
						counterparty_connection_id,
						client_id,
						counterparty_client_id,
					} => RawIbcEvent::OpenAckConnection {
						revision_height,
						revision_number,
						connection_id,
						counterparty_connection_id,
						client_id,
						counterparty_client_id,
					},
					MetadataIbcEvent::OpenConfirmConnection {
						revision_height,
						revision_number,
						connection_id,
						counterparty_connection_id,
						client_id,
						counterparty_client_id,
					} => RawIbcEvent::OpenConfirmConnection {
						revision_height,
						revision_number,
						connection_id,
						counterparty_connection_id,
						client_id,
						counterparty_client_id,
					},
					MetadataIbcEvent::OpenInitChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					} => RawIbcEvent::OpenInitChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					},
					MetadataIbcEvent::OpenTryChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					} => RawIbcEvent::OpenTryChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					},
					MetadataIbcEvent::OpenAckChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					} => RawIbcEvent::OpenAckChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					},
					MetadataIbcEvent::OpenConfirmChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					} => RawIbcEvent::OpenConfirmChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					},
					MetadataIbcEvent::CloseInitChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					} => RawIbcEvent::CloseInitChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					},
					MetadataIbcEvent::CloseConfirmChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					} => RawIbcEvent::CloseConfirmChannel {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						connection_id,
						counterparty_port_id,
						counterparty_channel_id,
					},
					MetadataIbcEvent::SendPacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						dest_port,
						dest_channel,
						sequence,
					} => RawIbcEvent::SendPacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						dest_port,
						dest_channel,
						sequence,
					},
					MetadataIbcEvent::WriteAcknowledgement {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						dest_port,
						dest_channel,
						sequence,
					} => RawIbcEvent::WriteAcknowledgement {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						dest_port,
						dest_channel,
						sequence,
					},
					MetadataIbcEvent::TimeoutPacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						sequence,
					} => RawIbcEvent::TimeoutPacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						sequence,
					},
					MetadataIbcEvent::TimeoutOnClosePacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						sequence,
					} => RawIbcEvent::TimeoutOnClosePacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						sequence,
					},
					MetadataIbcEvent::CreateClient {
						client_id,
						client_type,
						revision_height,
						revision_number,
						consensus_height,
						consensus_revision_number,
					} => RawIbcEvent::CreateClient {
						client_id,
						client_type,
						revision_height,
						revision_number,
						consensus_height,
						consensus_revision_number,
					},
					MetadataIbcEvent::UpdateClient {
						client_id,
						client_type,
						revision_height,
						revision_number,
						consensus_height,
						consensus_revision_number,
					} => RawIbcEvent::UpdateClient {
						client_id,
						client_type,
						revision_height,
						revision_number,
						consensus_height,
						consensus_revision_number,
					},
					MetadataIbcEvent::UpgradeClient {
						client_id,
						client_type,
						revision_height,
						revision_number,
						consensus_height,
						consensus_revision_number,
					} => RawIbcEvent::UpgradeClient {
						client_id,
						client_type,
						revision_height,
						revision_number,
						consensus_height,
						consensus_revision_number,
					},
					MetadataIbcEvent::ClientMisbehaviour {
						client_id,
						client_type,
						revision_height,
						revision_number,
						consensus_height,
						consensus_revision_number,
					} => RawIbcEvent::ClientMisbehaviour {
						client_id,
						client_type,
						revision_height,
						revision_number,
						consensus_height,
						consensus_revision_number,
					},
					MetadataIbcEvent::ReceivePacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						dest_port,
						dest_channel,
						sequence,
					} => RawIbcEvent::ReceivePacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						dest_port,
						dest_channel,
						sequence,
					},
					MetadataIbcEvent::AcknowledgePacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						sequence,
					} => RawIbcEvent::AcknowledgePacket {
						revision_height,
						revision_number,
						port_id,
						channel_id,
						sequence,
					},
					MetadataIbcEvent::AppModule { kind, module_id } =>
						RawIbcEvent::AppModule { kind, module_id },
					MetadataIbcEvent::Empty => RawIbcEvent::Empty,
					MetadataIbcEvent::ChainError => RawIbcEvent::ChainError,
				}
			}
		}
	};
}

#[macro_export]
macro_rules! define_send_ping_params {
	(
		$name: ident,
		$send_ping_params_type: ty,
		$raw_send_ping_params_type: ty
	) => {
		pub struct $name(pub $send_ping_params_type);

		impl From<$name> for $raw_send_ping_params_type {
			fn from(value: $name) -> Self {
				let params = value.0;
				Self {
					data: params.data,
					timeout_height_offset: params.timeout_height_offset,
					timeout_timestamp_offset: params.timeout_timestamp_offset,
					channel_id: params.channel_id,
				}
			}
		}
	};
}

#[macro_export]
macro_rules! define_transfer_params {
	(
		$name: ident,
		$transfer_params_type: ty,
		$raw_transfer_params_type: ty,
		$timeout_type: ty,
		$address_type: ty
	) => {
		pub struct $name(pub $transfer_params_type);

		impl<T> From<$name> for $raw_transfer_params_type
		where
			T: From<[u8; 32]>,
		{
			fn from(value: $name) -> Self {
				let params = value.0;
				Self {
					to: match params.to {
						MultiAddress::Id(id) => {
							let id: [u8; 32] = id.into();
							<$address_type>::Id(id.into())
						},
						MultiAddress::Raw(raw) => <$address_type>::Raw(raw),
					},

					source_channel: params.source_channel,
					timeout: match params.timeout {
						Timeout::Offset { timestamp, height } =>
							RawTimeout::Offset { timestamp, height },
						Timeout::Absolute { timestamp, height } =>
							RawTimeout::Absolute { timestamp, height },
					},
				}
			}
		}
	};
}

#[macro_export]
macro_rules! define_runtime_storage {
	(
		$name:ident,
		$head_data:ty,
		$id:ty,
		$para_lifecycle:ty,
		$beefy_authority_set:ty,
		$timestamp_now:expr,
		$paras_heads:expr,
		$paras_para_lifecycles:expr,
		$paras_parachains:expr,
		$grandpa_current_set_id:expr,
		$beefy_validator_set_id:expr,
		$beefy_authorities:expr,
		$mmr_leaf_beefy_next_authorities:expr,
		$babe_epoch_start:expr
	) => {
		pub struct $name;

		impl RuntimeStorage for $name {
			type HeadData = $head_data;
			type Id = $id;
			type ParaLifecycle = $para_lifecycle;
			type BeefyAuthoritySet = $beefy_authority_set;

			fn timestamp_now() -> StaticStorageAddress<DecodeStaticType<u64>, Yes, Yes, ()> {
				$timestamp_now
			}

			fn paras_heads(
				x: u32,
			) -> LocalStaticStorageAddress<DecodeStaticType<Self::HeadData>, Yes, (), Yes> {
				let storage = $paras_heads(&Self::Id::from(x).0);
				LocalStaticStorageAddress::new("Paras", "Heads", storage)
			}

			fn paras_para_lifecycles(
				x: u32,
			) -> LocalStaticStorageAddress<DecodeStaticType<Self::ParaLifecycle>, Yes, (), Yes> {
				let storage = $paras_para_lifecycles(&Self::Id::from(x).0);
				LocalStaticStorageAddress::new("Paras", "ParaLifecycles", storage)
			}

			fn paras_parachains(
			) -> LocalStaticStorageAddress<DecodeStaticType<Vec<Self::Id>>, Yes, Yes, ()> {
				let storage = $paras_parachains;
				LocalStaticStorageAddress::new("Paras", "Parachains", storage)
			}

			fn grandpa_current_set_id() -> StaticStorageAddress<DecodeStaticType<u64>, Yes, Yes, ()>
			{
				$grandpa_current_set_id
			}

			fn beefy_validator_set_id() -> StaticStorageAddress<DecodeStaticType<u64>, Yes, Yes, ()>
			{
				$beefy_validator_set_id
			}

			fn beefy_authorities() -> LocalStaticStorageAddress<
				DecodeStaticType<Vec<sp_beefy::crypto::Public>>,
				Yes,
				Yes,
				(),
			> {
				let storage = $beefy_authorities;
				LocalStaticStorageAddress::new("Beefy", "Authorities", storage)
			}

			fn mmr_leaf_beefy_next_authorities(
			) -> LocalStaticStorageAddress<DecodeStaticType<Self::BeefyAuthoritySet>, Yes, Yes, ()>
			{
				let storage = $mmr_leaf_beefy_next_authorities;
				LocalStaticStorageAddress::new("MmrLeaf", "BeefyNextAuthorities", storage)
			}

			fn babe_epoch_start() -> StaticStorageAddress<DecodeStaticType<(u32, u32)>, Yes, Yes, ()>
			{
				$babe_epoch_start
			}
		}
	};
}

#[macro_export]
macro_rules! define_runtime_transactions {
	(
		$name:ident,
		$deliver:ty,
		$transfer:ty,
		$sudo:ty,
		$send_ping:ty,
		$para_runtime_call:ty,
		$send_ping_params:ty,
		$transfer_params:ty,
		$transfer_wrapper:expr,
		$send_ping_params_wrapper:expr,
		$any: path,
		$ibc_deliver: expr,
		$ibc_transfer: expr,
		$sudo_sudo: expr,
		$ibc_ping_send_ping: expr
	) => {
		pub struct $name;

		impl RuntimeTransactions for $name {
			type Deliver = $deliver;
			type Transfer = $transfer;
			type Sudo = $sudo;
			type SendPing = $send_ping;

			type ParaRuntimeCall = $para_runtime_call;
			type SendPingParams = $send_ping_params;
			type TransferParams = $transfer_params;

			fn ibc_deliver(messages: Vec<Any>) -> StaticTxPayload<Self::Deliver> {
				use $any as Any;
				$ibc_deliver(
					messages
						.into_iter()
						.map(|x| Any { type_url: x.type_url.into(), value: x.value })
						.collect(),
				)
			}

			fn ibc_transfer(
				params: Self::TransferParams,
				asset_id: u128,
				amount: u128,
				memo: Option<()>,
			) -> StaticTxPayload<Self::Transfer> {
				$ibc_transfer(
					$transfer_wrapper(params).into(),
					asset_id,
					amount,
					memo.map(|_| MemoMessage),
				)
			}

			fn sudo_sudo(call: Self::ParaRuntimeCall) -> StaticTxPayload<Self::Sudo> {
				$sudo_sudo(call.0)
			}

			fn ibc_ping_send_ping(params: Self::SendPingParams) -> StaticTxPayload<Self::SendPing> {
				$ibc_ping_send_ping($send_ping_params_wrapper(params).into())
			}
		}
	};
}

#[macro_export]
macro_rules! define_event_record {
	($name:ident, $event_record:ty, $ibc_event_wrapper: expr, $phase: path, $pallet_event: path, $runtime_event: path) => {
		#[derive(Decode)]
		pub struct $name(pub $event_record);

		impl EventRecordT for $name {
			type IbcEvent = pallet_ibc::events::IbcEvent;

			fn phase(&self) -> Phase {
				use $phase as ParaPhase;
				match self.0.phase {
					ParaPhase::ApplyExtrinsic(i) => Phase::ApplyExtrinsic(i as u32),
					ParaPhase::Finalization => Phase::Finalization,
					ParaPhase::Initialization => Phase::Initialization,
				}
			}

			fn ibc_events(self) -> Option<Vec<pallet_ibc::events::IbcEvent>> {
				use $pallet_event as PalletEvent;
				use $runtime_event as RuntimeEvent;
				if let RuntimeEvent::Ibc(PalletEvent::Events { events }) = self.0.event.0 {
					let events = events
						.into_iter()
						.filter_map(|event| {
							let ev = event.ok()?;
							Some(pallet_ibc::events::IbcEvent::from($ibc_event_wrapper(ev)))
						})
						.collect();
					Some(events)
				} else {
					None
				}
			}
		}
	};
}

#[macro_export]
macro_rules! define_events {
	($name:ident, $events:ty, $ibc_event_wrapper: expr) => {
		#[derive(Decode)]
		pub struct $name(pub $events);

		impl IbcEventsT for $name {
			type IbcEvent = pallet_ibc::events::IbcEvent;

			fn events(self) -> Vec<Self::IbcEvent> {
				self.0
					.events
					.into_iter()
					.filter_map(|event| {
						let ev = event.ok()?;
						Some(Self::IbcEvent::from($ibc_event_wrapper(ev)))
					})
					.collect()
			}
		}

		impl StaticEvent for $name {
			const PALLET: &'static str = <$events>::PALLET;
			const EVENT: &'static str = <$events>::EVENT;

			fn is_event(pallet: &str, event: &str) -> bool {
				<$events>::is_event(pallet, event)
			}
		}
	};
}

#[macro_export]
macro_rules! define_runtime_event {
	($name:ident, $runtime_event:ty) => {
		#[derive(Decode)]
		pub struct $name(pub $runtime_event);
	};
}

#[macro_export]
macro_rules! define_runtime_call {
	($name:ident, $runtime_call: path, $pallet_params_wrapper: expr, $any_wrapper: expr, $call: path) => {
		#[derive(Decode)]
		pub struct $name(pub $runtime_call);

		impl RuntimeCall for $name {
			type PalletParams = PalletParams;

			fn extract_ibc_deliver_messages(self) -> Option<Vec<Any>> {
				use $call as IbcCall;
				use $runtime_call as RuntimeCall;
				match self.0 {
					RuntimeCall::Ibc(IbcCall::deliver { messages }) =>
						Some(messages.into_iter().map(|m| $any_wrapper(m).into()).collect()),
					_ => None,
				}
			}

			fn pallet_ibc_set_params(params: PalletParams) -> Self {
				use $call as IbcCall;
				use $runtime_call as RuntimeCall;
				Self(RuntimeCall::Ibc(IbcCall::set_params {
					params: $pallet_params_wrapper(params).into(),
				}))
			}
		}
	};
}

#[macro_export]
macro_rules! define_asset_id {
	($name:ident, $ty:ty) => {
		#[derive(Encode, Decode)]
		pub struct $name(pub $ty);

		impl From<u128> for $name {
			fn from(value: u128) -> Self {
				use $ty as CurrencyId;
				Self(CurrencyId(value))
			}
		}

		impl Into<u128> for $name {
			fn into(self) -> u128 {
				self.0 .0
			}
		}

		impl Clone for $name {
			fn clone(&self) -> Self {
				use $ty as CurrencyId;
				Self(CurrencyId(self.0 .0))
			}
		}

		impl Serialize for $name {
			fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
			where
				S: Serializer,
			{
				serializer.serialize_u128(self.0 .0)
			}
		}
	};
}
