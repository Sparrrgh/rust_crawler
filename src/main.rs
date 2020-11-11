use headless_chrome::{Browser, protocol::page::ScreenshotFormat, LaunchOptionsBuilder, Tab};
use std::fs::{File,write,create_dir};
use chrono::prelude::*;
use std::io::{self, BufRead};
use failure;
use std::{process,env};
use std::sync::Arc;

// It would be a good idea to specify a port, since choosing context-critical 
// ports could give good results regarding screenshots
fn screenshot_site(tab: &Arc<Tab>, site: String, path: &String) -> Result<(),failure::Error>{
    let png_data = tab.navigate_to(&site)?
        .set_default_timeout(std::time::Duration::from_secs(10))
        .capture_screenshot(ScreenshotFormat::PNG, None, true)?;
    let finpath = format!("{}/{}{}.png", path, &site.split("http://").collect::<Vec<&str>>()[1],Local::now());
    println!("{}", finpath);
    let _ = write(finpath, png_data)?;
    Ok(())
}

fn screenshot_block(endpoints: Vec<String>, path: String) -> Result<(),failure::Error>{
    
    let browser = Browser::new(LaunchOptionsBuilder::default().build().unwrap())?;
    let tab = browser.wait_for_initial_tab()?;
    create_dir(format!("{}",path))?;
    for e in endpoints{
        let _= screenshot_site(&tab, e, &path)?;
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut endpoints: Vec<String> = vec![];
    let path = if args.len() < 2 {
        println!("Usage: ./crawler endpoints directory");
        process::exit(-0x0100);
    } else {
        let file = File::open(args[1].to_string()).unwrap();
        for line in io::BufReader::new(file).lines(){
            endpoints.push(line.unwrap());
        }
        args[2].to_string()
    };
    // Debug
    for i in &endpoints{
        println!("[DEBUG] Endpoits are {}",i);
    }
    let _ = screenshot_block(endpoints,path);
    println!("Finished");
}
