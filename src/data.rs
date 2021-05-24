use std::env;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::utils;

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

pub fn update_ref(ref_value: &RefValue, deref: bool) -> std::io::Result<()> {
  // Using get_ref here to drill down to the commit, in the case that ref_value.path contains a symbolic ref.
  let path = match get_ref(&ref_value.path, deref) {
    Ok(ref_value) => ref_value.path,
    Err(err) => return Err(
      Error::new(err.kind(), format!("While trying to update ref ['{}'|{:?}], an error occured: {}", ref_value.path.display(), ref_value.value, err)))
  };

  if let Some(ref value) = ref_value.value {
    let value = if ref_value.symbolic {
      format!("ref:{}", value)
    }
    else {
      String::from(value)
    };

    update_ref_file(&path, &value)
  }
  else {
    panic!("Tried to update ref with an empty ref: {:?}", ref_value);
  }
}

pub fn get_ref(path: &Path, deref: bool) -> std::io::Result<RefValue> {
  match get_ref_file(&path, deref) {
    Some(maybe_ref_value) => maybe_ref_value,
    None => Ok(RefValue { symbolic: false, value: None, path: path.clone().to_path_buf() })
  }
}

pub fn set_head(oid: &str) -> std::io::Result<()> {
  let path = match generate_path(PathVariant::Head) {
    Ok(path) => path,
    Err(err) => return Err(Error::new(err.kind(), format!("Error when setting contents of HEAD -- {}", err)))
  };

  update_ref_file(&path, oid)
}

pub fn get_head() -> Option<std::io::Result<String>> {
  let path = match generate_path(PathVariant::Head) {
    Ok(path) => path,
    Err(err) => return Some(Err(Error::new(err.kind(), format!("Error when getting contents of HEAD -- {}", err))))
  };

  match get_ref_file(&path, false) {
    None => None,
    Some(maybe_ref_value) => {
      match maybe_ref_value {
        Ok(ref_value) => match ref_value.value {
          Some(value) => Some(Ok(value)),
          None => None
        },
        Err(err) => Some(Err(Error::new(err.kind(), format!("Error while getting contents of HEAD -- {}", err))))
      }
    }
  }
}

fn get_ref_file(path: &Path, deref: bool) -> Option<std::io::Result<RefValue>> {
  if !repository_initialized() {
    return Some(Err(Error::new(ErrorKind::NotFound, "A ugit repository does not exist")));
  }

  if !path.is_file() {
    return None;
  }

  let value = match recur_deref(path, deref) {
    Ok(value) => value,
    Err(err) => return Some(Err(err))
  };

  let symbolic = value.starts_with("ref:");
  let ref_value = RefValue { symbolic, value: Some(value), path: path.clone().to_path_buf() };
  Some(Ok(ref_value))
}

fn recur_deref(path: &Path, deref: bool) -> std::io::Result<String> {
  match fs::read_to_string(&path) {
    Err(err) => return Err(Error::new(err.kind(), format!("Error when reading from {} (recur_deref) -- {}", path.display(), err))),
    Ok(contents) => {
      if contents.starts_with("ref:") {
        let content_parts: Vec<&str> = contents.splitn(2, ":").collect();
        if deref {
          let path = PathBuf::from(content_parts[1]);
          recur_deref(&path, deref)
        }
        else {
          Ok(String::from(content_parts[1]))
        }
      }
      else {
        Ok(contents)
      }
    }
  }
}

fn update_ref_file(path: &Path, oid: &str) -> std::io::Result<()> {
  if !repository_initialized() {
    return Err(Error::new(ErrorKind::NotFound, "A ugit repository does not exist"));
  }
  else if !validate_user_given_ref(oid) {
    panic!("Tried to create a ref for something that is not a commit or another ref at {}", path.display());
  }

  fs::write(&path, oid)?;
  Ok(())
}

