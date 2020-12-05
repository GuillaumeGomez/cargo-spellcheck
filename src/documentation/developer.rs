
use std::fmt;
use std::fmt::{Display, Formatter};

use ra_ap_syntax::tokenize;
use regex::Regex;

use super::*;

/// Prefix string for a developer block comment
const BLOCK_COMMENT_PREFIX: &str = "/*";

/// Prefix string for a developer line comment
const LINE_COMMENT_PREFIX: &str = "//";

/// Prefix string for any other token type (i.e. we don't care)
const OTHER_PREFIX: &str = "";

/// Postfix string for a developer block comment
const BLOCK_COMMENT_POSTFIX: &str = "*/";

/// Postfix string for a developer line comment
const LINE_COMMENT_POSTFIX: &str = "";

/// Postfix string for any other token type (i.e. we don't care)
const OTHER_POSTFIX: &str = "";

lazy_static::lazy_static! {
  static ref BLOCK_COMMENT: Regex = Regex::new(r"^/\*(?s)(?P<content>.*)\*/$")
      .expect("Failed to create regular expression to identify (closed) developer block comments. \
          Please check this regex!");
  static ref LINE_COMMENT: Regex = Regex::new(r"^//([^[/|!]].*)$")
      .expect("Failed to create regular expression to identify developer line comments. \
          Please check this regex!");
}

/// A string token from a source string with the location at which it occurs in the source string
/// in 0 indexed bytes
#[derive(Debug)]
pub struct TokenWithLocation {
  /// The full contents of this token, including pre/post characters (like '//')
  content: String,
  /// The location of the start of this token in the source string, in bytes
  location: usize
}

/// A string token from a source string with the location at which it occurs in the source string
/// as line on which it occurs (1 indexed) and the column of its first character (0 indexed)
#[derive(Debug)]
pub struct TokenWithLineColumn {
  /// The full contents of this token, including pre/post characters (like '//')
  content: String,
  /// The first line on which the token appears in the source file (1 indexed)
  line: usize,
  /// The column where the first character of this token appears in the source file (0 indexed)
  column: usize
}

/// Is a token of type (developer) block comment, (developer) line comment or something else
#[derive(Debug, Eq, PartialEq)]
enum TokenType {
  BlockComment,
  LineComment,
  Other
}

impl Display for TokenType {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let kind = match self {
      TokenType::BlockComment => "developer block comment",
      TokenType::LineComment => "developer line comment",
      TokenType::Other => "not a developer comment"
    };
    write!(f, "{}", kind)
  }
}

impl TokenType {
  /// The prefix string for this type of token
  fn pre(&self) -> &str {
    match self {
      TokenType::BlockComment => BLOCK_COMMENT_PREFIX,
      TokenType::LineComment => LINE_COMMENT_PREFIX,
      TokenType::Other => OTHER_PREFIX
    }
  }
  /// The postfix string for this type of token
  fn post(&self) -> &str {
    match self {
      TokenType::BlockComment => BLOCK_COMMENT_POSTFIX,
      TokenType::LineComment => LINE_COMMENT_POSTFIX,
      TokenType::Other => OTHER_POSTFIX
    }
  }
  /// The length of the prefix for the token in characters
  fn pre_in_chars(&self) -> usize {
    self.pre().chars().count()
  }
  /// The length of the postfix for the token in characters
  fn post_in_chars(&self) -> usize {
    self.post().chars().count()
  }
}

/// A token from a source string with its variant (TokenType) and the line and column on which it
/// occurs according to the description for `TokenWithLineColumn`
#[derive(Debug)]
pub struct TokenWithType {
  /// Is the token a block developer comment, line developer comment or something else
  kind: TokenType,
  /// The full contents of this token, including pre/post characters (like '//')
  pub content: String,
  /// The first line on which the token appears in the source file (1 indexed)  pub line: usize,
  pub line: usize,
  /// The column where the first character of this token appears in the source file (0 indexed)
  pub column: usize,
}

