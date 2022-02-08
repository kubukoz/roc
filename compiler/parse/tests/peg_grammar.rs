

/*
To debug grammar, add `dbg!(&tokens);`, execute with:
```
cargo test --features peg/trace test_fibo -- --nocapture
```
With visualization, see necessary format for temp_trace.txt [here](https://github.com/fasterthanlime/pegviz):
```
cat compiler/parse/temp_trace.txt | pegviz --output ./pegviz.html
```
*/

#[cfg(test)]
mod test_peg_grammar {
    use std::fs;


  #[repr(u8)]
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  /// Tokens are full of very dense information to make checking properties about them
  /// very fast.
  /// Some bits have specific meanings: 
  /// * 0b_001*_****: "Identifier-like" things
  /// * 0b_01**_****: "Punctuation"
  ///     * 0b_0100_1***: []{}() INDENT/DEDENT
  ///         * 0b_0100_1**0 [{(INDENT
  ///         * 0b_0100_1**1 ]})DEDENT
  ///     * 0b_011*_**** Operators
  pub enum Token {
      LowercaseIdent      = 0b_0010_0000, 
      UppercaseIdent      = 0b_0011_0011,
      MalformedIdent      = 0b_0010_0001,
  
      KeywordIf           = 0b_0010_0010,
      KeywordThen         = 0b_0010_0011,
      KeywordElse         = 0b_0010_0100,
      KeywordWhen         = 0b_0010_0101,
      KeywordAs           = 0b_0010_0110,
      KeywordIs           = 0b_0010_0111,
      KeywordExpect       = 0b_0010_1000,
      KeywordApp          = 0b_0010_1001,
      KeywordInterface    = 0b_0010_1010,
      KeywordPackages     = 0b_0010_1011,
      KeywordImports      = 0b_0010_1100,
      KeywordProvides     = 0b_0010_1101,
      KeywordTo           = 0b_0010_1110,
      KeywordExposes      = 0b_0010_1111,
      KeywordEffects      = 0b_0011_0000,
      KeywordPlatform     = 0b_0011_0001,
      KeywordRequires     = 0b_0011_0010,
  
      Comma               = 0b_0100_0000,
      Colon               = 0b_0100_0001,
  
      OpenParen           = 0b_0100_1000,
      CloseParen          = 0b_0100_1001,
      OpenCurly           = 0b_0100_1010,
      CloseCurly          = 0b_0100_1011,
      OpenSquare          = 0b_0100_1100,
      CloseSquare         = 0b_0100_1101,
      OpenIndent          = 0b_0100_1110,
      CloseIndent         = 0b_0100_1111,
      SameIndent          = 0b_0101_0000,
  
      OpPlus              = 0b_0110_0000,
      OpMinus             = 0b_0110_0001,
      OpSlash             = 0b_0110_0010,
      OpPercent           = 0b_0110_0011,
      OpCaret             = 0b_0110_0100,
      OpGreaterThan       = 0b_0110_0101,
      OpLessThan          = 0b_0110_0110,
      OpAssignment        = 0b_0110_0111,
      OpPizza             = 0b_0110_1000,
      OpEquals            = 0b_0110_1001,
      OpNotEquals         = 0b_0110_1010,
      OpGreaterThanOrEq   = 0b_0110_1011,
      OpLessThanOrEq      = 0b_0110_1100,
      OpAnd               = 0b_0110_1101,
      OpOr                = 0b_0110_1110,
      OpDoubleSlash       = 0b_0110_1111,
      OpDoublePercent     = 0b_0111_0001,
      OpBackpassing       = 0b_0111_1010,
  
      TodoNextThing       = 0b_1000_0000,
  
      Malformed,
      MalformedOperator,
  
      PrivateTag,
  
      String,
  
      NumberBase,
      Number,
  
      QuestionMark,
  
      Underscore,
  
      Ampersand,
      Pipe,
      Dot,
      SpaceDot, // ` .` necessary to know difference between `Result.map .position` and `Result.map.position`
      Bang,
      LambdaStart,
      Arrow,
      FatArrow,
      Asterisk,
  }
  
  pub struct TokenTable {
      pub tokens: Vec<Token>,
      pub offsets: Vec<usize>,
      pub lengths: Vec<usize>,
  }
  
  pub struct LexState {
      indents: Vec<usize>,
  }
  
  trait ConsumeToken {
      fn token(&mut self, token: Token, offset: usize, length: usize);
  }

  struct TestConsumer{
    tokens: Vec<Token>,
  }

  impl ConsumeToken for TestConsumer {
    fn token(&mut self, token: Token, offset: usize, length: usize){
      self.tokens.push(token);
    }
  }

  fn test_tokenize(code_str: &str) -> Vec<Token> {
    let mut lex_state = LexState{ indents: Vec::new() };
    let mut consumer = TestConsumer{ tokens: Vec::new() };

    tokenize(
      &mut lex_state,
      code_str.as_bytes(),
      &mut consumer
    );

    consumer.tokens
  }
  
