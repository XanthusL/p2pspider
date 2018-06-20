extern crate bencode;
extern crate rand;

use rand::prelude::*;
use self::bencode::{Bencode, FromBencode, ToBencode};
use self::bencode::util::ByteString;
use std::io::Write;

mod lib;

fn main() {
    println!("hello world");
}

fn run() {
    let mut d = lib::dht::new_dht();
    d = d
        .local_id("0.0.0.0".as_bytes().to_vec())
        .max_friends_per_sec(50)
        .secret("tmp-secret".to_string());
    let (handles, rx) = d.start();
    loop {
        match rx.recv() {
            Ok(announce) => {
                // todo
                // if is_exist(announce.info_hash_hex){continue}
                // if in_block_list(announce.info_hash_hex){continue}
                let mut peer = lib::wire::new(announce.info_hash_hex.clone(), announce.peer.to_string()).unwrap();
                let data = peer.fetch().unwrap_or_else(|e| {
                    // todo add announce.peer to block list
                    vec![]
                });
                let mut dict = bencode::DictMap::new();
                if let Ok(ben) = bencode::from_vec(data) {
                    dict.insert(ByteString::from_str("info"), ben);
                    let bytes = Bencode::Dict(dict).to_bytes().unwrap_or(vec![]);
                    let _ = save(announce.info_hash_hex.clone(), bytes.to_vec()).or_else(|e| {
                        println!("{}", e.to_string());
                        Err(e)
                    });
                    if let Ok(t) = lib::wire::parse_data(bytes.to_vec(), announce.info_hash_hex.clone()) {
                        print!("{}", t)
                    }
                }
            }
            Err(_) => continue,
        }
    }
}

fn save(name: String, dat: Vec<u8>) -> Result<(), std::io::Error> {
    if dat.len() == 0 { return Ok(()); }

    let mut f = std::fs::File::create(format!("{}.torrent", name))?;
    f.write_all(dat.as_ref())?;
    Ok(())
}