// Refs may only point to commits or to other refs. This function is meant to check inside a given OID to see if it contains either of those.
fn validate_user_given_ref(oid: &str) -> bool {
  let oid = if oid.starts_with("ref:") {
    oid.splitn(2, ":").collect::<Vec<&str>>()[1]
  } else {
    oid
  };

  let path = generate_path(PathVariant::OID(oid)).unwrap();
  let contents = match fs::read(&path) {
    Ok(contents) => contents,
    Err(_) => return false
  };

  match String::from_utf8(contents.clone()) {
    Ok(s) => {
      // In this case, the given OID is a ref which is pointing to another OID.
      if utils::is_hex(&s) {
        return true;
      }
    },
    Err(_) => ()
  };

  let content_parts: Vec<_> = contents
    .splitn(2, |b| *b == b'\0')
    .collect();

  if content_parts[0] == b"commit" {
    true
  }
  else {
    false
  }
}

pub fn locate_ref_or_oid(s: &str) -> Option<std::io::Result<String>> {
  if !repository_initialized() {
    return Some(Err(Error::new(ErrorKind::NotFound, "A ugit repository does not exist")));
  }

  let get_ref_from_variant = |path_variant: PathVariant| get_ref_file(&generate_path(path_variant).unwrap(), false); 

  let mut count_of_refs_located = 0;
  let mut ret_ref_value = None;
  if let Some(maybe_ref_value) = get_ref_from_variant(PathVariant::Ref(RefVariant::Tag(s))) {
    if let Ok(ref_value) = maybe_ref_value {
      count_of_refs_located += 1;
      ret_ref_value = Some(ref_value);
    }
  }
  if let Some(maybe_ref_value) = get_ref_from_variant(PathVariant::Ref(RefVariant::Head(s))) {
    if let Ok(ref_value) = maybe_ref_value {
      count_of_refs_located += 1;
      ret_ref_value = Some(ref_value);
    }
  }
  if let Some(maybe_ref_value) = get_ref_from_variant(PathVariant::OID(s)) {
    if let Ok(ref_value) = maybe_ref_value {
      count_of_refs_located += 1;
      ret_ref_value = Some(ref_value);
    }
  }
  if s == "HEAD" || s == "@" {
    if let Some(maybe_ref_value) = get_ref_from_variant(PathVariant::Head) {
      if let Ok(ref_value) = maybe_ref_value {
        count_of_refs_located += 1;
        ret_ref_value = Some(ref_value);
      }
    }
  }

  match ret_ref_value {
    None => None,
    Some(ref_value) => if count_of_refs_located > 1 {
      Some(Err(Error::new(ErrorKind::InvalidInput, format!("Ref '{}' is ambiguous", s))))
    }
    else {
      let oid = ref_value.value.unwrap();
      Some(Ok(oid))
    }
  }
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
  #[cfg(test)]
  Ugit,
}