impl TokenWithType {
  /// Convert a `TokenWithLineColumn` to a `TokenWithType`. The kind is worked out from the content
  /// by checking against the developer block comment & line comment regexes.
  fn from (token: TokenWithLineColumn) -> Self {
    let kind = {
      if BLOCK_COMMENT.is_match(&token.content) {
        TokenType::BlockComment
      } else if LINE_COMMENT.is_match(&token.content) {
        TokenType::LineComment
      } else {
        TokenType::Other
      }
    };
    Self { kind, content: token.content, line: token.line, column: token.column }
  }
}

/// A convenience method that runs the complete 'pipeline' from string `source` file to all
/// `LiteralSet`s that can be created from developer comments in the source
pub fn extract_developer_comments(source: &str) -> Vec<LiteralSet> {
  let tokens = retain_only_developer_comments(
        token_with_line_column_to_token_with_type(
            tokens_with_location_to_tokens_with_line_and_column(
                source, source_to_tokens_with_location(source))));
  let mut literal_sets = vec!();
  let block_comments: Vec<&TokenWithType> = tokens.iter()
      .filter(|t| t.kind == TokenType::BlockComment).collect();
  let line_comments: Vec<&TokenWithType> = tokens.iter()
      .filter(|t| t.kind == TokenType::LineComment).collect();
  for set in literal_sets_from_line_comments(line_comments) {
    literal_sets.push(set);
  }
  for comment in block_comments {
    match literal_set_from_block_comment(comment) {
      Ok(ls) => literal_sets.push(ls),
      Err(s) => log::trace!(
          "Failed to create literal set from token with content \"{}\" due to \"{}\"",
          comment.content, s)
    }
  }
  literal_sets
}

/// Creates a series of `TokenWithLocation`s from a source string
pub fn source_to_tokens_with_location(source: &str) -> Vec<TokenWithLocation> {
  let ra_tokens = tokenize(source).0;
  let mut tokens = vec!();
  let mut location = 0;
  for token in ra_tokens {
    let length = usize::from(token.len);
    tokens.push(TokenWithLocation{
      content: source[location..location + length].to_string(),
      location
    });
    location += length;
  }
  tokens
}

/// Converts a series of `TokenWithLocation`s to `TokenWithLineColumn`s. Requires the source string
/// to calculate line & column from location.
pub fn tokens_with_location_to_tokens_with_line_and_column
    (source: &str, tokens_in: Vec<TokenWithLocation>) -> Vec<TokenWithLineColumn> {
  let mut tokens_out = vec!();
  for token in tokens_in {
    tokens_out.push(TokenWithLineColumn{
      content: token.content,
      line: count_lines(&source[..token.location]),
      column: calculate_column(&source[..token.location])
    });
  }
  tokens_out
}

/// Given a string, calculates the 1 indexed line number of the line on which the final character
/// of the string appears
pub fn count_lines(fragment: &str) -> usize {
  fragment.chars().into_iter().filter(|c| *c == '\n').count() + 1
}

/// Given a string, calculates the 0 indexed column number of the character *just after* the final
/// character in the string
pub fn calculate_column(fragment: &str) -> usize {
  match fragment.rfind('\n') {
    Some(p) => fragment.chars().count() - fragment[..p].chars().count() - 1,
    None => fragment.chars().count()
  }
}

/// Converts a series of `TokenWithLineColumn`s to `TokenWithType`s
fn token_with_line_column_to_token_with_type(tokens_in: Vec<TokenWithLineColumn>)
    -> Vec<TokenWithType> {
  tokens_in.into_iter().map(|t| TokenWithType::from(t)).collect()
}

/// Returns a vector containing only the tokens from the input vector which are developer comments
fn retain_only_developer_comments(tokens: Vec<TokenWithType>) -> Vec<TokenWithType> {
  tokens.into_iter()
      .filter(|t| t.kind != TokenType::Other)
      .collect()
}

