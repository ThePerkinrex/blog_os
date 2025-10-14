use std::path::PathBuf;

use clap::Parser;


#[derive(clap::Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands
}

#[derive(clap::Subcommand)]
enum Commands {
    Cp {
        source: PathBuf,
        dest: PathBuf
    }
}


fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Cp { source, mut dest } => {
            if !source.exists() {
                panic!("Source does not exist")
            }
            if dest.is_dir() {
                dest = dest.join(source.file_name().unwrap())
            }
            std::fs::copy(source, dest).unwrap();
        },
    }
}
