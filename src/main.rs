use std::io::Stdout;

use clap::Parser;
use anyhow::{Result, Context};

#[derive(Parser)]
#[command(group(
    clap::ArgGroup::new("operation")
        .required(true)
        .multiple(false),
))]
#[command(group(
    clap::ArgGroup::new("endianness")
        .required(false)
        .multiple(false),
))]
#[command(group(
    clap::ArgGroup::new("size")
        .required(true)
        .multiple(false),
))]
struct Args {
    #[arg(short = 's', group = "operation")]
    set: bool,
    #[arg(short = 'g', group = "operation")]
    get: bool,
    #[arg(short = 'a', group = "operation")]
    and: bool,
    #[arg(short = 'o', group = "operation")]
    or: bool,

    #[arg(short = 'B', group = "endianness")]
    big_endian: bool,
    #[arg(short = 'L', group = "endianness")]
    little_endian: bool,

    #[arg(short = 'b', group = "size")]
    byte: bool,
    #[arg(short = 'w', group = "size")]
    word: bool,
    #[arg(short = 'd', group = "size")]
    double_word: bool,
    #[arg(short = 'q', group = "size")]
    quad_word: bool,

    #[arg(required = true)]
    offset: String,
    #[arg(conflicts_with = "get", required_unless_present = "get")]
    value: Option<String>,
    
}
#[derive(Debug)]
enum Operation {
    Set,
    Get,
    And,
    Or,
}
impl Operation {
    fn from_args(args: &Args) -> Result<Self> {
        if args.set {
            Ok(Operation::Set)
        } else if args.get {
            Ok(Operation::Get)
        } else if args.and {
            Ok(Operation::And)
        } else if args.or {
            Ok(Operation::Or)
        } else {
            Err(anyhow::anyhow!("No operation specified"))
        }
    }
}
#[derive(Debug)]
enum Endianness {
    Big,
    Little,
}
impl Endianness {
    fn from_args(args: &Args) -> Self {
        if args.big_endian {
            Endianness::Big
        } else if args.little_endian {
            Endianness::Little
        } else {
            Endianness::Little
        }
    }
}
#[derive(Debug)]
enum Size {
    Byte,
    Word,
    DoubleWord,
    QuadWord,
}
impl Size {
    fn from_args(args: &Args) -> Self {
        if args.byte {
            Size::Byte
        } else if args.word {
            Size::Word
        } else if args.double_word {
            Size::DoubleWord
        } else if args.quad_word {
            Size::QuadWord
        } else {
            Size::Byte
        }
    }
}
fn from_args(args: &Args) -> Result<usize> {
    let input = args.offset.as_str();
    let (digits, radix) = if let Some(digits) = input.strip_prefix("0x") {
        (digits, 16)
    } else if let Some(digits) = input.strip_prefix("0b") {
        (digits, 2)
    } else {
        (input, 10)
    };
    let value = usize::from_str_radix(digits, radix)
        .with_context(|| format!("Invalid offset: {}", args.offset))?;
    Ok(value)
}
fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();

    let operation = Operation::from_args(&args)?;
    let endianness = Endianness::from_args(&args);
    let size = Size::from_args(&args);
    let offset = from_args(&args)?;
    let value = from_args(&args)?;
    println!("Operation: {:?}", operation);
    println!("Endianness: {:?}", endianness);
    println!("Size: {:?}", size);
    println!("Offset: {:?}", offset);
    println!("Value: {:?}", value);

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    match operation {
        Operation::Set => {
            // Handle set operation
        },
        Operation::Get => {
            // Handle get operation
        },
        Operation::And => {
            // Handle and operation
        },
        Operation::Or => {
            // Handle or operation
        },
    }
    Ok(())
}
