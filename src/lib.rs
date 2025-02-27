mod reward;
mod gpu;

extern crate byteorder;
extern crate console;
extern crate fs2;
extern crate hex;
extern crate itertools;
extern crate ocl;
extern crate ocl_extras;
extern crate rand;
extern crate rayon;
extern crate separator;
extern crate terminal_size;
extern crate tiny_keccak;

use std::error::Error;
use std::i64;
use std::io::prelude::*;
use std::fs::OpenOptions;
use std::time::{SystemTime, UNIX_EPOCH};

use byteorder::{ByteOrder, BigEndian, LittleEndian};
use console::Term;
use fs2::FileExt;
use hex::FromHex;
use itertools::Itertools;
use rand::{thread_rng, Rng};
use rayon::prelude::*;
use separator::Separatable;
use terminal_size::{Width, Height, terminal_size};
use tiny_keccak::Keccak;

// Export the GPU function
pub use gpu::gpu;

// workset size (tweak this!)
const WORK_SIZE: u32 = 0x4000000; // max. 0x15400000 to abs. max 0xffffffff

const WORK_FACTOR: u128 = (WORK_SIZE as u128) / 1_000_000;
const ZERO_BYTE: u8 = 0x00;
const EIGHT_ZERO_BYTES: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
const CONTROL_CHARACTER: u8 = 0xff;
const ZERO_REWARD: &str = "0";
const MAX_INCREMENTER: u64 = 0xffffffffffff;

/// Requires three hex-encoded arguments: the address of the contract that will
/// be calling CREATE2, the address of the caller of said contract *(assuming
/// the contract calling CREATE2 has frontrunning protection in place - if not
/// applicable to your use-case you can set it to the null address)*, and the
/// keccak-256 hash of the bytecode that is provided by the contract calling
/// CREATE2 that will be used to initialize the new contract. An additional set
/// of three optional values may be provided: a device to target for OpenCL GPU
/// search, a threshold for leading zeroes to search for, and a threshold for
/// total zeroes to search for.
#[derive(Clone)]
pub struct Config {
    pub factory_address: [u8; 20],
    pub calling_address: [u8; 20],
    pub init_code_hash: [u8; 32],
    pub gpu_device: u32,
    pub leading_zeroes_threshold: u8,
    pub total_zeroes_threshold: u8,
    pub prefix: Option<String>,
    pub leading_ones: u32,
}

/// Validate the provided arguments and construct the Config struct.
impl Config {
    pub fn new(mut args: std::env::Args) -> Result<Self, &'static str> {
        // get args, skipping first arg (program name)
        args.next();

        let mut factory_address_string = match args.next() {
            Some(arg) => arg,
            None => return Err("didn't get a factory_address argument."),
        };

        let mut calling_address_string = match args.next() {
            Some(arg) => arg,
            None => return Err("didn't get a calling_address argument."),
        };

        let mut init_code_hash_string = match args.next() {
            Some(arg) => arg,
            None => return Err("didn't get an init_code_hash argument."),
        };

        let gpu_device_string = match args.next() {
            Some(arg) => arg,
            None => String::from("255"), // indicates that CPU will be used.
        };

        // If we have a prefix, we don't need these thresholds
        let prefix_string = match args.next() {
            Some(arg) => {
                if arg.starts_with("0x") {
                    Some(without_prefix(arg))
                } else {
                    Some(arg)
                }
            },
            None => None,
        };

        // Only parse these if we don't have a prefix
        let (leading_zeroes_threshold, total_zeroes_threshold) = if prefix_string.is_some() {
            (0, 0) // Default to 0 when using prefix
        } else {
            let leading = match args.next() {
                Some(arg) => arg,
                None => String::from("7"),
            };

            let total = match args.next() {
                Some(arg) => arg,
                None => String::from("5"),
            };

            // Convert to u8
            let leading_parsed = match leading.parse::<u8>() {
                Ok(t) => t,
                Err(_) => return Err("invalid leading zeroes threshold value supplied.")
            };

            let total_parsed = match total.parse::<u8>() {
                Ok(t) => t,
                Err(_) => return Err("invalid total zeroes threshold value supplied.")
            };

            (leading_parsed, total_parsed)
        };

