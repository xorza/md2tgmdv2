use md2tgmdv2::Converter;

fn transform_expect_1(input: &str, expected: &str) {
    let chunks = Converter::default().go(input).unwrap();

    println!("chunks: {:?}", chunks);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], expected);
}

#[allow(dead_code)]
fn transform_expect_n(input: &str, expected: &str, max_chunk_length: usize) {
    let chunks = Converter::new(max_chunk_length).go(input).unwrap();
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
fn preserves_single_newline_between_lines() {
    transform_expect_1("hi\nhello", "hi\nhello");
}

#[test]
fn preserves_double_blank_line() {
    transform_expect_1("hi\n\nhello", "hi\nhello");
}

#[test]
fn converts_bold_list_item() {
    transform_expect_1("- **Split** it into", "⦁ *Split* it into");
}

#[test]
fn preserves_text_before_list() {
    transform_expect_1("test\n\n- **Split** it into", "test\n⦁ *Split* it into");
}

#[test]
fn preserves_text_before_list1() {
    transform_expect_1("test\n- **Split** it into", "test\n⦁ *Split* it into");
}

#[test]
fn escapes_parentheses_in_text() {
    transform_expect_1(
        "Optionally (hierarchical);",
        "Optionally \\(hierarchical\\);",
    );
}

#[test]
fn escapes_trailing_period_in_line() {
    transform_expect_1("the past.\n", "the past\\.");
}

#[test]
fn converts_bold_and_italics() {
    transform_expect_1(
        "into a **multi‑step compressor** and *never* feeding",
        "into a *multi‑step compressor* and _never_ feeding",
    );
}

#[test]
fn converts_heading_to_star_heading() {
    transform_expect_1("## 1. What", "*⭐ 1\\. What*");
}

#[test]
fn escapes_inline_code_markers() {
    transform_expect_1(
        "`messages = [{role: \"user\"|\"assistant\", content: string}, …]`",
        "`messages \\= \\[\\{role: \"user\"\\|\"assistant\", content: string\\}, …\\]`",
    );
}

#[test]
fn converts_list_after_blank_line() {
    transform_expect_1(
        "Assume:\n\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume:\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
    );
}

#[test]
fn converts_list_after_blank_line1() {
    transform_expect_1(
        "Assume.\n- `MODEL_CONTEXT_TOKENS` = max",
        "Assume\\.\n⦁ `MODEL\\_CONTEXT\\_TOKENS` \\= max",
    );
}

#[test]
fn escapes_inside_code_block_language() {
    transform_expect_1(
        "```text\ntoken_count(text)\n```",
        "```text\ntoken\\_count\\(text\\)\n```",
    );
}

#[test]
fn preserves_blockquote_blank_line_between_lines() {
    transform_expect_1("> You\n> \n> Hi", ">You\n>Hi");
}

#[test]
fn converts_list_items_inside_blockquote() {
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
         >*EXCLUDE OR MINIMIZE:*\n\
         >⦁ Greetings, small talk, and filler conversation\n\
         >⦁ Repetitive text that adds no new information",
    );
}

#[test]
fn preserves_blockquote_with_empty_line_without_split() {
    transform_expect_1("> 1234567890\n> \n> 1234567890", ">1234567890\n>1234567890");
}

#[test]
fn keeps_angle_bracket_text_inline() {
    transform_expect_1(
        ">hello <insert segment_summary>  ",
        ">hello <insert segment\\_summary\\>",
    );
}

#[test]
fn keeps_angle_bracket_text_on_own_line() {
    transform_expect_1(
        "> <insert segment_summary>  ",
        "><insert segment\\_summary\\>",
    );
}

#[test]
fn converts_ordered_list_to_bullets() {
    transform_expect_1("1. First\n2. Second", "1\\. First\n2\\. Second");
}

#[test]
fn nested_lists() {
    transform_expect_1(
        "1. First\n   - Second\n2. Third",
        "1\\. First\n  ⦁ Second\n2\\. Third",
    );
}

#[test]
fn preserves_nested_blockquote_levels() {
    transform_expect_1("> > Nested", ">>Nested");
}

#[test]
fn escapes_parentheses_in_link_url() {
    transform_expect_1(
        "[see docs](https://example.com/path(a)/page)",
        "[see docs](https://example\\.com/path\\(a\\)/page)",
    );
}

