extern crate create2crunch;

use std::env;
use std::process;

use create2crunch::{Config, cpu, gpu};

fn main() {
    let args = std::env::args();
    let config = match Config::new(args) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Problem parsing arguments: {}", err);
            process::exit(1);
        }
    };

    if config.gpu_device == 0 {
        println!("Using GPU device 0...");
        if let Err(e) = gpu(config) {
            eprintln!("GPU search failed: {}", e);
            process::exit(1);
        }
    } else if config.gpu_device == 255 {
        // Use CPU directly if specified
        println!("Using CPU search...");
        if let Err(e) = cpu(config) {
            eprintln!("Application error: {}", e);
            process::exit(1);
        }
    } else {
        // Use specified GPU device
        println!("Using GPU device {}...", config.gpu_device);
        if let Err(e) = gpu(config) {
            eprintln!("Application error: {}", e);
            process::exit(1);
        }
    }
}
