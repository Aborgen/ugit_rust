use std::fs;
use std::io::{Error, ErrorKind};
use std::path::Path;

use sha2::{Digest, Sha256};

const ROOT: &str = ".ugit";
const OBJECTS: &str = "objects";

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub enum ObjectType {
  Blob,
  Commit,
  Tree,
}

pub fn init() -> std::io::Result<()> {
  let objects_dir = format!("{}/{}", ROOT, OBJECTS);
  if Path::new(ROOT).exists() {
    return Err(Error::new(ErrorKind::AlreadyExists, "A ugit repository already exists"));
  }

  fs::create_dir_all(&objects_dir)?;
  return Ok(())
}

pub fn hash_object(file_contents: &[u8], object_type: ObjectType) -> std::io::Result<String> {
  if !Path::new(ROOT).exists() {
    return Err(Error::new(ErrorKind::NotFound, "A ugit repository does not exist in this directory"));
  }

  // ugit objects are their object type, followed by a null byte, and then the file contents
  let mut contents = match object_type {
    ObjectType::Blob => String::from("blob\0").into_bytes(),
    ObjectType::Commit => String::from("commit\0").into_bytes(),
    ObjectType::Tree => String::from("tree\0").into_bytes(),
  };

  contents.extend(file_contents);

  let mut hasher = Sha256::new();
  hasher.update(&contents);
  let object: [u8; 32] = hasher.finalize().into();
  let mut s = String::new();
  for byte in object.iter() {
    s = format!("{}{:x}", s, byte);
  }

  let file = format!("{}/{}/{}", ROOT, OBJECTS, s);
  fs::write(file, &contents)?;
  Ok(s)
}

pub fn get_object(oid: &str, expected_type: ObjectType) -> std::io::Result<String> {
  if !Path::new(ROOT).exists() {
    return Err(Error::new(ErrorKind::NotFound, "A ugit repository does not exist in this directory"));
  }

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
    return Err(Error::new(ErrorKind::InvalidData, format!("Object was expected to be a blob, but wasn't")));
  }
  else if expected_type == ObjectType::Commit && content_parts[0] != "commit" {
    return Err(Error::new(ErrorKind::InvalidData, format!("Object was expected to be a commit, but wasn't")));
  }
  else if expected_type == ObjectType::Tree && content_parts[0] != "tree" {
    return Err(Error::new(ErrorKind::InvalidData, format!("Object was expected to be a tree, but wasn't")));
  }

  Ok(String::from(content_parts[1]))
}

pub fn set_head(oid: &str) -> std::io::Result<()> {
  let s = format!("{}/HEAD", ROOT);
  let path = Path::new(&s);
  fs::write(path, oid)
}

pub fn get_head() -> Option<std::io::Result<String>> {
  let s = format!("{}/HEAD", ROOT);
  let path = Path::new(&s);
  if path.exists() {
    Some(fs::read_to_string(path))
  }
  else {
    None
  }
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
    let test_file = Path::new("test.txt");
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "bac94dbaf28c6916ef33cad50e4e1e88c3834f51dc7a5d40702a5cfdf324ab72";
    let s = format!("{}/{}/{}", ROOT, OBJECTS, test_text_as_hash);
    let path_with_hash = Path::new(&s);

    let _ = fs::remove_dir_all(ROOT);
    init().unwrap();
    fs::write(test_file, test_text).unwrap();

    hash_object(&test_file, ObjectType::Blob).unwrap();
    assert!(path_with_hash.is_file());
    let contents = fs::read_to_string(path_with_hash).unwrap();
    assert_eq!(contents, format!("blob\0{}", test_text));

    fs::remove_dir_all(ROOT).unwrap();
    fs::remove_file(test_file).unwrap();
  }

  #[test]
  #[serial]
  fn get_object_subcommand_returns_contents_of_file_with_specified_oid_hash() {
    let test_file = Path::new("test.txt");
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "bac94dbaf28c6916ef33cad50e4e1e88c3834f51dc7a5d40702a5cfdf324ab72";

    let _ = fs::remove_dir_all(ROOT);
    init().unwrap();
    fs::write(test_file, test_text).unwrap();
    hash_object(&test_file, ObjectType::Blob).unwrap();

    let contents = get_object(test_text_as_hash, ObjectType::Blob).unwrap();
    assert_eq!(&contents, test_text);

    fs::remove_dir_all(ROOT).unwrap();
    fs::remove_file(test_file).unwrap();
  }
}
