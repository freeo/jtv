//! Optional shell adapters emitted as reviewed static source.

/// Return the zsh adapter used for opt-in live history integration.
pub fn zsh_init() -> &'static str {
    include_str!("../assets/jtv-shell-init.zsh")
}

#[cfg(test)]
mod tests {
    #[test]
    fn zsh_adapter_has_no_startup_file_mutation_or_eval() {
        let source = super::zsh_init();
        assert!(source.contains("function jtv()"));
        assert!(source.contains("command jtv \"$@\""));
        assert!(source.contains("print -s -- \"$entry\""));
        assert!(!source.contains(".zshrc"));
        assert!(
            !source
                .lines()
                .any(|line| line.trim_start().starts_with("eval "))
        );
    }
}
