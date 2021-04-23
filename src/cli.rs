use clap::{App, SubCommand};

use crate::data;

pub fn cli() -> std::io::Result<()> {
  let matches = App::new(env!("CARGO_PKG_NAME"))
    .version(env!("CARGO_PKG_VERSION"))
    .author(env!("CARGO_PKG_AUTHORS"))
    .about(env!("CARGO_PKG_DESCRIPTION"))
    .subcommand(SubCommand::with_name("init")
      .about("Creates a new ugit repository"))
    .get_matches();

  if let Some(_) = matches.subcommand_matches("init") {
    init()?;
  }

  Ok(())
}

fn init() -> std::io::Result<()> {
  println!("Creating new ugit repository...");
  data::init()
}
