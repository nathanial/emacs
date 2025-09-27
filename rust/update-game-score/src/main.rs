use anyhow::{anyhow, bail, Context, Result};
use libc::{getegid, geteuid, getgid, getpwuid, getuid};
use std::cmp::Ordering;
use std::ffi::CStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

type UidT = libc::uid_t;

const MAX_ATTEMPTS: u32 = 5;
const MAX_DATA_LEN: usize = 1024;
const ONE_HOUR: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Debug)]
struct Cli {
    reverse: bool,
    max_scores: usize,
    user_prefix: Option<String>,
    score_key: String,
    score_value: String,
    score_data: String,
}

#[derive(Clone)]
struct ScoreEntry {
    score: String,
    user_data: String,
}

struct FileLock {
    path: PathBuf,
}

impl FileLock {
    fn acquire(target: &Path) -> Result<FileLock> {
        let mut lock_os = target.as_os_str().to_os_string();
        lock_os.push(".lockfile");
        let lock_path = PathBuf::from(lock_os);
        let mut attempts = 0u32;

        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(file) => {
                    drop(file);
                    return Ok(FileLock { path: lock_path });
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    attempts += 1;
                    let stale = match fs::metadata(&lock_path)
                        .and_then(|meta| meta.modified())
                    {
                        Ok(modified) => match SystemTime::now().duration_since(modified) {
                            Ok(elapsed) => elapsed > ONE_HOUR,
                            Err(_) => false,
                        },
                        Err(_) => false,
                    };

                    if attempts > MAX_ATTEMPTS || stale {
                        if let Err(remove_err) = fs::remove_file(&lock_path) {
                            if remove_err.kind() != io::ErrorKind::NotFound {
                                return Err(remove_err.into());
                            }
                        }
                        attempts = 0;
                    } else {
                        thread::sleep(Duration::from_secs(1));
                    }
                }
                Err(err) => return Err(err.into()),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn usage(stdout: bool, progname: &str) {
    let mut out: Box<dyn Write> = if stdout {
        Box::new(io::stdout())
    } else {
        Box::new(io::stderr())
    };
    let _ = writeln!(out, "Usage: {progname} [-m MAX] [-r] [-d DIR] game/scorefile SCORE DATA");
    let _ = writeln!(out, "       {progname} -h");
    let _ = writeln!(out, " -h\t\tDisplay this help.");
    let _ = writeln!(out, " -m MAX\t\tLimit the maximum number of scores to MAX.");
    let _ = writeln!(out, " -r\t\tSort the scores in increasing order.");
    let _ = writeln!(out, " -d DIR\t\tStore scores in DIR (only if not setuid).");
}

fn parse_cli(args: &[String]) -> Result<Cli> {
    let progname = args
        .first()
        .cloned()
        .unwrap_or_else(|| "update-game-score".to_string());

    let mut reverse = false;
    let mut max_scores = usize::MAX;
    let mut user_prefix = None;

    let mut idx = 1;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == "-h" {
            usage(true, &progname);
            std::process::exit(0);
        } else if arg == "-r" {
            reverse = true;
            idx += 1;
        } else if arg == "-m" {
            if idx + 1 >= args.len() {
                usage(false, &progname);
                bail!("Missing argument for -m");
            }
            let max = args[idx + 1]
                .parse::<i128>()
                .map_err(|_| anyhow!("Invalid MAX value"))?;
            if max < 0 {
                usage(false, &progname);
                bail!("Invalid MAX value");
            }
            max_scores = max.min(isize::MAX as i128) as usize;
            idx += 2;
        } else if arg == "-d" {
            if idx + 1 >= args.len() {
                usage(false, &progname);
                bail!("Missing argument for -d");
            }
            user_prefix = Some(args[idx + 1].clone());
            idx += 2;
        } else if arg.starts_with('-') {
            usage(false, &progname);
            bail!(format!("Invalid option: {arg}"));
        } else {
            break;
        }
    }

    if args.len() - idx != 3 {
        usage(false, &progname);
        bail!("Expected game/scorefile SCORE DATA");
    }

    Ok(Cli {
        reverse,
        max_scores,
        user_prefix,
        score_key: args[idx].clone(),
        score_value: args[idx + 1].clone(),
        score_data: args[idx + 2].clone(),
    })
}

fn normalize_integer(input: &str) -> Option<String> {
    let mut s = input.trim_start();
    let mut neg = false;
    if let Some(rest) = s.strip_prefix('-') {
        neg = true;
        s = rest;
    }
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let stripped = s.trim_start_matches('0');
    let digits = if stripped.is_empty() { "0" } else { stripped };
    if digits == "0" {
        neg = false;
    }
    let mut out = String::with_capacity(digits.len() + 1);
    if neg {
        out.push('-');
    }
    out.push_str(digits);
    Some(out)
}

fn get_user_id() -> Result<String> {
    unsafe {
        let uid: UidT = getuid();
        let pwd = getpwuid(uid);
        if pwd.is_null() {
            return Ok(uid.to_string());
        }
        let name = CStr::from_ptr((*pwd).pw_name)
            .to_str()
            .unwrap_or("");
        if name.contains(' ') || name.contains('\n') || name.is_empty() {
            Ok(uid.to_string())
        } else {
            Ok(name.to_string())
        }
    }
}

fn prefix_path(privileged: bool, user_prefix: &Option<String>) -> Result<PathBuf> {
    if privileged {
        if let Some(dir) = option_env!("HAVE_SHARED_GAME_DIR") {
            return Ok(PathBuf::from(dir));
        }
        bail!(
            "This program was compiled without HAVE_SHARED_GAME_DIR,\n\
             and should not run with elevated privileges."
        );
    }
    match user_prefix {
        Some(dir) => Ok(PathBuf::from(dir)),
        None => bail!("Not using a shared game directory, and no prefix given."),
    }
}

fn read_scores(path: &Path) -> Result<Vec<ScoreEntry>> {
    let data = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    if data.iter().any(|&b| b == 0) {
        bail!("Invalid data in score file");
    }
    let text = String::from_utf8(data).map_err(|_| anyhow!("Invalid data in score file"))?;
    let mut scores = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        if let Some(space) = line.find(' ') {
            let score = &line[..space];
            let user_data = &line[space + 1..];
            scores.push(ScoreEntry {
                score: score.to_string(),
                user_data: user_data.to_string(),
            });
        } else {
            bail!("Invalid data in score file");
        }
    }
    Ok(scores)
}

