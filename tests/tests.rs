use md2tgmdv2::{TELEGRAM_BOT_MAX_MESSAGE_LENGTH, transform};

fn transform_expect_1(input: &str, expected: &str) {
    let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], expected);
}

#[test]
fn test1() {
    transform_expect_1("- **Split** it into", "⦁ *Split* it into");
}
#[test]
fn test2() {
    transform_expect_1(
        "Optionally (hierarchical);",
        "Optionally \\(hierarchical\\);",
    );
}
#[test]
fn test3() {
    transform_expect_1("the past.\n", "the past\\.");
}

// #[test]
// fn transforms_2_fixture() {
//     let input = include_str!("2-input.md").trim_end();
//     let expected = include_str!("2-output.txt").trim_end();

//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

//     assert_eq!(
//         chunks.len(),
//         1,
//         "expected a single output chunk, got {:?}",
//         chunks
//     );
//     assert_eq!(chunks[0], expected);
// }

// #[test]
// fn transforms_3_fixture() {
//     let input = include_str!("3-input.md").trim_end();
//     let expected = include_str!("3-output.txt").trim_end();

//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

//     assert_eq!(
//         chunks.len(),
//         1,
//         "expected a single output chunk, got {:?}",
//         chunks
//     );
//     assert_eq!(chunks[0], expected);
// }

// #[test]
// fn transforms_4_fixture() {
//     let input = include_str!("4-input.md").trim_end();
//     let expected = include_str!("4-output.txt").trim_end();

//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

//     assert_eq!(
//         chunks.len(),
//         1,
//         "expected a single output chunk, got {:?}",
//         chunks
//     );
//     assert_eq!(chunks[0], expected);
// }

// #[test]
// fn transforms_5_fixture() {
//     let input = include_str!("5-input.md").trim_end();
//     let expected = include_str!("5-output.txt").trim_end();

//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

//     assert_eq!(
//         chunks.len(),
//         1,
//         "expected a single output chunk, got {:?}",
//         chunks
//     );
//     assert_eq!(chunks[0], expected);
// }

// #[test]
// fn transforms_6_fixture() {
//     let input = include_str!("6-input.md").trim_end();
//     let expected = include_str!("6-output.txt").trim_end();

//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

//     assert_eq!(
//         chunks.len(),
//         1,
//         "expected a single output chunk, got {:?}",
//         chunks
//     );
//     assert_eq!(chunks[0], expected);
// }

// #[test]
// fn transforms_7_fixture() {
//     const MAX_LEN: usize = 281;

//     let input = include_str!("7-input.md").trim_end();
//     let expected = include_str!("7-output.txt").trim_end();

//     let chunks = transform(input, MAX_LEN);
//     let actual = chunks.join("\n=========\n");

//     for (i, chunk) in chunks.iter().enumerate() {
//         assert!(
//             chunk.len() <= MAX_LEN,
//             "chunk {} exceeds limit: {} bytes (limit {})",
//             i,
//             chunk.len(),
//             MAX_LEN
//         );
//     }

//     assert_eq!(
//         chunks.len(),
//         2,
//         "expected {} output chunks, got {:?}",
//         chunks.len(),
//         chunks
//     );
//     assert_eq!(actual, expected);
// }

// #[test]
// fn transforms_1_fixture() {
//     const MAX_LEN: usize = 4090;

//     let input = include_str!("1-input.md").trim_end();
//     let expected = include_str!("1-output.txt").trim_end();

//     let chunks = transform(input, MAX_LEN);
//     let actual = chunks.join("\n=========\n");

//     for (i, chunk) in chunks.iter().enumerate() {
//         assert!(
//             chunk.len() <= MAX_LEN,
//             "chunk {} exceeds limit: {} bytes (limit {})",
//             i,
//             chunk.len(),
//             MAX_LEN
//         );
//     }
//     // assert_eq!(
//     //     chunks.len(),
//     //     5,
//     //     "expected {} output chunks, got {:?}",
//     //     chunks.len(),
//     //     chunks
//     // );
//     assert_eq!(actual, expected);
// }
