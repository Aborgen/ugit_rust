use std::collections::HashMap;
use std::env;
use std::io::{Error, ErrorKind};
use std::fs;
use std::path::Path;

use sha2::digest::generic_array::{GenericArray, typenum::U32};

use crate::data;
use data::ObjectType;

pub fn write_tree() -> std::io::Result<GenericArray<u8, U32>> {
  let path = env::current_dir()?;
  write_tree_recursive(&path)
}

fn write_tree_recursive(path: &Path) -> std::io::Result<GenericArray<u8, U32>> {
  if !path.is_dir() {
    return Err(Error::new(ErrorKind::InvalidInput, format!("Given path [{}] does not point to a directory", path.display())));
  }

  let mut entries: HashMap<ObjectType, Vec<(GenericArray<u8, U32>, String)>> = HashMap::new();
  entries.insert(ObjectType::Blob, Vec::new());
  entries.insert(ObjectType::Tree, Vec::new());

  for entry in fs::read_dir(path)? {
    let entry = entry?;
    let path = entry.path();
    let object_type: ObjectType;
    let oid: GenericArray<u8, U32>;
    if is_ignored(&path) {
      continue;
    }
    else if path.is_file() {
      let contents = fs::read(&path)?;
      object_type = ObjectType::Blob;
      oid = data::hash_object(&contents, object_type)?;
    }
    else if path.is_dir() {
      object_type = ObjectType::Tree;
      oid = write_tree_recursive(&path)?;
    }
    else {
      return Err(Error::new(ErrorKind::InvalidInput, format!("write_tree expects only files and directories [{}]", path.display())));
    }

    let filename = String::from(path.file_name().unwrap().to_str().unwrap());
    entries.entry(object_type).and_modify(|entry| entry.push((oid, filename)));
  }

  let contents = format!("{}\n{}",
    entries.get(&ObjectType::Blob)
      .unwrap()
      .iter()
      .map(|entry| format!("blob {:x} {}", entry.0, entry.1))
      .collect::<Vec<_>>()
      .join("\n"),
    entries.get(&ObjectType::Tree)
      .unwrap()
      .iter()
      .map(|entry| format!("tree {:x} {}", entry.0, entry.1))
      .collect::<Vec<_>>()
      .join("\n"),
  );

  let oid = data::hash_object(contents.as_bytes(), ObjectType::Tree)?;
  Ok(oid)
}

fn is_ignored(path: &Path) -> bool {
  path.ends_with(".ugit") || path.ends_with("target")

}
