use headless_chrome::{Browser, protocol::page::ScreenshotFormat, LaunchOptionsBuilder, Tab};
use std::fs::{File,write,create_dir};
use chrono::{Local};
use std::io::{self, BufRead};
use failure;
use std::{process,env};
use futures::future::join_all;
use std::sync::{Arc,Mutex};
use regex::Regex;
use hyper::{Client};
use tokio::{fs,task};
use hyper_tls::HttpsConnector;
use hyper_timeout::TimeoutConnector;
use std::time::Duration;

fn screenshot_site(tab: Arc<Tab>, site: String, path: String) -> Result<(),failure::Error>{
    let re = Regex::new(r"^(https?)://")?;
    let png_data = tab.set_default_timeout(Duration::from_secs(5))
    .navigate_to(&site)?
    .capture_screenshot(ScreenshotFormat::PNG, None, true)?;
    //Replace http(s), to not break the filename
    let finpath = format!("{}/{}{}.png", path, re.replace(&site, ""),Local::now());
    println!("{}", finpath);
    //TODO: Use tokio fs
    let _ = write(finpath, png_data)?;
    Ok(())
}

async fn visit_site(site: String, browser: Arc<Mutex<Browser>>, path: String) -> Result<(),failure::Error>{
    //Set timeouts for hyper-timeout
    let mut connector = TimeoutConnector::new(HttpsConnector::new());
    connector.set_connect_timeout(Some(Duration::from_secs(5)));
    connector.set_read_timeout(Some(Duration::from_secs(10)));
    connector.set_write_timeout(Some(Duration::from_secs(10)));
    let client = Client::builder().build::<_, hyper::Body>(connector);
    let uri = site.parse()?;
    let _resp = match client.get(uri).await{
        Ok(..) => {
            println!("Visiting {}", &site);
            //Unlock the browser's mutex, create a new tab and use it to screenshot the page
            let tab = browser.lock().unwrap().new_tab().unwrap();
            let _ = task::spawn_blocking(|| {
                let _ = screenshot_site(tab,site,path);
            }).await;
        },
        //This code sucks and probably the perf is horrible, but I couldn't find how to match io::ErrorKind::TimedOut
        //TODO: Find a way to use `io::ErrorKind::TimedOut`
        Err(e)=> if e.into_cause().unwrap().to_string() == "deadline has elapsed" {println!("{} timed out", &site)}
    };
    Ok(())
}

#[tokio::main]
async fn screenshot_block(endpoints: Vec<String>, path: String) -> Result<(),failure::Error>{
    let browser = Browser::new(LaunchOptionsBuilder::default().build().unwrap())?;
    let arc_browser = Arc::new(Mutex::new(browser));
    let mut fut = vec![];
    //Create directory where the screenshots will be saved
    if create_dir(format!("{}",path)).is_err(){println!("Directory \"{}\" already exists", path); process::exit(-1);}
    for e in endpoints{
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
    let path = if args.len() < 2 {
        println!("Usage: ./crawler endpoints directory");
        process::exit(-1);
    } else {
        let file = File::open(args[1].to_string());
        let file = match file{
            Ok(file) => file,
            Err(_) => {println!("File does not exist");process::exit(-1);}
        };
        for line in io::BufReader::new(file).lines(){
            endpoints.push(line.unwrap());
        }
        args[2].to_string()
    };
    let _ = screenshot_block(endpoints, path);
}