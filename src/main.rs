extern crate clap;
extern crate sha2;

mod cli;
mod data;

fn main() {
    match cli::cli() {
      Err(err) => println!("{}", err),
      Ok(_) => return,
    };
}
