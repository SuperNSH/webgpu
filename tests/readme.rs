//! The README's example is `examples/headless/samples/readme.app.tsx`, byte
//! for byte: that file is type-checked and mounted by `nsh test` in
//! `examples/headless` (`samples.test.tsx`), so the README shows tested code.

#[test]
fn the_readme_example_is_the_tested_sample() {
    let root = env!("CARGO_MANIFEST_DIR");
    let readme = std::fs::read_to_string(format!("{root}/README.md")).expect("README.md");
    let sample =
        std::fs::read_to_string(format!("{root}/examples/headless/samples/readme.app.tsx"))
            .expect("the sample");
    let example = readme.find("## Example").expect("an Example section");
    let start =
        readme[example..].find("```tsx\n").expect("a tsx block") + example + "```tsx\n".len();
    let end = readme[start..].find("```\n").expect("the block's end") + start;
    assert_eq!(&readme[start..end], sample);
}
