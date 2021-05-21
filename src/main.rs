use fantoccini::{Client, ClientBuilder};
use futures::{future::join_all, lock::Mutex};
use serde_json::json;
use std::fs::{create_dir, write, File};
use std::io::{self, BufRead};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use std::{env, process};
use tokio::{net::TcpSocket, task, time::timeout};
use webdriver::capabilities::Capabilities;

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

//Unlock the browser's mutex, create a new tab and use it to screenshot the page
async fn screenshot_endpoint(
    client: Arc<Mutex<Client>>,
    endpoint: String,
    path: String,
) -> Result<(), failure::Error> {
    let mut unlocked_client = client.lock().await;
    unlocked_client.goto(&endpoint).await?;
    let png_data = unlocked_client.screenshot().await?;
    //Dropping unlocked mutex, hoping for some perf
    drop(unlocked_client);
    let finpath = format!("{}/{}.png", path, endpoint.replace("://", "_"));
    let _ = write(finpath, png_data)?;
    Ok(())
}

async fn visit_endpoint(
    endpoint: SocketAddr,
    client: Arc<Mutex<Client>>,
    path: String,
) -> Result<(), failure::Error> {
    let socket = TcpSocket::new_v4()?;
    //Try and connect, if succeeds try and screenshot using the browser
    if timeout(Duration::from_secs(2), socket.connect(endpoint))
        .await
        .is_ok()
    {
        println!("Trying to screenshot {}", &endpoint);
        //Visiting using both HTTP and HTTPS
        for h in ["http", "https"].iter() {
            //[TODO] Using the browser to test both HTTP and HTTPS kills perf, should find a way
            //to test which one should I use before calling the browser
            let endpoint_s = format!("{}://{}", h, &endpoint);
            let cpath = path.clone();
            let cclient = client.clone();
            //Ignore the panic
            screenshot_endpoint(cclient, endpoint_s, cpath).await.ok();
        }
    }
    Ok(())
}

async fn screenshot_block(endpoints: Vec<SocketAddr>, path: String) -> Result<(), failure::Error> {
    //Create directory where the screenshots will be saved
    if create_dir(path.to_string()).is_err() {
        println!("Directory \"{}\" already exists", path);
        process::exit(-1);
    }
    let mut cap = Capabilities::new();
    // I don't care if an endpoint's certificate is not valid, just screenshot it
    cap.insert("acceptInsecureCerts".to_owned(), json!(true));
    //[TODO] Timeout should be customizable
    //Timeout set to 5 seconds
    cap.insert("timeouts".to_owned(), json!({"pageLoad":5000}));
    //Firefox only
    cap.insert(
        "moz:firefoxOptions".to_owned(),
        json!({"args":["-headless"]}),
    );
    //Create client
    let mut cbuilder = ClientBuilder::native();
    cbuilder.capabilities(cap);
    let mut client = match cbuilder.connect("http://localhost:4444").await {
        Ok(c) => c,
        Err(_) => {
            println!("Could not connect to webdriver");
            process::exit(-1);
        }
    };
    //Client will be closed manually
    client.persist().await?;
    let mut fut = vec![];
    let arc_client = Arc::new(Mutex::new(client));
    for e in endpoints {
        let carc_client = arc_client.clone();
        let cpath = path.clone();
        //Spawn a task for each endpoint to visit, push the future returned in the vector to join later
        let f = task::spawn(visit_endpoint(e, carc_client, cpath));
        fut.push(f);
    }
    let _ = join_all(fut).await;
    //Close client after we're done
    arc_client.lock().await.close().await?;
    println!("Client closed, scan finished");
    Ok(())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let mut endpoints: Vec<SocketAddr> = vec![];
    let mut path: String = String::default();
    if args.len() < 3 || args.len() > 5 {
        println!("Usage:  ./webserver_finder endpoints_file output_directory\n\t./webserver_finder start_address_block end_address_block port1,port2,...,portn output_directory");
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
    let _ = screenshot_block(endpoints, path).await;
}
