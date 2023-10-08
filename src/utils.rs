use std::io::{stdin, stdout, Write};
use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};
use log4rs::{Config, Handle};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log::LevelFilter;
use reqwest::Client;
use sha256::try_digest;

use crate::r#const;

pub fn get_user_input(text: &str) -> String {
    stdout().write_all(text.as_ref()).unwrap();
    stdout().flush().unwrap();

    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();

    return input.trim().to_string();
}

pub fn create_client() -> Client {
    let client = Client::builder().user_agent(r#const::HEADER).build();

    return match client {
        Ok(client) => client,
        Err(e) => panic!("Error creating client: {:?}", e),
    };
}

pub fn set_pb_main_style(pb: &ProgressBar) {
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.green/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));
}

pub fn set_pb_normal_style(pb: &ProgressBar) {
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));
}

pub fn set_pb_error_style(pb: &ProgressBar) {
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.red} [{elapsed_precise}] [{bar:40.red/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"));
}

pub fn calculate_checksum(file_name: &String) {
    // Ask user for check checksum
    let answer = get_user_input("Do you want to calculate the SHA256 checksum? [y/N]: ");

    if answer.is_empty() || answer.to_lowercase() != "y" {
        return;
    }

    println!("Calculating checksum...");
    let path = Path::new(&file_name);
    let checksum = try_digest(path).unwrap();
    println!("SHA256 Checksum: {:?}", checksum);
}

pub fn setup_logger() -> Handle {
    let file_logger = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)(utc)} - {h({l})}: {m}{n}",
        )))
        .build("normal_logs.log")
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("file_logger", Box::new(file_logger)))
        .build(
            Root::builder()
                .appender("file_logger")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    let handle = log4rs::init_config(config).unwrap();

    return handle;
}
