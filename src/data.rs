use std::fs;

use sha2::{Digest, Sha256};
use sha2::digest::generic_array::{GenericArray, typenum::U32};

const DIRECTORY_NAME: &str = ".ugit";
pub fn init() -> std::io::Result<()> {
  fs::create_dir(DIRECTORY_NAME)?;
  return Ok(())
}

pub fn hash_object(filename: &str) -> std::io::Result<GenericArray<u8, U32>> {
  let mut hasher = Sha256::new();
  let contents = fs::read_to_string(filename)?;
  hasher.update(contents);
  let result = hasher.finalize().into();
  Ok(result)
}
