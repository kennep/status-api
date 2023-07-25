# status-api

This is a very simple [Rocket](https://rocket.rs) application that can return the
result of various system checks as HTTP status codes. I use it to monitor the
state of "internal" services (i.e. services that are not exposed to the public
internet) using monitoring services such as [Uptime Robot](https://uptimerobot.com).

## Installing

### From built releases

Binaries for Linux and Windows can be downloaded from the [Releases](releases).

### From source

As this is a Rust program, it can be built using `cargo build` as usual.

## Configuration

The service is configured with additional options in the [Rocket.toml](https://rocket.rs/v0.5-rc/guide/configuration/)
file, which must be present in the application's current directory.

In the examples below, we use the `default` profile, but it's of course possible
to use any other Rocket configuration profile.

### Probes


The status checks are defined as a series of _probes_, where each probe has both
a name and a command to be run. For example, to define a probe called `hostname`
that runs the command `hostname` with no argument, the following configuration
section can be used:

```toml
[default.probes.hostname]
command = "hostname"
args = []
```

This will expose the endpoint `/probes/hostname` which you can call using
either HTTP `GET` or `HEAD`. The endpoint will return `200 OK` if the `hostname`
command was successfully run, and `503 Service Unavailable` if the `hostname`
command failed to run or exited with an error.

If the command to be run does not include the full path, it is looked up in
`PATH`.

Note that the commands are _not_ run through the shell, though this is possible
to arrange by specifying the shell as the command to run with the appropriate
arguments; for example (for Linux):

```toml
[default.probes.lastlog]
command = "/bin/sh"
args = ["-c", "find /var/log/lastlog -mmin -60|grep lastlog"]
```

Since the endpoints are unauthenticated, they intentionally do not expose any
more information than OK/success, though the output and exit status of the command
will be printed to the log.

### Rate limiting

Since running external commands is a relatively heavywheight operation, the service rate limits requests. By default, the rate limit is a maximum of 5 requests per minute per probe, though this can be changed by setting the `probe_reqs_per_min` option
as in the following example:

```toml
[default]
probe_reqs_per_min=1
```

This will set the rate limit to 1 request per minute per probe.
Note that it is not possible to define different rate limits per probe.
