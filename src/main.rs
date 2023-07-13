#[macro_use] extern crate rocket;

use std::io;
use std::collections::HashMap;
use async_process::Command;

use rocket::http::Status;
use rocket::serde::Deserialize;
use rocket::{State, fairing::AdHoc};

use rocket_governor::{Method, Quota, RocketGovernable, RocketGovernor};

use log::{info,warn};

pub struct RateLimitGuard;

impl<'r> RocketGovernable<'r> for RateLimitGuard {
    fn quota(_method: Method, _route_name: &str) -> Quota {
        Quota::per_minute(Self::nonzero(5u32))
    }
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct Probe {
    command: String,
    args: Option<Vec<String>>
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct Config {
    probes: HashMap<String, Probe>
}

#[get("/probes/<name>")]
async fn get_probe(name: String, config: &State<Config>, _limit: RocketGovernor<'_, RateLimitGuard>) -> (Status, &'static str) {
    let maybe_probe = find_probe(name, config);
    if let Some(probe) = maybe_probe {
        match run_command(probe).await {
            Ok(_) => (Status::Ok, "Ok\n"),
            Err(_) => (Status::ServiceUnavailable, "Failed\n")
        }
    } else {
        (Status::NotFound, "Not found\n")
    }
}

#[head("/probes/<name>")]
async fn head_probe(name: String, config: &State<Config>, limit: RocketGovernor<'_, RateLimitGuard>) -> Status {
    get_probe(name, config, limit).await.0
}

fn find_probe(name: String, config: &State<Config>) -> Option<&Probe> {
    config.probes.iter().find(|(pname, _)| **pname == name).map(|(_, p)| p)
}

async fn run_command(probe: &Probe) -> io::Result<()> {
    let args = probe.args.to_owned().unwrap_or(Vec::new());

    info!("Running command: {} {:?}", probe.command, args);
    let output_result = Command::new(&probe.command)
            .args(args)
            .output()
            .await;
    if let Err(command_err) = output_result {
        warn!("Could not execute command: {:?}", command_err);
        return Err(command_err)
    }

    let output = output_result.unwrap();
    info!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
    info!("Stdout: {}", String::from_utf8_lossy(&output.stdout));    
    if output.status.success() {
        info!("Command completed successfully");
        Ok(())
    } else {
        warn!("Command exited with code {}", output.status);
        Err(io::Error::new(io::ErrorKind::Other, format!("exited with code {}", output.status)))
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![get_probe, head_probe])
        .attach(AdHoc::config::<Config>())
}