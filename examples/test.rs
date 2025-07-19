use std::time::Duration;

use cputimeout::{timeout_cpu, Error};

// TODO: link to a c library and test if we are interposing allocations there
fn tt() {
    timeout_cpu(
        || {
            println!("here");
        },
        std::time::Duration::from_millis(100),
    )
    .unwrap();
    println!(".");
}

pub fn main2() {
    println!("a");
    tt();
    tt();
    tt();
    println!("c");
}

pub fn main() {
    let r = timeout_cpu(
        || timeout_cpu(|| loop {}, Duration::from_millis(50)),
        Duration::from_millis(100),
    );

    assert!(matches!(r, Ok(Err(Error::TimedOut))));

    // outer smaller than inner
    let r = timeout_cpu(
        || {
            let r = timeout_cpu(|| loop {}, Duration::from_millis(50000));

            println!("? {:?}", r);
            r
        },
        Duration::from_millis(50),
    );
    assert!(matches!(r, Err(Error::TimedOut)), "{:?}", r);
}
