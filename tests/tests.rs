use md2tgmdv2::{TG_MAX_LEN, transform};

#[test]
fn transforms_second_fixture() {
    let input = include_str!("2-input.md");
    let expected = include_str!("2-output.txt").trim_end_matches('\n');

    let chunks = transform(input, TG_MAX_LEN);

    assert_eq!(
        chunks.len(),
        1,
        "expected a single output chunk, got {:?}",
        chunks
    );
    assert_eq!(chunks[0], expected);
}
