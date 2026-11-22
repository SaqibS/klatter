//! klatter: Klatter on Linux and FreeBSD.
//!
//! Speaks text from the command line or standard input through the system's
//! audio (PipeWire, PulseAudio or ALSA; on FreeBSD also /dev/dsp), into a WAV
//! file, or as raw PCM; and is what Speech Dispatcher runs for Orca (see
//! speech-dispatcher/klatter.conf). Settings come from
//! ~/.config/klatter/klatter.conf, then the command line. Audio goes out as it
//! is made, so speech starts at once.

use klatter::{DEFAULT_WPM, Item, PARAMETERS, Pronunciations, Voice, speak_stream_with_pronunciations};
use std::io::{self, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitCode, Stdio};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The 20 rates Klattara offers, as the NVDA add-on and the apps offer them.
const SAMPLE_RATES: [u32; 20] = [
    8000, 10000, 11000, 11025, 12000, 16000, 20000, 22000, 22050, 24000, 32000, 40000, 44000, 44100, 48000, 64000,
    80000, 88000, 88200, 96000,
];
const DEFAULT_SAMPLE_RATE: u32 = 16000;
const MIN_WPM: f64 = 30.0;
const MAX_WPM: f64 = 1920.0;
const MIN_HZ: f64 = 40.0;
const MAX_HZ: f64 = 400.0;

const HELP: &str = "\
klatter: the Klatter speech synthesizer

Usage: klatter [options] [text...]
Speaks the text, or standard input if none is given.

Voice
  -r, --rate WPM           words per minute, 30 to 1920 (219 is natural)
  -p, --pitch HZ           base pitch, 40 to 400 hertz (100 is natural)
  -i, --inflection PCT     how much the pitch moves, 0 to 200 percent of natural
      --volume PCT         0 to 200 percent (100 is full level)
      --flutter PCT        how much the pitch wavers, 0 to 100 (0, off, unless set)
      --set CODE=VALUE     any voice-quality setting, such as hs=110 or br=20
                           (may be given more than once; --list-settings lists them)

Output
  -s, --sample-rate HZ     one of 8000 10000 11000 11025 12000 16000 20000 22000 22050 24000
                           32000 40000 44000 44100 48000 64000 80000 88000 88200 96000 (16000)
  -w, --wav FILE           save a WAV file (- for standard output) instead of playing
      --raw                write 16-bit little-endian mono PCM to standard output
      --player COMMAND     play through this command, which reads raw PCM on its input;
                           {rate} becomes the sample rate (default: paplay, pw-cat or aplay;
                           on FreeBSD, /dev/dsp when none is installed)

Pronunciation
  -d, --dictionary FILE    pronunciations: lines of a word and its ARPAbet sounds, such as
                           \"klatter K L AE1 T ER0\" (may be given more than once)

Speech Dispatcher (used by speech-dispatcher/klatter.conf)
      --sd-rate N --sd-pitch N --sd-volume N   Speech Dispatcher's -100 to 100 settings

Other
      --config FILE        read settings from FILE (default ~/.config/klatter/klatter.conf)
      --no-config          read no settings file
      --list-settings      list the voice-quality settings with their ranges
      --show-settings      show the settings that would be used, and stop
  -h, --help               this help
      --version            the version
";

#[derive(Clone, Debug)]
struct Settings {
    wpm: f64,
    pitch: f64,
    inflection: f64,
    volume: f64,
    quality: klatter::Quality,
    sample_rate: u32,
    dictionaries: Vec<PathBuf>,
    player: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            wpm: DEFAULT_WPM,
            pitch: 100.0,
            inflection: 100.0,
            volume: 100.0,
            quality: klatter::Quality::default(),
            sample_rate: DEFAULT_SAMPLE_RATE,
            dictionaries: Vec::new(),
            player: None,
        }
    }
}

