// source: ProxyAuthK8S@8fe220b (apps/swaggergen/src/main.rs).
//
// Prints the OpenAPI spec: `cargo run -p swaggergen > ui/openapi.json`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", my_crate::openapi_spec().to_pretty_json()?);
    Ok(())
}
