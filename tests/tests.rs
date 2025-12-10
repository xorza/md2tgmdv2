use md2tgmdv2::{TELEGRAM_BOT_MAX_MESSAGE_LENGTH, transform};

fn transform_expect_1(input: &str, expected: &str) {
    let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], expected);
}

fn transform_expect_n(input: &str, expected: &str, max_chunk_length: usize) {
    let chunks = transform(input, max_chunk_length);
    let actual = chunks.join("===");

    assert_eq!(actual, expected);
}

#[test]
fn test1() {
    transform_expect_1("hi\nhello", "hi\nhello");
    transform_expect_1("- **Split** it into", "⦁ *Split* it into");
    transform_expect_1(
        "Optionally (hierarchical);",
        "Optionally \\(hierarchical\\);",
    );
    transform_expect_1("the past.\n", "the past\\.");
    transform_expect_1(
        "into a **multi‑step compressor** and *never* feeding",
        "into a *multi‑step compressor* and _never_ feeding",
    );
    transform_expect_1("## 1. What", "*✏ 1\\. What*");
    transform_expect_1(
        "`messages = [{role: \"user\"|\"assistant\", content: string}, …]`",
        "`messages \\= \\[\\{role: \"user\"\\|\"assistant\", content: string\\}, …\\]`",
    );
    transform_expect_1(
        "Assume:\n\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume:\n\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
    );
    transform_expect_1(
        "Assume:\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume:\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
    );
    transform_expect_1(
        "```text\ntoken_count(text)\n```",
        "```text\ntoken\\_count\\(text\\)\n```",
    );
    transform_expect_1("> You.\n>  ", ">You\\.");
    transform_expect_1("> You\n> \n> Hi", ">You\n>\n>Hi");
    transform_expect_1(
        "> - Greetings\n> - Repetitive",
        ">⦁ Greetings\n>⦁ Repetitive",
    );
    transform_expect_1("> **GOAL:** ", ">*GOAL:*");

    transform_expect_n("12345 12345", "12345===12345", 5);
    transform_expect_n("12345 12345", "12345===12345", 10);
    transform_expect_n("12345 12345", "12345 12345", 11);

    transform_expect_n(
        "```\n1234567890\n1234567890\n```",
        "```\n1234567890\n```===```\n1234567890\n```",
        18,
    );
    transform_expect_n(
        "```\n1234567890\n1234567890\n```",
        "```\n1234567890\n```===```\n1234567890\n```",
        19,
    );
    transform_expect_n(
        "```\n1234567890\n1234567890\n```",
        "```\n1234567890\n```===```\n1234567890\n```",
        28,
    );
    transform_expect_n(
        "```\n1234567890\n1234567890\n```",
        "```\n1234567890\n1234567890\n```",
        29,
    );

    transform_expect_n(
        //
        "this text is 30ty chars long11\n```\n1234567890\n1234567890\n```",
        "this text is 30ty chars long11===```\n1234567890\n1234567890\n```",
        40,
    );
}
// #[test]
// fn test2() {
//     let input = include_str!("1-input.md");
//     let expected = include_str!("1-output.txt");
//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);
//     let actual = chunks.join("===");

//     std::fs::write("tests/1-output.txt", &actual).unwrap();

//     assert_eq!(actual, expected);
// }
