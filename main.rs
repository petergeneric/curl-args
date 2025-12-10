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

#[derive(Debug, Deserialize)]
struct Config {
    opts: Opts,
    auth: Auth,
}

#[derive(Debug, Deserialize)]
struct Opts {
    hosts: HashMap<String, Vec<String>>,
    #[serde(rename = "defaultAccept")]
    default_accept: String,
}

#[derive(Debug, Deserialize)]
struct Auth {
    hosts: HashMap<String, String>,
    keys: HashMap<String, String>,
}

fn main() -> Result<()> {
    // Read the config file
    let home_dir = dirs::home_dir()
        .context("Could not determine home directory")?;
    let config_path = home_dir.join(".ccurlrc");

    let config_str = fs::read_to_string(&config_path)
        .with_context(|| format!("Could not read config file: {}", config_path.display()))?;
    let config: Config = serde_json::from_str(&config_str)
        .with_context(|| format!("Invalid JSON in config file: {}", config_path.display()))?;

    // Store our commandline args
    let mut curl_args: Vec<String> = std::env::args().skip(1).collect();
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

    // If curlArgs does not already have an Accept header, apply the default one from config
	if !curl_args
		.iter()
		.any(|arg| arg.to_lowercase().starts_with("accept:"))
	{
		extra.extend(vec!["-H".to_owned(), config.opts.default_accept.clone()]);
	}


    // Test if there are any special flags (e.g. '--trace'). If found, replace them with their corresponding headers
    if let Some(index) = curl_args.iter().position(|arg| arg == "--trace") {
        let now = chrono::Local::now();
        let trace_header = format!(
            "X-Correlation-ID: {}/{}{}{}",
            whoami::username(),
            now.hour(),
            now.minute(),
            now.second()
        );
        
        let trace_args = vec![
            "-H".to_owned(),
            "X-Trace-Verbose: true".to_owned(),
            "-H".to_owned(),
            trace_header
        ];
        extra.extend(trace_args);
        curl_args.remove(index);
    }


    // Run the child process
    let mut cmd = Command::new("curl");
    cmd.args(extra.iter().chain(curl_args.iter()));
    cmd.stdin(Stdio::inherit());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    let mut child_process = cmd.spawn()
        .context("Failed to execute curl. Is curl installed and in PATH?")?;
    let exit_status = child_process.wait()
        .context("Failed to wait for curl process")?;
    std::process::exit(exit_status.code().unwrap_or(1))
}