  fn tokenize(
      state: &mut LexState,
      bytes: &[u8],
      consumer: &mut impl ConsumeToken,
  ) {
      let mut i = 0;
  
      while i < bytes.len() {
          let bytes = &bytes[i..];
  
          let (token, len) = match bytes[0] {
              b'(' => (Token::OpenParen, 1),
              b')' => (Token::CloseParen, 1),
              b'{' => (Token::OpenCurly, 1),
              b'}' => (Token::CloseCurly, 1),
              b'[' => (Token::OpenSquare, 1),
              b']' => (Token::CloseSquare, 1),
              b',' => (Token::Comma, 1),
              b'_' => lex_underscore(bytes),
              b'@' => lex_private_tag(bytes),
              b'a'..=b'z' => lex_ident(false, bytes),
              b'A'..=b'Z' => lex_ident(true, bytes),
              b'0'..=b'9' => lex_number(bytes),
              b'-' | b':' | b'!' | b'.' | b'*' | b'/' | b'&' |
              b'%' | b'^' | b'+' | b'<' | b'=' | b'>' | b'|' | b'\\' => lex_operator(bytes),
              b' ' => {
                  match skip_whitespace(bytes) {
                    SpaceDotOrSpaces::SpacesWSpaceDot(skip) => {
                      i += skip;
                      (Token::SpaceDot, 1)
                    },
                    SpaceDotOrSpaces::Spaces(skip) => {
                      i += skip;
                      continue;
                    }
                  }
                  
              }
              b'\n' => {
                  // TODO: add newline to side_table
                  let skip_newline_return = skip_newlines_and_comments(bytes);

                  match skip_newline_return {
                    SkipNewlineReturn::SkipWIndent(skipped_lines, curr_line_indent) => {
                      add_indents(skipped_lines, curr_line_indent, state, consumer, &mut i);
                      continue;
                    }
                    SkipNewlineReturn::WSpaceDot(skipped_lines, curr_line_indent) => {
                      add_indents(skipped_lines, curr_line_indent, state, consumer, &mut i);
                      (Token::SpaceDot, 1)
                    }
                  }
                  
              }
              b'#' => {
                  // TODO: add comment to side_table
                  i += skip_comment(bytes);
                  continue;
              }
              b'"' => lex_string(bytes),
              b => todo!("handle {:?}", b as char),
          };
  
          consumer.token(token, i, len);
          i += len;
      }
  }

  fn add_indents(skipped_lines: usize, curr_line_indent: usize, state: &mut LexState, consumer: &mut impl ConsumeToken, curr_byte_ctr: &mut usize) {
    *curr_byte_ctr += skipped_lines;

    if let Some(&prev_indent) = state.indents.last() {
      if curr_line_indent > prev_indent {
        state.indents.push(curr_line_indent);
        consumer.token(Token::OpenIndent, *curr_byte_ctr, 0);
      } else {
        *curr_byte_ctr += curr_line_indent;

        if prev_indent == curr_line_indent {
          consumer.token(Token::SameIndent, *curr_byte_ctr, 0);
        } else if curr_line_indent < prev_indent {
          // safe unwrap because we check first
          while state.indents.last().is_some() && curr_line_indent < *state.indents.last().unwrap() {
            state.indents.pop();
            consumer.token(Token::CloseIndent, *curr_byte_ctr, 0);
          }
        }

      }
    } else if curr_line_indent > 0 {
      state.indents.push(curr_line_indent);
      consumer.token(Token::OpenIndent, *curr_byte_ctr, 0);
    } else {
      consumer.token(Token::SameIndent, *curr_byte_ctr, 0);
    }
  }
  
  impl TokenTable {
      pub fn new(text: &str) -> TokenTable {
          let mut tt = TokenTable {
              tokens: Vec::new(),
              offsets: Vec::new(),
              lengths: Vec::new(),
          };
  
          let mut offset = 0;
          let mut state = LexState::new();
  
          // while let Some((token, skip, length)) = Token::lex_single(&mut state, &text.as_bytes()[offset..]) {
          //     tt.tokens.push(token);
          //     offset += skip;
          //     tt.offsets.push(offset);
          //     offset += length;
          //     tt.lengths.push(length);
          // }
  
          tt
      }
  }
  
  impl LexState {
      pub fn new() -> LexState {
          LexState {
              indents: Vec::new(),
          }
      }
  }
  
  fn skip_comment(bytes: &[u8]) -> usize {
      let mut skip = 0;
      while skip < bytes.len() && bytes[skip] != b'\n' {
          skip += 1;
      }
      if (skip + 1) < bytes.len() && bytes[skip] == b'\n' && bytes[skip+1] == b'#'{
        skip += 1;
      }

      skip
  }
  
  #[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
  struct Indent(usize);

  enum SpaceDotOrSpaces {
    SpacesWSpaceDot(usize),
    Spaces(usize)
  }
  
  fn skip_whitespace(bytes: &[u8]) -> SpaceDotOrSpaces {
      debug_assert!(bytes[0] == b' ');
  
      let mut skip = 0;
      while skip < bytes.len() && bytes[skip] == b' ' {
          skip += 1;
      }

      if skip < bytes.len() && bytes[skip] == b'.' {
        SpaceDotOrSpaces::SpacesWSpaceDot(skip)
      } else {
        SpaceDotOrSpaces::Spaces(skip)
      }
  }

  enum SkipNewlineReturn {
    SkipWIndent(usize, usize),
    WSpaceDot(usize, usize)
  }
  
  // also skips lines that contain only whitespace
  fn skip_newlines_and_comments(bytes: &[u8]) -> SkipNewlineReturn {
      let mut skip = 0;
      let mut indent = 0;
  
      while skip < bytes.len() && bytes[skip] == b'\n' {
          skip += indent + 1;


          if bytes.len() > skip {
            if bytes[skip] == b' ' {
              let space_dot_or_spaces = skip_whitespace(&bytes[skip..]);

              match space_dot_or_spaces {
                SpaceDotOrSpaces::SpacesWSpaceDot(spaces) => {
                  return SkipNewlineReturn::WSpaceDot(skip, spaces)
                }
                SpaceDotOrSpaces::Spaces(spaces) => {
                  if bytes.len() > (skip + spaces) {
                    if bytes[skip + spaces] == b'\n' {
                      indent = 0;
                      skip += spaces;
                    } else if bytes[skip+spaces] == b'#' {
                      let comment_skip = skip_comment(&bytes[(skip + spaces)..]);
    
                      indent = 0;
                      skip += spaces + comment_skip;
                    } else {
                      indent = spaces;
                    }
                  } else {
                    indent = spaces;
                  }
                }
              }              
            } else {
              while bytes[skip] == b'#' {
                let comment_skip = skip_comment(&bytes[skip..]);

                indent = 0;
                skip += comment_skip;
              }
            }
      }
    }
    
    SkipNewlineReturn::SkipWIndent(skip, indent)
  }
  
  fn is_op_continue(ch: u8) -> bool {
      matches!(ch, b'-' | b':' | b'!' | b'.' | b'*' | b'/' | b'&' |
                  b'%' | b'^' | b'+' | b'<' | b'=' | b'>' | b'|' | b'\\')
  }
  
