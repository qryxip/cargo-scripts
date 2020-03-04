use cargo_scripts::{Context, Opt};

use human_panic::setup_panic;
use structopt::StructOpt as _;

fn main() -> anyhow::Result<()> {
    setup_panic!();
    cargo_scripts::run(Opt::from_args(), Context::new()?)
}
