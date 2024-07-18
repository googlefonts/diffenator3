pub fn report(result: serde_json::Value, pretty: bool) {
    if pretty {
        println!("{}", serde_json::to_string_pretty(&result).expect("foo"));
    } else {
        println!("{}", serde_json::to_string(&result).expect("foo"));
    }
    std::process::exit(0);
}
