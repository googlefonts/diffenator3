use std::error::Error;

pub(crate) fn die(doing: &str, err: impl Error) -> ! {
    eprintln!("Error {}: {}", doing, err);
    eprintln!();
    eprintln!("Caused by:");
    if let Some(cause) = err.source() {
        for (i, e) in std::iter::successors(Some(cause), |e| (*e).source()).enumerate() {
            eprintln!("   {}: {}", i, e);
        }
    }
    std::process::exit(1);
}