/// Attempts to create a `LiteralSet` from a token assuming it is block comment. Returns `None` if
/// the token kind is not `TokenKind::BlockComment`, if the token content does not match the
/// block comment regex, or if any line cannot be added by `LiteralSet::add_adjacent`
fn literal_set_from_block_comment(token: &TokenWithType) -> Result<LiteralSet, String> {
  if token.kind != TokenType::BlockComment {
    return Err(format!("Got token of type {}, need {}", token.kind, TokenType::BlockComment));
  }
  if !BLOCK_COMMENT.is_match(&token.content) {
    return Err(format!(
        "Token claimed to be of type {}, but improperly delimited - actual content \"{}\"",
        TokenType::BlockComment, token.content));
  }
  let number_of_lines = token.content.split("\n").count();
  let mut lines = token.content.split("\n");
  if number_of_lines == 1 {
    let literal = match TrimmedLiteral::from(
        CommentVariant::Unknown, &token.content, token.kind.pre_in_chars(),
        token.kind.post_in_chars(), token.line, token.column) {
      Err(s) => return Err(format!(
          "Failed to create literal from single line block comment, content \"{}\" - caused by \"{}\"",
          token.content, s)),
      Ok(l) => l
    };
    Ok(LiteralSet::from(literal))
  } else {
    let next_line = match lines.next() {
      None => return Err(format!(
        "BUG! Expected block comment \"{}\" to have at least two lines", token.content)),
      Some(l) => l
    };
    let literal = match TrimmedLiteral::from(
        CommentVariant::Unknown, next_line, token.kind.pre_in_chars(), 0,
        token.line, token.column) {
      Err(s) => return Err(format!("Failed to create literal from block comment with content \"{}\" \
          due to error \"{}\"",
          next_line, s)),
      Ok(l) => l
    };
    let mut literal_set = LiteralSet::from(literal);
    let mut line_number = token.line;
    while let Some(next_line) = lines.next() {
      line_number += 1;
      let post = if next_line.ends_with(BLOCK_COMMENT_POSTFIX) {
        TokenType::BlockComment.post_in_chars()
      } else {
        0
      };
      let literal = match TrimmedLiteral::from(
          CommentVariant::Unknown, next_line, 0, post, line_number, 0) {
        Err(s) => return Err(format!("Failed to create literal from content \"{}\" due to error \"{}\"",
            next_line, s)),
        Ok(l) => l
      };
      match literal_set.add_adjacent(literal) {
        Ok(_) => (),
        Err(_) => return Err(format!("Failed to add line with content {} to literal set", next_line))
      }
    }
    Ok(literal_set)
  }
}

/// Attempt to create a literal from a developer line comment token. Returns `None` if the token's
/// kind is not `TokenType::LineComment` or if the call to `TrimmedLiteral::from` fails.
fn literal_from_line_comment(token: &TokenWithType) -> Result<TrimmedLiteral, String> {
  match token.kind {
    TokenType::LineComment => TrimmedLiteral::from(
        CommentVariant::Unknown, &token.content, token.kind.pre_in_chars(),
        token.kind.post_in_chars(), token.line, token.column),
    _ => Err(format!("Expected a token of type {}, got {}", TokenType::LineComment, token.kind))
  }
}

/// Converts a vector of tokens into a vector of `LiteralSet`s based on the developer line comments
/// in the input. Should be called with only line comment tokens in the input, but it is safe to
/// call it with other token types included.
fn literal_sets_from_line_comments(tokens: Vec<&TokenWithType>) -> Vec<LiteralSet> {
  let mut sets = vec!();
  for token in tokens {
    if token.kind != TokenType::LineComment {
      continue;
    }
    let literal = match literal_from_line_comment(token) {
      Err(s) => {
        log::trace!("Failed to create literal from line comment with content \"{}\" due to \"{}\"",
            token.content, s);
        continue;
      },
      Ok(l) => l
    };
    match sets.pop() {
      None => sets.push(LiteralSet::from(literal)),
      Some(mut s) => {
        match s.add_adjacent(literal) {
          Err(literal) => sets.push(LiteralSet::from(literal)),
          Ok(_) => sets.push(s)
        }
      }
    }
  }
  sets
}

#[cfg(test)]
mod tests {
  use crate::documentation::developer::*;

  #[test]
  fn test_count_lines_correctly_counts_lines() {
    // Note: lines are 1 indexed
    assert_eq!(count_lines(""), 1);
    assert_eq!(count_lines("test"), 1);
    assert_eq!(count_lines("test\ntest"), 2);
    assert_eq!(count_lines("test\ntest\n something else \n"), 4);
    assert_eq!(count_lines("\n test\ntest\n something else \n"), 5);
  }

