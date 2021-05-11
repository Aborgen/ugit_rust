use std::fs;
use std::path::Path;

use clap::{App, Arg, SubCommand};

use crate::base;
use crate::data;
use data::ObjectType;

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
    .subcommand(SubCommand::with_name("write-tree")
      .about("Stores current working directory to the object database"))
    .subcommand(SubCommand::with_name("read-tree")
      .about("Replaces current working directory with the one stored under provided OID")
      .arg(Arg::with_name("OID")
        .help("The resulting hash of the current working directory that has previously been hashed by the write-tree command")
        .required(true)
        .index(1)))
    .subcommand(SubCommand::with_name("commit")
      .about("Creates a new snapshot of the observed directory with a description")
      .arg(Arg::with_name("message")
        .long("message")
        .short("m")
        .takes_value(true)
        .value_name("TEXT")
        .required(true)
        .help("Description of the new commit")))
    .subcommand(SubCommand::with_name("log")
      .about("Prints descending list of commits")
      .arg(Arg::with_name("OID")
        .help("An optional starting point. By default, it will start from HEAD")
        .index(1)))
    .subcommand(SubCommand::with_name("checkout")
      .about("Sets HEAD to given commit OID, and updates observed directory with the contents of that commit")
      .arg(Arg::with_name("OID")
        .help("The commit identifier to set HEAD to")
        .required(true)
        .index(1)))
    .subcommand(SubCommand::with_name("tag")
      .about("Creates an alias NAME for either the given OID or HEAD")
      .arg(Arg::with_name("NAME")
        .help("The name of the tag to be created")
        .required(true)
        .index(1))
      .arg(Arg::with_name("OID")
        .help("The optional commit OID to be aliased")
        .required(false)
        .index(2)))
    .get_matches();

  if let Some(_) = matches.subcommand_matches("init") {
    init()?;
  }
  else if let Some(matches) = matches.subcommand_matches("hash-object") {
    // Can simply unwrap, as FILE arg's presence is required by clap
    let file = Path::new(matches.value_of("FILE").unwrap());
    hash_object(&file)?;
  }
  else if let Some(matches) = matches.subcommand_matches("cat-file") {
    // Can simply unwrap, as OID arg's presence is required by clap
    let oid = base::try_resolve_as_ref(matches.value_of("OID").unwrap())?;
    cat_file(&oid)?;
  }
  else if let Some(_) = matches.subcommand_matches("write-tree") {
    write_tree()?;
  }
  else if let Some(matches) = matches.subcommand_matches("read-tree") {
    // Can simply unwrap, as OID arg's presence is required by clap
    let oid = base::try_resolve_as_ref(matches.value_of("OID").unwrap())?;
    read_tree(&oid)?;
  }
  else if let Some(matches) = matches.subcommand_matches("commit") {
    // Can simply unwrap, as TEXT arg's presence is required by clap
    let message = matches.value_of("message").unwrap();
    commit(&message)?;
  }
  else if let Some(matches) = matches.subcommand_matches("log") {
    let oid = matches.value_of("OID");
    log(oid)?;
  }
  else if let Some(matches) = matches.subcommand_matches("checkout") {
    // Can simply unwrap, as OID arg's presence is required by clap
    let oid = base::try_resolve_as_ref(matches.value_of("OID").unwrap())?;
    checkout(&oid)?;
  }
  else if let Some(matches) = matches.subcommand_matches("tag") {
    // Can simply unwrap, as NAME arg's presence is required by clap
    let name = matches.value_of("NAME").unwrap();
    let oid = matches.value_of("OID");
    tag(&name, oid)?;
  }

  Ok(())
}

fn init() -> std::io::Result<()> {
  data::init()?;
  println!("Creating new ugit repository...");
  Ok(())
}

fn hash_object(filename: &Path) -> std::io::Result<()> {
  let contents = fs::read(filename)?;
  let hash = data::hash_object(&contents, ObjectType::Blob)?;
  println!("{}", hash);
  Ok(())
}

fn cat_file(oid: &str) -> std::io::Result<()> {
  let contents = data::get_object(oid, ObjectType::Blob)?;
  print!("{}", contents);
  Ok(())
}

fn write_tree() -> std::io::Result<()> {
  let hash = base::write_tree()?;
  println!("{}", hash);
  Ok(())
}

fn read_tree(oid: &str) -> std::io::Result<()> {
  base::read_tree(oid)?;
  println!("Restored current working directory [{}]", oid);
  Ok(())
}

fn commit(message: &str) -> std::io::Result<()> {
  let hash = base::commit(message)?;
  println!("Successfully created commit: [{}]", hash);
  Ok(())
}

fn log(oid: Option<&str>) -> std::io::Result<()> {
  let oid = match oid {
    Some(oid) => String::from(oid),
    None => match data::get_head() {
      Some(oid) => oid?,
      None => return Ok(())
    }
  };

  let mut oid = Some(oid);
  while let Some(s) = oid {
    let s = base::try_resolve_as_ref(&s)?;
    let commit = base::get_commit(&s)?;
    println!("commit {}", s);
    
    for line in commit.message.lines() {
      print!("\n{fill}{}", line, fill=" ".repeat(10));
    }

    oid = commit.parent;
    if oid.is_some() {
      println!("\n");
    }
  }

  println!("");
  Ok(())
}

fn checkout(oid: &str) -> std::io::Result<()> {
  base::checkout(oid)
}

fn tag(name: &str, oid: Option<&str>) -> std::io::Result<()> {
  let oid = match oid {
    Some(oid) => {
      base::try_resolve_as_ref(oid)?
    },
    None => {
      match data::get_head() {
        Some(oid) => oid?,
        None => return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "A ugit repository does not exist"))
      }
    }
  };

  base::create_tag(name, &oid)
}
