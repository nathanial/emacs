use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

#[derive(Clone, Copy)]
enum Mode {
    Encode,
    Decode,
}

struct Options {
    mode: Mode,
    iso: bool,
    group_mask: usize,
    files: Vec<String>,
}

fn parse_args(args: &[String]) -> Result<Options> {
    let progname = args
        .first()
        .cloned()
        .unwrap_or_else(|| "hexl".to_string());

    let mut mode = Mode::Encode;
    let mut iso = false;
    let mut group_mask = 0x01usize;
    let mut files: Vec<String> = Vec::new();

    let mut idx = 1;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == "--" {
            files.extend(args[(idx + 1)..].iter().cloned());
            break;
        } else if arg.starts_with('-') && arg.len() > 1 {
            match arg.as_str() {
                "-hex" => {}
                "-iso" => iso = true,
                "-un" | "-de" => mode = Mode::Decode,
                "-group-by-8-bits" => group_mask = 0x00,
                "-group-by-16-bits" => group_mask = 0x01,
                "-group-by-32-bits" => group_mask = 0x03,
                "-group-by-64-bits" => group_mask = 0x07,
                _ => {
                    return Err(anyhow!(
                        "{}: invalid switch: \"{}\".",
                        progname, arg
                    ))
                }
            }
        } else {
            files.push(arg.clone());
        }
        idx += 1;
    }

    if files.is_empty() {
        files.push("-".into());
    }

    Ok(Options {
        mode,
        iso,
        group_mask,
        files,
    })
}

fn open_input(name: &str) -> Result<Box<dyn Read>> {
    if name == "-" {
        Ok(Box::new(io::stdin().lock()))
    } else {
        let file = File::open(Path::new(name))
            .map_err(|e| anyhow!("{}: {}", name, e))?;
        Ok(Box::new(file))
    }
}

fn encode_file<R: Read>(mut reader: R, iso: bool, group_mask: usize) -> Result<()> {
    let mut stdout = io::stdout().lock();
    let mut buffer = [0u8; 16];
    let mut address: u64 = 0;

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }

        write!(stdout, "{address:08x}: ")?;
        let mut ascii = String::with_capacity(n + 1);
        ascii.push(' ');
        for i in 0..16 {
            if i < n {
                let byte = buffer[i];
                write!(stdout, "{:02x}", byte)?;
                let glyph = if byte < 0x20
                    || (byte >= 0x7f && (!iso || byte < 0xa0))
                {
                    '.'
                } else {
                    byte as char
                };
                ascii.push(glyph);
            } else {
                stdout.write_all(b"  ")?;
            }

            if (i & group_mask) == group_mask {
                stdout.write_all(b" ")?;
            }
        }
        writeln!(stdout, "{ascii}")?;
        address += 0x10;
    }

    stdout.flush()?;
    Ok(())
}

fn decode_file(name: &str, group_mask: usize) -> Result<()> {
    let reader: Box<dyn Read> = if name == "-" {
        Box::new(io::stdin().lock())
    } else {
        Box::new(File::open(Path::new(name)).map_err(|e| anyhow!("{}: {}", name, e))?)
    };
    let mut stdout = io::stdout().lock();
    let buf_reader = BufReader::new(reader);
    for line_res in buf_reader.lines() {
        let line = line_res?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(space) = line.find(' ') {
            let mut pos = space + 1;
            let bytes = line.as_bytes();
            for i in 0..16 {
                while pos < bytes.len() && bytes[pos] == b' ' {
                    pos += 1;
                }
                if pos + 1 >= bytes.len() {
                    break;
                }
                let hi = match hex_value(bytes[pos]) {
                    Some(v) => v,
                    None => break,
                };
                let lo = match hex_value(bytes[pos + 1]) {
                    Some(v) => v,
                    None => break,
                };
                pos += 2;
                stdout.write_all(&[(hi << 4) | lo])?;
                if (i & group_mask) == group_mask && pos < bytes.len() && bytes[pos] == b' ' {
                    pos += 1;
                }
            }
        }
    }
    stdout.flush()?;
    Ok(())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    let opts = match parse_args(&raw_args) {
        Ok(opts) => opts,
        Err(err) => {
            let prog = raw_args
                .first()
                .cloned()
                .unwrap_or_else(|| "hexl".to_string());
            eprintln!("{}", err);
            eprintln!("usage: {} [-de] [-iso]", prog);
            std::process::exit(1);
        }
    };

    if execute(&opts) {
        std::process::exit(1);
    }
}

fn execute(opts: &Options) -> bool {
    let mut had_error = false;

    for file in opts.files.iter() {
        let result = match opts.mode {
            Mode::Encode => match open_input(file) {
                Ok(reader) => encode_file(reader, opts.iso, opts.group_mask),
                Err(err) => Err(err),
            },
            Mode::Decode => decode_file(file, opts.group_mask),
        };

        if let Err(err) = result {
            eprintln!("{}", err);
            had_error = true;
        }
    }

    had_error
}