        // strip 0x from args if applicable
        if factory_address_string.starts_with("0x") {
            factory_address_string = without_prefix(factory_address_string)
        }

        if calling_address_string.starts_with("0x") {
            calling_address_string = without_prefix(calling_address_string)
        }

        if init_code_hash_string.starts_with("0x") {
            init_code_hash_string = without_prefix(init_code_hash_string)
        }

        // convert main arguments from hex string to vector of bytes
        let factory_address_vec: Vec<u8> = match Vec::from_hex(
            &factory_address_string
        ) {
            Ok(t) => t,
            Err(_) => {
                return Err("could not decode factory address argument.")
            }
        };

        let calling_address_vec: Vec<u8> = match Vec::from_hex(
            &calling_address_string
        ) {
            Ok(t) => t,
            Err(_) => {
                return Err("could not decode calling address argument.")
            }
        };

        let init_code_hash_vec: Vec<u8> = match Vec::from_hex(
            &init_code_hash_string
        ) {
            Ok(t) => t,
            Err(_) => {
                return Err(
                    "could not decode initialization code hash argument."
                )
            }
        };

        // validate length of each argument (20, 20, 32)
        if factory_address_vec.len() != 20 {
            return Err("invalid length for factory address argument.")
        }

        if calling_address_vec.len() != 20 {
            return Err("invalid length for calling address argument.")
        }

        if init_code_hash_vec.len() != 32 {
            return Err("invalid length for initialization code hash argument.")
        }

        // convert from vector to fixed array
        let factory_address = to_fixed_20(factory_address_vec);
        let calling_address = to_fixed_20(calling_address_vec);
        let init_code_hash = to_fixed_32(init_code_hash_vec);

        // convert gpu arguments to u8 values
        let gpu_device: u32 = match gpu_device_string
                                               .parse::<u32>() {
            Ok(t) => t,
            Err(_) => {
                return Err(
                    "invalid gpu device value."
                )
            }
        };

        // Validate prefix if provided
        let prefix = if let Some(prefix_str) = prefix_string {
            // Validate that the prefix contains only valid hex characters
            if !prefix_str.chars().all(|c| c.is_digit(16)) {
                return Err("prefix must contain only valid hexadecimal characters");
            }
            
            Some(prefix_str)
        } else {
            None
        };

        // return the config object
        Ok(
          Self {
            factory_address,
            calling_address,
            init_code_hash,
            gpu_device,
            leading_zeroes_threshold,
            total_zeroes_threshold,
            prefix,
            leading_ones: 0,
          }
        )
    }
}

