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
    let home_dir = match dirs::home_dir() {
        Some(path) => path,
        None => panic!("Couldn't find home directory"),
    };
    let config_path = home_dir.join(".ccurlrc");

    // Read the config file
    let config_str = fs::read_to_string(&config_path).unwrap();
    let config: Config = serde_json::from_str(&config_str).unwrap();

    // Store our commandline args
    let mut curl_args: Vec<String> = std::env::args().skip(1).collect();
    let mut extra: Vec<String> = vec![];

    // Find hostnames from the command line args
    let hostnames: Vec<String> = curl_args
        .iter()
        .filter(|opt| !opt.starts_with('-') && opt.contains("://"))
        .map(|opt| match Url::parse(opt) {
            Ok(url) => Some(url.host_str().unwrap().to_lowercase()),
            Err(_) => None,
        })
        .filter(|hostname| hostname.is_some())
        .map(|hostname| hostname.unwrap())
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
    let mut child_process = cmd.spawn().unwrap();
    let child_process_exit_code = child_process.wait().unwrap().code().unwrap();
    std::process::exit(child_process_exit_code);

    #[allow(unreachable_code)]
    Ok(())
}
