use clap::{App, Arg, SubCommand};

use crate::data;

pub fn cli() -> std::io::Result<()> {
  let matches = App::new(env!("CARGO_PKG_NAME"))
    .version(env!("CARGO_PKG_VERSION"))
    .author(env!("CARGO_PKG_AUTHORS"))
    .about(env!("CARGO_PKG_DESCRIPTION"))
    .subcommand(SubCommand::with_name("init")
      .about("Creates a new ugit repository"))
    .subcommand(SubCommand::with_name("hash-object")
      .about("Returns the SHA2 hash of a file")
      .arg(Arg::with_name("FILE")
        .help("The path to a file to be hashed")
        .required(true)
        .index(1)))
    .get_matches();

  if let Some(_) = matches.subcommand_matches("init") {
    init()?;
  }
  else if let Some(matches) = matches.subcommand_matches("hash-object") {
    // Can simply unwrap, as FILE arg's presence is handled by clap
    let file = matches.value_of("FILE").unwrap();
    hash_object(file)?;
  }

  Ok(())
}

fn init() -> std::io::Result<()> {
  println!("Creating new ugit repository...");
  data::init()
}

fn hash_object(filename: &str) -> std::io::Result<()> {
  let hash = data::hash_object(filename)?;
  println!("{:x}", hash);
  Ok(())
}