#[derive(Default)]
struct Request {
    text: Vec<String>,
    wav: Option<String>,
    raw: bool,
    show: bool,
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("klatter: {message}");
    std::process::exit(2)
}

fn number(name: &str, value: &str) -> f64 {
    match value.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => v,
        _ => fail(format!("{name} needs a number, not \"{value}\"")),
    }
}

impl Settings {
    /// One setting by name, from the config file or the command line.
    fn apply(&mut self, key: &str, value: &str) -> Result<(), String> {
        let key = key.trim().to_ascii_lowercase().replace('_', "-");
        let value = value.trim();
        let num = || value.parse::<f64>().ok().filter(|v| v.is_finite()).ok_or(format!("{key} needs a number"));
        match key.as_str() {
            "rate" | "wpm" => self.wpm = num()?.clamp(MIN_WPM, MAX_WPM),
            "pitch" => self.pitch = num()?.clamp(MIN_HZ, MAX_HZ),
            "inflection" => self.inflection = num()?.clamp(0.0, 200.0),
            "volume" => self.volume = num()?.clamp(0.0, 200.0),
            "sample-rate" => {
                let rate = num()? as u32;
                if !SAMPLE_RATES.contains(&rate) {
                    return Err(format!("{rate} is not one of the sample rates"));
                }
                self.sample_rate = rate;
            }
            "dictionary" => self.dictionaries.push(expand_home(value)),
            "player" => self.player = Some(value.to_string()),
            "flutter" => {
                self.quality.set("fl", num()?);
            }
            code => {
                // Any voice-quality code (hs, br, f4, ...), or "set" with code=value.
                if klatter::quality::index_of(code).is_none() {
                    return Err(format!("unknown setting \"{code}\""));
                }
                self.quality.set(code, num()?);
            }
        }
        Ok(())
    }

    fn voice(&self) -> Voice {
        Voice {
            wpm: self.wpm,
            pitch_hz: self.pitch,
            inflection: self.inflection / 100.0,
            volume: self.volume / 100.0,
            quality: self.quality,
        }
    }
}

fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(path),
    }
}

fn read_config(settings: &mut Settings, path: &PathBuf, required: bool) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if required => fail(format!("cannot read {}: {e}", path.display())),
        Err(_) => return,
    };
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            eprintln!("klatter: {} line {}: expected name = value", path.display(), n + 1);
            continue;
        };
        if let Err(e) = settings.apply(key, value) {
            eprintln!("klatter: {} line {}: {e}", path.display(), n + 1);
        }
    }
}

fn default_config() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("klatter").join("klatter.conf")
}

