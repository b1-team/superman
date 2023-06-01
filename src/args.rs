use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, help = "Pid to kill")]
    pub pid: u32,

    #[arg(short, required = false, help = "Recursive kill process")]
    pub recursive: bool,

    #[arg(
        short,
        long,
        default_value_t = 500,
        required = false,
        requires = "recursive",
        help = "Kill interval time (milliseconds)"
    )]
    pub time: u64,
}
