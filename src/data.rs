use std::env;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;

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
  // Create .ugit/objects
  fs::create_dir(generate_path(PathVariant::Objects)?)?;
  // Create .ugit/refs
  fs::create_dir(generate_path(PathVariant::Refs)?)?;
  // Create directories within .ugit/refs
  fs::create_dir(generate_path(PathVariant::Heads)?)?;
  fs::create_dir(generate_path(PathVariant::Tags)?)?;

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

pub fn update_ref(ref_value: &RefValue) -> std::io::Result<()> {
  if ref_value.symbolic {
    panic!("This method may not be called with a symbolic ref [{:?}]", ref_value.value);
  }

  let maybe_path = generate_path(PathVariant::Ref(ref_value.rtype));
  if let Some(ref value) = ref_value.value {
    update_internal_file(&maybe_path, &value)
  }
  else {
    panic!("Tried to update ref with an empty ref: {:?}", ref_value);
  }
}

pub fn get_ref(ref_variant: RefVariant) -> std::io::Result<RefValue> {
  let maybe_path = generate_path(PathVariant::Ref(ref_variant));
  let value = match get_from_internal_file(&maybe_path) {
    Some(value) => Some(value?),
    None => None
  };

  Ok(RefValue { symbolic: false, value, rtype: ref_variant })
}

pub fn set_head(oid: &str) -> std::io::Result<()> {
  let maybe_path = generate_path(PathVariant::Head);
  update_internal_file(&maybe_path, oid)
}

pub fn get_head() -> Option<std::io::Result<String>> {
  let maybe_path = generate_path(PathVariant::Head);
  get_from_internal_file(&maybe_path)
}

fn get_from_internal_file(maybe_path: &std::io::Result<PathBuf>) -> Option<std::io::Result<String>> {
  let path = match maybe_path {
    Ok(path) => path,
    Err(err) => return Some(Err(Error::new(ErrorKind::NotFound, format!("Error when getting contents of internal file -- {}", err))))
  };

  if !path.is_file() {
    return None;
  }

  let contents = fs::read_to_string(&path);
  match contents {
    Err(err) => Some(Err(Error::new(err.kind(), format!("Error when reading from {} -- {}", path.display(), err)))),
    Ok(contents) => {
      if contents.starts_with("ref:") {
        let content_parts: Vec<&str> = contents.splitn(2, ":").collect();
        let path = PathBuf::from(content_parts[1]);
        get_from_internal_file(&Ok(path))
      }
      else {
        Some(Ok(contents))
      }
    }
  }
}

fn update_internal_file(maybe_path: &std::io::Result<PathBuf>, oid: &str) -> std::io::Result<()> {
  let path = match maybe_path {
    Ok(path) => path,
    Err(err) => return Err(Error::new(ErrorKind::NotFound, format!("Error when getting contents of internal file -- {}", err)))
  };

  fs::write(&path, oid)?;
  Ok(())
}


pub enum PathVariant<'a> {
  Head,
  Heads,
  Objects,
  OID(&'a str),
  Ref(RefVariant<'a>),
  Refs,
  Root,
  Tags,
  Ugit,
}

#[derive(Clone, Copy, Debug)]
pub enum RefVariant<'a> {
  Head(&'a str),
  Tag(&'a str),
}

#[derive(Clone, Debug)]
pub struct RefValue<'a> {
  pub symbolic: bool,
  pub value: Option<String>,
  pub rtype: RefVariant<'a>,
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
    PathVariant::Heads => {
      path.push("refs");
      path.push("heads");
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
    PathVariant::Ref(ref_variant) => {
      match ref_variant {
        RefVariant::Head(name) => {
          path.push("refs");
          path.push("heads");
          path.push(name);
        },
        RefVariant::Tag(name) => {
          path.push("refs");
          path.push("tags");
          path.push(name);
        },
      };

      path
    },
    PathVariant::Refs => {
      path.push("refs");
      path
    },
    PathVariant::Root => path.parent().unwrap().to_path_buf(),
    PathVariant::Tags => {
      path.push("refs");
      path.push("tags");
      path
    },
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
  use std::path::Path;
  use serial_test::serial;
  use super::*;

  #[test]
  #[serial]
  fn init_subcommand_creates_expected_directory_tree() {
    create_test_directory();
    {
      assert!(generate_path(PathVariant::Ugit).unwrap().exists());
      assert!(generate_path(PathVariant::Objects).unwrap().exists());
      assert!(generate_path(PathVariant::Refs).unwrap().exists());
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
