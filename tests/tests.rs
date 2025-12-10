use md2tgmdv2::{TELEGRAM_BOT_MAX_MESSAGE_LENGTH, transform};

fn transform_expect_1(input: &str, expected: &str) {
    let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], expected);
}

fn transform_expect_n(input: &str, expected: &str, max_chunk_length: usize) {
    let chunks = transform(input, max_chunk_length);
    let actual = chunks.join("\n===\n");

    assert_eq!(actual, expected);
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
#[test]
fn test4() {
    transform_expect_1(
        "into a **multi‑step compressor** and *never* feeding",
        "into a *multi‑step compressor* and _never_ feeding",
    );
}
#[test]
fn test5() {
    transform_expect_1("## 1. What", "*✏ 1\\. What*");
}
#[test]
fn test6() {
    transform_expect_1(
        "`messages = [{role: \"user\"|\"assistant\", content: string}, …]`",
        "`messages \\= \\[\\{role: \"user\"\\|\"assistant\", content: string\\}, …\\]`",
    );
}
#[test]
fn test7() {
    transform_expect_1(
        "Assume:\n\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume:\n\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
    );
}
#[test]
fn test8() {
    transform_expect_1(
        "Assume:\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume:\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
    );
}
#[test]
fn test9() {
    transform_expect_1(
        "```text\ntoken_count(text)\n```",
        "```text\ntoken\\_count\\(text\\)\n```",
    );
}
#[test]
fn test10() {
    transform_expect_1("> You.\n>  ", ">You\\.");
}
#[test]
fn test11() {
    transform_expect_1("> You\n> \n> Hi", ">You\n>\n>Hi");
}
#[test]
fn test12() {
    transform_expect_1(
        "> - Greetings\n> - Repetitive",
        ">⦁ Greetings\n>⦁ Repetitive",
    );
}
#[test]
fn test13() {
    transform_expect_1("> **GOAL:** ", ">*GOAL:*");
}
#[test]
fn test14() {
    transform_expect_n("12345 12345", "12345\n===\n12345", 5);
    transform_expect_n("12345 12345", "12345\n===\n12345", 6);
    transform_expect_n("12345 12345", "12345\n===\n12345", 10);
    transform_expect_n("12345 12345", "12345 12345", 11);
}
#[test]
fn test15() {
    transform_expect_n(
        "```pseudo\n1234567890\n1234567890\n```",
        "```pseudo\n1234567890\n```===```pseudo\n1234567890\n```",
        24,
    );
}
