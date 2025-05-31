//! Substrate Node Template CLI library.
#![warn(missing_docs)]
#![feature(type_alias_impl_trait)]
#![allow(clippy::result_large_err)]
#![allow(clippy::type_complexity)]

mod benchmarking;
mod chain_spec;
mod cli;
mod command;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
    command::run()
}