#[derive(Clone, Copy, Debug)]
pub enum RefVariant<'a> {
  Head(&'a str),
  Tag(&'a str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefValue {
  pub symbolic: bool,
  pub value: Option<String>,
  pub path: PathBuf,
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
    #[cfg(test)]
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
#![allow(non_snake_case)]
  use std::panic;
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
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "bac94dbaf28c6916ef33cad50e4e1e88c3834f51dc7a5d40702a5cfdf324ab72";
    create_test_directory();
    {
      let path_with_hash = generate_path(PathVariant::OID(test_text_as_hash)).unwrap();
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
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let test_text_as_hash = "bac94dbaf28c6916ef33cad50e4e1e88c3834f51dc7a5d40702a5cfdf324ab72";
    create_test_directory();
    {
      hash_object(test_text.as_bytes(), ObjectType::Blob).unwrap();

      let contents = get_object(test_text_as_hash, ObjectType::Blob).unwrap();
      assert_eq!(contents, test_text);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn update_ref_creates_a_ref_to_a_commit() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
      let path = generate_path(PathVariant::Ref(RefVariant::Tag("Test tag"))).unwrap();
      let ref_value = RefValue { symbolic: false, value: Some(commit_oid.clone()), path: path.clone() };
      update_ref(&ref_value, true).expect("Issue when updating ref");

      let contents = fs::read_to_string(path).unwrap();
      assert_eq!(contents, commit_oid);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn update_ref_creates_a_symbolic_ref_to_a_commit() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
      let path = generate_path(PathVariant::Ref(RefVariant::Head("Test branch"))).unwrap();
      let ref_value = RefValue { symbolic: true, value: Some(commit_oid.clone()), path: path.clone() };
      update_ref(&ref_value, true).expect("Issue when updating ref");

      let contents = fs::read_to_string(path).unwrap();
      let content_parts: Vec<_> = contents.splitn(2, ":").collect();
      assert_eq!(content_parts[0], "ref");
      assert_eq!(content_parts[1], commit_oid);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn update_ref_creates_a_ref_to_another_ref() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let ref_name = "Test branch";
      let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
      // Create first ref
      let first_ref_oid = {
        let path = generate_path(PathVariant::Ref(RefVariant::Head(ref_name))).unwrap();
        let ref_value = RefValue { symbolic: false, value: Some(commit_oid.clone()), path: path.clone() };
        update_ref(&ref_value, true).expect("Issue when updating ref");
        fs::read_to_string(path).unwrap()
      };

      let path = generate_path(PathVariant::Ref(RefVariant::Tag("Tag to ref"))).unwrap();
      // Currently, cannot pass ref directly to update_ref: when using ugit, the CLI converts from ref down to the bare commit.
      let ref_value = RefValue { symbolic: false, value: Some(first_ref_oid), path: path.clone() };
      update_ref(&ref_value, true).expect("Issue when updating ref");

      let contents = fs::read_to_string(path).unwrap();
      assert_eq!(contents, commit_oid);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  #[should_panic(expected="empty ref")]
  fn update_ref_panics_if_tried_to_create_ref_to_nothing() {
    let result;
    create_test_directory();
    {
      let ref_value = RefValue { symbolic: false, value: None, path: PathBuf::from("New Ref") };
      result = panic::catch_unwind(|| update_ref(&ref_value, true).unwrap());
    }
    delete_test_directory();

    if let Err(err) = result {
      panic::resume_unwind(err);
    }
  }

  #[test]
  #[serial]
  #[should_panic(expected="not a commit or another ref")]
  fn update_ref_panics_if_tried_to_create_ref_of_not_a_commit_or_another_ref() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let result;
    create_test_directory();
    {
      let oid = hash_object(&test_text.as_bytes(), ObjectType::Blob).unwrap();
      let ref_value = RefValue { symbolic: false, value: Some(oid), path: PathBuf::from("New Ref") };
      result = panic::catch_unwind(|| update_ref(&ref_value, true).unwrap());
    }
    delete_test_directory();

    if let Err(err) = result {
      panic::resume_unwind(err);
    }
  }

  #[test]
  #[serial]
  fn update_ref_returns_an_error_if_repository_is_not_initialized() {
    let ref_value = RefValue { symbolic: false, value: None, path: PathBuf::from("") };
    assert!(update_ref(&ref_value, true).is_err());
  }

  #[test]
  #[serial]
  fn get_ref_returns_an_empty_ref_value_when_ref_does_not_exist() {
    create_test_directory();
    {
      let path = Path::new("Doesn't exist");
      let expected = RefValue { symbolic: false, value: None, path: path.clone().to_path_buf() };
      let result = get_ref(&path, true).expect("Issue when getting ref");
      assert_eq!(result, expected);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn get_ref_returns_a_ref_value_when_ref_exists() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
      let path = generate_path(PathVariant::Ref(RefVariant::Head("Test branch"))).unwrap();
      let ref_value = RefValue { symbolic: false, value: Some(commit_oid.clone()), path: path.clone() };
      update_ref(&ref_value, true).expect("Issue when updating ref");

      let expected = RefValue { symbolic: false, value: Some(commit_oid), path: path.clone().to_path_buf() };
      let result = get_ref(&path, true).expect("Issue when getting ref");
      assert_eq!(result, expected);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn get_ref_returns_a_ref_value_with_a_none_value_if_given_path_is_not_a_file() {
    create_test_directory();
    {
      let result = get_ref(&Path::new("nothing"), true).expect("Issue when getting ref");
      assert!(result.value.is_none());

      fs::create_dir("GoodData").unwrap();
      let result = get_ref(&Path::new("GoodData"), true).expect("Issue when getting ref");
      assert!(result.value.is_none());
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn get_ref_returns_an_error_if_repository_is_not_initialized() {
    let path = Path::new("");
    assert!(get_ref(&path, true).is_err());
  }

  #[test]
  #[serial]
  fn set_head_updates_the_contents_of_HEAD_given_valid_oid() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
      set_head(&commit_oid).expect("Issue when updating ref");

      let path = generate_path(PathVariant::Head).unwrap();
      let contents = fs::read_to_string(path).unwrap();
      assert_eq!(contents, commit_oid);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn set_head_returns_an_error_if_repository_is_not_initialized() {
    assert!(set_head("").is_err());
  }

  #[test]
  #[serial]
  fn set_head_creates_a_ref_to_another_ref() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
      // Create first ref
      let ref_oid = {
        let path = generate_path(PathVariant::Ref(RefVariant::Head("Test Branch"))).unwrap();
        let ref_value = RefValue { symbolic: false, value: Some(commit_oid.clone()), path: path.clone() };
        update_ref(&ref_value, true).expect("Issue when updating ref");
        fs::read_to_string(path).unwrap()
      };

      // Currently, cannot pass ref directly to set_head: when using ugit, the CLI converts from ref down to the bare commit.
      set_head(&ref_oid).expect("Issue when updating ref");

      let path = generate_path(PathVariant::Head).unwrap();
      let contents = fs::read_to_string(path).unwrap();
      assert_eq!(contents, commit_oid);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  #[should_panic(expected="commit or another ref")]
  fn set_head_panics_if_set_to_not_a_commit_or_another_ref() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let result;
    create_test_directory();
    {
      let oid = hash_object(&test_text.as_bytes(), ObjectType::Blob).unwrap();
      result = panic::catch_unwind(|| set_head(&oid).unwrap());
    }
    delete_test_directory();

    if let Err(err) = result {
      panic::resume_unwind(err);
    }
  }

  #[test]
  #[serial]
  fn get_head_returns_none_when_head_does_not_exist() {
    create_test_directory();
    {
      let path = Path::new(".ugit/HEAD");
      if path.is_file() {
        fs::remove_file(&path).unwrap();
      }

      let result = get_head();
      assert!(result.is_none());
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn get_head_returns_contents_of_HEAD_when_HEAD_exists() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let path = Path::new(".ugit/HEAD");
      fs::write(&path, test_text).unwrap();

      let result = get_head().unwrap().unwrap();
      assert_eq!(result, test_text);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn get_head_returns_an_error_if_repository_is_not_initialized() {
    assert!(get_head().unwrap().is_err());
  }

  #[test]
  #[serial]
  fn validate_user_given_ref_returns_false_if_given_oid_does_not_exist() {
    create_test_directory();
    {
      let result = validate_user_given_ref("Nothin'");
      assert_eq!(result, false);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn validate_user_given_ref_returns_false_if_given_oid_does_not_point_to_an_oid_or_a_commit() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let oid = hash_object(&test_text.as_bytes(), ObjectType::Blob).unwrap();
      let result = validate_user_given_ref(&oid);
      assert_eq!(result, false);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn validate_user_given_ref_returns_true_given_an_oid_that_points_to_a_commit() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let oid = hash_object(&test_text.as_bytes(), ObjectType::Commit).unwrap();
      let result = validate_user_given_ref(&oid);
      assert_eq!(result, true);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn locate_ref_or_oid_returns_commit_oid_that_tag_points_to_given_only_name() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let tag_name = "Test Tag";
    create_test_directory();
    {
      let ref_oid = {
        let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
        let path = generate_path(PathVariant::Ref(RefVariant::Tag(tag_name))).unwrap();
        let ref_value = RefValue { symbolic: false, value: Some(commit_oid.clone()), path: path.clone() };
        update_ref(&ref_value, true).expect("Issue when updating ref");
        fs::read_to_string(path).unwrap()
      };

      let result = locate_ref_or_oid(tag_name).unwrap().unwrap();
      assert_eq!(result, ref_oid);
    }
    delete_test_directory();
  }


  #[test]
  #[serial]
  fn locate_ref_or_oid_returns_commit_oid_that_branch_points_to_given_only_name() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let head_name = "Test Head";
    create_test_directory();
    {
      let ref_oid = {
        let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
        let path = generate_path(PathVariant::Ref(RefVariant::Head(head_name))).unwrap();
        let ref_value = RefValue { symbolic: false, value: Some(commit_oid.clone()), path: path.clone() };
        update_ref(&ref_value, true).expect("Issue when updating ref");
        fs::read_to_string(path).unwrap()
      };

      let result = locate_ref_or_oid(head_name).unwrap().unwrap();
      assert_eq!(result, ref_oid);
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn locate_ref_or_oid_returns_contents_of_given_oid() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
      let result = locate_ref_or_oid(&commit_oid).unwrap().unwrap();
      assert!(result.contains(test_text));
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn locate_ref_or_oid_returns_contents_of_HEAD() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    create_test_directory();
    {
      let path = Path::new(".ugit/HEAD");
      fs::write(&path, test_text).unwrap();
      let result1 = locate_ref_or_oid("@").unwrap().unwrap();
      let result2 = locate_ref_or_oid("HEAD").unwrap().unwrap();
      assert!(result1.contains(test_text));
      assert!(result2.contains(test_text));
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn locate_ref_or_oid_returns_an_error_if_a_tag_and_a_branch_have_the_same_name() {
    let test_text = "Excepturi velit rem modi. Ut non ipsa aut ad dignissimos et molestias placeat. Iste est perspiciatis ab et commodi.";
    let ref_name = "Interesting";
    create_test_directory();
    {
      {
        let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
        let path = generate_path(PathVariant::Ref(RefVariant::Tag(ref_name))).unwrap();
        let ref_value = RefValue { symbolic: false, value: Some(commit_oid.clone()), path: path.clone() };
        update_ref(&ref_value, true).expect("Issue when updating ref");
        fs::read_to_string(path).unwrap()
      };

      {
        let commit_oid = hash_object(test_text.as_bytes(), ObjectType::Commit).expect("Issue when hashing a commit");
        let path = generate_path(PathVariant::Ref(RefVariant::Head(ref_name))).unwrap();
        let ref_value = RefValue { symbolic: false, value: Some(commit_oid.clone()), path: path.clone() };
        update_ref(&ref_value, true).expect("Issue when updating ref");
        fs::read_to_string(path).unwrap()
      };

      let result = locate_ref_or_oid(ref_name).unwrap();
      assert!(result.is_err());
    }
    delete_test_directory();
  }

  #[test]
  #[serial]
  fn locate_ref_or_oid_returns_an_error_if_repository_is_not_initialized() {
    let result = locate_ref_or_oid("").unwrap();
    assert!(result.is_err());
  }

  #[test]
  #[serial]
  fn locate_ref_or_oid_returns_none_if_a_ref_or_id_is_not_found() {
    create_test_directory();
    {
      let result = locate_ref_or_oid("Good Ref Name");
      assert!(result.is_none());
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
