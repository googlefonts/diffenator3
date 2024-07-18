use super::Report;

pub fn report(result: Report, pretty: bool) {
    if pretty {
        println!("{}", serde_json::to_string_pretty(&result).expect("foo"));
    } else {
        println!("{}", serde_json::to_string(&result).expect("foo"));
    }
    std::process::exit(0);
}