/// Given a Config object with a factory address, a caller address, and a
/// keccak-256 hash of the contract initialization code, search for salts that
/// will enable the factory contract to deploy a contract to a gas-efficient
/// address via CREATE2.
///
/// The 32-byte salt is constructed as follows:
///   - the 20-byte calling address (to prevent frontrunning)
///   - a random 6-byte segment (to prevent collisions with other runs)
///   - a 6-byte nonce segment (incrementally stepped through during the run)
///
/// When a salt that will result in the creation of a gas-efficient contract
/// address is found, it will be appended to `efficient_addresses.txt` along
/// with the resultant address and the "value" (i.e. approximate rarity) of the
/// resultant address.
pub fn cpu(config: Config) -> Result<(), Box<dyn Error>> {
    // (create if necessary) and open a file where found salts will be written
    let file = OpenOptions::new()
                 .append(true)
                 .create(true)
                 .open("efficient_addresses.txt")
                 .expect(
                   "Could not create or open `efficient_addresses.txt` file."
                 );

    // create object for computing rewards (relative rarity) for a given address
    let rewards = reward::Reward::new();

    // set "footer" of hash message using initialization code hash from config
    let footer: [u8; 32] = config.init_code_hash;

    // create a random number generator
    let mut rng = thread_rng();

    // begin searching for addresses
    loop {
        // create a random 6-byte salt using the random number generator
        let salt_random_segment = rng.gen_iter::<u8>()
                                    .take(6)
                                    .collect::<Vec<u8>>();

        // header: 0xff ++ factory ++ caller ++ salt_random_segment (47 bytes)
        let mut header_vec: Vec<u8> = vec![CONTROL_CHARACTER];
        header_vec.extend(config.factory_address.iter());
        header_vec.extend(config.calling_address.iter());
        header_vec.extend(salt_random_segment.clone());

        // convert the header vector to a fixed-length array
        let header: [u8; 47] = to_fixed_47(&header_vec);

        // create new hash object
        let mut hash_header = Keccak::new_keccak256();

        // update hash with header
        hash_header.update(&header);

        // iterate over a 6-byte nonce and compute each address
        (0..MAX_INCREMENTER)
          .into_par_iter() // parallelization
          .map(|x| u64_to_fixed_6(&x)) // convert int nonces to fixed arrays
          .for_each(|salt_incremented_segment| {
            // clone the partially-hashed object
            let mut hash = hash_header.clone();

            // update with body and footer (total: 38 bytes)
            hash.update(&salt_incremented_segment);
            hash.update(&footer);

            // hash the payload and get the result
            let mut res: [u8; 32] = [0; 32];
            hash.finalize(&mut res);

            // truncate first 12 bytes from the hash to derive address
            let mut address_bytes: [u8; 20] = Default::default();
            address_bytes.copy_from_slice(&res[12..]);
            
            // get the address that results from the hash
            let address_hex_string = hex::encode(&address_bytes);
            
            // Check if the address matches the desired prefix
            let prefix_match = if let Some(ref prefix) = config.prefix {
                address_hex_string.starts_with(prefix)
            } else {
                // If no prefix is specified, fall back to the original zero-byte checks
                let total = res
                    .iter()
                    .dropping(12)
                    .filter(|&n| *n == ZERO_BYTE)
                    .count();
                    
                if total <= 2 {
                    return; // Skip addresses with fewer than 3 zero bytes
                }
                
                // get the leading zero bytes associated with the address
                let mut leading = 0;
                for (i, b) in res.iter().dropping(12).enumerate() {
                    if b != &ZERO_BYTE {
                        leading = i; // set leading on finding non-zero byte
                        break;       // stop searching upon locating
                    }
                }
                
                // look up the reward amount
                let key = leading * 20 + total;
                rewards.get(&key) != ZERO_REWARD
            };
            
            // proceed if an efficient address or prefix match has been found
            if prefix_match {
                // Calculate leading and total for reward calculation
                let total = res
                    .iter()
                    .dropping(12)
                    .filter(|&n| *n == ZERO_BYTE)
                    .count();
                    
                let mut leading = 0;
                for (i, b) in res.iter().dropping(12).enumerate() {
                    if b != &ZERO_BYTE {
                        leading = i; // set leading on finding non-zero byte
                        break;       // stop searching upon locating
                    }
                }

                // get the address that results from the hash
                let address = format!("{}", &address_hex_string);

                // get the full salt used to create the address
                let header_hex_string = hex::encode(&header_vec);
                let body_hex_string = hex::encode(salt_incremented_segment
                                                    .to_vec());
                let full_salt = format!(
                  "0x{}{}",
                  &header_hex_string[42..],
                  &body_hex_string
                );

                // encode address and set up a variable for the checksum
                let address_encoded = address.as_bytes();
                let mut checksum_address = "0x".to_string();

                // create new hash object for computing the checksum
                let mut checksum_hash = Keccak::new_keccak256();

                // update with utf8-encoded address (total: 20 bytes)
                checksum_hash.update(&address_encoded);

                // hash the payload and get the result
                let mut checksum_res: [u8; 32] = [0; 32];
                checksum_hash.finalize(&mut checksum_res);
                let address_hash = hex::encode(checksum_res);

                // compute the address checksum using the above hash
                for nibble in 0..address.len() {
                    let hash_character = i64::from_str_radix(
                      &address_hash
                        .chars()
                        .nth(nibble)
                        .unwrap()
                        .to_string(),
                      16
                    ).unwrap();
                    let character = address.chars().nth(nibble).unwrap();
                    if hash_character > 7 {
                        checksum_address = format!(
                          "{}{}",
                          checksum_address,
                          character.to_uppercase().to_string()
                        );
                    } else {
                        checksum_address = format!(
                          "{}{}",
                          checksum_address,
                          character.to_string()
                        );
                    }
                }

                // display the salt and the address.
                let output = format!(
                  "{} => {} => {}",
                  full_salt,
                  checksum_address,
                  rewards.get(&(leading * 20 + total))
                );
                println!("{}", &output);

                // create a lock on the file before writing
                file.lock_exclusive().expect("Couldn't lock file.");

                // write the result to file
                writeln!(&file, "{}", &output).expect(
                  "Couldn't write to `efficient_addresses.txt` file."
                );

                // release the file lock
                file.unlock().expect("Couldn't unlock file.");

                // Print a success message
                println!("Found address with prefix '{}': {}", config.prefix.as_ref().unwrap(), checksum_address);
                println!("Salt: 0x{}{}{}", hex::encode(&config.calling_address), hex::encode(&salt_random_segment), hex::encode(&salt_incremented_segment));
                
                // Exit the program with success
                std::process::exit(0);
            }
        });
    }
}