  fn lex_operator(bytes: &[u8]) -> (Token, usize) {
      let mut i = 0;
      while i < bytes.len() && is_op_continue(bytes[i]) {
          i += 1;
      }
      let tok = match &bytes[0..i] {
          b"+" => Token::OpPlus,
          b"-" => Token::OpMinus,
          b"*" => Token::Asterisk,
          b"/" => Token::OpSlash,
          b"%" => Token::OpPercent,
          b"^" => Token::OpCaret,
          b">" => Token::OpGreaterThan,
          b"<" => Token::OpLessThan,
          b"." => Token::Dot,
          b"=" => Token::OpAssignment,
          b":" => Token::Colon,
          b"|" => Token::Pipe,
          b"\\" => Token::LambdaStart,
          b"|>" => Token::OpPizza,
          b"==" => Token::OpEquals,
          b"!" => Token::Bang,
          b"!=" => Token::OpNotEquals,
          b">=" => Token::OpGreaterThanOrEq,
          b"<=" => Token::OpLessThanOrEq,
          b"&&" => Token::OpAnd,
          b"&" => Token::Ampersand,
          b"||" => Token::OpOr,
          b"//" => Token::OpDoubleSlash,
          b"%%" => Token::OpDoublePercent,
          b"->" => Token::Arrow,
          b"<-" => Token::OpBackpassing,
          op => {
              dbg!(std::str::from_utf8(op).unwrap());
              Token::MalformedOperator
          }
      };
      (tok, i)
  }
  
  fn is_ident_continue(ch: u8) -> bool {
      matches!(ch, b'a'..=b'z'|b'A'..=b'Z'|b'0'..=b'9'|b'_')
  }
  
  fn lex_private_tag(bytes: &[u8]) -> (Token, usize) {
      debug_assert!(bytes[0] == b'@');
      let mut i = 1;
      while i < bytes.len() && is_ident_continue(bytes[i]) {
          i += 1;
      }
      (Token::PrivateTag, i)
  }
  
  fn lex_ident(uppercase: bool, bytes: &[u8]) -> (Token, usize) {
      let mut i = 0;
      while i < bytes.len() && is_ident_continue(bytes[i]) {
          i += 1;
      }
      let tok = match &bytes[0..i] {
          b"if" => Token::KeywordIf,
          b"then" => Token::KeywordThen,
          b"else" => Token::KeywordElse,
          b"when" => Token::KeywordWhen,
          b"as" => Token::KeywordAs,
          b"is" => Token::KeywordIs,
          b"expect" => Token::KeywordExpect,
          b"app" => Token::KeywordApp,
          b"interface" => Token::KeywordInterface,
          b"packages" => Token::KeywordPackages,
          b"imports" => Token::KeywordImports,
          b"provides" => Token::KeywordProvides,
          b"to" => Token::KeywordTo,
          b"exposes" => Token::KeywordExposes,
          b"effects" => Token::KeywordEffects,
          b"platform" => Token::KeywordPlatform,
          b"requires" => Token::KeywordRequires,
          ident => {
              if ident.contains(&b'_') {
                  Token::MalformedIdent
              } else if uppercase {
                  Token::UppercaseIdent
              } else {
                  Token::LowercaseIdent
              }
          },
      };
      (tok, i)
  }
  
  fn lex_underscore(bytes: &[u8]) -> (Token, usize) {
      let mut i = 0;
      while i < bytes.len() && is_ident_continue(bytes[i]) {
          i += 1;
      }
      (Token::Underscore, i)
  }
  
  fn is_int_continue(ch: u8) -> bool {
      matches!(ch, b'0'..=b'9' | b'_')
  }
  
  fn lex_number(bytes: &[u8]) -> (Token, usize) {
      let mut i = 0;
      while i < bytes.len() && is_int_continue(bytes[i]) {
          i += 1;
      }
  
      if i < bytes.len() && bytes[i] == b'.' {
          i += 1;
          while i < bytes.len() && is_int_continue(bytes[i]) {
              i += 1;
          }
      }
  
      (Token::Number, i)
  }
  
