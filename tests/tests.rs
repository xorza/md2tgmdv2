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
