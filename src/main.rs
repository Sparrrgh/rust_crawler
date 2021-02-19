use chrono::Local;
use futures::future::join_all;
use headless_chrome::{protocol::page::ScreenshotFormat, Browser, LaunchOptionsBuilder, Tab};
use regex::Regex;
use std::fs::{create_dir, write, File};
use std::io::{self, BufRead};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{env, process};
use tokio::net::TcpSocket;
use tokio::task;
use tokio::time::timeout;

fn get_range_sockets(curr_ip: [u8; 4], last_ip: [u8; 4], portlist: Vec<u16>) -> Vec<SocketAddr> {
    let mut endpoints: Vec<SocketAddr> = vec![];
    let mut start_ip = [0_u8; 4];
    start_ip.copy_from_slice(&curr_ip);
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
    let ndiff = as_u32(&last_ip) - as_u32(&start_ip) + 1;
    println!("{} endpoints to test\nGenerating endpoint list...", ndiff);
    let mut a = 1;
    //[TODO] Do while?
    //I don't like it, but the first ip must be pushed. There must be a better way but I'm lazy
    for i in &portlist {
        endpoints.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(start_ip)), *i));
    }
    while a < ndiff {
        start_ip[3] += 1;
        a += 1;
        //Check if the address is invalid
        for b in (0..4).rev() {
            if start_ip[b] == 255_u8 {
                if b != 0 {
                    start_ip[b] = if b == 3 { 1 } else { 0 };
                    start_ip[b - 1] += 1;
                }
                //Skip broadcast blocks
                a += if b == 3 { 2 } else { base.pow((b as u32) ^ 3) };
            }
        }
        for i in &portlist {
            endpoints.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(start_ip)), *i));
        }
    }
    endpoints
}

fn screenshot_endpoint(
    tab: Arc<Tab>,
    endpoint: String,
    path: String,
) -> Result<(), failure::Error> {
    let re = Regex::new(r"^(https?)://")?;
    let png_data = tab
        .set_default_timeout(Duration::from_secs(5))
        .navigate_to(&endpoint)?
        .wait_until_navigated()?
        .capture_screenshot(ScreenshotFormat::PNG, None, true)?;
    //Replace http(s)://, to not break the filename
    let finpath = format!(
        "{}/{}-{}.png",
        path,
        re.replace(&endpoint, ""),
        Local::now()
    );
    // println!("{}", finpath);
    let _ = write(finpath, png_data)?;
    Ok(())
}

async fn visit_endpoint(
    endpoint: SocketAddr,
    browser: Arc<Mutex<Browser>>,
    path: String,
) -> Result<(), failure::Error> {
    let socket = TcpSocket::new_v4()?;
    if timeout(Duration::from_secs(2), socket.connect(endpoint))
        .await
        .is_ok()
    {
        println!("Trying to screenshot {}", &endpoint);
        //Visiting using both HTTP and HTTPS
        for h in ["http", "https"].iter() {
            //Unlock the browser's mutex, create a new tab and use it to screenshot the page
            //I'm pretty sure this causes some trouble in case you visit the page with the incorrect protocol...
            //Probably the connection is closed and subsequently the request using HTTPS won't go through
            let tab = browser.lock().unwrap().new_tab().unwrap();
            let endpoint_s = format!("{}://{}", h, &endpoint);
            let cpath = path.clone();
            let _ = task::spawn_blocking(|| {
                let _ = screenshot_endpoint(tab, endpoint_s, cpath);
            })
            .await;
        }
    }
    Ok(())
}

#[tokio::main]
async fn screenshot_block(endpoints: Vec<SocketAddr>, path: String) -> Result<(), failure::Error> {
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
        //Spawn a task for each endpoint to visit, push the future returned in the vector to join later
        let f = task::spawn(visit_endpoint(e, carc_browser, cpath));
        fut.push(f);
    }
    let _ = join_all(fut).await;
    Ok(())
}

fn main() {
    //TODO Add port range/list
    let args: Vec<String> = env::args().collect();
    let mut endpoints: Vec<SocketAddr> = vec![];
    let mut path: String = String::default();
    if args.len() < 3 || args.len() > 5 {
        println!("Usage:  ./crawler endpoints_file output_directory\n\t./crawler start_address_block end_address_block port1,port2,...,portn output_directory");
        process::exit(-1);
    } else if args.len() == 3 {
        //Get ip addresses and ports from file
        let file = File::open(args[1].to_string());
        path = args[2].to_string();
        let file = match file {
            Ok(file) => file,
            Err(_) => {
                println!("File does not exist");
                process::exit(-1);
            }
        };
        for line in io::BufReader::new(file).lines() {
            endpoints.push(line.unwrap().parse().unwrap());
        }
    } else {
        //Test range of ip addresses
        let ip1_str = args[1].to_string();
        let ip2_str = args[2].to_string();
        let portlist = args[3]
            .to_string()
            .split(',')
            .map(|x| x.parse::<u16>().expect("Invalid port"))
            .collect::<Vec<u16>>();
        path = args[4].to_string();
        let verify_ip = |ip: String| -> Vec<u8> {
            ip.split('.')
                .map(|x| x.parse::<u8>().expect("Invalid IP address"))
                .filter(|x| *x < 255_u8)
                .collect::<Vec<u8>>()
        };
        let ip1 = verify_ip(ip1_str);
        let ip2 = verify_ip(ip2_str);
        if ip1.iter().count() != 4 || ip1.iter().count() != 4 {
            println!("Invalid IP address");
            process::exit(-1);
        }
        //Convert vectors to arrays
        let mut curr_ip = [0_u8; 4];
        let mut last_ip = [0_u8; 4];
        curr_ip[..4].clone_from_slice(&ip1[..4]);
        last_ip[..4].clone_from_slice(&ip2[..4]);
        for i in (0..4).step_by(3) {
            if ip1[i] == 0 || ip2[i] == 0 {
                println!("Invalid IP address");
                process::exit(-1);
            }
        }
        //Create sockets for the range of ip addresses/ports
        endpoints = get_range_sockets(curr_ip, last_ip, portlist);
    };
    println!("List generated, visting endpoints...");
    let _ = screenshot_block(endpoints, path);
}
