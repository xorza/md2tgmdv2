use md2tgmdv2::{TG_MAX_LEN, transform};

#[test]
fn transforms_2_fixture() {
    let input = include_str!("2-input.md").trim_end();
    let expected = include_str!("2-output.txt").trim_end();

    let chunks = transform(input, TG_MAX_LEN);

    assert_eq!(
        chunks.len(),
        1,
        "expected a single output chunk, got {:?}",
        chunks
    );
    assert_eq!(chunks[0], expected);
}

#[test]
fn transforms_3_fixture() {
    let input = include_str!("3-input.md").trim_end();
    let expected = include_str!("3-output.txt").trim_end();

    let chunks = transform(input, TG_MAX_LEN);

    assert_eq!(
        chunks.len(),
        1,
        "expected a single output chunk, got {:?}",
        chunks
    );
    assert_eq!(chunks[0], expected);
}

#[test]
fn transforms_4_fixture() {
    let input = include_str!("4-input.md").trim_end();
    let expected = include_str!("4-output.txt").trim_end();

    let chunks = transform(input, TG_MAX_LEN);

    assert_eq!(
        chunks.len(),
        1,
        "expected a single output chunk, got {:?}",
        chunks
    );
    assert_eq!(chunks[0], expected);
}
