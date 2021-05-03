use std::env;
use std::io::{Error, ErrorKind};
use std::fs;
use std::path::{Path, PathBuf};

use crate::data;
use data::{Commit, ObjectType};

pub fn write_tree() -> std::io::Result<String> {
  let path = env::current_dir()?;
  write_tree_recursive(&path)
}

pub fn read_tree(root_oid: &str) -> std::io::Result<()> {
  let dir = env::current_dir().unwrap();
  empty_current_directory(&dir)?;
  let tree = get_tree(root_oid, &dir)?;
  for tuple in tree {
    let (path, oid) = tuple;
    fs::create_dir_all(&path.parent().unwrap())?;
    let contents = data::get_object(&oid, ObjectType::Blob)?;
    fs::write(&path, contents)?;
  }

  Ok(())
}

pub fn commit(message: &str) -> std::io::Result<String> {
  let oid = write_tree()?;
  let commit = match data::get_head() {
    Some(head) => {
      let head = head?;
      format!("tree {}\nparent {}\n\n{}", oid, head, message)
    },
    None => format!("tree {}\n\n{}", oid, message)
  };

  let oid = data::hash_object(commit.as_bytes(), ObjectType::Commit)?;
  data::set_head(&oid)?;
  Ok(oid)
}

pub fn get_commit(oid: &str) -> std::io::Result<Commit> {
  let mut tree = "";
  let mut parent = None;
  let commit = data::get_object(oid, ObjectType::Commit)?;

  let mut lines = commit.lines();
  for line in lines.by_ref() {
    if line == "" {
      break;
    }

    let object_parts: Vec<_> = line.splitn(2, " ").collect();
    if object_parts[0] == "tree" {
      tree = object_parts[1];
    }
    else if object_parts[0] == "parent" {
      parent = Some(String::from(object_parts[1]));
    }
    else {
      panic!("Unimplemented branch of get_commit: {}", object_parts[0]);
    }
  }

  let mut message = String::from(lines.by_ref().next().unwrap());
  for line in lines {
    message = format!("{}\n{}", message, line);
  }

  if tree == "" {
    return Err(Error::new(ErrorKind::InvalidData, format!("Missing tree row of commit")));
  }

  Ok(
    Commit {
      tree: String::from(tree),
      parent,
      message,
    }
  )
}

fn write_tree_recursive(path: &Path) -> std::io::Result<String> {
  if !path.is_dir() {
    return Err(Error::new(ErrorKind::InvalidInput, format!("Given path [{}] does not point to a directory", path.display())));
  }

  let mut entries: Vec<(&str, String, String)> = Vec::new();
  for entry in fs::read_dir(path)? {
    let entry = entry?;
    let path = entry.path();
    let object_type;
    let oid;
    if is_ignored(&path) {
      continue;
    }
    else if path.is_file() {
      let contents = fs::read(&path)?;
      object_type = "blob";
      oid = data::hash_object(&contents, ObjectType::Blob)?;
    }
    else if path.is_dir() {
      object_type = "tree";
      oid = write_tree_recursive(&path)?;
    }
    else {
      return Err(Error::new(ErrorKind::InvalidInput, format!("write_tree expects only files and directories [{}]", path.display())));
    }

    let filename = String::from(path.file_name().unwrap().to_str().unwrap());
    entries.push((object_type, oid, filename));
  }

  let contents = entries
      .iter()
      .map(|entry| format!("{} {} {}", entry.0, entry.1, entry.2))
      .collect::<Vec<_>>()
      .join("\n");

  let oid = data::hash_object(contents.as_bytes(), ObjectType::Tree)?;
  Ok(oid)
}

fn get_tree(oid: &str, base_path: &PathBuf) -> std::io::Result<Vec<(PathBuf, String)>> {
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
      let mut recur_results = get_tree(&oid, &path)?;
      result.append(&mut recur_results);
    }
    else {
      return Err(Error::new(ErrorKind::InvalidInput, format!("Unimplemented object type [{}]", object_type)));
    }
  }

  Ok(result)
}

fn empty_current_directory(root: &Path) -> std::io::Result<()> {
  for entry in fs::read_dir(root)? {
    let entry = entry?.path();
    if is_ignored(&entry) {
      continue;
    }
    else if entry.is_file() {
      fs::remove_file(entry)?;
    }
    else if entry.is_dir() {
      fs::remove_dir_all(entry)?;
    }
  }

  Ok(())
}

fn is_ignored(path: &Path) -> bool {
  path.ends_with(".ugit") || path.ends_with("target")
}
