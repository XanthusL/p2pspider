extern crate rand;

use rand::prelude::*;

mod dht;

fn main() {
    let uuu:u16=123;
    let iii= uuu as i32;
    println!("iii:{}",iii );

    // basic usage with random():
    let x: u8 = random();
    println!("{}", x);

    let y = random::<f64>();
    println!("{}", y);

    println!("{:?}", dht::rand_bytes(5));
    let a = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let b = vec![11, 12, 13, 14, 15, 16, 17, 18, 19, 110];
    println!("{:?}", dht::neighbour_id(a, &b));
    println!("{:?}", new_foo());
}

#[derive(Debug)]
struct Foo {
    bar: i32
}

fn new_foo() -> Foo {
    Foo {
        bar: 1234,
    }
}