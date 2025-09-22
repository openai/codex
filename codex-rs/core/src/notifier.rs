pub(crate) trait Notifier {
    /// Print summary of effective configuration and user prompt.
    fn print_config_summary(&mut self, config: &Config, prompt: &str);
}
