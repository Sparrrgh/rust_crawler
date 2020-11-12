use headless_chrome::{Browser, protocol::page::ScreenshotFormat, LaunchOptionsBuilder, Tab};
use std::fs::{File,write,create_dir};
use chrono::{Local};
use std::io::{self, BufRead};
use failure;
use std::{process,env};
use std::sync::Arc;
use regex::Regex;

fn screenshot_site(tab: &Arc<Tab>, site: String, path: &String, re: &Regex) -> Result<(),failure::Error>{
    // This is ugly, but most of time it fails because I can't navigate that endpoint and it's normal
    // during a scan. Throwing a error would probably thwart performance, I should test that.
    let png_data = tab.set_default_timeout(std::time::Duration::from_secs(5))
        .navigate_to(&site)?
        .capture_screenshot(ScreenshotFormat::PNG, None, true)?;
    let finpath = format!("{}/{}{}.png", path, re.replace(&site, ""),Local::now());
    println!("{}", finpath);
    //TODO: Use BufWriter
    let _ = write(finpath, png_data)?;
    Ok(())
}

fn screenshot_block(endpoints: Vec<String>, path: String) -> Result<(),failure::Error>{
    let browser = Browser::new(LaunchOptionsBuilder::default().build().unwrap())?;
    let _tab = browser.wait_for_initial_tab()?;
    if create_dir(format!("{}",path)).is_err(){println!("Directory \"{}\" already exists", path); process::exit(-1);}
    let re = Regex::new(r"^(https|http)://").unwrap();
    let mut _c = 0;
    for e in endpoints{
        if let Err(err)= screenshot_site(&browser.new_tab()?, e, &path, &re){
            println!("{}",err);
        }
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
        //TODO: Check if file exists
        let file = File::open(args[1].to_string()).unwrap();
        for line in io::BufReader::new(file).lines(){
            endpoints.push(line.unwrap());
        }
        args[2].to_string()
    };
    let _ = screenshot_block(endpoints,path);
}