use std::fs;

const DIRECTORY_NAME: &str = ".ugit";
pub fn init() -> std::io::Result<()> {
  fs::create_dir(DIRECTORY_NAME)?;
  return Ok(())
}
