pub mod macros;

// pub mod dali;
pub mod default;
// pub mod picasso_kusama;
// pub mod picasso_rococo;
// pub mod composable;

pub use default::{
	DefaultConfig, DefaultConfig as ComposableConfig, DefaultConfig as PicassoKusamaConfig,
	DefaultConfig as PicassoRococoConfig,
};
// pub use composable::ComposableConfig;
// pub use picasso_kusama::PicassoKusamaConfig;
// pub use picasso_rococo::PicassoRococoConfig;

use codec::{Decode, Encode};
use light_client_common::config::{AsInner, BeefyAuthoritySetT};
use sp_core::H256;

#[derive(Encode, Decode)]
pub struct DummyBeefyAuthoritySet;

impl BeefyAuthoritySetT for DummyBeefyAuthoritySet {
	fn root(&self) -> H256 {
		unimplemented!("DummyBeefyAuthoritySet root")
	}

	fn len(&self) -> u32 {
		unimplemented!("DummyBeefyAuthoritySet len")
	}
}

impl AsInner for DummyBeefyAuthoritySet {
	type Inner = ();

	fn from_inner(_inner: Self::Inner) -> Self {
		Self
	}
}

pub fn unimplemented<T>(s: &'static str) -> T {
	unimplemented!("{s}")
}
