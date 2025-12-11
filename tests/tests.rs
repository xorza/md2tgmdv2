#![allow(unused_imports)]

use md2tgmdv2::{Converter, TELEGRAM_BOT_MAX_MESSAGE_LENGTH};

fn transform_expect_1(input: &str, expected: &str) {
    let chunks = Converter::default().go(input).unwrap();

    println!("chunks: {:?}", chunks);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], expected);
}

// fn transform_expect_n(input: &str, expected: &str, max_chunk_length: usize) {
//     let chunks = transform(input, max_chunk_length);
//     let actual = chunks.join("===");

//     assert_eq!(actual, expected);
//     for (i, chunk) in chunks.iter().enumerate() {
//         assert!(
//             chunk.len() <= max_chunk_length,
//             "chunk {} length {} exceeds max_chunk_length {}",
//             i,
//             chunk.len(),
//             max_chunk_length
//         );
//     }
// }

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
    transform_expect_1("## 1. What", "*⭐ 1\\. What*");
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
        "Assume:\n\n\r\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume:\n\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
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
fn preserves_blockquote_blank_line_before_heading() {
    transform_expect_1(
        "> - Any decisions made, final answers given, or conclusions reached\n\
         > - Any explicit open questions or TODO items mentioned\n\
         > \n\
         > **EXCLUDE OR MINIMIZE:**\n\
         > - Greetings, small talk, and filler conversation\n\
         > - Repetitive text that adds no new information\n",
        ">⦁ Any decisions made, final answers given, or conclusions reached\n\
         >⦁ Any explicit open questions or TODO items mentioned\n\
         >\n\
         >*EXCLUDE OR MINIMIZE:*\n\
         >\n\
         >⦁ Greetings, small talk, and filler conversation\n\
         >⦁ Repetitive text that adds no new information",
    );
}

// #[test]
// fn splits_words_to_fit_len_5() {
//     transform_expect_n("12345 12345", "12345===12345", 5);
// }

// #[test]
// fn splits_words_to_fit_len_10() {
//     transform_expect_n("12345 12345", "12345===12345", 10);
// }

// #[test]
// fn keeps_words_when_len_allows() {
//     transform_expect_n("12345 12345", "12345 12345", 11);
// }

// #[test]
// fn splits_code_block_line_len_18() {
//     transform_expect_n(
//         "```\n1234567890\n1234567890\n```",
//         "```\n1234567890\n```===```\n1234567890\n```",
//         18,
//     );
// }

// #[test]
// fn splits_code_block_line_len_19() {
//     transform_expect_n(
//         "```\n1234567890\n1234567890\n```",
//         "```\n1234567890\n```===```\n1234567890\n```",
//         19,
//     );
// }

// #[test]
// fn splits_code_block_line_len_28() {
//     transform_expect_n(
//         "```\n1234567890\n1234567890\n```",
//         "```\n1234567890\n```===```\n1234567890\n```",
//         28,
//     );
// }

// #[test]
// fn keeps_code_block_line_len_29() {
//     transform_expect_n(
//         "```\n1234567890\n1234567890\n```",
//         "```\n1234567890\n1234567890\n```",
//         29,
//     );
// }

// #[test]
// fn splits_mixed_text_and_code_block() {
//     transform_expect_n(
//         "this text is 30ty chars long11\n```\n1234567890\n1234567890\n```",
//         "this text is 30ty chars long11===```\n1234567890\n1234567890\n```",
//         40,
//     );
// }

// #[test]
// fn removes_empty_lines_on_split_3() {
//     transform_expect_n("1234567890\n\n1234567890", "1234567890===1234567890", 10);
// }

// #[test]
// fn preserves_empty_lines_no_split() {
//     transform_expect_1(
//         "> 1234567890\n> \n> 1234567890",
//         ">1234567890\n>\n>1234567890",
//     );
// }

// #[test]
// fn text_in_angle_brackets_should_not_be_removed() {
//     transform_expect_1(
//         "> <insert segment_summary>  ",
//         "><insert segment\\_summary\\>",
//     );
// }

// #[test]
// fn ordered_list_items_convert() {
//     transform_expect_1("1. First\n2. Second", "⦁ First\n⦁ Second");
// }

// #[test]
// fn nested_blockquote_preserves_levels() {
//     transform_expect_1("> > Nested", ">>Nested");
// }

// #[test]
// fn inline_link_escapes_parens_in_url() {
//     transform_expect_1(
//         "[see docs](https://example.com/path(a)/page)",
//         "[see docs](https://example.com/path\\(a\\)/page)",
//     );
// }

// #[test]
// fn heading_followed_by_list_no_blank_line() {
//     transform_expect_1("## Heading\n- item", "*⭐ Heading*\n⦁ item");
// }

// #[test]
// fn asdasd() {
//     transform_expect_1(
//         "some test\n\n---\n\nsome more test",
//         "some test\n\n————————\n\nsome more test",
//     );
// }
// #[test]
// fn asdasd2() {
//     transform_expect_1(
//         "some test\n---\nsome more test",
//         "*⭐ some test*\n\nsome more test",
//     );
// }

// #[test]
// fn url_not_split_across_chunks() {
//     let input = "1234567890123456789012345678901234567890123456789012345678901234567890 [see docs](https://example.com/path";
//     let expected = "1234567890123456789012345678901234567890123456789012345678901234567890===[see docs](https://example.com/path";
//     transform_expect_n(input, expected, 80);
// }

// #[test]
// fn preserve_newlines() {
//     let input = "- text\n\nmore text";
//     let expected = "⦁ text\n\nmore text";
//     transform_expect_1(input, expected);
// }

// #[test]
// fn asd() {
//     let input = "> - Any explicit\n>\n> **text**\n> - greetings";
//     let expected = ">⦁ Any explicit\n>\n>*text*\n>⦁ greetings";
//     transform_expect_1(input, expected);
// }

// #[test]
// fn test1() {
//     let input = include_str!("1-input.md");
//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);
//     let actual = chunks.join("===");
//     let expected = include_str!("1-output.txt");

//     std::fs::write("tests/1-output.txt", &actual).unwrap();
//     // assert_eq!(actual, expected);
// }

// #[test]
// fn test2() {
//     let input = include_str!("2-input.md");
//     let chunks = transform(input, 99999);
//     let actual = chunks.join("===");
//     let expected = include_str!("2-output.txt");

//     std::fs::write("tests/2-output.txt", &actual).unwrap();
//     assert_eq!(actual, expected);
// }

// #[test]
// fn test3() {
//     let input = include_str!("3-input.md");
//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);
//     let actual = chunks.join("===");
//     let expected = include_str!("3-output.txt");

//     std::fs::write("tests/3-output.txt", &actual).unwrap();
//     // assert_eq!(actual, expected);
// }

// #[test]
// fn test4() {
//     let input = include_str!("4-input.md");
//     let chunks = transform(input, TELEGRAM_BOT_MAX_MESSAGE_LENGTH);
//     let actual = chunks.join("===");
//     let expected = include_str!("4-output.txt");

//     // std::fs::write("tests/4-output.txt", &actual).unwrap();
//     assert_eq!(actual, expected);
// }