fn parse_args() -> (Settings, Request) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut settings = Settings::default();
    // The config file first, so the command line wins.
    let mut config = Some(default_config());
    let mut required = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--no-config" => config = None,
            "--config" => {
                config = args.get(i + 1).map(|p| expand_home(p));
                required = true;
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    if let Some(path) = &config {
        read_config(&mut settings, path, required);
    }
    let mut request = Request::default();
    let mut i = 0;
    let value = |i: &mut usize, name: &str| -> String {
        *i += 1;
        args.get(*i).cloned().unwrap_or_else(|| fail(format!("{name} needs a value")))
    };
    let set = |s: &mut Settings, key: &str, v: &str| {
        if let Err(e) = s.apply(key, v) {
            fail(e)
        }
    };
    while i < args.len() {
        let arg = args[i].clone();
        // --name=value as well as --name value.
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let take = |i: &mut usize| inline.clone().unwrap_or_else(|| value(i, &name));
        match name.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0)
            }
            "--version" => {
                println!("klatter {VERSION}");
                std::process::exit(0)
            }
            "--list-settings" => {
                for p in PARAMETERS {
                    println!("{:<4} {:<18} {} to {} {}, {} by default. {}", p.code, p.name, p.min, p.max, p.unit, p.default, p.help);
                }
                std::process::exit(0)
            }
            "-r" | "--rate" => set(&mut settings, "rate", &take(&mut i)),
            "-p" | "--pitch" => set(&mut settings, "pitch", &take(&mut i)),
            "-i" | "--inflection" => set(&mut settings, "inflection", &take(&mut i)),
            "--volume" => set(&mut settings, "volume", &take(&mut i)),
            "--flutter" => set(&mut settings, "flutter", &take(&mut i)),
            "--set" => {
                let v = take(&mut i);
                let Some((code, n)) = v.split_once('=') else { fail("--set needs CODE=VALUE, such as hs=110") };
                set(&mut settings, code, n)
            }
            "-s" | "--sample-rate" => set(&mut settings, "sample-rate", &take(&mut i)),
            "-d" | "--dictionary" => set(&mut settings, "dictionary", &take(&mut i)),
            "-w" | "--wav" => request.wav = Some(take(&mut i)),
            "--raw" => request.raw = true,
            "--player" => settings.player = Some(take(&mut i)),
            "--config" => i += 1,
            "--no-config" => {}
            "--show-settings" => request.show = true,
            // Speech Dispatcher gives -100 to 100 (its generic module multiplies
            // its -1 to 1 by 100): rate and pitch double or halve at the ends,
            // around what the config set; volume runs from silent to full.
            "--sd-rate" => {
                let n = number(&name, &take(&mut i)).clamp(-100.0, 100.0);
                settings.wpm = (settings.wpm * 2f64.powf(n / 100.0 * 2.0)).clamp(MIN_WPM, MAX_WPM);
            }
            "--sd-pitch" => {
                let n = number(&name, &take(&mut i)).clamp(-100.0, 100.0);
                settings.pitch = (settings.pitch * 2f64.powf(n / 100.0)).clamp(MIN_HZ, MAX_HZ);
            }
            "--sd-volume" => {
                let n = number(&name, &take(&mut i)).clamp(-100.0, 100.0);
                settings.volume = settings.volume * (n + 100.0) / 200.0;
            }
            "--" => {
                request.text.extend(args[i + 1..].iter().cloned());
                break;
            }
            s if s.starts_with('-') && s.len() > 1 => fail(format!("unknown option {s} (see --help)")),
            _ => request.text.push(arg),
        }
        i += 1;
    }
    (settings, request)
}

fn find_in_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

/// FreeBSD's own sound, OSS: /dev/dsp at 16-bit little-endian mono.
#[cfg(target_os = "freebsd")]
mod oss {
    use std::fs::{File, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::os::raw::{c_int, c_ulong};
    unsafe extern "C" {
        fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    }
    const SNDCTL_DSP_SPEED: c_ulong = 0xC004_5002;
    const SNDCTL_DSP_SETFMT: c_ulong = 0xC004_5005;
    const SNDCTL_DSP_CHANNELS: c_ulong = 0xC004_5006;
    const AFMT_S16_LE: c_int = 0x10;
    pub fn open(rate: u32) -> Option<File> {
        let device = OpenOptions::new().write(true).open("/dev/dsp").ok()?;
        let fd = device.as_raw_fd();
        let set = |request: c_ulong, value: c_int| -> Option<c_int> {
            let mut v = value;
            (unsafe { ioctl(fd, request, &mut v as *mut c_int) } != -1).then_some(v)
        };
        (set(SNDCTL_DSP_SETFMT, AFMT_S16_LE)? == AFMT_S16_LE).then_some(())?;
        (set(SNDCTL_DSP_CHANNELS, 1)? == 1).then_some(())?;
        set(SNDCTL_DSP_SPEED, rate as c_int)?;
        Some(device)
    }
}

fn start_player(settings: &Settings) -> (Option<Child>, Box<dyn Write>) {
    let rate = settings.sample_rate;
    let command = match &settings.player {
        Some(c) => c.replace("{rate}", &rate.to_string()),
        None if find_in_path("paplay") => format!("paplay --raw --format=s16le --rate={rate} --channels=1"),
        None if find_in_path("pw-cat") => format!("pw-cat --playback --format=s16 --rate={rate} --channels=1 -"),
        None if find_in_path("aplay") => format!("aplay -q -t raw -f S16_LE -r {rate} -c 1 -"),
        None => {
            #[cfg(target_os = "freebsd")]
            if let Some(device) = oss::open(rate) {
                return (None, Box::new(BufWriter::with_capacity(8192, device)));
            }
            fail("found no audio player (paplay, pw-cat or aplay); save a WAV with -w, or name one with --player")
        }
    };
    let mut parts = command.split_whitespace();
    let program = parts.next().unwrap_or_else(|| fail("the player command is empty"));
    match Command::new(program).args(parts).stdin(Stdio::piped()).spawn() {
        Ok(mut child) => {
            let stdin = child.stdin.take().expect("piped");
            (Some(child), Box::new(BufWriter::with_capacity(8192, stdin)))
        }
        Err(e) => fail(format!("could not start the player \"{program}\": {e}")),
    }
}

fn load_dictionaries(paths: &[PathBuf]) -> Pronunciations {
    let mut words = Pronunciations::default();
    for path in paths {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| fail(format!("cannot read {}: {e}", path.display())));
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((word, phones)) = line.split_once(char::is_whitespace) else { continue };
            if let Err(e) = words.insert(word, phones.trim()) {
                eprintln!("klatter: {} line {}: {e:?}", path.display(), n + 1);
            }
        }
    }
    words
}

