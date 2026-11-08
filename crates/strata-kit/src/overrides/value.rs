//! A row's value, read for its token's kind.

use toml::de::DeValue;

use super::Problem;
use crate::theme::Rgba8;
use crate::tokens::{Kind, Token, Value};

/// `value` as `token` takes it, or why not. Whatever this returns, `token.set` accepts.
pub(super) fn read(token: &'static Token, value: &DeValue<'_>) -> Result<Value, Problem> {
    let name = token.name();
    let wrong = || Problem::Kind {
        name,
        expected: token.kind(),
        found: value.type_str(),
    };
    match token.kind() {
        Kind::Color => match value {
            DeValue::String(s) => color(s).map(Value::Color).ok_or(Problem::BadColor {
                name,
                value: s.to_string(),
            }),
            _ => Err(wrong()),
        },
        Kind::Length | Kind::Number => {
            let x = number(value).ok_or_else(wrong)?;
            let (min, max) = token.range().unwrap_or((f32::NEG_INFINITY, f32::INFINITY));
            if !(x.is_finite() && min <= x && x <= max) {
                return Err(Problem::Range { name, min, max });
            }
            Ok(if token.kind() == Kind::Length {
                Value::Length(x)
            } else {
                Value::Number(x)
            })
        }
        Kind::Flag => match value {
            DeValue::Boolean(b) => Ok(Value::Flag(*b)),
            _ => Err(wrong()),
        },
    }
}

/// A colour string, as a theme file writes one.
fn color(s: &str) -> Option<rune::Color> {
    Rgba8::parse(s).map(Rgba8::color)
}

/// A TOML integer or float as an `f32`: infinite when an integer doesn't fit, so the range
/// check rejects it. `None` for anything else.
fn number(value: &DeValue<'_>) -> Option<f32> {
    match value {
        DeValue::Integer(i) => {
            Some(i64::from_str_radix(i.as_str(), i.radix()).map_or(f32::INFINITY, |v| v as f32))
        }
        DeValue::Float(x) => Some(x.as_str().parse::<f64>().map_or(f32::NAN, |v| v as f32)),
        _ => None,
    }
}
