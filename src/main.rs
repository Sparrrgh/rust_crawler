use headless_chrome::{Browser, protocol::page::ScreenshotFormat, LaunchOptionsBuilder, Tab};
use std::fs::{File,write,create_dir};
use chrono::{Local};
use std::io::{self, BufRead};
use failure;
use std::{process,env};
use std::sync::Arc;
use regex::Regex;
use hyper::{Client, StatusCode};
use tokio::{fs,task};
use futures::future::join_all;

fn screenshot_site(tab: Arc<Tab>, site: String, path: String) -> Result<(),failure::Error>{
    let re = Regex::new(r"^(https|http)://")?;
    let png_data = tab.set_default_timeout(std::time::Duration::from_secs(5))
        .navigate_to(&site)?
        .capture_screenshot(ScreenshotFormat::PNG, None, true)?;
    let finpath = format!("{}/{}{}.png", path, re.replace(&site, ""),Local::now());
    println!("{}", finpath);    
    let _ = write(finpath, png_data)?;
    Ok(())
}

async fn visit_site(site: String) -> Result<String,failure::Error>{
    //Watch out, this does not support HTTPS
    //TODO: Support HTTPS
    let client : Client<hyper::client::HttpConnector> = Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(5))
        .http2_only(false)
        .build_http();
    //Can I set a timeout? Because if the page doesn't respond it will take a fuckton of time for it to realize
    let uri = site.parse()?;    
    let resp = match client.get(uri).await{
        Ok(x) => x,
        Err(..) => return Ok(String::from(""))
    };
    println!("Visited site {}", site);
    //There are a lot more interesting things I can check actually. Maybe instead of checkinf if it's 200 I should check
    //if it's connecting
    if resp.status() == StatusCode::OK{
        Ok(site)
    } else {
        Ok(String::from(""))
    }
}

#[tokio::main]
async fn screenshot_block(endpoints: Vec<String>, path: String) -> Result<(),failure::Error>{
    let mut futures = vec![];
    let browser = Browser::new(LaunchOptionsBuilder::default().build().unwrap())?;
    if create_dir(format!("{}",path)).is_err(){println!("Directory \"{}\" already exists", path); process::exit(-1);}
    for e in endpoints{
        let fut = task::spawn(visit_site(e));
        futures.push(fut);
        //This does NOT scale
        //It will create a huge vector full of empty strings!
        //Maybe use Option instead of Result, I guess pushing none renders an empty array anyways
    }
    let results = join_all(futures).await;
    //Remove empty strings
    let res_e = results.iter()
        .map(|x| x.as_ref().unwrap().as_ref().unwrap())
        .filter(|x| **x != String::from(""))
        .map(|x| x.to_string());
    
    //Here I screenshot each endpoint that responded
    //Can crash if it encounters some weird responding endpoints
    //Ex: Basic auth
    //TODO: Catch crash
    println!("Screenshotting responding endpoints");
    for r in res_e{
        let new_tab = browser.new_tab()?;
        let cpath = path.clone();
        let _ = task::spawn_blocking(|| {
            let _ = screenshot_site(new_tab,r,cpath);
        }).await;
    }
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