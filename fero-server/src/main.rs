#[macro_use]
extern crate clap;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate failure;
extern crate fero_proto;
extern crate futures;
extern crate gpgme;
extern crate grpcio;
extern crate libyubihsm;
#[macro_use]
extern crate log;
extern crate loggerv;
extern crate num;
extern crate pretty_good;
extern crate protobuf;
extern crate yasna;

mod database;
mod hsm;
mod service;

use std::io::{self, Read};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

use clap::{Arg, App};
use failure::Error;
use futures::Future;
use futures::sync::oneshot;
use grpcio::{Environment, Server, ServerBuilder};

use fero_proto::fero_grpc::create_fero;

fn create_server(
    address: &str,
    port: u16,
    database: &str,
    hsm_connector: &str,
    hsm_authkey: u16,
    hsm_password: &str,
) -> Result<Server, Error> {
    ServerBuilder::new(Arc::new(Environment::new(1)))
        .register_service(create_fero(service::FeroService::new(
            database::Configuration::new(database),
            hsm::HsmSigner::new(hsm_connector, hsm_authkey, hsm_password)?,
        )))
        .bind(address, port)
        .build()
        .map_err(|e| e.into())
}

pub fn main() {
    if let Err(e) = run() {
        println!("{:?}", e);
        ::std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let args = App::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .arg(
            Arg::with_name("ADDRESS")
                .short("a")
                .long("address")
                .takes_value(true)
                .help("The address on which to listen")
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("PORT")
                .short("p")
                .long("port")
                .takes_value(true)
                .help("The port on which to bind")
                .default_value("50051"),
        )
        .arg(
            Arg::with_name("DATABASE")
                .short("d")
                .long("database")
                .takes_value(true)
                .help("The path to the sqlite database")
                .default_value("fero.db"),
        )
        .arg(
            Arg::with_name("VERBOSITY")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("The level of verbosity"),
        )
        .arg(
            Arg::with_name("HSM_CONNECTOR_URL")
                .short("c")
                .long("connector-url")
                .takes_value(true)
                .default_value("http://127.0.0.1:12345")
                .help("The URL for the HSM connector"),
        )
        .arg(
            Arg::with_name("HSM_AUTHKEY")
                .short("k")
                .long("authkey")
                .takes_value(true)
                .help("The YubiHSM2 AuthKey to use")
        )
        .arg(
            Arg::with_name("HSM_PASSWORD")
                .short("w")
                .long("password")
                .takes_value(true)
                .help("The password for the HSM AuthKey")
        )
        .get_matches();

    loggerv::init_with_verbosity(args.occurrences_of("VERBOSITY")).unwrap();

    let address = args.value_of("ADDRESS").expect("address flag");
    let port = u16::from_str(args.value_of("PORT").expect("port flag"))?;

    let database = args.value_of("DATABASE").expect("database flag");

    let hsm_connector_url = args.value_of("HSM_CONNECTOR_URL").expect("connector URL flag");
    let hsm_authkey = u16::from_str(args.value_of("HSM_AUTHKEY").expect("authkey flag"))?;
    let hsm_password = args.value_of("HSM_PASSWORD").expect("HSM password flag");

    let mut server = create_server(
        address,
        port,
        database,
        hsm_connector_url,
        hsm_authkey,
        hsm_password,
    )?;

    server.start();
    let (tx, rx) = oneshot::channel();
    thread::spawn(move || {
        warn!("Press ENTER to exit...");
        let _ = io::stdin().read(&mut [0]).unwrap();
        tx.send(())
    });
    let _ = rx.wait();
    server.shutdown().wait()?;

    Ok(())
}