  fn lex_string(bytes: &[u8]) -> (Token, usize) {
      let mut i = 0;
      assert_eq!(bytes[i], b'"');
      i += 1;
  
      while i < bytes.len() {
          match bytes[i] {
              b'"' => break,
              // TODO: escapes
              _ => i += 1,
          }
      }
  
      assert_eq!(bytes[i], b'"');
      i += 1;
  
      (Token::String, i)
  }

type T = Token;

#[test]
fn test_indent_tokenization_1() {
  let tokens = test_tokenize(r#"showBool = \b ->
    when b is
        True ->
            "True""#);
  
  assert_eq!(
    tokens,
    [T::LowercaseIdent, T::OpAssignment, T::LambdaStart, T::LowercaseIdent, T::Arrow,
    T::OpenIndent, T::KeywordWhen, T::LowercaseIdent, T::KeywordIs,
    T::OpenIndent, T::UppercaseIdent, T::Arrow,
    T::OpenIndent, T::String]
  );
}

#[test]
fn test_indent_tokenization_2() {
  let tokens = test_tokenize(r#"showBool = \b ->
    when b is
        True ->
            "True"
"#);
  
  assert_eq!(
    tokens,
    [T::LowercaseIdent, T::OpAssignment, T::LambdaStart, T::LowercaseIdent, T::Arrow,
    T::OpenIndent, T::KeywordWhen, T::LowercaseIdent, T::KeywordIs,
    T::OpenIndent, T::UppercaseIdent, T::Arrow,
    T::OpenIndent, T::String,
    T::CloseIndent, T::CloseIndent, T::CloseIndent]
  );
}

#[test]
fn test_tokenization_line_with_only_spaces() {
  let tokens = test_tokenize(r#"\key ->
  when dict is
      Empty ->
          4
  
      Node ->
          5"#);

  assert_eq!(
    tokens,
    [T::LambdaStart, T::LowercaseIdent, T::Arrow,
    T::OpenIndent, T::KeywordWhen, T::LowercaseIdent, T::KeywordIs,
    T::OpenIndent, T::UppercaseIdent, T::Arrow,
    T::OpenIndent, T::Number,
    T::CloseIndent,
    T::UppercaseIdent, T::Arrow,
    T::OpenIndent, T::Number]
  );
}


#[test]
fn test_tokenization_empty_lines_and_comments() {
  let tokens = test_tokenize(r#"a = 5

# com1
# com2
b = 6"#);

  assert_eq!(
    tokens,[T::LowercaseIdent, T::OpAssignment, T::Number,
    T::SameIndent, T::LowercaseIdent, T::OpAssignment, T::Number]);
}



#[test]
fn test_tokenization_when_branch_comments() {
  let tokens = test_tokenize(r#"when errorCode is
  # A -> Task.fail InvalidCharacter
  # B -> Task.fail IOError
  _ ->
      Task.succeed -1"#);

  assert_eq!(
    tokens,[T::KeywordWhen, T::LowercaseIdent, T::KeywordIs,
    T::OpenIndent, T::Underscore, T::Arrow, T::OpenIndent, T::UppercaseIdent, T::Dot, T::LowercaseIdent, T::OpMinus, T::Number]);
}

// Inspired by https://ziglang.org/documentation/0.7.1/#Grammar
// license information can be found in the LEGAL_DETAILS file in
// the root directory of this distribution.
// Thank you zig contributors!
peg::parser!{
    grammar tokenparser() for [T] {

      pub rule module() =
        header() module_defs()? indented_end()

      pub rule full_expr() = 
        op_expr()
        / [T::OpenIndent] op_expr() close_or_end()

      pub rule op_expr() = pizza_expr()

      rule common_expr() =
          closure()
          / expect()
          / if_expr()
          / when()
          / backpass()
          / list()
          / record()
          / record_update()
          / parens_around()
          / [T::Number]
          / [T::NumberBase]
          / [T::String]
          / module_var()
          / tag()
          / accessor_function()
          / defs()
          / annotation()
          / [T::LowercaseIdent]
      pub rule expr() =
          access()
          / apply()
          / common_expr()

        pub rule closure() =
          [T::LambdaStart] args() [T::Arrow] closure_body()

        rule closure_body() =
          [T::OpenIndent] full_expr() ([T::CloseIndent] / end_of_file())
          / [T::SameIndent]? full_expr()

        rule args() =
          (arg() [T::Comma])* arg()

        rule arg() =
          [T::Underscore]
          / ident()
          / record_destructure()


        rule tag() =
          private_tag()
          / [T::UppercaseIdent]

        rule private_tag() = [T::PrivateTag] {}


        rule list() = empty_list()
                    / [T::OpenSquare] (expr() [T::Comma])* expr()? [T::Comma]? [T::CloseSquare] { }
        rule empty_list() = [T::OpenSquare] [T::CloseSquare]


        rule record() =
          empty_record()
          / [T::OpenCurly] assigned_fields_i() [T::CloseCurly]

        rule assigned_fields() =
          (assigned_field() [T::SameIndent]? [T::Comma] [T::SameIndent]?)* [T::SameIndent]? assigned_field()? [T::Comma]?

        rule assigned_fields_i() =
          [T::OpenIndent] assigned_fields() [T::CloseIndent]
          / [T::SameIndent]? assigned_fields() [T::SameIndent]?
           

        rule assigned_field() =
          required_value()
          / [T::LowercaseIdent]

        rule required_value() =
          [T::LowercaseIdent] [T::Colon] full_expr()

        rule empty_record() = [T::OpenCurly] [T::CloseCurly]

        rule record_update() = [T::OpenCurly] expr() [T::Ampersand] assigned_fields_i() [T::CloseCurly]

        rule record_type() =
          empty_record()
          / [T::OpenCurly] record_field_types_i() [T::CloseCurly]

        rule record_type_i() = 
          [T::OpenIndent] record_type() [T::CloseIndent]?
          / record_type()

        rule record_field_types_i() =
          [T::OpenIndent] record_field_types() [T::CloseIndent]
          / record_field_types()

        rule record_field_types() =
          ([T::SameIndent]? record_field_type() [T::SameIndent]? [T::Comma])* ([T::SameIndent]? record_field_type() [T::Comma]?)?

        rule record_field_type() =
          ident() [T::Colon] type_annotation()


        rule parens_around() = [T::OpenParen] full_expr() [T::CloseParen]

        rule if_expr() = [T::KeywordIf] full_expr() [T::KeywordThen] full_expr()
                            [T::KeywordElse] full_expr()

        rule expect() = [T::KeywordExpect] expr()

        pub rule backpass() =
          backpass_pattern() [T::OpBackpassing] expr()

        rule common_pattern() =
          [T::LowercaseIdent]
          / module_var()
          / concrete_type()
          / parens_around()
          / tag()

        rule backpass_pattern() =
          common_pattern()
          / [T::Underscore]
          / record_destructure()
          / [T::Number]
          / [T::NumberBase]
          / [T::String]
          / list()

        // for applies without line breaks between args: Node color rK rV  
        rule apply_arg_pattern() =
          accessor_function()
          / access()
          / record()
          / common_pattern()
          / [T::Number]
          / [T::NumberBase]
          / [T::String]
          / list()

        // for applies where the arg is on its own line:
        // Effect.after
        //    transform a  
        rule apply_arg_line_pattern() =
          record()
          / closure()
          / apply()
          / common_pattern()

        rule apply_start_pattern() =
          access()
          / common_pattern()

        rule record_destructure() =
          empty_record()
          / [T::OpenCurly] (ident() [T::Comma])* ident() [T::Comma]? [T::CloseCurly]

        rule access() =
          access_start() [T::Dot] ident()

        rule access_start() =
          [T::LowercaseIdent]
          / record()
          / parens_around()

        rule accessor_function() =
          [T::SpaceDot] ident()
          / [T::Dot] ident()

        pub rule header() =
          __ almost_header() header_end()

        pub rule almost_header() =
          app_header()
          / interface_header()
          / platform_header()

        rule app_header() =
          [T::KeywordApp] [T::String] [T::OpenIndent]? packages() imports() provides()// check String to be non-empty?
        
        rule interface_header() =
          [T::KeywordInterface] module_name() [T::OpenIndent]? exposes() imports()

        rule platform_header() =
          [T::KeywordPlatform] [T::String] [T::OpenIndent]? requires() exposes() packages() imports() provides() effects()// check String to be nonempty?

        rule header_end() =
          ([T::CloseIndent]
          / &[T::SameIndent])? // & to not consume the SameIndent
        rule packages() =
          __ [T::KeywordPackages] record() 

        rule imports() =
          __ [T::KeywordImports] imports_list()

        rule imports_list() =
          empty_list()
          / [T::OpenSquare] (imports_entry() [T::Comma])* imports_entry()? [T::Comma]? [T::CloseSquare]

        rule imports_entry() =
          ([T::LowercaseIdent] [T::Dot])?
          module_name()
          ([T::Dot] exposes_list() )?

        rule exposes_list() =
          [T::OpenCurly] (exposes_entry() [T::Comma])* exposes_entry()? [T::Comma]? [T::CloseCurly]
        rule exposes_entry() =
          ident()

        rule provides() =
          __ [T::KeywordProvides] provides_list() ([T::KeywordTo] provides_to())?

        rule provides_to() =
         [T::String]
          / ident()
        
        rule provides_list() =
          empty_list()
          / [T::OpenSquare] exposed_names() [T::CloseSquare]

        rule exposes() =
          __ [T::KeywordExposes] [T::OpenSquare] exposed_names() [T::CloseSquare]

        rule exposed_names() =
          (ident() [T::Comma])* ident()? [T::Comma]?

        rule requires() =
          [T::KeywordRequires] requires_rigids() [T::OpenCurly] typed_ident() [T::CloseCurly]

        rule requires_rigids() =
          empty_record()
          / [T::OpenCurly] (requires_rigid() [T::Comma])* requires_rigid() [T::Comma]? [T::CloseCurly]

        rule requires_rigid() =
          [T::LowercaseIdent] ([T::FatArrow] [T::UppercaseIdent])?

        pub rule typed_ident() =
          [T::LowercaseIdent] [T::Colon] type_annotation()

        pub rule effects() =
          __ [T::KeywordEffects] effect_name() record_type_i()

        rule effect_name() =
          [T::LowercaseIdent] [T::Dot] [T::UppercaseIdent]


        rule module_name() =
          [T::UppercaseIdent] ([T::Dot] [T::UppercaseIdent])*

        rule ident() =
          [T::UppercaseIdent]
          / [T::LowercaseIdent]


        // content of type_annotation without Colon(:)
        pub rule type_annotation() =
          function_type()
          / type_annotation_no_fun()

        rule type_annotation_no_fun() =
          [T::OpenParen] type_annotation_no_fun() [T::CloseParen]
          / [T::OpenIndent] type_annotation_no_fun() close_or_end()
          / tag_union()
          / apply_type()
          / bound_variable()
          / record_type()
          / inferred()
          / wildcard()
        // TODO inline type alias

        rule type_annotation_paren_fun() =
          type_annotation_no_fun()
          / [T::OpenParen] function_type() [T::CloseParen]

        rule tag_union() =
          empty_list()
          / [T::OpenSquare] tags() [T::CloseSquare] type_variable()?

        rule tags() =
          [T::OpenIndent] tags_only() [T::CloseIndent]
          / tags_only()

        rule tags_only() =
          ([T::SameIndent]? apply_type() [T::SameIndent]? [T::Comma] [T::SameIndent]? )* ([T::SameIndent]? apply_type() [T::Comma]?)?

        rule type_variable() =
          [T::Underscore]
          / bound_variable()

        rule bound_variable() =
          [T::LowercaseIdent]

        // The `*` type variable, e.g. in (List *)  
        rule wildcard() =
          [T::Asterisk]

        // '_', indicating the compiler should infer the type  
        rule inferred() =
          [T::Underscore]

        rule function_type() =
          ( type_annotation_paren_fun() ([T::Comma] type_annotation_paren_fun())* [T::Arrow])? type_annotation_paren_fun()

        pub rule apply_type() =
          concrete_type() apply_type_args()?
        rule concrete_type() =
          [T::UppercaseIdent] ([T::Dot] [T::UppercaseIdent])*
        rule apply_type_args() =
          type_annotation_no_fun() type_annotation_no_fun()*

        rule _() =
          ([T::SameIndent])?

        // the rules below allow us to set assoicativity and precedence
        rule unary_op() =
          [T::OpMinus]
          / [T::Bang]
        rule unary_expr() =
          unary_op()* expr()

        rule mul_level_op() =
          [T::Asterisk]
          / [T::OpSlash] 
          / [T::OpDoubleSlash] 
          / [T::OpPercent]
          / [T::OpDoublePercent]
        rule mul_level_expr() =
          unary_expr() (mul_level_op() unary_expr())*

        rule add_level_op() =
          [T::OpPlus]
          / [T::OpMinus]
        rule add_level_expr() =
          mul_level_expr() (add_level_op() mul_level_expr())*
        
        rule compare_op() =
          [T::OpEquals] // ==
          / [T::OpNotEquals]
          / [T::OpLessThan]
          / [T::OpGreaterThan]
          / [T::OpLessThanOrEq]
          / [T::OpGreaterThanOrEq]
        rule compare_expr() =
          add_level_expr() (compare_op() add_level_expr())?

        rule bool_and_expr() =
          compare_expr() ([T::OpAnd] compare_expr())*

        rule bool_or_expr() =
          bool_and_expr() ([T::OpOr] bool_and_expr())*


        rule pizza_expr() =
          bool_or_expr() pizza_end()?

        rule pizza_end() =
          [T::SameIndent]? [T::OpPizza] [T::SameIndent]? bool_or_expr() pizza_end()*
          / [T::SameIndent]? [T::OpPizza] [T::OpenIndent] bool_or_expr() pizza_end()* close_or_end()
          / [T::OpenIndent] [T::OpPizza] [T::SameIndent]? bool_or_expr() pizza_end()* close_or_end()
          / [T::OpenIndent] [T::OpPizza] [T::OpenIndent] bool_or_expr() pizza_end()* close_double_or_end()

        rule close_or_end() =
          [T::CloseIndent]
          / end_of_file()

        rule close_double_or_end() =
          [T::CloseIndent] [T::CloseIndent]
          / [T::CloseIndent] end_of_file()
          / end_of_file()

        //TODO support right assoicative caret(^), for example: 2^2

        pub rule defs() =
          def() ([T::SameIndent]? def())* [T::SameIndent]? full_expr()

        pub rule def() =
            annotated_body()
            / annotation()
            / body()
            / alias()
            / expect()

        pub rule module_defs() =
          ([T::SameIndent]? def())+

        rule annotation() =
        annotation_pre_colon() [T::Colon] type_annotation()

        rule annotation_pre_colon() =
          apply()
          / tag()
          / ident()

        rule body() =
        ident() [T::OpAssignment] [T::OpenIndent] full_expr() ([T::SameIndent]? full_expr())* ([T::CloseIndent] / end_of_file())
        /  ident() [T::OpAssignment] full_expr() end_of_file()?

        rule annotated_body() =
          annotation() [T::SameIndent] body()

        rule alias() =
          apply_type() [T::Colon] type_annotation()
          
        pub rule when() =
          [T::KeywordWhen] expr() [T::KeywordIs] when_branches()

        rule when_branches() =
          [T::OpenIndent] when_branch()+ close_or_end()
          / when_branch()+

        pub rule when_branch() =
          matchable() ([T::Pipe] full_expr())* ([T::KeywordIf] full_expr())? [T::Arrow] when_branch_body() 

        rule when_branch_body() =
          [T::OpenIndent] full_expr() ([T::CloseIndent] / end_of_file())
          / full_expr()

        rule matchable() =
          type_annotation_no_fun()
          / expr()

        rule var() =
          [T::LowercaseIdent]
          / module_var()

        rule module_var() =
          module_name() [T::Dot] [T::LowercaseIdent]

        pub rule apply() =
          apply_start_pattern() apply_args()

        pub rule apply_args() =
        [T::OpenIndent] apply_arg_line_pattern() single_line_apply_args()? ([T::CloseIndent]/indented_end())
          / apply_arg_pattern()+

        rule single_line_apply_args() =
          [T::SameIndent] apply_arg_line_pattern() ( (single_line_apply_args()*) / indented_end() )
          / ([T::OpenIndent] apply_arg_line_pattern() single_line_apply_args()* ([T::CloseIndent] / indented_end()))

        rule apply_expr() =
          var()
          / tag()

        rule end() =
          [T::CloseIndent]
          / &[T::SameIndent] // & to not consume the SameIndent
          / end_of_file()

        rule indented_end() =
          ([T::OpenIndent] / [T::CloseIndent] / [T::SameIndent])* end_of_file()

        // for optionalindents
        // underscore rules do not require parentheses  
        rule __() =
          (
            [T::OpenIndent]
          / [T::CloseIndent]
          / [T::SameIndent]
        )?

        rule end_of_file() =
         ![_]

    }
}

#[test]
fn test_basic_expr() {
    assert_eq!(tokenparser::expr(&[T::OpenSquare, T::CloseSquare]), Ok(()));
    assert_eq!(tokenparser::expr(&[T::OpenCurly, T::CloseCurly]), Ok(()));

    assert_eq!(tokenparser::expr(&[T::OpenParen, T::OpenSquare, T::CloseSquare, T::CloseParen]), Ok(()));

    assert_eq!(tokenparser::expr(&[T::Number]), Ok(()));
    assert_eq!(tokenparser::expr(&[T::String]), Ok(()));

    assert_eq!(tokenparser::expr(&[T::KeywordIf, T::Number, T::KeywordThen, T::Number, T::KeywordElse, T::Number]), Ok(()));

    assert_eq!(tokenparser::expr(&[T::KeywordExpect, T::Number]), Ok(()));

    assert_eq!(tokenparser::expr(&[T::LowercaseIdent, T::OpBackpassing, T::Number]), Ok(()));    
}

#[test]
fn test_app_header_1() {
  let tokens = test_tokenize( r#"app "test-app" packages {} imports [] provides [] to blah"#);
  
  assert_eq!(tokenparser::header(&tokens), Ok(()));
}

#[test]
fn test_app_header_2() {
  let tokens = test_tokenize( r#"
app "test-app"
    packages { pf: "platform" }
    imports []
    provides [ main ] to pf
"#);

  assert_eq!(tokenparser::header(&tokens), Ok(()));
}

#[test]
fn test_interface_header() {
  let tokens = test_tokenize( r#"
interface Foo.Bar.Baz exposes [] imports []"#);

  assert_eq!(tokenparser::header(&tokens), Ok(()));
}

#[test]
fn test_interface_header_2() {
  let tokens = test_tokenize( r#"

  interface Base64.Encode
      exposes [ toBytes ]
      imports [ Bytes.Encode.{ Encoder } ]"#);

  assert_eq!(tokenparser::header(&tokens), Ok(()));
}

#[test]
fn test_platform_header_1() {

    let tokens = test_tokenize( r#"platform "rtfeldman/blah" requires {} { main : {} } exposes [] packages {} imports [] provides [] effects fx.Blah {}"#);
    
    assert_eq!(tokenparser::header(&tokens), Ok(()));
}

#[test]
fn test_platform_header_2() {

    let tokens = test_tokenize( r#"platform "examples/cli"
    requires {}{ main : Task {} [] } # TODO FIXME
    exposes []
    packages {}
    imports [ Task.{ Task } ]
    provides [ mainForHost ]
    effects fx.Effect
        {
            getLine : Effect Str,
            putLine : Str -> Effect {},
            twoArguments : Int, Int -> Effect {}
        }"#);
    
    assert_eq!(tokenparser::header(&tokens), Ok(()));
}

#[test]
fn test_annotated_def() {
  let tokens = test_tokenize( r#"test1 : Bool
test1 =
    example1 == [ 2, 4 ]"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_record_def_1() {

    let tokens = test_tokenize( r#"x = { content: 4 }"#);

    assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_record_def_2() {

  let tokens = test_tokenize( r#"x =
   { content: 4 }"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_record_def_3() {

  let tokens = test_tokenize( r#"x =
   {
     a: 4,
     b: 5
    }"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_record_def_4() {

  let tokens = test_tokenize( r#"x =
   {
     a: 4,
     b: 5,
     c: 6,
    }"#);
  
  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_record_def_5() {

  let tokens = test_tokenize( r#"x =
   {
   a: 4,
   }"#);
   
  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_typed_ident() {
  // main : Task {} []
  assert_eq!(tokenparser::typed_ident(&[
    T::LowercaseIdent, T::Colon, T::UppercaseIdent, T::OpenCurly, T::CloseCurly, T::OpenSquare, T::CloseSquare
  ]), Ok(()));
}

#[test]
fn test_order_of_ops() {
  // True || False && True || False
  assert_eq!(tokenparser::full_expr(&[T::UppercaseIdent, T::OpOr, T::UppercaseIdent, T::OpAnd, T::UppercaseIdent, T::OpOr, T::UppercaseIdent]), Ok(()));
}

fn file_to_string(file_path: &str) -> String {
  // it's ok to panic in a test
  fs::read_to_string(file_path).unwrap()
}

fn example_path(sub_path: &str) -> String {
  let examples_dir = "../../examples/".to_string();

  let file_path = examples_dir + sub_path;

  file_to_string(
    &file_path
  )
}

#[test]
fn test_hello() {
  let tokens = test_tokenize(&example_path("hello-world/Hello.roc"));
  
  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_fibo() {
  let tokens = test_tokenize(&example_path("fib/Fib.roc"));
  dbg!(&tokens);
  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_annotation() {
  let tokens = test_tokenize( r#"ConsList a : [ Cons a (ConsList a), Nil ]"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_apply_type() {
  let tokens = test_tokenize( r#"Cons a (ConsList a)"#);

  assert_eq!(tokenparser::apply_type(&tokens), Ok(()));
}

#[test]
fn test_apply_expect_fail_1() {
  assert!(tokenparser::apply(&[
    T::LowercaseIdent, T::LowercaseIdent,T::CloseIndent, T::UppercaseIdent
  ]).is_err());
}

#[test]
fn test_apply_expect_fail_2() {
  let tokens = test_tokenize( r#"eval a
b"#);

  assert!(tokenparser::apply(&tokens).is_err());
}

#[test]
fn test_backpass_expect_fail() {
  let tokens = test_tokenize( r#"lastName <- 4
5"#);

  assert!(tokenparser::backpass(&tokens).is_err());
}

#[test]
fn test_when_1() {
  let tokens = test_tokenize( r#"when list is
  Cons _ rest ->
      1 + len rest

  Nil ->
      0"#);
  dbg!(&tokens);
  assert_eq!(tokenparser::when(&tokens), Ok(()));
}

#[test]
fn test_when_2() {
  let tokens = test_tokenize( r#"when list is
  Nil ->
      Cons a

  Nil ->
      Nil"#);

  assert_eq!(tokenparser::when(&tokens), Ok(()));
}

#[test]
fn test_cons_list() {
  let tokens = test_tokenize(&example_path("effect/ConsList.roc"));
  dbg!(&tokens);
  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_when_in_defs() {
  let tokens = test_tokenize(r#"fromBytes = \bytes ->
  when bytes is
      Ok v -> v
"#);

  dbg!(&tokens);
  assert_eq!(tokenparser::module_defs(&tokens), Ok(()));
}

#[test]
fn test_base64() {
  let tokens = test_tokenize(&example_path("benchmarks/Base64.roc"));
  dbg!(&tokens);
  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_base64_test() {
  let tokens = test_tokenize(&example_path("benchmarks/TestBase64.roc"));
  
  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_when_branch() {
  let tokens = test_tokenize(r#"Ok path -> path"#);

  assert_eq!(tokenparser::when_branch(&tokens), Ok(()));
}

#[test]
fn test_def_in_def() {
  let tokens = test_tokenize(r#"example =
  cost = 1

  cost"#);
  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_backpass_in_def() {
  let tokens = test_tokenize(r#"main =
  lastName <- 4
  Stdout.line "Hi!""#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_astar_test() {
  let tokens = test_tokenize(&example_path("benchmarks/TestAStar.roc"));

  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_cli_echo() {
  let tokens = test_tokenize(&example_path("cli/Echo.roc"));

  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_pizza_1() {
  let tokens = test_tokenize(r#"closure = \_ ->
  Task.succeed {}
      |> Task.map (\_ -> x)"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_pizza_one_line() {
  let tokens = test_tokenize(r#"5 |> fun"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_same_indent_1() {
  let tokens = test_tokenize(r#"5
|> fun"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_same_indent_2() {
  let tokens = test_tokenize(r#"5
|>
fun"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_indented_1_a() {
  let tokens = test_tokenize(r#"5
  |> fun"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_indented_1_b() {
  let tokens = test_tokenize(r#"5
  |> fun
"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_indented_2_a() {
  let tokens = test_tokenize(r#"5
  |>
    fun"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_indented_2_b() {
  let tokens = test_tokenize(r#"5
  |>
    fun
  "#);
  dbg!(&tokens);
  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_indented_2_c() {
  let tokens = test_tokenize(r#"5
  |>
    fun
  
"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_mixed_indent_1_a() {
  let tokens = test_tokenize(r#"5
|>
    fun"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_mixed_indent_1_b() {
  let tokens = test_tokenize(r#"5
|>
    fun
"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_mixed_indent_2_a() {
  let tokens = test_tokenize(r#"5
  |>
  fun"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_pizza_mixed_indent_2_b() {
  let tokens = test_tokenize(r#"5
  |>
  fun"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_longer_pizza() {
  let tokens = test_tokenize(r#"5 |> fun a |> fun b"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_deeper_pizza() {
  let tokens = test_tokenize(r#"5
|> fun a 
|> fun b"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_deeper_indented_pizza_a() {
  let tokens = test_tokenize(r#"5
  |> fun a 
  |> fun b"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_deeper_indented_pizza_b() {
  let tokens = test_tokenize(r#"5
  |> fun a 
  |> fun b
"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_deep_mixed_indent_pizza_a() {
  let tokens = test_tokenize(r#"5
  |> fun a |> fun b
  |> fun c d
  |> fun "test"
    |> List.map Str.toI64
       |> g (1 + 1)"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_deep_mixed_indent_pizza_b() {
  let tokens = test_tokenize(r#"5
  |> fun a |> fun b
  |> fun c d
  |> fun "test"
    |> List.map Str.toI64
       |> g (1 + 1)
"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_bool_or() {
  let tokens = test_tokenize(r#"a || True || b || False"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}

#[test]
fn test_closure_file() {
  let tokens = test_tokenize(&example_path("benchmarks/Closure.roc"));

  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_def_with_indents() {
  let tokens = test_tokenize(r#"main =
  Task.after
      Task.getInt
      \n ->
          queens n"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_nqueens() {
  let tokens = test_tokenize(&example_path("benchmarks/NQueens.roc"));
  dbg!(&tokens);
  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_quicksort_help() {
  let tokens = test_tokenize(r#"quicksortHelp = \list, low, high ->
  if low < high then
      when partition low is
          Pair ->
              partitioned
                  |> quicksortHelp low
  else
      list"#);
  dbg!(&tokens);
  assert_eq!(tokenparser::def(&tokens), Ok(()));
}


#[test]
fn test_quicksort() {
  let tokens = test_tokenize(&example_path("benchmarks/Quicksort.roc"));
  dbg!(&tokens);
  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_indented_closure_apply() {
  let tokens = test_tokenize(r#"
  effect
  \result -> result"#);

  assert_eq!(tokenparser::apply_args(&tokens), Ok(()));
}

#[test]
fn test_task() {
  let tokens = test_tokenize(&example_path("benchmarks/platform/Task.roc"));
  dbg!(&tokens);
  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_pizza_line() {
  let tokens = test_tokenize(r#"unoptimized
      |> Num.toStr
      |> Task.putLine"#);

  assert_eq!(tokenparser::full_expr(&tokens), Ok(()));
}

#[test]
fn test_defs_w_apply() {
  let tokens = test_tokenize(r#"unoptimized = eval e

42"#);
  dbg!(&tokens);
  assert_eq!(tokenparser::defs(&tokens), Ok(()));
}

#[test]
fn test_indented_apply_defs() {
  let tokens = test_tokenize(r#"main =
  after
      \n ->
          e = 5

          4

Expr : I64"#);

  assert_eq!(tokenparser::module_defs(&tokens), Ok(()));
}

#[test]
fn test_cfold() {
  let tokens = test_tokenize(&example_path("benchmarks/CFold.roc"));

  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_apply_with_comment() {
  let tokens = test_tokenize(r#"main =
  Task.after
      \n ->
        e = mkExpr n 1 # comment
        unoptimized = eval e
        optimized = eval (constFolding (reassoc e))
        
        optimized"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_multi_defs() {
  let tokens = test_tokenize(r#"main =
    tree : I64
    tree = 0

    tree"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

// TODO fix slow execution; likely a problem with apply
#[test]
fn test_perf_issue() {
  let tokens = test_tokenize(r#"main =
  tree = insert 0 {} Empty

  tree
      |> Task.putLine

nodeInParens : RedBlackTree k v, (k -> Str), (v -> Str) -> Str
nodeInParens = \tree, showKey, showValue ->
  when tree is
      _ ->
          "(\(inner))"

RedBlackTree k v : [ Node NodeColor k v (RedBlackTree k v) (RedBlackTree k v), Empty ]

Key k : Num k

balance = \color ->
  when right is
      Node Red ->
          when left is
              _ ->
                  Node color rK rV (Node Red key value left)

      _ ->
          5"#);
  dbg!(&tokens);
  assert_eq!(tokenparser::module_defs(&tokens), Ok(()));
}

#[test]
fn test_rbtree_insert() {
  let tokens = test_tokenize(&example_path("benchmarks/RBTreeInsert.roc"));

  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_closure_1() {
  let tokens = test_tokenize(r#"\key ->
      when dict is
          Empty ->
              4

          Node ->
              5"#);

  dbg!(&tokens);
  assert_eq!(tokenparser::closure(&tokens), Ok(()));
}


#[test]
fn test_closure_2() {
  let tokens = test_tokenize(r#"\key ->
      when dict is
          Empty ->
              Node Red

          Node nColor ->
              when key is  
                  GT ->
                      balance nColor"#);

  assert_eq!(tokenparser::closure(&tokens), Ok(()));
}

#[test]
fn test_nested_apply() {
  let tokens = test_tokenize(r#"after = \effect ->
  Effect.after
      transform a

map : Str"#);
  dbg!(&tokens);

  assert_eq!(tokenparser::module_defs(&tokens), Ok(()));
}

#[test]
fn test_deep_indented_defs() {
  let tokens = test_tokenize(r#"after = \effect ->
  after
      \result ->
          transform a

map : Str"#);
  dbg!(&tokens);

  assert_eq!(tokenparser::module_defs(&tokens), Ok(()));
}

#[test]
fn test_rbtree_ck() {
  let tokens = test_tokenize(&example_path("benchmarks/RBTreeCk.roc"));

  assert_eq!(tokenparser::module(&tokens), Ok(()));
}

#[test]
fn test_record_type_def() {
  let tokens = test_tokenize(r#"Model position :
  {
      evaluated : Set,
  }"#);

  assert_eq!(tokenparser::def(&tokens), Ok(()));
}

#[test]
fn test_apply_with_acces() {
  let tokens = test_tokenize(r#"Dict.get model.costs"#);

  assert_eq!(tokenparser::apply(&tokens), Ok(()));
}

#[test]
fn test_space_dot() {
  let tokens = test_tokenize(r#"Result.map .position"#);

  assert_eq!(tokenparser::op_expr(&tokens), Ok(()));
}


#[test]
fn test_astar() {
  let tokens = test_tokenize(&example_path("benchmarks/AStar.roc"));

  assert_eq!(tokenparser::module(&tokens), Ok(()));
}



}
