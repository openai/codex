use clap::Command;

fn main() {
    let cmd = Command::new(env!("CARGO_CRATE_NAME"))
        .multicall(true)
        .arg_required_else_help(true)
        .subcommand_value_name("APPLET")
        .subcommand_help_heading("APPLETS")
        .subcommand(Command::new("hostname").about("show hostname part of FQDN"))
        .subcommand(Command::new("dnsdomainname").about("show domain name part of FQDN"));

    match cmd.get_matches().subcommand_name() {
        Some("hostname") => println!("www"),
        Some("dnsdomainname") => println!("example.com"),
        _ => unreachable!("parser should ensure only valid subcommand names are used"),
    }
}
