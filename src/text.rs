pub(crate) const WELCOME_BILLBOARD: &'static str = "By participating in this community, you agree to follow the Rust Code of Conduct, as linked below. Please click the :white_check_mark: below to acknowledge and gain access to the channels.

  https://www.rust-lang.org/policies/code-of-conduct

If you see someone behaving inappropriately, or otherwise against the Code of Conduct, please contact the mods using `@mods` or by DM'ing a mod from the sidebar.  ";

pub(crate) fn ban_message(reason: &str, hours: u64) -> String {
    format!("You have been banned from The Rust Programming Language discord server for {}. The ban will expire in {} hours. If you feel this action was taken unfairly, you can reach the Rust moderation team at discord-mods@rust-lang.org", reason, hours)
}
