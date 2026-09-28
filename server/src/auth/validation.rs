//! Account field rules shared by registration, sign-in, and password reset.
//!
//! Every rule returns a closed code; the HTTP edge renders them as field
//! errors and never echoes the submitted value.

use serde::Serialize;

pub const PASSWORD_MIN_CHARS: usize = 12;
pub const PASSWORD_MAX_CHARS: usize = 128;
pub const DISPLAY_NAME_MAX_CHARS: usize = 80;
pub const EMAIL_MAX_CHARS: usize = 254;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldProblem {
    Invalid,
    Empty,
    TooShort,
    TooLong,
}

/// Field errors for one request; `None` means the field passed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<FieldProblem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<FieldProblem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<FieldProblem>,
}

impl FieldErrors {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.email.is_none() && self.password.is_none() && self.display_name.is_none()
    }
}

/// The comparison key for an email: trimmed and lowercased.
#[must_use]
pub fn email_key(email: &str) -> String {
    email.trim().to_lowercase()
}

/// A deliberately small shape check: one `@`, a non-empty local part, a
/// dotted domain, no whitespace, at most 254 characters.
#[must_use]
pub fn email_problem(email: &str) -> Option<FieldProblem> {
    let email = email.trim();
    if email.is_empty() {
        return Some(FieldProblem::Empty);
    }
    if email.chars().count() > EMAIL_MAX_CHARS {
        return Some(FieldProblem::TooLong);
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Some(FieldProblem::Invalid);
    };
    let well_formed = !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(char::is_whitespace);
    (!well_formed).then_some(FieldProblem::Invalid)
}

#[must_use]
pub fn password_problem(password: &str) -> Option<FieldProblem> {
    let length = password.chars().count();
    if length < PASSWORD_MIN_CHARS {
        Some(FieldProblem::TooShort)
    } else if length > PASSWORD_MAX_CHARS {
        Some(FieldProblem::TooLong)
    } else {
        None
    }
}

#[must_use]
pub fn display_name_problem(display_name: &str) -> Option<FieldProblem> {
    let name = display_name.trim();
    if name.is_empty() {
        Some(FieldProblem::Empty)
    } else if name.chars().count() > DISPLAY_NAME_MAX_CHARS {
        Some(FieldProblem::TooLong)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_shapes() {
        for valid in [
            "owner@example.com",
            " Owner@Example.COM ",
            "a+b@sub.example.dev",
        ] {
            assert_eq!(email_problem(valid), None, "{valid}");
        }
        assert_eq!(email_problem("  "), Some(FieldProblem::Empty));
        for invalid in [
            "owner",
            "owner@",
            "@example.com",
            "owner@example",
            "owner@.com",
            "owner@example.",
            "a b@example.com",
            "a@b@example.com",
        ] {
            assert_eq!(
                email_problem(invalid),
                Some(FieldProblem::Invalid),
                "{invalid}"
            );
        }
        assert_eq!(
            email_problem(&format!("{}@example.com", "a".repeat(250))),
            Some(FieldProblem::TooLong)
        );
        assert_eq!(email_key(" Owner@Example.COM "), "owner@example.com");
    }

    #[test]
    fn password_length_is_counted_in_characters() {
        assert_eq!(
            password_problem(&"a".repeat(11)),
            Some(FieldProblem::TooShort)
        );
        assert_eq!(password_problem(&"a".repeat(12)), None);
        assert_eq!(password_problem(&"é".repeat(128)), None);
        assert_eq!(
            password_problem(&"a".repeat(129)),
            Some(FieldProblem::TooLong)
        );
    }

    #[test]
    fn display_names_are_trimmed_and_bounded() {
        assert_eq!(display_name_problem("   "), Some(FieldProblem::Empty));
        assert_eq!(display_name_problem(" Ada "), None);
        assert_eq!(
            display_name_problem(&"x".repeat(81)),
            Some(FieldProblem::TooLong)
        );
    }
}