  #[test]
  fn test_calculate_column_correctly_calculates_final_column_of_last_line() {
    // Note: next column after last, in chars, zero indexed
    assert_eq!(calculate_column(""), 0);
    assert_eq!(calculate_column("test中"), 5);
    assert_eq!(calculate_column("test\n"), 0);
    assert_eq!(calculate_column("test\ntest2"), 5);
    assert_eq!(calculate_column("test\ntest中2"), 6);
    assert_eq!(calculate_column("test\ntest中2\n中3"), 2);
  }

  #[test]
  fn test_source_to_token_with_location_calculates_correct_locations() {
    {
      let tokens = source_to_tokens_with_location("/* test */\n// test");
      assert_eq!(tokens.get(0).unwrap().location, 0); // Block comment
      assert_eq!(tokens.get(1).unwrap().location, 10); // Whitespace
      assert_eq!(tokens.get(2).unwrap().location, 11); // Line comment
    }
    {
      let tokens = source_to_tokens_with_location("/* te中st */\n// test");
      assert_eq!(tokens.get(0).unwrap().location, 0); // Block comment
      assert_eq!(tokens.get(1).unwrap().location, 13); // Whitespace
      assert_eq!(tokens.get(2).unwrap().location, 14); // Line comment
    }
    {
      let tokens = source_to_tokens_with_location("/* te中st */\n// test\nfn 中(){\t}");
      assert_eq!(tokens.get(0).unwrap().location, 0); // Block comment
      assert_eq!(tokens.get(1).unwrap().location, 13); // Whitespace
      assert_eq!(tokens.get(2).unwrap().location, 14); // Line comment
      assert_eq!(tokens.get(3).unwrap().location, 21); // Whitespace
      assert_eq!(tokens.get(4).unwrap().location, 22); // Function keyword
      assert_eq!(tokens.get(5).unwrap().location, 24); // Whitespace
      assert_eq!(tokens.get(6).unwrap().location, 25); // Function name
      assert_eq!(tokens.get(7).unwrap().location, 28); // Open bracket
      assert_eq!(tokens.get(8).unwrap().location, 29); // Close bracket
      assert_eq!(tokens.get(9).unwrap().location, 30); // Open curly bracket
      assert_eq!(tokens.get(10).unwrap().location, 31); // Whitespace
      assert_eq!(tokens.get(11).unwrap().location, 32); // Close curly bracket
    }
  }

  #[test]
  fn test_tokens_with_line_column_values_set_correctly() {
    {
      let source = "/* test */\n// test";
      let tokens = source_to_tokens_with_location(source);
      let tokens = tokens_with_location_to_tokens_with_line_and_column(
        source, tokens);
      assert_eq!(tokens.get(0).unwrap().line, 1); // Block comment
      assert_eq!(tokens.get(0).unwrap().column, 0);
      assert_eq!(tokens.get(1).unwrap().line, 1); // Whitespace
      assert_eq!(tokens.get(1).unwrap().column, 10);
      assert_eq!(tokens.get(2).unwrap().line, 2); // Line comment
      assert_eq!(tokens.get(2).unwrap().column, 0);
    }
    {
      let source = "/* te中st */\n// test";
      let tokens = source_to_tokens_with_location(source);
      let tokens = tokens_with_location_to_tokens_with_line_and_column(
        source, tokens);
      assert_eq!(tokens.get(0).unwrap().line, 1); // Block comment
      assert_eq!(tokens.get(0).unwrap().column, 0);
      assert_eq!(tokens.get(1).unwrap().line, 1); // Whitespace
      assert_eq!(tokens.get(1).unwrap().column, 11);
      assert_eq!(tokens.get(2).unwrap().line, 2); // Line comment
      assert_eq!(tokens.get(2).unwrap().column, 0);
    }
    {
      let source = "/* te中st */\n// test\nfn 中(){\t}";
      let tokens = source_to_tokens_with_location(source);
      let tokens = tokens_with_location_to_tokens_with_line_and_column(
        source, tokens);
      assert_eq!(tokens.get(0).unwrap().line, 1); // Block comment
      assert_eq!(tokens.get(0).unwrap().column, 0);
      assert_eq!(tokens.get(1).unwrap().line, 1); // Whitespace
      assert_eq!(tokens.get(1).unwrap().column, 11);
      assert_eq!(tokens.get(2).unwrap().line, 2); // Line comment
      assert_eq!(tokens.get(2).unwrap().column, 0);
      assert_eq!(tokens.get(3).unwrap().line, 2); // Whitespace
      assert_eq!(tokens.get(3).unwrap().column, 7);
      assert_eq!(tokens.get(4).unwrap().line, 3); // Function keyword
      assert_eq!(tokens.get(4).unwrap().column, 0);
      assert_eq!(tokens.get(5).unwrap().line, 3); // Whitespace
      assert_eq!(tokens.get(5).unwrap().column, 2);
      assert_eq!(tokens.get(6).unwrap().line, 3); // Function name
      assert_eq!(tokens.get(6).unwrap().column, 3);
      assert_eq!(tokens.get(7).unwrap().line, 3); // Open bracket
      assert_eq!(tokens.get(7).unwrap().column, 4);
      assert_eq!(tokens.get(8).unwrap().line, 3); // Close bracket
      assert_eq!(tokens.get(8).unwrap().column, 5);
      assert_eq!(tokens.get(9).unwrap().line, 3); // Open curly bracket
      assert_eq!(tokens.get(9).unwrap().column, 6);
      assert_eq!(tokens.get(10).unwrap().line, 3); // Whitespace
      assert_eq!(tokens.get(10).unwrap().column, 7);
      assert_eq!(tokens.get(11).unwrap().line, 3); // Close curly bracket
      assert_eq!(tokens.get(11).unwrap().column, 8);
    }
  }

