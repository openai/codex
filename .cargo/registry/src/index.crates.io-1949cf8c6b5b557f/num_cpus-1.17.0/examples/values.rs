extern crate num_cpus;

fn main() {
    println!("Logical CPUs: {}", num_cpus::get());
    println!("Physical CPUs: {}", num_cpus::get_physical());
}
