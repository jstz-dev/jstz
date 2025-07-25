use jstz_runtime::JstzRuntime;

pub fn main() {
    println!("Initialize jstz runtime");
    let mut runtime = JstzRuntime::new(Default::default());
    let result: usize = runtime.execute_with_result("2+2").unwrap();
    println!("Done: {result}");
}