/// Remove the `0x` prefix from a hex string.
fn without_prefix(string: String) -> String {
    string
      .char_indices()
      .nth(2)
      .and_then(|(i, _)| string.get(i..))
      .unwrap()
      .to_string()
}

/// Convert a properly-sized vector to a fixed array of 20 bytes.
fn to_fixed_20(bytes: std::vec::Vec<u8>) -> [u8; 20] {
    let mut array = [0; 20];
    let bytes = &bytes[..array.len()];
    array.copy_from_slice(bytes);
    array
}

/// Convert a properly-sized vector to a fixed array of 32 bytes.
fn to_fixed_32(bytes: std::vec::Vec<u8>) -> [u8; 32] {
    let mut array = [0; 32];
    let bytes = &bytes[..array.len()];
    array.copy_from_slice(bytes);
    array
}

/// Convert a properly-sized vector to a fixed array of 47 bytes.
fn to_fixed_47(bytes: &std::vec::Vec<u8>) -> [u8; 47] {
    let mut array = [0; 47];
    let bytes = &bytes[..array.len()];
    array.copy_from_slice(bytes);
    array
}

/// Convert a properly-sized vector to a fixed array of 4 bytes.
fn to_fixed_4(bytes: &std::vec::Vec<u8>) -> [u8; 4] {
    let mut array = [0; 4];
    let bytes = &bytes[..array.len()];
    array.copy_from_slice(bytes);
    array
}

/// Convert a 64-bit unsigned integer to a fixed array of six bytes.
fn u64_to_fixed_6(x: &u64) -> [u8; 6] {
    let mask: u64 = 0xff;
    let b1: u8 = ((x >> 40) & mask) as u8;
    let b2: u8 = ((x >> 32) & mask) as u8;
    let b3: u8 = ((x >> 24) & mask) as u8;
    let b4: u8 = ((x >> 16) & mask) as u8;
    let b5: u8 = ((x >> 8) & mask) as u8;
    let b6: u8 = (x & mask) as u8;
    [b1, b2, b3, b4, b5, b6]
}

/// Convert 64-bit unsigned integer to little-endian fixed array of eight bytes.
fn u64_to_le_fixed_8(x: &u64) -> [u8; 8] {
    let mask: u64 = 0xff;
    let b1: u8 = ((x >> 56) & mask) as u8;
    let b2: u8 = ((x >> 48) & mask) as u8;
    let b3: u8 = ((x >> 40) & mask) as u8;
    let b4: u8 = ((x >> 32) & mask) as u8;
    let b5: u8 = ((x >> 24) & mask) as u8;
    let b6: u8 = ((x >> 16) & mask) as u8;
    let b7: u8 = ((x >> 8) & mask) as u8;
    let b8: u8 = (x & mask) as u8;
    [b8, b7, b6, b5, b4, b3, b2, b1]
}
