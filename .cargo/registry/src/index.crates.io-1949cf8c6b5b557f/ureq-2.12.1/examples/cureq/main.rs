use std::error;
use std::fmt;
use std::io;
use std::thread;
use std::time::Duration;
use std::{env, sync::Arc};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::ClientConfig;
use rustls_pki_types::{CertificateDer, ServerName, UnixTime};
use ureq;

#[derive(Debug)]
struct StringError(String);

impl error::Error for StringError {}

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for StringError {
    fn from(source: String) -> Self {
        Self(source)
    }
}

#[derive(Debug)]
struct Error {
    source: Box<dyn error::Error>,
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl From<StringError> for Error {
    fn from(source: StringError) -> Self {
        Error {
            source: source.into(),
        }
    }
}

impl From<ureq::Error> for Error {
    fn from(source: ureq::Error) -> Self {
        Error {
            source: source.into(),
        }
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Error {
            source: source.into(),
        }
    }
}

fn perform(
    agent: &ureq::Agent,
    method: &str,
    url: &str,
    data: &[u8],
    print_headers: bool,
) -> Result<(), Error> {
    let req = agent.request(method, url);
    let response = if method == "GET" && data.len() == 0 {
        req.call()?
    } else {
        req.send_bytes(data)?
    };
    if print_headers {
        println!(
            "{} {} {}",
            response.http_version(),
            response.status(),
            response.status_text()
        );
        for h in response.headers_names() {
            println!("{}: {}", h, response.header(&h).unwrap_or_default());
        }
        println!();
    }
    let mut reader = response.into_reader();
    io::copy(&mut reader, &mut io::stdout())?;
    Ok(())
}

#[derive(Debug)]
struct AcceptAll {}

impl ServerCertVerifier for AcceptAll {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer,
        _intermediates: &[CertificateDer],
        _server_name: &ServerName,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        todo!()
    }
}

fn main() {
    match main2() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn main2() -> Result<(), Error> {
    let mut args = env::args();
    if args.next().is_none() {
        println!(
            r##"Usage: {:#?} url [url ...]

-i            Include headers when printing response
-X <method>   Use the given request method (GET, POST, etc)
-d <string>   Use the given data as the request body (useful for POST)
--wait <n>    Wait n seconds between requests
-k            Ignore certificate errors
-m <time>     Max time for the entire request
-ct <time>    Connection timeout
--native-tls  Use native-tls

Fetch url and copy it to stdout.
"##,
            env::current_exe()?
        );
        return Ok(());
    }
    env_logger::init();
    let mut builder = ureq::builder()
        .timeout_connect(Duration::from_secs(30))
        .timeout(Duration::from_secs(300));
    let mut nonflags: Vec<String> = vec![];

    let mut print_headers: bool = false;
    let mut method: String = "GET".into();
    let mut data: Vec<u8> = vec![];
    let mut wait = Duration::new(0, 0);
    while let Some(arg) = args.next() {
        match arg.as_ref() {
            "-i" => print_headers = true,
            "-X" => method = args.next().expect("flag -X requires a value"),
            "-d" => data = args.next().expect("flag -d requires a value").into(),
            "--wait" => {
                let wait_string: String = args.next().expect("flag --wait requires a value").into();
                let wait_seconds: u64 = wait_string.parse().expect("invalid --wait flag");
                wait = Duration::from_secs(wait_seconds);
            }
            "-k" => {
                let client_config = ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(Arc::new(AcceptAll {}))
                    .with_no_client_auth();
                builder = builder.tls_config(Arc::new(client_config));
            }
            "--native-tls" => {
                builder = builder.tls_connector(Arc::new(native_tls::TlsConnector::new().unwrap()));
            }
            "-m" => {
                let t: f32 = args
                    .next()
                    .expect("Timeout arg")
                    .parse()
                    .expect("Parse timeout");

                builder = builder.timeout(Duration::from_millis((t * 1000.0) as u64));
            }
            "-ct" => {
                let t: f32 = args
                    .next()
                    .expect("Connection timeout arg")
                    .parse()
                    .expect("Parse connection timeout");

                builder = builder.timeout_connect(Duration::from_millis((t * 1000.0) as u64));
            }
            arg => {
                if arg.starts_with("-") {
                    Err(StringError(format!("unrecognized flag '{}'", arg)))?;
                }
                nonflags.push(arg.to_owned());
            }
        }
    }

    let agent = builder.build();

    for (i, url) in nonflags.iter().enumerate() {
        perform(&agent, &method, &url, &data, print_headers)?;
        if i != nonflags.len() - 1 {
            thread::sleep(wait);
        }
    }
    Ok(())
}
