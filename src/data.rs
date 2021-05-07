use std::env;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

static GIT_DIR: &str = ".ugit";

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub enum ObjectType {
  Blob,
  Commit,
  Tree,
}

pub struct Commit {
  pub message: String,
  pub parent: Option<String>,
  pub tree: String,
}

pub fn init() -> std::io::Result<()> {
  if repository_initialized() {
    return Err(Error::new(ErrorKind::AlreadyExists, "A ugit repository already exists"));
  }

  let mut root = env::current_dir().expect("Issue when getting cwd");
  root.push(GIT_DIR);
  fs::create_dir(&root)?;

  fs::create_dir(generate_path(PathVariant::Objects)?)?;
  return Ok(())
}

pub fn hash_object(file_contents: &[u8], object_type: ObjectType) -> std::io::Result<String> {
  if !repository_initialized() {
    return Err(Error::new(ErrorKind::NotFound, "A ugit repository does not exist"));
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
  let object = hasher.finalize();
  let oid = format!("{:x}", object);
  let file_path = generate_path(PathVariant::OID(&oid)).unwrap();
  fs::write(&file_path, &contents)?;
  Ok(oid)
}

// TODO: get_object should return Vec<u8>: if the ObjectType is a blob, it is possible that read_to_string will fail if the
//       blob's contents contains any invalid utf-8 bytes.
pub fn get_object(oid: &str, expected_type: ObjectType) -> std::io::Result<String> {
  if !repository_initialized() {
    return Err(Error::new(ErrorKind::NotFound, "A ugit repository does not exist"));
  }

  let file_path = generate_path(PathVariant::OID(&oid)).unwrap();
  if !&file_path.exists() {
    return Err(Error::new(ErrorKind::NotFound, format!("A file with the given OID does not exist [{}]", &file_path.display()).as_str()));
  }

  let contents = fs::read_to_string(&file_path)?;
  let content_parts: Vec<_> = contents
    .splitn(2, char::from(0))
    .collect();

  if expected_type == ObjectType::Blob && content_parts[0] != "blob" {
    return Err(Error::new(ErrorKind::InvalidData, format!("Object was expected to be a blob, but was a [{}]", content_parts[0])));
  }
  else if expected_type == ObjectType::Commit && content_parts[0] != "commit" {
    return Err(Error::new(ErrorKind::InvalidData, format!("Object was expected to be a commit, but was a [{}]", content_parts[0])));
  }
  else if expected_type == ObjectType::Tree && content_parts[0] != "tree" {
    return Err(Error::new(ErrorKind::InvalidData, format!("Object was expected to be a tree, but was a [{}]", content_parts[0])));
  }

  Ok(String::from(content_parts[1]))
}

pub fn set_head(oid: &str) -> std::io::Result<()> {
  let path = generate_path(PathVariant::Head)?;
  fs::write(&path, oid)?;
  Ok(())
}

pub fn get_head() -> Option<std::io::Result<String>> {
  let path = match generate_path(PathVariant::Head) {
    Ok(path) => path,
    Err(err) => return Some(Err(Error::new(ErrorKind::NotFound, format!("Error when getting HEAD -- {}", err).as_str())))
  };

  if !path.is_file() {
    return None;
  }

  Some(fs::read_to_string(&path))
}

pub enum PathVariant<'a> {
  Head,
  Objects,
  OID(&'a str),
  Root,
  Ugit,
}

pub fn generate_path(variant: PathVariant) -> std::io::Result<PathBuf> {
  let mut path = match get_repository() {
    Some(path) => path,
    None => return Err(Error::new(ErrorKind::NotFound, "A ugit repository does not exist")),
  };

  let path = match variant {
    PathVariant::Head => {
      path.push("HEAD");
      path
    },
    PathVariant::Objects => {
      path.push("objects");
      path
    },
    PathVariant::OID(oid) => {
      path.push("objects");
      path.push(oid);
      path
    },
    PathVariant::Root => path.parent().unwrap().to_path_buf(),
    PathVariant::Ugit => path,
  };

  Ok(path)
}

fn repository_initialized() -> bool {
  match get_repository() {
    Some(_) => true,
    None => false
  }
}

fn get_repository() -> Option<PathBuf> {
  let cwd = env::current_dir().expect("Issue when getting cwd");
  for path in cwd.ancestors() {
    let mut path = path.clone().to_path_buf();
    path.push(&GIT_DIR);
    if path.exists() {
      return Some(path);
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use serial_test::serial;
  use super::*;

  #[test]
  #[serial]
  fn init_subcommand_creates_expected_directory_tree() {
    create_test_directory();
    {
      let dir = generate_path(PathVariant::Ugit).unwrap();
      assert!(dir.exists());
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn hash_object_subcommand_creates_copy_of_file_named_as_hash_of_same_file() {
    let test_file = Path::new("test.txt");
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "bac94dbaf28c6916ef33cad50e4e1e88c3834f51dc7a5d40702a5cfdf324ab72";
    create_test_directory();
    {
      let path_with_hash = generate_path(PathVariant::OID(test_text_as_hash)).unwrap();
      fs::write(test_file, test_text).unwrap();

      hash_object(test_text.as_bytes(), ObjectType::Blob).unwrap();
      assert!(path_with_hash.is_file());
      let contents = fs::read_to_string(path_with_hash).unwrap();
      assert_eq!(contents, format!("blob\0{}", test_text));
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn get_object_subcommand_returns_contents_of_file_with_specified_oid_hash() {
    let test_file = Path::new("test.txt");
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "bac94dbaf28c6916ef33cad50e4e1e88c3834f51dc7a5d40702a5cfdf324ab72";
    create_test_directory();
    {
      fs::write(test_file, test_text).unwrap();
      hash_object(test_text.as_bytes(), ObjectType::Blob).unwrap();

      let contents = get_object(test_text_as_hash, ObjectType::Blob).unwrap();
      assert_eq!(&contents, test_text);
    }
    delete_test_directory();
  }

  fn create_test_directory() {
    fs::create_dir("TEST").expect("Issue when creating test directory");
    env::set_current_dir("TEST").expect("Issue when cding into test directory");
    init().expect("Issue when initing test .ugit repository");
  }

  fn delete_test_directory() {
    env::set_current_dir("..").expect("Issue when cding one up from test directory");
    let path = Path::new("TEST");
    if !path.is_dir() {
      let cwd = env::current_dir().expect("Issue when geting cwd");
      panic!("Cannot see test directory in cwd: {}", cwd.display());
    }

    fs::remove_dir_all(&path).expect("Issue when deleting test directory");
  }
}
