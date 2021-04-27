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
    .subcommand(SubCommand::with_name("cat-file")
      .about("Writes contents of file with given OID to stdout")
      .arg(Arg::with_name("OID")
        .help("The resulting hash of a file that has previously been hashed by the hash-object command")
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
  else if let Some(matches) = matches.subcommand_matches("cat-file") {
    // Can simply unwrap, as OID arg's presence is handled by clap
    let oid = matches.value_of("OID").unwrap();
    cat_file(oid)?;
  }

  Ok(())
}

fn init() -> std::io::Result<()> {
  let result = data::init();
  if let Ok(_) = result {
    println!("Creating new ugit repository...");
  }

  result
}

fn hash_object(filename: &str) -> std::io::Result<()> {
  let hash = data::hash_object(filename)?;
  println!("{:x}", hash);
  Ok(())
}

fn cat_file(oid: &str) -> std::io::Result<()> {
  let contents = data::get_object(oid)?;
  println!("{}", contents);
  Ok(())
}
