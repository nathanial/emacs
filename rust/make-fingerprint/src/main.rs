use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/placeholder.rs"));

fn usage(prog: &str) {
    println!("make-fingerprint [-r] FILE: replace or compute a hash");
    println!("Usage: {prog} [-r] FILE");
}

fn parse_args(args: &[String]) -> Result<(bool, PathBuf)> {
    let prog = args
        .first()
        .cloned()
        .unwrap_or_else(|| "make-fingerprint".to_string());
    let mut raw = false;
    let mut idx = 1;
    while idx < args.len() {
        match args[idx].as_str() {
            "-r" => {
                raw = true;
                idx += 1;
            }
            "-h" => {
                usage(&prog);
                std::process::exit(0);
            }
            arg if arg.starts_with('-') => {
                usage(&prog);
                bail!("{prog}: invalid option {arg}");
            }
            _ => break,
        }
    }

    if args.len() - idx != 1 {
        bail!("{prog}: missing or extra file operand");
    }

    Ok((raw, PathBuf::from(&args[idx])))
}

fn compute_digest(buf: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(buf);
    let result = hasher.finalize();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&result);
    digest
}

fn replace_fingerprint(file: &mut fs::File, buf: &[u8], digest: &[u8]) -> Result<()> {
    let mut found = false;
    if PLACEHOLDER.len() <= buf.len() {
        for (idx, window) in buf.windows(PLACEHOLDER.len()).enumerate() {
            if window == PLACEHOLDER {
                file.seek(SeekFrom::Start(idx as u64))?;
                file.write_all(digest)?;
                found = true;
            }
        }
    }
    if !found {
        bail!("missing fingerprint");
    }
    Ok(())
}

fn run(raw: bool, path: &PathBuf) -> Result<()> {
    let metadata = fs::metadata(path).with_context(|| format!("{}", path.display()))?;
    if !metadata.is_file() {
        bail!("{} is not a regular file", path.display());
    }
    let filesize = metadata.len();
    if filesize >= isize::MAX as u64 {
        bail!("{}: file too big", path.display());
    }

    let mut file = if raw {
        OpenOptions::new().read(true).open(path)?
    } else {
        OpenOptions::new().read(true).write(true).open(path)?
    };

    let mut buf = vec![0u8; filesize as usize];
    file.read_exact(&mut buf)
        .map_err(|_| anyhow::anyhow!("Error: could not read {}", path.display()))?;

    let digest = compute_digest(&buf);

    if raw {
        for byte in digest.iter() {
            print!("{:02X}", byte);
        }
        io::stdout().flush()?;
    } else {
        replace_fingerprint(&mut file, &buf, &digest)
            .with_context(|| format!("{}", path.display()))?;
        file.sync_data().ok();
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let prog = args
        .first()
        .cloned()
        .unwrap_or_else(|| "make-fingerprint".to_string());
    let (raw, path) = match parse_args(&args) {
        Ok(values) => values,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    if let Err(err) = run(raw, &path) {
        eprintln!("{}: {}", prog, err);
        std::process::exit(1);
    }
}
