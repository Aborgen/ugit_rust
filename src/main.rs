extern crate clap;

mod cli;
mod data;

fn main() {
    match cli::cli() {
      Err(err) => println!("{}", err),
      Ok(_) => return,
    };
}
