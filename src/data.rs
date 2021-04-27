use std::fs;
use std::io::{Error, ErrorKind};
use std::path::Path;

use sha2::{Digest, Sha256};
use sha2::digest::generic_array::{GenericArray, typenum::U32};

const ROOT: &str = ".ugit";
const OBJECTS: &str = "objects";
pub fn init() -> std::io::Result<()> {
  let objects_dir = format!("{}/{}", ROOT, OBJECTS);
  if Path::new(&objects_dir).exists() {
    return Err(Error::new(ErrorKind::AlreadyExists, "A ugit repository already exists"));
  }

  fs::create_dir_all(&objects_dir)?;
  return Ok(())
}

pub fn hash_object(filename: &str) -> std::io::Result<GenericArray<u8, U32>> {
  let mut hasher = Sha256::new();
  let contents = fs::read_to_string(filename)?;
  hasher.update(&contents);
  let object = hasher.finalize().into();
  let file = format!("{}/{:x}", OBJECTS_NAME, object);
  fs::write(file, &contents)?;
  Ok(object)
}
