use std::fs;
use std::io::{Error, ErrorKind};
use std::path::Path;

use sha2::{Digest, Sha256};
use sha2::digest::generic_array::{GenericArray, typenum::U32};

const ROOT: &str = ".ugit";
const OBJECTS: &str = "objects";

#[derive(PartialEq)]
pub enum ObjectType {
  Blob,
}

pub fn init() -> std::io::Result<()> {
  let objects_dir = format!("{}/{}", ROOT, OBJECTS);
  if Path::new(&objects_dir).exists() {
    return Err(Error::new(ErrorKind::AlreadyExists, "A ugit repository already exists"));
  }

  fs::create_dir_all(&objects_dir)?;
  return Ok(())
}

pub fn hash_object(filename: &str, object_type: ObjectType) -> std::io::Result<GenericArray<u8, U32>> {
  let mut hasher = Sha256::new();
  let contents = fs::read_to_string(filename)?;
  let contents = match object_type {
    ObjectType::Blob => format!("blob\0{}", contents),
  };

  hasher.update(&contents);
  let object = hasher.finalize().into();
  let file = format!("{}/{}/{:x}", ROOT, OBJECTS, object);
  fs::write(file, &contents)?;
  Ok(object)
}

pub fn get_object(oid: &str, expected_type: ObjectType) -> std::io::Result<String> {
  let s = format!("{}/{}/{}", ROOT, OBJECTS, oid);
  let path = Path::new(&s);
  if !path.exists() {
    return Err(Error::new(ErrorKind::NotFound, "A file with the given OID does not exist"));
  }

  let contents = fs::read_to_string(path)?;
  let content_parts: Vec<_> = contents
    .splitn(2, char::from(0))
    .collect();

  if expected_type == ObjectType::Blob && content_parts[0] != "blob" {
    return Err(Error::new(ErrorKind::InvalidData, format!("Object was expected to be a blob, but was stored as a {}", content_parts[0])));
  }

  Ok(String::from(content_parts[1]))
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
    let test_text_as_hash = "bac94dbaf28c6916ef33cad50e4e1e88c3834f51dc7a5d40702a5cfdf324ab72";
    let s = format!("{}/{}/{}", ROOT, OBJECTS, test_text_as_hash);
    let path_with_hash = Path::new(&s);

    let _ = fs::remove_dir_all(ROOT);
    init().unwrap();
    fs::write(test_file, test_text).unwrap();

    hash_object(test_file, ObjectType::Blob).unwrap();
    assert!(path_with_hash.is_file());
    let contents = fs::read_to_string(path_with_hash).unwrap();
    assert_eq!(contents, format!("blob\0{}", test_text));

    fs::remove_dir_all(ROOT).unwrap();
    fs::remove_file(test_file).unwrap();
  }

  #[test]
  #[serial]
  fn get_object_subcommand_returns_contents_of_file_with_specified_oid_hash() {
    let test_file = "test.txt";
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "bac94dbaf28c6916ef33cad50e4e1e88c3834f51dc7a5d40702a5cfdf324ab72";

    let _ = fs::remove_dir_all(ROOT);
    init().unwrap();
    fs::write(test_file, test_text).unwrap();
    hash_object(test_file, ObjectType::Blob).unwrap();

    let contents = get_object(test_text_as_hash, ObjectType::Blob).unwrap();
    assert_eq!(&contents, test_text);

    fs::remove_dir_all(ROOT).unwrap();
    fs::remove_file(test_file).unwrap();
  }
}
