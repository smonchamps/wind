//! Conservative CSS for HTML inserted into the live editor (audit D2).

use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, ToCss, Token,
};

const PROPERTIES: &[&str] = &[
    "color",
    "background-color",
    "font-weight",
    "font-style",
    "text-decoration",
    "text-align",
    "font-family",
    "font-size",
    "line-height",
    "white-space",
    "vertical-align",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-width",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "border-style",
    "border-color",
    "border-collapse",
    "border-spacing",
];

struct Declarations;

impl<'i> DeclarationParser<'i> for Declarations {
    type Declaration = String;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _: &ParserState,
    ) -> Result<String, ParseError<'i, ()>> {
        let name = name.to_ascii_lowercase();
        if !PROPERTIES.contains(&name.as_str()) {
            return Err(input.new_custom_error(()));
        }
        let mut tokens = Vec::new();
        while !input.is_exhausted() {
            let token = input.next()?.clone();
            let serialized = match &token {
                Token::Function(function)
                    if (name.contains("color") || name.starts_with("border"))
                        && ["rgb", "rgba", "hsl", "hsla"]
                            .iter()
                            .any(|f| function.eq_ignore_ascii_case(f)) =>
                {
                    let value = input.parse_nested_block(|nested| {
                        let mut parts = Vec::new();
                        while !nested.is_exhausted() {
                            let part = nested.next()?.clone();
                            let allowed = match &part {
                                Token::Number { value, .. } => {
                                    value.is_finite() && value.abs() <= 360.0
                                }
                                Token::Percentage { unit_value, .. } => {
                                    unit_value.is_finite() && unit_value.abs() <= 1.0
                                }
                                Token::Dimension { value, unit, .. } => {
                                    unit.eq_ignore_ascii_case("deg")
                                        && value.is_finite()
                                        && value.abs() <= 360.0
                                }
                                Token::Comma | Token::Delim('/') => true,
                                _ => false,
                            };
                            if !allowed {
                                return Err(nested.new_custom_error(()));
                            }
                            parts.push(part.to_css_string());
                        }
                        Ok(parts.join(" "))
                    })?;
                    format!("{}({value})", function.to_ascii_lowercase())
                }
                _ if scalar_allowed(&name, &token) => token.to_css_string(),
                _ => return Err(input.new_custom_error(())),
            };
            tokens.push(serialized);
        }
        if tokens.is_empty() {
            return Err(input.new_custom_error(()));
        }
        Ok(format!("{name}:{}", tokens.join(" ")))
    }
}

fn scalar_allowed(name: &str, token: &Token<'_>) -> bool {
    match token {
        Token::Ident(value) if name == "font-size" => [
            "xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large", "initial",
            "inherit",
        ]
        .iter()
        .any(|item| value.eq_ignore_ascii_case(item)),
        Token::Ident(_) => true,
        Token::QuotedString(_) | Token::Comma => name == "font-family",
        Token::Hash(_) | Token::IDHash(_) => name.contains("color") || name.starts_with("border"),
        Token::Number { value, .. } => {
            value.is_finite()
                && *value >= 0.0
                && (*value == 0.0
                    || (name == "font-weight" && *value <= 1000.0)
                    || (name == "line-height" && *value <= 3.0))
        }
        // Absolute lengths avoid exponential growth through nested em/% font sizes.
        Token::Dimension { value, unit, .. } => {
            let pixels = if unit.eq_ignore_ascii_case("px") {
                *value
            } else if unit.eq_ignore_ascii_case("pt") {
                *value * 4.0 / 3.0
            } else {
                return false;
            };
            let cap = match name {
                "font-size" => 96.0,
                "line-height" => 192.0,
                name if name.starts_with("padding") => 64.0,
                name if name.starts_with("border") => 16.0,
                _ => return false,
            };
            pixels.is_finite() && (0.0..=cap).contains(&pixels)
        }
        _ => false,
    }
}

impl<'i> AtRuleParser<'i> for Declarations {
    type Prelude = ();
    type AtRule = String;
    type Error = ();
}
impl<'i> QualifiedRuleParser<'i> for Declarations {
    type Prelude = ();
    type QualifiedRule = String;
    type Error = ();
}
impl<'i> RuleBodyItemParser<'i, String, ()> for Declarations {
    fn parse_declarations(&self) -> bool {
        true
    }
    fn parse_qualified(&self) -> bool {
        false
    }
}

pub(crate) fn clean_style(style: &str) -> String {
    let mut input = ParserInput::new(style);
    let mut parser = Parser::new(&mut input);
    RuleBodyParser::new(&mut parser, &mut Declarations)
        .filter_map(Result::ok)
        .collect::<Vec<_>>()
        .join(";")
}