  #[test]
  fn test_identify_token_type_assigns_block_comment_type_to_block_comments() {
    let block_comments = vec!(
        TokenWithLineColumn {
          content: "/* Block Comment */".to_string(),
          line: 0,
          column: 0
        },
        TokenWithLineColumn {
          content: "/* Multiple Line\nBlock Comment */".to_string(),
          line: 0,
          column: 0
        }
    );
    for token in block_comments {
      assert_eq!(identify_token_type(token).kind, TokenType::BlockComment);
    }
  }

  #[test]
  fn test_identify_token_type_assigns_line_comment_type_to_line_comments() {
    let line_comments = vec!(
      TokenWithLineColumn {
        content: "// Line Comment ".to_string(),
        line: 0,
        column: 0
      }
    );
    for token in line_comments {
      assert_eq!(identify_token_type(token).kind, TokenType::LineComment);
    }
  }

  #[test]
  fn test_identify_token_type_assigns_other_type_to_non_developer_comments() {
    let not_developer_comments = vec!(
      TokenWithLineColumn {
        content: "fn".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: " ".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: "\n".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: "function_name".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: "(".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: ")".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: ";".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: "{".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: "}".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: "/// Outer documentation comment".to_string(),
        line: 0,
        column: 0
      },
      TokenWithLineColumn {
        content: "//! Inner documentation comment".to_string(),
        line: 0,
        column: 0
      }
    );
    for token in not_developer_comments {
      assert_eq!(identify_token_type(token).kind, TokenType::Other);
    }
  }

