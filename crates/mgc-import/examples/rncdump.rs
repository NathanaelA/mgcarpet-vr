//! Dev aid: decompress a standalone RNC file (SEARCH.DAT, BUILD*-*.TAB/DAT).
fn main() {
    let mut args = std::env::args().skip(1);
    let (inp, out) = (args.next().expect("in"), args.next().expect("out"));
    let data = std::fs::read(&inp).unwrap();
    let raw = mgc_import::rnc::decompress(&data).expect("rnc");
    std::fs::write(&out, &raw).unwrap();
    println!("{} -> {} ({} bytes)", inp, out, raw.len());
}
