use cpulimit::timeout_cpu;

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

pub fn main() {
    println!("a");
    tt();
    tt();
    tt();
    println!("c");
}
