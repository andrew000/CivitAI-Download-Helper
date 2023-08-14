use std::fs::File;
use std::io;
use std::io::{BufWriter, Error, Seek, Write};
use std::path::Path;
use std::str::FromStr;

use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar};
use log::error;
use reqwest::Client;
use tokio::time::sleep;

use crate::r#const::MAX_TRIES;
use crate::utils::{set_pb_error_style, set_pb_main_style, set_pb_normal_style};

mod r#const;
mod utils;

async fn get_file_info(client: &Client, url: &String) -> (String, u64, String) {
    let res = match client.get(url).send().await {
        Ok(res) => res,
        Err(e) => panic!("Error getting file info: {:?}", e),
    };

    let file_name = match res.headers().get("Content-Disposition") {
        Some(header) => match header.to_str() {
            Ok(header) => header.split("filename=").collect::<Vec<&str>>()[1]
                .to_string()
                .replace("\"", ""),
            Err(e) => panic!("Error converting header to string: {:?}", e),
        },
        None => panic!("Error getting `Content-Disposition` header"),
    };

    let file_size = match res.content_length() {
        Some(file_size) => file_size,
        None => panic!("Error getting file size"),
    };

    return (file_name, file_size, res.url().to_string());
}

fn get_download_ranges(file_size: u64, threads: u8) -> Vec<(u64, u64)> {
    let mut ranges = Vec::new();

    let block_size = file_size / threads as u64;

    for i in 0..threads {
        let start = i as u64 * block_size;
        let end = if i == threads - 1 {
            file_size
        } else {
            (i + 1) as u64 * block_size
        };
        ranges.push((start, end));
    }
    return ranges;
}

async fn download_part<'a>(
    client: &'a Client,
    url: &'a String,
    filename: &'a String,
    start: &'a u64,
    end: &'a u64,
    pb: ProgressBar,
    main_pb: ProgressBar,
) -> Result<(), Error> {
    pb.set_position(0);

    let file = match File::options().write(true).open(&filename) {
        Ok(file) => file,
        Err(e) => panic!("Error opening file: {:?}", e),
    };

    let mut buffer = BufWriter::new(file);

    'downloading: for i in 0..MAX_TRIES {
        buffer.seek(io::SeekFrom::Start(*start)).unwrap();

        let stream = match client
            .get(url)
            .header("Range", format!("bytes={}-{}", start, end))
            .send()
            .await
        {
            Ok(stream) => stream,
            Err(_) => {
                error!(
                    "Error getting stream for range: {}-{}. Try: {}",
                    start, end, i
                );
                sleep(std::time::Duration::from_secs(3)).await;
                continue 'downloading;
            }
        };

        let mut byte_stream = stream.bytes_stream();

        while let Some(item) = byte_stream.next().await {
            match item {
                Ok(data) => match buffer.write_all(&data) {
                    Ok(_) => {
                        pb.inc(data.len() as u64);
                        main_pb.inc(data.len() as u64);
                    }
                    Err(_) => {
                        error!(
                            "Error writing to file for range: {}-{}. Try: {}",
                            start, end, i
                        );
                        main_pb.set_position(main_pb.position() - pb.position());
                        pb.set_position(0);
                        set_pb_error_style(&pb);
                        sleep(std::time::Duration::from_secs(3)).await;
                        continue 'downloading;
                    }
                },
                Err(_) => {
                    error!(
                        "Error getting data for range: {}-{}. Try: {}",
                        start, end, i
                    );
                    main_pb.set_position(main_pb.position() - pb.position());
                    pb.set_position(0);
                    set_pb_error_style(&pb);
                    sleep(std::time::Duration::from_secs(3)).await;
                    continue 'downloading;
                }
            };
        }
        buffer.flush().unwrap();
        drop(buffer);
        return Ok(());
    }
    error!("Max tries exceeded for range: {}-{}", start, end);
    return Err(Error::new(io::ErrorKind::Other, "Max tries exceeded"));
}

async fn start_download_process(
    client: &Client,
    url: &String,
    filename: &String,
    ranges: &Vec<(u64, u64)>,
) {
    if !Path::new(&filename).exists() {
        match File::create(&filename) {
            Ok(file) => file,
            Err(e) => panic!("Error creating file: {:?}", e),
        };
    }

    let multi_progress =
        MultiProgress::with_draw_target(indicatif::ProgressDrawTarget::stdout_with_hz(3));

    let main_pb = ProgressBar::new(ranges.last().unwrap().1 - ranges.first().unwrap().0);
    set_pb_main_style(&main_pb);
    multi_progress.add(main_pb.clone());

    let mut download_futures = Vec::new();

    for (start, end) in ranges {
        let pb = ProgressBar::new(*end - *start);
        set_pb_normal_style(&pb);
        multi_progress.add(pb.clone());
        download_futures.push(download_part(
            &client,
            &url,
            &filename,
            &start,
            &end,
            pb.clone(),
            main_pb.clone(),
        ));
    }

    futures_util::future::join_all(download_futures).await;
}

#[tokio::main]
async fn main() {
    let _handle = utils::setup_logger();

    let url = utils::get_user_input("Enter URL: ");
    let mut threads = utils::get_user_input("Enter number of threads [24]: ");

    if threads.is_empty() {
        threads = "24".to_string();
    }

    let threads = <u8>::from_str(&threads).unwrap();

    let client = utils::create_client();
    let (file_name, file_size, download_url) = get_file_info(&client, &url).await;
    let download_ranges = get_download_ranges(file_size, threads);
    println!("File name: {:?}", file_name);
    println!(
        "File size: {:?}MB | {:?} bytes",
        file_size / 1024 / 1024,
        file_size
    );

    if threads > 1 {
        println!(
            "Ranges #0 - #{}: {:?} - {:?}",
            threads - 1,
            download_ranges.first().unwrap(),
            download_ranges.last().unwrap()
        );
    } else {
        println!("Range #0: {:?}", download_ranges[0]);
    }

    start_download_process(&client, &download_url, &file_name, &download_ranges).await;

    // Calc checksum sha256
    utils::calculate_checksum(&file_name);

    // Wait for user input to exit
    utils::get_user_input("Press any key to exit...");
}
