use chrono::Local;
use futures::future::join_all;
use headless_chrome::{protocol::page::ScreenshotFormat, Browser, LaunchOptionsBuilder, Tab};
use hyper::Client;
use hyper_timeout::TimeoutConnector;
use hyper_tls::HttpsConnector;
use regex::Regex;
use std::fs::{create_dir, write, File};
use std::io::{self, BufRead};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, process};
use tokio::task;

fn screenshot_site(tab: Arc<Tab>, site: String, path: String) -> Result<(), failure::Error> {
    let re = Regex::new(r"^(https?)://")?;
    let png_data = tab
        .set_default_timeout(Duration::from_secs(5))
        .navigate_to(&site)?
        .wait_until_navigated()?
        .capture_screenshot(ScreenshotFormat::PNG, None, true)?;
    //Replace http(s), to not break the filename
    let finpath = format!("{}/{}-{}.png", path, re.replace(&site, ""), Local::now());
    // println!("{}", finpath);
    let _ = write(finpath, png_data)?;
    Ok(())
}

async fn visit_site(
    site: String,
    browser: Arc<Mutex<Browser>>,
    path: String,
) -> Result<(), failure::Error> {
    //Set timeouts for hyper-timeout
    let mut connector = TimeoutConnector::new(HttpsConnector::new());
    connector.set_connect_timeout(Some(Duration::from_secs(5)));
    connector.set_read_timeout(Some(Duration::from_secs(10)));
    connector.set_write_timeout(Some(Duration::from_secs(10)));
    let client = Client::builder().build::<_, hyper::Body>(connector);
    let uri = site.parse()?;
    let _resp = match client.get(uri).await {
        Ok(..) => {
            // println!("Visiting {}", &site);
            //Unlock the browser's mutex, create a new tab and use it to screenshot the page
            let tab = browser.lock().unwrap().new_tab().unwrap();
            let _ = task::spawn_blocking(|| {
                let _ = screenshot_site(tab, site, path);
            })
            .await;
        }
        //This code sucks and probably the perf is horrible, but I couldn't find how to match io::ErrorKind::TimedOut
        //TODO: Find a way to use `io::ErrorKind::TimedOut`
        Err(e) => {
            if e.into_cause().unwrap().to_string() == "deadline has elapsed" {
                println!("{} timed out", &site)
            }
        }
    };
    Ok(())
}

#[tokio::main]
async fn screenshot_block(endpoints: Vec<String>, path: String) -> Result<(), failure::Error> {
    let browser = Browser::new(LaunchOptionsBuilder::default().build().unwrap())?;
    let arc_browser = Arc::new(Mutex::new(browser));
    let mut fut = vec![];
    //Create directory where the screenshots will be saved
    if create_dir(path.to_string()).is_err() {
        println!("Directory \"{}\" already exists", path);
        process::exit(-1);
    }
    for e in endpoints {
        let carc_browser = arc_browser.clone();
        let cpath = path.clone();
        //Spawn a task for each site to visit, push the future returned in the vector to join later
        let f = task::spawn(visit_site(e, carc_browser, cpath));
        fut.push(f);
    }
    let _ = join_all(fut).await;
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut endpoints: Vec<String> = vec![];
    let path = if args.len() < 3 || args.len() > 4 {
        println!("Usage:  ./crawler endpoints_file output_directory\n\t./crawler start_address_block end_address_block output_directory");
        process::exit(-1);
    } else if args.len() == 3 {
        let file = File::open(args[1].to_string());
        let file = match file {
            Ok(file) => file,
            Err(_) => {
                println!("File does not exist");
                process::exit(-1);
            }
        };
        for line in io::BufReader::new(file).lines() {
            endpoints.push(line.unwrap());
        }
        args[2].to_string()
    } else {
        //Give the user the ability to test from an IP address to another
        let ip1_str = args[1].to_string();
        let ip2_str = args[2].to_string();
        let verify_ip = |ip: String| -> Vec<u8> {
            ip.split('.')
                .map(|x| x.parse::<u8>().unwrap())
                .filter(|x| *x < 255_u8)
                .collect::<Vec<u8>>()
        };
        let ip1 = verify_ip(ip1_str);
        let ip2 = verify_ip(ip2_str);
        if ip1.iter().count() != 4 || ip1.iter().count() != 4{
            println!("Invalid IP address");
            process::exit(-1);
        }
        //Convert vectors to arrays
        let mut curr_ip = [0_u8;4];
        let mut last_ip = [0_u8;4];
        curr_ip[..4].clone_from_slice(&ip1[..4]);
        last_ip[..4].clone_from_slice(&ip2[..4]);
        for i in (0..4).step_by(3) {
            if ip1[i] == 0 || ip2[i] == 0 {
                println!("Invalid IP address");
                process::exit(-1);
            }
        }
        let base = 256_u32;
        //Convert an array of 4 u8 to a u32
        let as_u32 = |array: &[u8; 4]| -> u32 {
            let mut res: u32 = 0;
            for i in 0..4 {
                res |= (array[i] as u32) << (8 * (i ^ 3));
            }
            res
        };
        //Calculate difference between the ip addresses
        //+1 to comprehend the last ip of the block
        let ndiff = as_u32(&last_ip) - as_u32(&curr_ip) + 1;
        println!("{} endpoints to test\nGenerating endpoint list...", ndiff);
        let build_url = |arr: [u8; 4]| -> String {
            format!("http://{}.{}.{}.{}", arr[0], arr[1], arr[2], arr[3])
        };
        let mut a = 1;
        //I don't like it, but the first ip must be pushed. There must be a better way but I'm lazy
        endpoints.push(build_url(curr_ip));
        while a < ndiff {
            curr_ip[3] += 1;
            a += 1;
            //Check if the address is invalid
            for b in (0..4).rev() {
                if curr_ip[b] == 255_u8 {
                    if b != 0 {
                        curr_ip[b] = if b == 3 { 1 } else { 0 };
                        curr_ip[b - 1] += 1;
                    }
                    //Skip broadcast blocks
                    a += if b == 3 { 2 } else { base.pow((b as u32) ^ 3) };
                }
            }
            endpoints.push(build_url(curr_ip));
        }
        args[3].to_string()
    };
    println!("List generated, visting endpoints...");
    let _ = screenshot_block(endpoints, path);
}
