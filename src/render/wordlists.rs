use lazy_static::lazy_static;

lazy_static! {
    pub static ref LATIN: Vec<u8> = {
        let mut latin: Vec<u8> = Vec::new();
        let compressed = include_bytes!("../../test-data/Latin.txt.br");
        brotli::BrotliDecompress(&mut compressed.as_ref(), &mut latin)
            .expect("Could not decompress");
        latin
    };
}
