extern crate clap;
extern crate sha2;

mod base;
mod cli;
mod data;

fn main() {
  match cli::cli() {
    Err(err) => println!("{}", err),
    Ok(_) => ()
  };
}
