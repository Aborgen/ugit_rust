use std::env;
use std::io::{Error, ErrorKind};
use std::fs;
use std::path::Path;

use crate::data;
use data::ObjectType;

pub fn write_tree() -> std::io::Result<()> {
  let path = env::current_dir()?;
  write_tree_recursive(&path)
}

fn write_tree_recursive(path: &Path) -> std::io::Result<()> {
  if !path.is_dir() {
    return Err(Error::new(ErrorKind::InvalidInput, format!("Given path [{}] does not point to a directory", path.display())));
  }

  for entry in fs::read_dir(path)? {
    let entry = entry?;
    let path = entry.path();
    if is_ignored(&path) {
      continue;
    }
    else if path.is_file() {
      let oid = data::hash_object(&path, ObjectType::Blob)?;
      println!("{:x} [{}]", oid, path.display());
    }
    else if path.is_dir() {
      write_tree_recursive(&path)?;
    }
  }

  Ok(())
}

fn is_ignored(path: &Path) -> bool {
  path.ends_with(".ugit") || path.ends_with("target")

}
