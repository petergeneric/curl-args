use anyhow::{Context, Result};
use chrono::Timelike;
use serde::Deserialize;
use std::fs;
use std::process::{Command, Stdio};
use url::Url;

#[derive(Debug, Deserialize)]
struct Config {
    opts: Opts,
    auth: Auth,
}

#[derive(Debug, Deserialize)]
struct Opts {
    hosts: std::collections::HashMap<String, Vec<String>>,
    #[serde(rename = "defaultAccept")]
    default_accept: String,
}

#[derive(Debug, Deserialize)]
struct Auth {
    hosts: std::collections::HashMap<String, String>,
    keys: std::collections::HashMap<String, String>,
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
    for hostname in &hostnames {
        if let Some(opt_array) = config.opts.hosts.get(hostname) {
            extra.extend(opt_array.iter().cloned());
            break;
        }
    }

    // Read the Authorization header value associated with this hostname from config
    for hostname in &hostnames {
        if let Some(key) = config.auth.hosts.get(hostname) {
            // If the key is an alias, resolve it
            let key = if let Some(resolved_key) = config.auth.keys.get(key) {
                resolved_key
            } else {
                key
            };
            // Add the Authorization header to the arg list
            extra.extend(vec!["-H".to_owned(), format!("Authorization: {}", key)]);
            break;
        }
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
