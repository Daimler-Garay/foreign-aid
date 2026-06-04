use crate::domain::validation::ValidationError;

pub fn validate_display_name(display_name: &str) -> Result<(), ValidationError> {
    if display_name.trim().is_empty() {
        return Err(ValidationError::BlankDisplayName);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_display_name() {
        let error = validate_display_name("   ").expect_err("blank name should fail");

        assert_eq!(error, ValidationError::BlankDisplayName);
    }

    #[test]
    fn accepts_non_blank_display_name() {
        validate_display_name("Alice").expect("valid name should pass");
    }
}
