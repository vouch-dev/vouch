use env_logger;
use structopt::StructOpt;

mod command;
mod common;
mod extension;
mod package;
mod peer;
mod registry;
mod review;
mod store;

fn main() {
    env_logger::Builder::from_env("VOUCH_LOG").init();

    let commands = command::Opts::from_args();
    match command::run_command(commands.command) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(-2)
        }
    }
}
