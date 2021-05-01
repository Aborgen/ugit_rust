use std::collections::HashMap;
use std::env;
use std::io::{Error, ErrorKind};
use std::fs;
use std::path::{Path, PathBuf};

use crate::data;
use data::ObjectType;

pub fn write_tree() -> std::io::Result<String> {
  let path = env::current_dir()?;
  write_tree_recursive(&path)
}

pub fn read_tree(root_oid: &str) -> std::io::Result<()> {
  let dir = env::current_dir().unwrap();
  let tree = get_tree(root_oid, dir)?;
  for tuple in tree {
    let (path, oid) = tuple;
    fs::create_dir_all(&path.parent().unwrap())?;
    let contents = data::get_object(&oid, ObjectType::Blob)?;
    fs::write(&path, contents)?;
  }

  Ok(())
}

fn write_tree_recursive(path: &Path) -> std::io::Result<String> {
  if !path.is_dir() {
    return Err(Error::new(ErrorKind::InvalidInput, format!("Given path [{}] does not point to a directory", path.display())));
  }

  let mut entries: HashMap<ObjectType, Vec<(String, String)>> = HashMap::new();
  entries.insert(ObjectType::Blob, Vec::new());
  entries.insert(ObjectType::Tree, Vec::new());

  for entry in fs::read_dir(path)? {
    let entry = entry?;
    let path = entry.path();
    let object_type: ObjectType;
    let oid;
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
      .map(|entry| format!("blob {} {}", entry.0, entry.1))
      .collect::<Vec<_>>()
      .join("\n"),
    entries.get(&ObjectType::Tree)
      .unwrap()
      .iter()
      .map(|entry| format!("tree {} {}", entry.0, entry.1))
      .collect::<Vec<_>>()
      .join("\n"),
  );

  let oid = data::hash_object(contents.as_bytes(), ObjectType::Tree)?;
  Ok(oid)
}

fn get_tree(oid: &str, base_path: PathBuf) -> std::io::Result<Vec<(PathBuf, String)>> {
  let mut result = Vec::new();
  let object = data::get_object(oid, ObjectType::Tree)?;
  for line in object.lines() {
    let object_parts: Vec<String> = line.splitn(3, " ").map(|obj| String::from(obj)).collect();
    let object_type = object_parts[0].clone();
    let oid = object_parts[1].clone();

    let mut path = base_path.clone();
    path.push(&object_parts[2]);
    if object_type == "blob" {
      result.push((path.clone(), oid));
    }
    else if object_type == "tree" {
      get_tree(&oid, path)?;
    }
    else {
      return Err(Error::new(ErrorKind::InvalidInput, format!("Unimplemented object type [{}]", object_type)));
    }
  }

  Ok(result)
}

fn is_ignored(path: &Path) -> bool {
  path.ends_with(".ugit") || path.ends_with("target")

}
