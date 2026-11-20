mod battery;
#[allow(dead_code)]
mod render;

fn main() {
    match battery::read() {
        Some(state) => println!("{state:?}"),
        None => println!("no battery or unknown level"),
    }
}