fn main() -> ExitCode {
    let (settings, request) = parse_args();
    if request.show {
        let v = settings.voice();
        println!("rate {} words per minute\npitch {} hertz\ninflection {} percent\nvolume {} percent\nsample rate {} hertz",
            v.wpm, v.pitch_hz, settings.inflection, settings.volume, settings.sample_rate);
        for p in PARAMETERS {
            println!("{} ({}) {}", p.name, p.code, v.quality.get(p.code).unwrap_or(p.default));
        }
        return ExitCode::SUCCESS;
    }
    let text = if request.text.is_empty() {
        let mut t = String::new();
        if io::stdin().read_to_string(&mut t).is_err() {
            fail("could not read standard input as UTF-8");
        }
        t
    } else {
        request.text.join(" ")
    };
    let words = load_dictionaries(&settings.dictionaries);
    let voice = settings.voice();
    let rate = settings.sample_rate;
    let (mut child, mut out): (Option<Child>, Box<dyn Write>) = if request.wav.is_some() {
        (None, Box::new(io::sink()))
    } else if request.raw {
        (None, Box::new(BufWriter::with_capacity(8192, io::stdout())))
    } else {
        start_player(&settings)
    };
    let mut all: Vec<i16> = Vec::new();
    let keep = request.wav.is_some();
    let mut pcm = Vec::with_capacity(8192);
    let mut broken = false;
    speak_stream_with_pronunciations(&[Item::Text(&text)], rate, &voice, &words, |samples, _| {
        if keep {
            all.extend_from_slice(samples);
            return true;
        }
        pcm.clear();
        pcm.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
        // A player that has gone away (as when Speech Dispatcher stops speech) ends this.
        if out.write_all(&pcm).and_then(|_| out.flush()).is_err() {
            broken = true;
            return false;
        }
        true
    });
    if let Some(to) = request.wav {
        let mut bytes = Vec::new();
        klatter::wav::write_wav(&mut bytes, &all, rate).unwrap_or_else(|e| fail(e));
        let written = if to == "-" { io::stdout().write_all(&bytes) } else { std::fs::write(&to, &bytes) };
        if let Err(e) = written {
            fail(format!("cannot write {to}: {e}"));
        }
        return ExitCode::SUCCESS;
    }
    drop(out);
    if let Some(child) = child.as_mut() {
        let _ = child.wait();
    }
    if broken { ExitCode::from(1) } else { ExitCode::SUCCESS }
}
