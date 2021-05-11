use std::env;
use std::io::{Error, ErrorKind};
use std::fs;
use std::path::{Path, PathBuf};

use crate::data;
use data::{Commit, ObjectType, PathVariant, RefVariant};

pub fn write_tree() -> std::io::Result<String> {
  let path = data::generate_path(PathVariant::Root)?;
  write_tree_recursive(&path)
}

pub fn read_tree(root_oid: &str) -> std::io::Result<()> {
  let dir = env::current_dir().unwrap();
  empty_current_directory()?;
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
  let commit = match data::get_ref(RefVariant::Head) {
    Some(head) => {
      let head = head?;
      format!("tree {}\nparent {}\n\n{}", oid, head, message)
    },
    None => format!("tree {}\n\n{}", oid, message)
  };

  let oid = data::hash_object(commit.as_bytes(), ObjectType::Commit)?;
  data::update_ref(RefVariant::Head, &oid)?;
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

pub fn checkout(oid: &str) -> std::io::Result<()> {
  let commit = get_commit(oid)?;
  read_tree(&commit.tree)?;
  data::update_ref(RefVariant::Head, oid)
}

pub fn create_tag(name: &str, oid: &str) -> std::io::Result<()> {
  data::update_ref(RefVariant::Tag(name), oid)
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

// Dangerous function.
fn empty_current_directory() -> std::io::Result<()> {
  let mut root = env::current_dir().unwrap();
  root.push(".ugit");
  if !root.is_dir() {
    root.pop();
    panic!("Tried to empty a directory without a ugit repository: {}", root.display());
  }

  root.pop();
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

#[cfg(test)]
mod tests {
  use serial_test::serial;
  use super::*;

  #[derive(Clone, Debug)]
  struct DirNode {
    pub name: String,
    pub children: Option<DirChildren>,
  }

  #[derive(Clone, Debug)]
  struct DirChildren(Vec<DirNode>);
  impl DirChildren {
    pub fn new(children: &[DirNode]) -> Self {
      Self { 0: children.to_vec() }
    }
  }

  impl IntoIterator for DirChildren {
    type Item = DirNode;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
      self.0.into_iter()
    }
  }

  impl DirNode {
    pub fn foreach<F1, F2>(&self, dir_func: F1, else_func: F2)
    where
      F1: Fn(&DirNode) -> bool + Copy,
      F2: Fn(&DirNode) -> bool + Copy,
    {
      self.foreach_recursive(&self, dir_func, else_func);
    }

    fn foreach_recursive<F1, F2>(&self, root: &DirNode, dir_func: F1, else_func: F2) -> bool
    where
      F1: Fn(&DirNode) -> bool + Copy,
      F2: Fn(&DirNode) -> bool + Copy,
    {

      if !dir_func(&root) {
        return false;
      }

      env::set_current_dir(&root.name).expect(format!("Cannot cd to {}", &root.name).as_str());
      if let Some(children) = root.children.clone() {
        for child in children.into_iter() {
          let result = if child.children.is_some() {
            self.foreach_recursive(&child, dir_func, else_func)
          }
          else {
            else_func(&child)
          };

          if !result {
            panic!("Issue within foreach at [{:?}]", child);
          }
        }
      }

      env::set_current_dir("..").expect(format!("Cannot cd to dir above {}", &root.name).as_str());
      true
    }
  }

  impl Default for DirNode {
    fn default() -> Self {
      DirNode {
        name: String::from("TEST"),
        children: Some(DirChildren::new(&[
          DirNode {
            name: String::from("index.html"),
            children: None,
          },
          DirNode {
            name: String::from("style.css"),
            children: None,
          },
          DirNode {
            name: String::from("One"),
            children: Some(DirChildren::new(&[
              DirNode {
                name: String::from("Two"),
                children: Some(DirChildren::new(&[
                  DirNode {
                    name: String::from(".SuperSecretFile"),
                    children: None,
                  },
                ])),
              },
            ])),
          },
        ])),
      }
    }
  }

  #[test]
  #[serial]
  fn empty_current_directory_clears_everything_in_current_directory() {
    let (_, cleanup) = create_test_directory();
    assert!(fs::read_dir(".").unwrap().count() > 1);

    empty_current_directory().expect("Some issue having to do with emptying the current directory");
    // The iterator from read_dir will always include at least '.ugit'
    assert_eq!(fs::read_dir(".").unwrap().count(), 1);
    cleanup();
  }

  #[test]
  #[serial]
  fn write_tree_returns_an_oid_of_the_entire_directory() {
    let (dir_tree, cleanup) = create_test_directory();
    let expected = "2104e4d38c58b6477d2f901aa07190d55e63fd1f93cf0f309014e272912040b6";
    let oid = write_tree().expect("Issue when writing tree");
    assert_eq!(expected, oid);

    let dir_func = |node: &DirNode| {
      let path = Path::new(&node.name);
      let oid = write_tree_recursive(&path).expect("Issue when writing tree recursively");
      let oid_file = data::generate_path(PathVariant::OID(&oid)).expect(format!("Issue when generating a path for OID {}", &oid).as_str());
      let contents = fs::read_to_string(&oid_file).expect(format!("Issue with reading OID [{}]", oid).as_str());
      // The file generated from write_tree_recursive represents the directory, and contains the oids, filenames, and directory names within it
      if let Some(children) = node.children.clone() {
        for child in children.into_iter() {
          assert!(contents.contains(&child.name));
        }
      }

      true
    };

    // Assure that each file in dir_tree has been hashed and copied to the ugit repository correctly
    let file_func = |node: &DirNode| {
      let original_contents = fs::read(&node.name)
        .expect(format!("Issue when reading test file {}", node.name).as_str());

      let oid = data::hash_object(&original_contents, ObjectType::Blob).expect("Issue when hashing object");
      let oid_file = data::generate_path(PathVariant::OID(&oid)).expect(format!("Issue when generating a path for OID {}", &oid).as_str());
      let contents = fs::read(&oid_file)
        .expect("Issue when reading from OID");

      let content_parts: Vec<_> = contents.splitn(2, |b| *b == 0).collect();
      assert_eq!(content_parts[1], original_contents);
      true
    };

    let next_tree = DirNode {
      name: String::from("."),
      children: dir_tree.children.clone(),
    };

    next_tree.foreach(dir_func, file_func);
    env::set_current_dir(&dir_tree.name).expect("Issue when cding to test directory");
    cleanup();
  }

  #[test]
  #[serial]
  fn read_tree_replaces_repository_root_with_snapshot_taken_from_write_tree() {
    let (dir_tree, cleanup) = create_test_directory();
    let oid = write_tree().expect("Issue when writing tree");
    empty_current_directory().expect("Issue when emptying root directory");
    assert_eq!(fs::read_dir(".").unwrap().count(), 1);

    read_tree(&oid).expect("Issue when restoring from write_tree snapshot");
    let dir_func = |node: &DirNode| {
      let path = Path::new(&node.name);
      println!("is {} in {}", path.display(), env::current_dir().unwrap().display());
      assert!(path.is_dir());
      true
    };

    let file_func = |node: &DirNode| {
      let path = Path::new(&node.name);
      assert!(path.is_file());
      true
    };

    let next_tree = DirNode {
      name: String::from("."),
      children: dir_tree.children.clone(),
    };

    next_tree.foreach(dir_func, file_func);
    env::set_current_dir(&dir_tree.name).expect("Issue when cding to test directory");
    cleanup();
  }

  fn create_test_directory() -> (DirNode, impl Fn()) {
    let dir_tree = DirNode::default();
    let root = PathBuf::from(&dir_tree.name);
    if root.exists() {
      fs::remove_dir_all(&root).expect("Issue when cleaning up possible leftovers");
    }

    create_test_directory_recur(&dir_tree, PathBuf::new());
    env::set_current_dir(&root).expect("Issue when cding one up from test directory");
    data::init().expect("Issue when initing test repository");
    (
      dir_tree, move || {
        env::set_current_dir("..").expect("Issue when cding one up from test directory");
        if !root.is_dir() {
          let cwd = env::current_dir().expect("Issue when geting cwd");
          panic!("Cannot see test directory in cwd: {}", cwd.display());
        }

        fs::remove_dir_all(&root).expect("Issue when deleting test directory");
      }
    )
  }

  fn create_test_directory_recur(root: &DirNode, dir_name: PathBuf) {
    let dir_func = |node: &DirNode| -> bool {
      let mut path = dir_name.clone();
      path.push(&node.name);
      match fs::create_dir(&path) {
        Ok(_) => true,
        Err(_) => false
      }
    };

    let else_func = |node: &DirNode| -> bool {
      let mut path = dir_name.clone();
      path.push(&node.name);
      match fs::write(&path, "") {
        Ok(_) => true,
        Err(_) => false
      }
    };

    root.foreach(dir_func, else_func);
  }
}
