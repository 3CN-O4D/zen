use zen_syntax::lex::lex;
use zen_syntax::span::FileId;
#[test]
fn probe() {
    let src = "if a then b else c";
    let lx = lex(src, FileId(0));
    for t in &lx.toks {
        println!("{:?} {}..{}", t.kind, t.lo, t.hi);
    }
}
