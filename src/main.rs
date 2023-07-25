#[macro_use]
extern crate rocket;

use std::collections::HashMap;
use std::io;
use std::num::NonZeroU32;

use async_process::Command;

use rocket::http::Status;
use rocket::serde::Deserialize;
use rocket::{fairing::AdHoc, State};

use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};

use log::{info, warn};

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct Probe {
    command: String,
    args: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct Config {
    probe_reqs_per_min: Option<NonZeroU32>,
    probes: HashMap<String, Probe>,
}

#[get("/probes/<name>")]
async fn get_probe(
    name: String,
    config: &State<Config>,
    ratelimits: &State<DefaultKeyedRateLimiter<String>>,
) -> (Status, &'static str) {
    if ratelimits.check_key(&name).is_err() {
        return (Status::TooManyRequests, "Too many requests\n");
    }

    let maybe_probe = find_probe(name, config);
    if let Some(probe) = maybe_probe {
        match run_command(probe).await {
            Ok(_) => (Status::Ok, "Ok\n"),
            Err(_) => (Status::ServiceUnavailable, "Failed\n"),
        }
    } else {
        (Status::NotFound, "Not found\n")
    }
}

#[head("/probes/<name>")]
async fn head_probe(
    name: String,
    config: &State<Config>,
    ratelimits: &State<DefaultKeyedRateLimiter<String>>,
) -> Status {
    get_probe(name, config, ratelimits).await.0
}

fn find_probe(name: String, config: &State<Config>) -> Option<&Probe> {
    config
        .probes
        .iter()
        .find(|(pname, _)| **pname == name)
        .map(|(_, p)| p)
}

async fn run_command(probe: &Probe) -> io::Result<()> {
    let args = probe.args.to_owned().unwrap_or(Vec::new());

    info!("Running command: {} {:?}", probe.command, args);
    let output_result = Command::new(&probe.command).args(args).output().await;
    if let Err(command_err) = output_result {
        warn!("Could not execute command: {:?}", command_err);
        return Err(command_err);
    }

    let output = output_result.unwrap();
    info!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
    info!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
    if output.status.success() {
        info!("Command completed successfully");
        Ok(())
    } else {
        warn!("Command exited with code {}", output.status);
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("exited with code {}", output.status),
        ))
    }
}

#[launch]
fn rocket() -> _ {
    let rocket = rocket::build();
    let figment = rocket.figment();
    let config = figment.extract::<Config>().expect("config");
    let quota = config
        .probe_reqs_per_min
        .unwrap_or(NonZeroU32::new(5).unwrap());

    let ratelimits: DefaultKeyedRateLimiter<String> = RateLimiter::keyed(Quota::per_minute(quota));

    rocket
        .manage(ratelimits)
        .mount("/", routes![get_probe, head_probe])
        .attach(AdHoc::config::<Config>())
}
