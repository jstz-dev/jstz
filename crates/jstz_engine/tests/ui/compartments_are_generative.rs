use jstz_engine::alloc_compartment;

fn main() {
    alloc_compartment!(a);
    alloc_compartment!(b);
    assert_eq!(a, b);
}
