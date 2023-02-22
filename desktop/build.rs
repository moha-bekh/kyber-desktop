use vergen::EmitBuilder;

fn main() {
    EmitBuilder::builder()
        .git_describe(false, true)
        .emit()
        .unwrap();
}
