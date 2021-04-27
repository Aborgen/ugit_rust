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
  let file = format!("{}/{}/{:x}", ROOT, OBJECTS, object);
  fs::write(file, &contents)?;
  Ok(object)
}

pub fn get_object(oid: &str) -> std::io::Result<String> {
  let s = format!("{}/{}/{}", ROOT, OBJECTS, oid);
  let path = Path::new(&s);
  if !path.exists() {
    return Err(Error::new(ErrorKind::NotFound, "A file with the given OID does not exist"));
  }

  return fs::read_to_string(path);
}

#[cfg(test)]
mod tests {
  use serial_test::serial;
  use super::*;

  #[test]
  #[serial]
  fn init_subcommand_creates_expected_directory_tree() {
    let _ = fs::remove_dir_all(ROOT);

    init().unwrap();
    let objects_dir = format!("{}/{}", ROOT, OBJECTS);
    assert!(Path::new(&objects_dir).exists());

    fs::remove_dir_all(ROOT).unwrap();
  }

  #[test]
  #[serial]
  fn hash_object_subcommand_creates_copy_of_file_named_as_hash_of_same_file() {
    let test_file = "test.txt";
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "7efc67d0e4a88d7783770a9dd90d0a08bd63b29affb86e0a8cbd24fd5c63f587";
    let s = format!("{}/{}/{}", ROOT, OBJECTS, test_text_as_hash);
    let path_with_hash = Path::new(&s);

    let _ = fs::remove_dir_all(ROOT);
    init().unwrap();
    fs::write(test_file, test_text).unwrap();

    hash_object(test_file).unwrap();
    assert!(path_with_hash.is_file());
    let contents = fs::read_to_string(path_with_hash).unwrap();
    assert_eq!(&contents, test_text);

    fs::remove_dir_all(ROOT).unwrap();
    fs::remove_file(test_file).unwrap();
  }

  #[test]
  #[serial]
  fn get_object_subcommand_returns_contents_of_file_with_specified_oid_hash() {
    let test_file = "test.txt";
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "7efc67d0e4a88d7783770a9dd90d0a08bd63b29affb86e0a8cbd24fd5c63f587";

    let _ = fs::remove_dir_all(ROOT);
    init().unwrap();
    fs::write(test_file, test_text).unwrap();
    hash_object(test_file).unwrap();

    let contents = get_object(test_text_as_hash).unwrap();
    assert_eq!(&contents, test_text);

    fs::remove_dir_all(ROOT).unwrap();
    fs::remove_file(test_file).unwrap();
  }
}
