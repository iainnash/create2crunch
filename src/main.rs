extern crate create2crunch;

use std::env;
use std::process;
use std::error::Error;

use create2crunch::{Config, cpu, gpu};

fn main() -> Result<(), Box<dyn Error>> {
    // Collect args into a vector for easier indexing
    let args: Vec<String> = std::env::args().collect();

    // Parse the leading ones parameter (default to 8 if not provided)
    let leading_ones = if args.len() > 1 {
        // First argument is the number of leading ones
        args[1].parse::<u32>().unwrap_or(8)
    } else {
        8
    };

    // Parse other parameters from args
    let factory_address = if args.len() > 2 {
        parse_address(&args[2])?
    } else {
        [0; 20]
    };

    let calling_address = if args.len() > 3 {
        parse_address(&args[3])?
    } else {
        [0; 20]
    };

    let init_code_hash = if args.len() > 4 {
        parse_hash(&args[4])?
    } else {
        [0; 32]
    };

    let gpu_device = if args.len() > 5 {
        args[5].parse::<u32>().unwrap_or(0)
    } else {
        0
    };

    // Create the configuration with all required fields
    let config = Config {
        factory_address,
        calling_address,
        init_code_hash,
        gpu_device,
        leading_ones,
        leading_zeroes_threshold: 0,  // Default value
        total_zeroes_threshold: 0,    // Default value
        prefix: None,                 // Default value
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

    Ok(())
}

// Helper function to parse an address from a hex string
fn parse_address(address_str: &str) -> Result<[u8; 20], Box<dyn Error>> {
    let address_str = if address_str.starts_with("0x") {
        &address_str[2..]
    } else {
        address_str
    };
    
    // Ensure the string has an even length
    let address_str = if address_str.len() % 2 != 0 {
        format!("0{}", address_str) // Pad with a leading zero if needed
    } else {
        address_str.to_string()
    };
    
    let bytes = hex::decode(&address_str)?;
    if bytes.len() != 20 {
        // If we don't have exactly 20 bytes, pad or truncate
        let mut result = [0u8; 20];
        let copy_len = std::cmp::min(bytes.len(), 20);
        result[20 - copy_len..].copy_from_slice(&bytes[..copy_len]);
        return Ok(result);
    }
    
    let mut result = [0u8; 20];
    result.copy_from_slice(&bytes);
    Ok(result)
}

// Helper function to parse a hash from a hex string
fn parse_hash(hash_str: &str) -> Result<[u8; 32], Box<dyn Error>> {
    let hash_str = if hash_str.starts_with("0x") {
        &hash_str[2..]
    } else {
        hash_str
    };
    
    // Ensure the string has an even length
    let hash_str = if hash_str.len() % 2 != 0 {
        format!("0{}", hash_str) // Pad with a leading zero if needed
    } else {
        hash_str.to_string()
    };
    
    let bytes = hex::decode(&hash_str)?;
    if bytes.len() != 32 {
        // If we don't have exactly 32 bytes, pad or truncate
        let mut result = [0u8; 32];
        let copy_len = std::cmp::min(bytes.len(), 32);
        result[32 - copy_len..].copy_from_slice(&bytes[..copy_len]);
        return Ok(result);
    }
    
    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}
