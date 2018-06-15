extern crate rand;

use rand::prelude::*;
mod lib;
fn main() {
    let uuu: u16 = 123;
    let iii = uuu as i32;
    println!("iii:{}", iii);

    // basic usage with random():
    let x: u8 = random();
    println!("{}", x);

    let y = random::<f64>();
    println!("{}", y);

    println!("{:?}", lib::dht::rand_bytes(5));
    let a = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let b = vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 110];
    println!("{:?}", lib::dht::neighbour_id(a, &b));
}

fn run() {
    let mut d = lib::dht::new_dht();
    d = d
        .local_id("0.0.0.0".as_bytes().to_vec())
        .max_friends_per_sec(50)
        .secret("tmp-secret".to_string());
    let (handles,rx)= d.start();
    loop {
        match rx.recv() {
            Ok(a)=>{

            },
            Err(_)=>continue,
        }
    }
}