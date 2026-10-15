use std::io::{self, Read, Write};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;

#[derive(Parser)]
#[command(version, about = "Read or modify binary data at a byte offset")]
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
    /// Replace the selected bytes
    #[arg(short = 's', group = "operation")]
    set: bool,
    /// Extract the selected bytes unchanged (cannot be used with -B or -L)
    #[arg(short = 'g', group = "operation", conflicts_with = "endianness")]
    get: bool,
    /// Apply a bitwise AND mask
    #[arg(short = 'a', group = "operation")]
    and: bool,
    /// Apply a bitwise OR mask
    #[arg(short = 'o', group = "operation")]
    or: bool,

    /// Interpret values as big-endian
    #[arg(short = 'B', group = "endianness")]
    big_endian: bool,
    /// Interpret values as little-endian (default)
    #[arg(short = 'L', group = "endianness")]
    little_endian: bool,

    /// Select 1 byte
    #[arg(short = 'b', group = "size")]
    byte: bool,
    /// Select 2 bytes
    #[arg(short = 'w', group = "size")]
    word: bool,
    /// Select 4 bytes
    #[arg(short = 'd', group = "size")]
    double_word: bool,
    /// Select 8 bytes
    #[arg(short = 'q', group = "size")]
    quad_word: bool,

    /// Byte offset (decimal, 0x hexadecimal, or 0b binary)
    #[arg(required = true)]
    offset: String,
    /// Value or mask in the same integer formats as OFFSET
    #[arg(conflicts_with = "get", required_unless_present = "get")]
    value: Option<String>,
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    Set(u64),
    Get,
    And(u64),
    Or(u64),
}

impl Operation {
    fn from_args(args: &Args, size: Size) -> Result<Self> {
        if args.get {
            return Ok(Operation::Get);
        }

        let input = args.value.as_deref().context("No value specified")?;
        let value = parse_integer(input, "value")?;
        let maximum = u64::MAX >> (64 - size.bytes() * 8);
        ensure!(
            value <= maximum,
            "Value {input} does not fit in {} byte(s)",
            size.bytes()
        );

        if args.set {
            Ok(Operation::Set(value))
        } else if args.and {
            Ok(Operation::And(value))
        } else if args.or {
            Ok(Operation::Or(value))
        } else {
            bail!("No operation specified")
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Endianness {
    Big,
    Little,
}

impl Endianness {
    fn from_args(args: &Args) -> Self {
        if args.big_endian {
            Endianness::Big
        } else {
            Endianness::Little
        }
    }

    fn decode(self, bytes: &[u8]) -> u64 {
        let mut padded = [0; 8];
        match self {
            Endianness::Big => {
                padded[8 - bytes.len()..].copy_from_slice(bytes);
                u64::from_be_bytes(padded)
            }
            Endianness::Little => {
                padded[..bytes.len()].copy_from_slice(bytes);
                u64::from_le_bytes(padded)
            }
        }
    }

    fn encode(self, value: u64, bytes: &mut [u8]) {
        match self {
            Endianness::Big => bytes.copy_from_slice(&value.to_be_bytes()[8 - bytes.len()..]),
            Endianness::Little => bytes.copy_from_slice(&value.to_le_bytes()[..bytes.len()]),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Size {
    Byte,
    Word,
    DoubleWord,
    QuadWord,
}

impl Size {
    fn from_args(args: &Args) -> Result<Self> {
        if args.byte {
            Ok(Size::Byte)
        } else if args.word {
            Ok(Size::Word)
        } else if args.double_word {
            Ok(Size::DoubleWord)
        } else if args.quad_word {
            Ok(Size::QuadWord)
        } else {
            bail!("No size specified")
        }
    }

    fn bytes(self) -> usize {
        match self {
            Size::Byte => 1,
            Size::Word => 2,
            Size::DoubleWord => 4,
            Size::QuadWord => 8,
        }
    }
}

struct Filter {
    operation: Operation,
    endianness: Endianness,
    size: Size,
    offset: u64,
}

impl Filter {
    fn from_args(args: &Args) -> Result<Self> {
        let size = Size::from_args(args)?;
        let offset = parse_integer(&args.offset, "offset")?;
        offset
            .checked_add(size.bytes() as u64)
            .context("Offset plus size exceeds the supported range")?;

        Ok(Self {
            operation: Operation::from_args(args, size)?,
            endianness: Endianness::from_args(args),
            size,
            offset,
        })
    }

    fn run(&self, reader: &mut impl Read, writer: &mut impl Write) -> Result<()> {
        let mut prefix = reader.by_ref().take(self.offset);
        if matches!(self.operation, Operation::Get) {
            let skipped = io::copy(&mut prefix, &mut io::sink())
                .context("Failed to read input before offset")?;
            ensure!(
                skipped == self.offset,
                "Input is too short for offset and size"
            );
        } else {
            let copied =
                io::copy(&mut prefix, writer).context("Failed to copy input before offset")?;
            io::copy(&mut io::repeat(0).take(self.offset - copied), writer)
                .context("Failed to zero-fill input before offset")?;
        }

        // Only the selected field is buffered; the rest of the input is streamed.
        let width = self.size.bytes();
        let mut bytes = Vec::with_capacity(width);
        reader
            .by_ref()
            .take(width as u64)
            .read_to_end(&mut bytes)
            .context("Failed to read selected bytes")?;

        if matches!(self.operation, Operation::Get) {
            ensure!(
                bytes.len() == width,
                "Input is too short for offset and size"
            );
            writer
                .write_all(&bytes)
                .context("Failed to write selected bytes")?;
            return Ok(());
        }

        // Missing bytes are appended in stream order, before endian conversion.
        bytes.resize(width, 0);
        let original = self.endianness.decode(&bytes);
        let value = match self.operation {
            Operation::Set(value) => value,
            Operation::And(mask) => original & mask,
            Operation::Or(mask) => original | mask,
            Operation::Get => unreachable!(),
        };
        self.endianness.encode(value, &mut bytes);
        writer
            .write_all(&bytes)
            .context("Failed to write selected bytes")?;
        io::copy(reader, writer).context("Failed to copy input after selected bytes")?;
        Ok(())
    }
}

fn parse_integer(input: &str, name: &str) -> Result<u64> {
    let (digits, radix) = if let Some(digits) = input.strip_prefix("0x") {
        (digits, 16)
    } else if let Some(digits) = input.strip_prefix("0b") {
        (digits, 2)
    } else {
        (input, 10)
    };
    u64::from_str_radix(digits, radix).with_context(|| format!("Invalid {name}: {input}"))
}

fn main() -> Result<()> {
    let filter = Filter::from_args(&Args::parse())?;
    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    filter.run(&mut reader, &mut writer)?;
    writer.flush().context("Failed to flush output")
}