#[test]
fn renders_image_as_link() {
    transform_expect_1(
        "![logo](https://example.com/path(a)/img.png)",
        "[logo](https://example\\.com/path\\(a\\)/img\\.png)",
    );
}

#[test]
fn heading_followed_by_list_without_blank_line() {
    transform_expect_1("## Heading\n- item", "*⭐ Heading*\n⦁ item");
}

#[test]
fn converts_thematic_break_to_em_dash_bar() {
    transform_expect_1(
        "some test\n\n---\n\nsome more test",
        "some test\n\n———\n\nsome more test",
    );
}

#[test]
fn converts_thematic_break_after_line_to_heading() {
    transform_expect_1(
        "some test\n---\nsome more test",
        "*⭐ some test*\nsome more test",
    );
}

#[test]
fn preserves_newlines_around_list() {
    let input = "- text\n\nmore text";
    let expected = "⦁ text\nmore text";
    transform_expect_1(input, expected);
}

#[test]
fn converts_blockquote_with_list_and_bold() {
    let input = "> - Any explicit\n>\n> **text**\n> - greetings";
    let expected = ">⦁ Any explicit\n>*text*\n>⦁ greetings";
    transform_expect_1(input, expected);
}

#[test]
fn converts_blockquote_heading_and_list_item() {
    let input = "> **GOAL:**\n> - Merge.";
    let expected = ">*GOAL:*\n>⦁ Merge\\.";
    transform_expect_1(input, expected);
}

#[test]
fn asdadasd() {
    let input = "a\n\n> b";
    let expected = "a\n>b";
    transform_expect_1(input, expected);
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
fn url_not_split_across_chunks() {
    let input = "1234567890123456789012345678901234567890123456789012345678901234567890 [see docs](https://example.com/path)";
    let expected = "1234567890123456789012345678901234567890123456789012345678901234567890===[see docs](https://example\\.com/path)";
    transform_expect_n(input, expected, 80);
}

#[test]
fn asd() {
    transform_expect_1(
        "```rust\n1234567890\n```\n```java\n1234567890\n```",
        "```rust\n1234567890\n```\n```java\n1234567890\n```",
    );
}

// #[test]
// fn test1() -> anyhow::Result<()> {
//     let input = include_str!("1-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("1-output.txt");

//     std::fs::write("tests/1-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }

// #[test]
// fn test2() -> anyhow::Result<()> {
//     let input = include_str!("2-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("2-output.txt");

//     std::fs::write("tests/2-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }

// #[test]
// fn test3() -> anyhow::Result<()> {
//     let input = include_str!("3-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("3-output.txt");

//     std::fs::write("tests/3-output.txt", &actual).unwrap();
//     // assert_eq!(actual, expected);

//     Ok(())
// }

// #[test]
// fn test4() -> anyhow::Result<()> {
//     let input = include_str!("4-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("4-output.txt");

//     std::fs::write("tests/4-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }

// #[test]
// fn test5() -> anyhow::Result<()> {
//     let input = include_str!("5-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("5-output.txt");

//     std::fs::write("tests/5-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }

// #[test]
// fn test6() -> anyhow::Result<()> {
//     let input = include_str!("6-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("6-output.txt");

//     std::fs::write("tests/6-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }
// #[test]
// fn test7() -> anyhow::Result<()> {
//     let input = include_str!("7-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("7-output.txt");

//     std::fs::write("tests/7-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }

// #[test]
// fn test8() -> anyhow::Result<()> {
//     let input = include_str!("8-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("8-output.txt");

//     std::fs::write("tests/8-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }

// #[test]
// fn test9() -> anyhow::Result<()> {
//     let input = include_str!("9-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("9-output.txt");

//     std::fs::write("tests/9-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }

// #[test]
// fn test10() -> anyhow::Result<()> {
//     let input = include_str!("10-input.md");
//     let chunks = Converter::default().go(input)?;
//     let actual = chunks.join("===");
//     let _expected = include_str!("10-output.txt");

//     std::fs::write("tests/10-output.txt", &actual).unwrap();
//     //assert_eq!(actual, expected);

//     Ok(())
// }
