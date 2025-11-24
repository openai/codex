// SPDX-License-Identifier: Apache-2.0 OR MIT

// This is an idiomatic Rust rewrite of a C example:
// https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git/tree/samples/landlock/sandboxer.c

use anyhow::{anyhow, bail, Context};
use landlock::{
    path_beneath_rules, Access, AccessFs, AccessNet, BitFlags, NetPort, PathBeneath, PathFd,
    Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus, Scope, ABI,
};
use std::env;
use std::ffi::OsStr;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::CommandExt;
use std::process::Command;

const ENV_FS_RO_NAME: &str = "LL_FS_RO";
const ENV_FS_RW_NAME: &str = "LL_FS_RW";
const ENV_TCP_BIND_NAME: &str = "LL_TCP_BIND";
const ENV_TCP_CONNECT_NAME: &str = "LL_TCP_CONNECT";
const ENV_SCOPED_NAME: &str = "LL_SCOPED";

struct PathEnv {
    paths: Vec<u8>,
    access: BitFlags<AccessFs>,
}

impl PathEnv {
    /// Create an object able to iterate PathBeneath rules
    ///
    /// # Arguments
    ///
    /// * `name`: String identifying an environment variable containing paths requested to be
    ///   allowed. Paths are separated with ":", e.g. "/bin:/lib:/usr:/proc". In case an empty
    ///   string is provided, NO restrictions are applied.
    /// * `access`: Set of access-rights allowed for each of the parsed paths.
    fn new<'a>(name: &'a str, access: BitFlags<AccessFs>) -> anyhow::Result<Self> {
        Ok(Self {
            paths: env::var_os(name)
                .ok_or(anyhow!("missing environment variable {name}"))?
                .into_vec(),
            access,
        })
    }

    fn iter(&self) -> impl Iterator<Item = anyhow::Result<PathBeneath<PathFd>>> + '_ {
        let is_empty = self.paths.is_empty();
        path_beneath_rules(
            self.paths
                .split(|b| *b == b':')
                // Skips the first empty element of an empty string.
                .skip_while(move |_| is_empty)
                .map(OsStr::from_bytes),
            self.access,
        )
        .map(|r| Ok(r?))
    }
}

struct PortEnv {
    ports: Vec<u8>,
    access: AccessNet,
}

impl PortEnv {
    fn new<'a>(name: &'a str, access: AccessNet) -> anyhow::Result<Self> {
        Ok(Self {
            ports: env::var_os(name).unwrap_or_default().into_vec(),
            access,
        })
    }

    fn iter(&self) -> impl Iterator<Item = anyhow::Result<NetPort>> + '_ {
        let is_empty = self.ports.is_empty();
        self.ports
            .split(|b| *b == b':')
            // Skips the first empty element of an empty string.
            .skip_while(move |_| is_empty)
            .map(OsStr::from_bytes)
            .map(|port| {
                let port = port
                    .to_str()
                    .context("failed to convert port string")?
                    .parse::<u16>()
                    .context("failed to convert port to 16-bit integer")?;
                Ok(NetPort::new(port, self.access))
            })
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = env::args_os();
    let program_name = args
        .next()
        .context("Missing the sandboxer program name (i.e. argv[0])")?;

    let cmd_name = args.next().ok_or_else(|| {
        let program_name = program_name.to_string_lossy();
        eprintln!(
            "usage: {ENV_FS_RO_NAME}=\"...\" {ENV_FS_RW_NAME}=\"...\" [other environment variables] {program_name} <cmd> [args]...\n"
        );
        eprintln!("Execute the given command in a restricted environment.");
        eprintln!("Multi-valued settings (lists of ports, paths, scopes) are colon-delimited.\n");
        eprintln!("Mandatory settings:");
        eprintln!("* {ENV_FS_RO_NAME}: paths allowed to be used in a read-only way");
        eprintln!("* {ENV_FS_RW_NAME}: paths allowed to be used in a read-write way\n");
        eprintln!("Optional settings (when not set, their associated access check is always allowed, which is different from an empty string which means an empty list):");
        eprintln!("* {ENV_TCP_BIND_NAME}: ports allowed to bind (server)");
        eprintln!("* {ENV_TCP_CONNECT_NAME}: ports allowed to connect (client)");
        eprintln!("* {ENV_SCOPED_NAME}: actions denied on the outside of the Landlock domain:");
        eprintln!("  - \"a\" to restrict opening abstract unix sockets");
        eprintln!("  - \"s\" to restrict sending signals");
        eprintln!(
            "\nExample:\n\
                {ENV_FS_RO_NAME}=\"${{PATH}}:/lib:/usr:/proc:/etc:/dev/urandom\" \
                {ENV_FS_RW_NAME}=\"/dev/null:/dev/full:/dev/zero:/dev/pts:/tmp\" \
                {ENV_TCP_BIND_NAME}=\"9418\" \
                {ENV_TCP_CONNECT_NAME}=\"80:443\" \
                {ENV_SCOPED_NAME}=\"a:s\" \
                {program_name} bash -i\n"
        );
        anyhow!("Missing command")
    })?;

    let abi = ABI::V6;
    let mut ruleset = Ruleset::default().handle_access(AccessFs::from_all(abi))?;
    let ruleset_ref = &mut ruleset;

    if env::var_os(ENV_TCP_BIND_NAME).is_some() {
        ruleset_ref.handle_access(AccessNet::BindTcp)?;
    }
    if env::var_os(ENV_TCP_CONNECT_NAME).is_some() {
        ruleset_ref.handle_access(AccessNet::ConnectTcp)?;
    }

    if let Some(scoped) = env::var_os(ENV_SCOPED_NAME) {
        let mut abstract_scoping = false;
        let mut signal_scoping = false;
        let scopes = scoped.to_string_lossy();
        let is_empty = scopes.is_empty();
        for scope in scopes.split(':').skip_while(move |_| is_empty) {
            match scope {
                "a" => {
                    if abstract_scoping {
                        bail!("Duplicate scope 'a'");
                    }
                    ruleset_ref.scope(Scope::AbstractUnixSocket)?;
                    abstract_scoping = true;
                }
                "s" => {
                    if signal_scoping {
                        bail!("Duplicate scope 's'");
                    }
                    ruleset_ref.scope(Scope::Signal)?;
                    signal_scoping = true;
                }
                _ => bail!("Unknown scope \"{scope}\""),
            }
        }
    }

    let status = ruleset
        .create()?
        .add_rules(PathEnv::new(ENV_FS_RO_NAME, AccessFs::from_read(abi))?.iter())?
        .add_rules(PathEnv::new(ENV_FS_RW_NAME, AccessFs::from_all(abi))?.iter())?
        .add_rules(PortEnv::new(ENV_TCP_BIND_NAME, AccessNet::BindTcp)?.iter())?
        .add_rules(PortEnv::new(ENV_TCP_CONNECT_NAME, AccessNet::ConnectTcp)?.iter())?
        .restrict_self()
        .expect("Failed to enforce ruleset");

    if status.ruleset == RulesetStatus::NotEnforced {
        bail!("Landlock is not supported by the running kernel.");
    }

    eprintln!("Executing the sandboxed command...");
    Err(Command::new(cmd_name)
        .env_remove(ENV_FS_RO_NAME)
        .env_remove(ENV_FS_RW_NAME)
        .env_remove(ENV_TCP_BIND_NAME)
        .env_remove(ENV_TCP_CONNECT_NAME)
        .args(args)
        .exec()
        .into())
}
