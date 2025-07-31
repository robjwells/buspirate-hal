// TODO: Patch out the warning-generating code.
#[allow(clippy::all)]
#[allow(unused_imports)]
mod bpio_generated;

mod buspirate;
mod eh_i2c;
mod error;
mod util;

pub mod modes;
pub mod transfer;

use bpio_generated::bpio;
use util::{Request, Response};

pub use buspirate::{open, BusPirate};
pub use error::Error;
