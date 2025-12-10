use anyhow::{Context, Result};
use chrono::Timelike;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::process::{Command, Stdio};
use url::Url;

/// Finds the first matching value for any hostname in the given map
fn find_for_hostname<'a, V>(hostnames: &[String], map: &'a HashMap<String, V>) -> Option<&'a V> {
    hostnames.iter().find_map(|h| map.get(h))
}

#[derive(Debug, Deserialize, Default)]
struct Config {
    #[serde(default)]
    opts: Opts,
    #[serde(default)]
    auth: Auth,
}

#[derive(Debug, Deserialize)]
struct Opts {
    #[serde(default)]
    hosts: HashMap<String, Vec<String>>,
    #[serde(rename = "defaultAccept", default = "default_accept")]
    default_accept: String,
}

fn default_accept() -> String {
    "application/json, */*".to_owned()
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            hosts: HashMap::new(),
            default_accept: default_accept(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct Auth {
    #[serde(default)]
    hosts: HashMap<String, String>,
    #[serde(default)]
    keys: HashMap<String, String>,
}

fn print_help() {
    println!(
        "ccurl {} - curl wrapper with automatic auth injection

USAGE:
    ccurl [OPTIONS] [curl arguments...]

SPECIAL FLAGS:
    --help          Show this help message
    --trace         Add X-Correlation-ID and X-Trace-Verbose headers
    --ccurlverbose  Show debug information

CONFIG:
    Reads from ~/.ccurlrc (JSON format)
    See ccurlrc.example.json for configuration options.",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    let verbose = args.iter().any(|arg| arg == "--ccurlverbose");
    let mut curl_args: Vec<String> = args.into_iter().filter(|arg| arg != "--ccurlverbose").collect();

    // Read the config file
    let home_dir = dirs::home_dir()
        .context("Could not determine home directory")?;
    let config_path = home_dir.join(".ccurlrc");

    if verbose {
        eprintln!("[ccurl] Config: {}", config_path.display());
    }

    let config: Config = if config_path.exists() {
        let config_str = fs::read_to_string(&config_path)
            .with_context(|| format!("Could not read config file: {}", config_path.display()))?;
        serde_json::from_str(&config_str)
            .with_context(|| format!("Invalid JSON in config file: {}", config_path.display()))?
    } else {
        if verbose {
            eprintln!("[ccurl] Config file not found, using defaults");
        }
        Config::default()
    };
    let mut extra: Vec<String> = vec![];

    // Find hostnames from the command line args
    let hostnames: Vec<String> = curl_args
        .iter()
        .filter(|opt| !opt.starts_with('-') && opt.contains("://"))
        .filter_map(|opt| {
            Url::parse(opt)
                .ok()
                .and_then(|url| url.host_str().map(|h| h.to_lowercase()))
        })
        .collect();

    if verbose {
        eprintln!("[ccurl] Hostnames: {:?}", hostnames);
    }

    // Read the options associated with this hostname from config
    if let Some(opt_array) = find_for_hostname(&hostnames, &config.opts.hosts) {
        extra.extend(opt_array.iter().cloned());
    }

    // Read the Authorization header value associated with this hostname from config
    if let Some(key) = find_for_hostname(&hostnames, &config.auth.hosts) {
        let resolved_key = config.auth.keys.get(key).unwrap_or(key);
        extra.push("-H".to_owned());
        extra.push(format!("Authorization: {}", resolved_key));
    }

    // If curl_args does not already have an Accept header, apply the default one from config
    let has_accept = curl_args.windows(2).any(|pair| {
        pair[0] == "-H" && pair[1].to_lowercase().trim_start().starts_with("accept:")
    });
    if !has_accept {
        extra.push("-H".to_owned());
        extra.push(format!("Accept: {}", config.opts.default_accept));
    }


    // Handle --trace flag: remove all occurrences and add tracing headers
    let has_trace = curl_args.iter().any(|arg| arg == "--trace");
    curl_args.retain(|arg| arg != "--trace");
    if has_trace {
        let now = chrono::Local::now();
        extra.push("-H".to_owned());
        extra.push("X-Trace-Verbose: true".to_owned());
        extra.push("-H".to_owned());
        extra.push(format!(
            "X-Correlation-ID: {}/{}{}{}",
            whoami::username(),
            now.hour(),
            now.minute(),
            now.second()
        ));
    }

    if verbose {
        eprintln!("[ccurl] Extra args: {:?}", extra);
    }

    // Run the child process
    let mut child_process = Command::new("curl")
        .args(extra.iter().chain(curl_args.iter()))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to execute curl. Is curl installed and in PATH?")?;
    let exit_status = child_process.wait()
        .context("Failed to wait for curl process")?;
    std::process::exit(exit_status.code().unwrap_or(1))
}
