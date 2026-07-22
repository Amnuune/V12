#[cfg(test)]
mod tests {
    use v12_frontend::ParsedProgram;
    use crate::Lifter;

    #[test]
    fn dump_while_ir() {
        let src = r#"
let i = 0;
while (i < 3) {
    console.log(i);
    i = i + 1;
}
console.log("done");
"#;
        let parsed = ParsedProgram::from_source(src.to_string()).unwrap();
        let program = parsed.program();
        let ir = Lifter::new().lift_program(&program).unwrap();
        let main_id = ir.main.unwrap();
        let func = &ir.functions[main_id];
        let blocks: Vec<_> = func.blocks.iter().collect();
        eprintln!("Blocks: {}", blocks.len());
        for (i, (bid, blk)) in blocks.iter().enumerate() {
            eprintln!("Block[{}] id={:?} name={:?}: {} insts, last={:?}", 
                i, bid, blk.name, blk.insts.len(),
                blk.insts.last().map(|x| &x.op));
        }
    }
}
