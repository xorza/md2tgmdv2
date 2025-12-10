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
    for (i, chunk) in chunks.iter().enumerate() {
        assert!(
            chunk.len() <= max_chunk_length,
            "chunk {} length {} exceeds max_chunk_length {}",
            i,
            chunk.len(),
            max_chunk_length
        );
    }
}

#[test]
fn preserves_single_newline() {
    transform_expect_1("hi\nhello", "hi\nhello");
}

#[test]
fn preserves_double_newline() {
    transform_expect_1("hi\n\nhello", "hi\n\nhello");
}

#[test]
fn converts_simple_list_item() {
    transform_expect_1("- **Split** it into", "⦁ *Split* it into");
}

#[test]
fn converts_text_followed_by_list() {
    transform_expect_1("test\n\n- **Split** it into", "test\n\n⦁ *Split* it into");
}

#[test]
fn escapes_parentheses() {
    transform_expect_1(
        "Optionally (hierarchical);",
        "Optionally \\(hierarchical\\);",
    );
}

#[test]
fn escapes_trailing_period() {
    transform_expect_1("the past.\n", "the past\\.");
}

#[test]
fn converts_emphasis_and_italics() {
    transform_expect_1(
        "into a **multi‑step compressor** and *never* feeding",
        "into a *multi‑step compressor* and _never_ feeding",
    );
}

#[test]
fn converts_heading() {
    transform_expect_1("## 1. What", "*✏ 1\\. What*");
}

#[test]
fn escapes_inline_code() {
    transform_expect_1(
        "`messages = [{role: \"user\"|\"assistant\", content: string}, …]`",
        "`messages \\= \\[\\{role: \"user\"\\|\"assistant\", content: string\\}, …\\]`",
    );
}

#[test]
fn list_after_blank_line() {
    transform_expect_1(
        "Assume:\n\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume:\n\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
    );
}

#[test]
fn list_without_blank_line() {
    transform_expect_1(
        "Assume:\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume:\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
    );
}

#[test]
fn preserves_code_block_language_and_escapes() {
    transform_expect_1(
        "```text\ntoken_count(text)\n```",
        "```text\ntoken\\_count\\(text\\)\n```",
    );
}

#[test]
fn preserves_blockquote_blank_line() {
    transform_expect_1("> You\n> \n> Hi", ">You\n>\n>Hi");
}

#[test]
fn converts_list_inside_blockquote() {
    transform_expect_1(
        "> - Greetings\n> - Repetitive",
        ">⦁ Greetings\n>⦁ Repetitive",
    );
}

#[test]
fn converts_bold_inside_blockquote() {
    transform_expect_1("> **GOAL:** ", ">*GOAL:*");
}

#[test]
fn splits_words_to_fit_len_5() {
    transform_expect_n("12345 12345", "12345===12345", 5);
}

#[test]
fn splits_words_to_fit_len_10() {
    transform_expect_n("12345 12345", "12345===12345", 10);
}

#[test]
fn keeps_words_when_len_allows() {
    transform_expect_n("12345 12345", "12345 12345", 11);
}

#[test]
fn splits_code_block_line_len_18() {
    transform_expect_n(
        "```\n1234567890\n1234567890\n```",
        "```\n1234567890\n```===```\n1234567890\n```",
        18,
    );
}

#[test]
fn splits_code_block_line_len_19() {
    transform_expect_n(
        "```\n1234567890\n1234567890\n```",
        "```\n1234567890\n```===```\n1234567890\n```",
        19,
    );
}

#[test]
fn splits_code_block_line_len_28() {
    transform_expect_n(
        "```\n1234567890\n1234567890\n```",
        "```\n1234567890\n```===```\n1234567890\n```",
        28,
    );
}

#[test]
fn keeps_code_block_line_len_29() {
    transform_expect_n(
        "```\n1234567890\n1234567890\n```",
        "```\n1234567890\n1234567890\n```",
        29,
    );
}

#[test]
fn splits_mixed_text_and_code_block() {
    transform_expect_n(
        "this text is 30ty chars long11\n```\n1234567890\n1234567890\n```",
        "this text is 30ty chars long11===```\n1234567890\n1234567890\n```",
        40,
    );
}

#[test]
fn removes_empty_lines_on_split_3() {
    transform_expect_n("1234567890\n\n1234567890", "1234567890===1234567890", 10);
}

#[test]
fn preserves_empty_lines_no_split() {
    transform_expect_1(
        "> 1234567890\n> \n> 1234567890",
        ">1234567890\n>\n>1234567890",
    );
}

#[test]
fn text_in_angle_brackets_should_not_be_removed() {
    transform_expect_1(
        "> <insert segment_summary>  ",
        "><insert segment\\_summary\\>",
    );
}

#[test]
fn test1() {
    let input = include_str!("2-input.md");
    let chunks = transform(input, 99999);
    let actual = chunks.join("===");
    let expected = include_str!("2-output.txt");

    // std::fs::write("tests/2-output.txt", &actual).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn test3() {
    let input = include_str!("1-input.md");
    let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);
    let actual = chunks.join("===");

    // std::fs::write("tests/1-output.txt", &actual).unwrap();

    let expected = include_str!("1-output.txt");
    assert_eq!(actual, expected);
}