  #[test]
  fn retain_only_developer_comments_removes_non_comment_tokens() {
    let block_comment = "/* A block comment */";
    let line_comment = "// A line comment";
    let function_keyword = "fn";
    let function_name = "func中";
    let left_bracket = "(";
    let right_bracket = ")";
    let left_brace = "{";
    let right_brace = "}";
    let left_add = "1";
    let right_add = "2";
    let plus = "+";
    let semicolon = ";";
    let newline = "\n";
    let whitespace = " ";
    let source = format!("{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        block_comment, newline,
        function_keyword, whitespace, function_name, left_bracket, right_bracket, whitespace,
        left_brace, newline, whitespace, whitespace, line_comment, newline, whitespace, whitespace,
        left_add, whitespace, plus, whitespace, right_add, semicolon, newline, right_brace);

    let should_be_excluded = vec!(function_keyword, function_name, left_bracket, newline,
        right_bracket, left_brace, right_brace, left_add, plus, right_add, whitespace, semicolon);

    let tokens = source_to_tokens_with_location(&source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(&source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    let filtered = retain_only_developer_comments(tokens);
    for token in filtered {
      for content in &should_be_excluded {
        assert_ne!(&token.content, content);
      }
    }
  }

  #[test]
  fn retain_only_developer_comments_removes_documentation_comment_tokens() {
    let block_comment = "/* A block comment */";
    let line_comment = "// A line comment";
    let inner_documentation_comment = "//! An inner documentation comment";
    let outer_documentation_comment = "/// An outer documentation comment";
    let newline = "\n";
    let source = format!("{}{}{}{}{}{}{}{}", block_comment, newline, line_comment, newline,
        outer_documentation_comment, newline, inner_documentation_comment, newline);

    let should_be_excluded = vec!(outer_documentation_comment, inner_documentation_comment);

    let tokens = source_to_tokens_with_location(&source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(&source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    let filtered = retain_only_developer_comments(tokens);
    for token in filtered {
      for content in &should_be_excluded {
        assert_ne!(&token.content, content);
      }
    }
  }

  #[test]
  fn retain_only_developer_comments_keeps_developer_comment_tokens() {
    let block_comment = "/* A block comment */";
    let line_comment = "// A line comment";
    let function_keyword = "fn";
    let function_name = "func中";
    let left_bracket = "(";
    let right_bracket = ")";
    let left_brace = "{";
    let right_brace = "}";
    let left_add = "1";
    let right_add = "2";
    let plus = "+";
    let semicolon = ";";
    let newline = "\n";
    let whitespace = " ";
    let source = format!("{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
        block_comment, newline,
        function_keyword, whitespace, function_name, left_bracket, right_bracket, whitespace,
        left_brace, newline, whitespace, whitespace, line_comment, newline, whitespace, whitespace,
        left_add, whitespace, plus, whitespace, right_add, semicolon, newline, right_brace);

    let should_be_included = vec!(block_comment, line_comment);

    let tokens = source_to_tokens_with_location(&source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(&source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    let filtered = retain_only_developer_comments(tokens);
    for content in should_be_included {
      let matches: Vec<&TokenWithType> = filtered.iter()
          .filter(|t| t.content == content)
          .collect();
      assert!(matches.len() > 0);
    }
  }

  #[test]
  fn test_single_line_block_comment_literal_correctly_created() {
    let source = "/* block 种 comment */";
    let tokens = source_to_tokens_with_location(source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    assert_eq!(tokens.len(), 1);
    let token = tokens.into_iter().last().unwrap();
    let literal_set = literal_set_from_block_comment(&token);
    assert!(literal_set.is_ok());
    let literal_set = literal_set.unwrap();
    assert_eq!(literal_set.len(), 1);
    let literal = literal_set.literals().into_iter().last().unwrap();
    assert_eq!(literal.pre(), TokenType::BlockComment.pre_in_chars());
    assert_eq!(literal.post(), TokenType::BlockComment.post_in_chars());
    assert_eq!(literal.len_in_chars(), source.chars().count() - 4);
    assert_eq!(literal.len(), source.len() - 4);
    let span = &literal.span();
    assert_eq!(span.start.line, 1);
    assert_eq!(span.start.column, 2);
    assert_eq!(span.end.line, 1);
    assert_eq!(span.end.column, source.chars().count() - 2 - 1);
  }

  #[test]
  fn test_single_line_indented_block_comment_literal_correctly_created() {
    let source = "    /* block 种 comment */";
    let tokens = source_to_tokens_with_location(source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    assert!(tokens.len() > 0);
    let token = tokens.into_iter().last().unwrap();
    let literal_set = literal_set_from_block_comment(&token);
    assert!(literal_set.is_ok());
    let literal_set = literal_set.unwrap();
    assert_eq!(literal_set.len(), 1);
    let literal = literal_set.literals().into_iter().last().unwrap();
    let indent_size = "    ".len(); // Also chars, because ASCII
    assert_eq!(literal.pre(), TokenType::BlockComment.pre_in_chars());
    assert_eq!(literal.post(), TokenType::BlockComment.post_in_chars());
    assert_eq!(literal.len_in_chars(), source.chars().count() - indent_size - 4);
    assert_eq!(literal.len(), source.len() - indent_size - 4);
    let span = &literal.span();
    assert_eq!(span.start.line, 1);
    assert_eq!(span.start.column, indent_size + 2);
    assert_eq!(span.end.line, 1);
    assert_eq!(span.end.column, source.chars().count() - 2 - 1);
  }

  #[test]
  fn test_multi_line_block_comment_literal_correctly_created() {
    let source = "/* block\n 种 \ncomment */";
    let tokens = source_to_tokens_with_location(source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    assert_eq!(tokens.len(), 1);
    let token = tokens.into_iter().last().unwrap();
    let literal_set = literal_set_from_block_comment(&token);
    assert!(literal_set.is_ok());
    let literal_set = literal_set.unwrap();
    assert_eq!(literal_set.len(), 3);
    let literals = literal_set.literals();
    {
      let literal = literals.get(0).unwrap();
      assert_eq!(literal.pre(), TokenType::BlockComment.pre_in_chars());
      assert_eq!(literal.post(), "".chars().count());
      assert_eq!(literal.len_in_chars(), " block".chars().count());
      assert_eq!(literal.len(), " block".len());
      let span = &literal.span();
      assert_eq!(span.start.line, 1);
      assert_eq!(span.start.column, 2);
      assert_eq!(span.end.line, 1);
      assert_eq!(span.end.column, "/* block".chars().count() - 1);
    }
    {
      let literal = literals.get(1).unwrap();
      assert_eq!(literal.pre(), "".chars().count());
      assert_eq!(literal.post(), "".chars().count());
      assert_eq!(literal.len_in_chars(), " 种 ".chars().count());
      assert_eq!(literal.len(), " 种 ".len());
      let span = &literal.span();
      assert_eq!(span.start.line, 2);
      assert_eq!(span.start.column, 0);
      assert_eq!(span.end.line, 2);
      assert_eq!(span.end.column, " 种 ".chars().count() - 1);
    }
    {
      let literal = literals.get(2).unwrap();
      assert_eq!(literal.pre(), "".chars().count());
      assert_eq!(literal.post(), TokenType::BlockComment.post_in_chars());
      assert_eq!(literal.len_in_chars(), "comment ".chars().count());
      assert_eq!(literal.len(), "comment ".len());
      let span = &literal.span();
      assert_eq!(span.start.line, 3);
      assert_eq!(span.start.column, 0);
      assert_eq!(span.end.line, 3);
      assert_eq!(span.end.column, "comment ".chars().count() - 1);
    }
  }

  #[test]
  fn test_not_developer_comments_block_comment_converter_does_not_create_literals() {
    let source = "// line comment\n/// Outer documentation\nfn test(){\n \
        //! Inner documentation\n\tlet i = 1 + 2;\n}";
    let tokens = source_to_tokens_with_location(source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    for token in tokens {
      assert!(literal_set_from_block_comment(&token).is_err());
    }
  }

  #[test]
  fn test_non_line_comment_tokens_line_comment_to_literal_does_not_create_literals() {
    let source = "/* Block comment */\nfn test(i: usize) {\n  let j = 1 + i;\n  j\n}";
    let tokens = source_to_tokens_with_location(source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    for token in tokens {
      assert!(literal_from_line_comment(&token).is_err());
    }
  }

  #[test]
  fn test_documentation_line_comment_tokens_line_comment_to_literal_does_not_create_literals() {
    let source = "/// Outer \nfn(){\n//! Inner \n}";
    let tokens = source_to_tokens_with_location(source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    for token in tokens {
      assert!(literal_from_line_comment(&token).is_err());
    }
  }

  #[test]
  fn test_developer_line_comment_tokens_line_comment_to_literal_create_literals_with_correct_data() {
    let source = "// First line comment\nconst ZERO: usize = 0; // A constant ";
    let tokens = source_to_tokens_with_location(source);
    let tokens = tokens_with_location_to_tokens_with_line_and_column(source, tokens);
    let tokens = token_with_line_column_to_token_with_type(tokens);
    let filtered = retain_only_developer_comments(tokens);
    assert_eq!(filtered.len(), 2);
    let literals: Vec<Result<TrimmedLiteral, String>> = filtered.into_iter()
      .map(|t| literal_from_line_comment(&t))
      .collect();
    {
      let literal = literals.get(0).unwrap();
      assert!(literal.is_ok());
      let literal = literal.as_ref().unwrap();
      assert_eq!(literal.pre(), TokenType::LineComment.pre_in_chars());
      assert_eq!(literal.post(), TokenType::LineComment.post_in_chars());
      assert_eq!(literal.len_in_chars(), " First line comment".chars().count());
      assert_eq!(literal.len(), " First line comment".len());
      let span = &literal.span();
      assert_eq!(span.start.line, 1);
      assert_eq!(span.start.column, 2);
      assert_eq!(span.end.line, 1);
      assert_eq!(span.end.column, 2 + " First line comment".chars().count() - 1);
    }
    {
      let literal = literals.get(1).unwrap();
      assert!(literal.is_ok());
      let literal = literal.as_ref().unwrap();
      assert_eq!(literal.pre(), TokenType::LineComment.pre_in_chars());
      assert_eq!(literal.post(), TokenType::LineComment.post_in_chars());
      assert_eq!(literal.len_in_chars(), " A constant ".chars().count());
      assert_eq!(literal.len(), " A constant ".len());
      let span = &literal.span();
      assert_eq!(span.start.line, 2);
      assert_eq!(span.start.column, 25);
      assert_eq!(span.end.line, 2);
      assert_eq!(span.end.column, 25 + " A constant ".chars().count() - 1);
    }
  }
  #[test]
  fn test_single_line_comment_put_in_one_literal_set() {
    let content = " line comment";
    let source = format!("//{}", content);
    let tokens = retain_only_developer_comments(
        token_with_line_column_to_token_with_type(
            tokens_with_location_to_tokens_with_line_and_column(&source,
              source_to_tokens_with_location(&source))));
    let literal_sets = literal_sets_from_line_comments(tokens.iter().collect());
    assert_eq!(literal_sets.len(), 1);
    let literal_set = literal_sets.get(0).unwrap();
    let all_literals = literal_set.literals();
    let literal = all_literals.get(0);
    assert!(literal.is_some());
    let literal = literal.unwrap();
    assert!(literal.as_str().contains(content));
  }

  #[test]
  fn test_adjacent_line_comments_put_in_same_literal_set() {
    let content_1 = " line comment 1 ";
    let content_2 = " line comment 2 ";
    let source = format!("//{}\n//{}", content_1, content_2);
    let tokens = retain_only_developer_comments(
        token_with_line_column_to_token_with_type(
            tokens_with_location_to_tokens_with_line_and_column(&source,
                source_to_tokens_with_location(&source))));
    let literal_sets = literal_sets_from_line_comments(tokens.iter().collect());
    assert_eq!(literal_sets.len(), 1);
    let literal_set = literal_sets.get(0).unwrap();
    let all_literals = literal_set.literals();
    assert_eq!(all_literals.len(), 2);
    {
      let literal = all_literals.get(0).unwrap();
      assert!(literal.as_str().contains(content_1));
    }
    {
      let literal = all_literals.get(1).unwrap();
      assert!(literal.as_str().contains(content_2));
    }
  }

  #[test]
  fn test_non_adjacent_line_comments_put_in_different_literal_sets() {
    let content_1 = " line comment 1 ";
    let content_2 = " line comment 2 ";
    let source = format!("//{}\nfn(){{}}\n//{}", content_1, content_2);
    let tokens = retain_only_developer_comments(
      token_with_line_column_to_token_with_type(
          tokens_with_location_to_tokens_with_line_and_column(&source,
              source_to_tokens_with_location(&source))));
    let literal_sets = literal_sets_from_line_comments(tokens.iter().collect());
    assert_eq!(literal_sets.len(), 2);
    {
      let literal_set = literal_sets.get(0).unwrap();
      let all_literals = literal_set.literals();
      assert_eq!(all_literals.len(), 1);
      let literal = all_literals.get(0).unwrap();
      assert!(literal.as_str().contains(content_1));
    }
    {
      let literal_set = literal_sets.get(1).unwrap();
      let all_literals = literal_set.literals();
      assert_eq!(all_literals.len(), 1);
      let literal = all_literals.get(0).unwrap();
      assert!(literal.as_str().contains(content_2));
    }
  }
}