fn compare_scores(a: &ScoreEntry, b: &ScoreEntry) -> Ordering {
    let mut sa = a.score.as_str();
    let mut sb = b.score.as_str();
    let nega = sa.starts_with('-');
    let negb = sb.starts_with('-');
    if nega != negb {
        return if nega {
            Ordering::Greater
        } else {
            Ordering::Less
        };
    }
    if nega {
        sa = &b.score[1..];
        sb = &a.score[1..];
    }
    let lena = sa.len();
    let lenb = sb.len();
    if lena != lenb {
        return if lenb < lena {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    sb.cmp(sa)
}

fn sort_scores(scores: &mut Vec<ScoreEntry>, reverse: bool) {
    if reverse {
        scores.sort_by(|a, b| compare_scores(b, a));
    } else {
        scores.sort_by(compare_scores);
    }
}

fn write_scores(path: &Path, mode: u32, scores: &[ScoreEntry]) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut builder = tempfile::Builder::new();
    builder.prefix(".tmp-score-");
    let mut tempfile = builder.tempfile_in(dir)?;
    tempfile
        .as_file_mut()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    for entry in scores {
        writeln!(tempfile, "{} {}", entry.score, entry.user_data)?;
    }
    tempfile.persist(path)?;
    Ok(())
}

fn enforce_limit(scores: &mut Vec<ScoreEntry>, max_scores: usize, reverse: bool) {
    if max_scores >= scores.len() {
        return;
    }
    if reverse {
        let remove = scores.len() - max_scores;
        scores.drain(0..remove);
    } else {
        scores.truncate(max_scores);
    }
}

fn execute(cli: Cli) -> Result<()> {
    let running_suid;
    let running_sgid;
    unsafe {
        running_suid = getuid() != geteuid();
        running_sgid = getgid() != getegid();
    }
    if running_suid && running_sgid {
        bail!("This program can run either suid or sgid, but not both.");
    }

    let prefix = prefix_path(running_suid || running_sgid, &cli.user_prefix)?;
    let score_path = prefix.join(&cli.score_key);

    let normalized = normalize_integer(&cli.score_value).ok_or_else(|| anyhow!("Invalid score"))?;

    let mut data = cli.score_data.clone();
    if data.len() > MAX_DATA_LEN {
        data.truncate(MAX_DATA_LEN);
    }
    if let Some(pos) = data.find('\n') {
        data.truncate(pos);
    }
    let user = get_user_id()?;
    let combined = format!("{} {}", user, data);

    let _lock = FileLock::acquire(&score_path)?;
    let mut scores = read_scores(&score_path)?;
    scores.push(ScoreEntry {
        score: normalized,
        user_data: combined,
    });
    sort_scores(&mut scores, cli.reverse);
    enforce_limit(&mut scores, cli.max_scores, cli.reverse);
    let mode = if running_sgid { 0o664 } else { 0o644 };
    write_scores(&score_path, mode, &scores)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cli = match parse_cli(&args) {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    if let Err(err) = execute(cli) {
        eprintln!("{}", err);
        std::process::exit(1);
    }
}
