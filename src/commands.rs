//! Command parsing and handling infrastructure.
//!
//! Provides base command types and parsing for bot commands. Bot implementations define their
//! commands via [`MessageHandler::commands`](crate::server::handler::MessageHandler::commands).
//!
//! ## Command Styles
//!
//! Two command styles are supported:
//!
//! - Slash style: `/remind 30m`
//! - Word style: `start newsletter`
//!
//! The style can be set globally using the [`Commands::style()`] method. By default, slash style is
//! used.

use std::fmt::Write;

/// The style of command syntax used by the bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStyle {
    /// Commands start with `/` (e.g. `/help`, `/remind 30`).
    Slash,
    /// Commands start with a word (e.g. `help`, `remind 30`).
    Word,
}

/// Parsed command from user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParsedCommand<'a> {
    /// Show help text.
    Help,
    /// A registered custom command.
    Registered { name: &'a str, args: &'a str },
    /// Unknown command (not registered).
    Unknown { name: &'a str, args: &'a str },
    /// Not a command - regular message.
    None(&'a str),
}

/// A custom command definition with its name and description.
struct CustomCommand {
    name: String,
    description: String,
}

/// Command configuration for a bot.
///
/// Defines which commands the bot supports and how unknown commands are handled.
/// Returned by [`MessageHandler::commands`](crate::server::handler::MessageHandler::commands).
///
/// # Example
///
/// ```rust
/// # use threema_gateway_bot::commands::{Commands, CommandStyle};
/// let commands = Commands::new()
///     .style(CommandStyle::Slash)
///     .register("remind", "Set a reminder")
///     .register("list", "List your reminders")
///     .handle_unknown(true);
/// ```
pub struct Commands {
    pub(crate) style: CommandStyle,
    custom_commands: Vec<CustomCommand>,
    pub(crate) handle_unknown: bool,
}

impl Commands {
    /// Create an empty command configuration.
    ///
    /// Base commands (like `/help`) are always included.
    /// The default command style is [`CommandStyle::Slash`].
    pub fn new() -> Self {
        Self {
            style: CommandStyle::Slash,
            custom_commands: Vec::new(),
            handle_unknown: false,
        }
    }

    /// Set the command style.
    pub fn style(mut self, style: CommandStyle) -> Self {
        self.style = style;
        self
    }

    /// Register a custom command with a name and description.
    ///
    /// Registered commands are dispatched to
    /// [`MessageHandler::handle_command`](crate::server::handler::MessageHandler::handle_command)
    /// and included in the auto-generated help text.
    pub fn register(mut self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.custom_commands.push(CustomCommand {
            name: name.into(),
            description: description.into(),
        });
        self
    }

    /// Enable dispatching unknown commands to the handler.
    ///
    /// By default, unknown commands (not registered via [`register`](Self::register)) auto-respond
    /// with help text. When enabled, unknown commands are dispatched to
    /// [`MessageHandler::handle_command`](crate::server::handler::MessageHandler::handle_command).
    ///
    /// Note: This will only work for [`CommandStyle::Slash`], since word style command parsing
    /// cannot differentiate between unknown commands and plain text.
    pub fn handle_unknown(mut self, enabled: bool) -> Self {
        self.handle_unknown = enabled;
        self
    }
}

impl Default for Commands {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal registry used by the server for parsing and help text generation.
pub(crate) struct CommandRegistry {
    description: Option<String>,
    commands: Commands,
}

impl CommandRegistry {
    /// Build a registry from a description and command configuration.
    pub(crate) fn new(description: Option<String>, commands: Commands) -> Self {
        Self {
            description,
            commands,
        }
    }

    /// Whether unknown commands should be dispatched to the handler.
    pub(crate) fn handle_unknown(&self) -> bool {
        self.commands.handle_unknown
    }

    /// Format a command name with the appropriate prefix for the current style.
    pub(crate) fn format_command(&self, name: &str) -> String {
        match self.commands.style {
            CommandStyle::Slash => format!("/{name}"),
            CommandStyle::Word => name.to_string(),
        }
    }

    /// Parse a text message into a command.
    ///
    /// Commands are case-sensitive.
    pub(crate) fn parse<'a>(&self, text: &'a str) -> ParsedCommand<'a> {
        let trimmed = text.trim();

        match self.commands.style {
            CommandStyle::Slash => {
                let Some(rest) = trimmed.strip_prefix('/') else {
                    return ParsedCommand::None(trimmed);
                };
                let Some(name) = rest.split_whitespace().next() else {
                    // Bare `/` with no command name
                    return ParsedCommand::None(trimmed);
                };
                let args = rest.strip_prefix(name).map(|s| s.trim()).unwrap_or("");

                if name == "help" {
                    ParsedCommand::Help
                } else if self.commands.custom_commands.iter().any(|c| c.name == name) {
                    ParsedCommand::Registered { name, args }
                } else {
                    ParsedCommand::Unknown { name, args }
                }
            }
            CommandStyle::Word => {
                let Some(name) = trimmed.split_whitespace().next() else {
                    return ParsedCommand::None(trimmed);
                };
                let args = trimmed.strip_prefix(name).map(|s| s.trim()).unwrap_or("");

                if name == "help" {
                    ParsedCommand::Help
                } else if self.commands.custom_commands.iter().any(|c| c.name == name) {
                    ParsedCommand::Registered { name, args }
                } else {
                    // Note: In case of "Word" style (with no command prefix) we cannot
                    // differentiate unknown commands from plain text, so we always return
                    // `ParsedCommand::None`.
                    ParsedCommand::None(trimmed)
                }
            }
        }
    }

    /// Generate help text, optionally preceded by a message.
    pub(crate) fn help_text_with_prelude(&self, prelude: Option<&str>) -> String {
        let help = self.help_text();
        match prelude {
            Some(prelude) => format!("{prelude}\n\n---\n\n{help}"),
            None => help,
        }
    }

    /// Generate help text from the registered commands.
    pub(crate) fn help_text(&self) -> String {
        let mut text = String::new();
        let prefix = match self.commands.style {
            CommandStyle::Slash => "/",
            CommandStyle::Word => "",
        };

        if let Some(ref description) = self.description {
            writeln!(text, "{description}\n").unwrap();
        }

        writeln!(text, "Available Commands:\n").unwrap();
        writeln!(text, "{prefix}help - Show this help message").unwrap();
        for cmd in &self.commands.custom_commands {
            writeln!(text, "{prefix}{} - {}", cmd.name, cmd.description).unwrap();
        }
        text.truncate(text.trim_end().len());
        text
    }
}

/// Default messages for common bot responses.
pub(crate) mod messages {
    /// Access denied for unauthorized users.
    pub const ACCESS_DENIED: &str = "Sorry, you are not authorized to use this service. Please contact the administrator if you believe this is an error.";

    /// Generic error message.
    pub const GENERIC_ERROR: &str =
        "Sorry, I encountered an error processing your request. Please try again.";
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse_slash_style {
        use super::*;

        fn registry() -> CommandRegistry {
            let commands = Commands::new()
                .style(CommandStyle::Slash)
                .register("remind", "Set a reminder")
                .register("list", "List your reminders");
            CommandRegistry::new(None, commands)
        }

        #[test]
        fn help_command() {
            let r = registry();
            assert_eq!(r.parse("/help"), ParsedCommand::Help);
            assert_eq!(r.parse("  /help  "), ParsedCommand::Help);
        }

        #[test]
        fn registered_command() {
            let r = registry();
            assert_eq!(
                r.parse("/remind 30 Take a break"),
                ParsedCommand::Registered {
                    name: "remind",
                    args: "30 Take a break",
                }
            );
            assert_eq!(
                r.parse("/list"),
                ParsedCommand::Registered {
                    name: "list",
                    args: "",
                }
            );
        }

        #[test]
        fn unknown_command() {
            let r = registry();
            assert_eq!(
                r.parse("/foo bar"),
                ParsedCommand::Unknown {
                    name: "foo",
                    args: "bar",
                }
            );
            assert_eq!(
                r.parse("/unknown"),
                ParsedCommand::Unknown {
                    name: "unknown",
                    args: "",
                }
            );
        }

        #[test]
        fn regular_message() {
            let r = registry();
            assert_eq!(r.parse("Hello"), ParsedCommand::None("Hello"));
            assert_eq!(r.parse("  Hello  "), ParsedCommand::None("Hello"));
        }

        #[test]
        fn bare_slash() {
            let r = registry();
            assert_eq!(r.parse("/"), ParsedCommand::None("/"));
            assert_eq!(r.parse("  /  "), ParsedCommand::None("/"));
        }
    }

    mod parse_word_style {
        use super::*;

        fn registry() -> CommandRegistry {
            let commands = Commands::new()
                .style(CommandStyle::Word)
                .register("remind", "Set a reminder")
                .register("list", "List your reminders");
            CommandRegistry::new(None, commands)
        }

        #[test]
        fn help_command() {
            let r = registry();
            assert_eq!(r.parse("help"), ParsedCommand::Help);
            assert_eq!(r.parse("  help  "), ParsedCommand::Help);
        }

        #[test]
        fn registered_command() {
            let r = registry();
            assert_eq!(
                r.parse("remind 30 Take a break"),
                ParsedCommand::Registered {
                    name: "remind",
                    args: "30 Take a break",
                }
            );
            assert_eq!(
                r.parse("list"),
                ParsedCommand::Registered {
                    name: "list",
                    args: "",
                }
            );
        }

        #[test]
        fn regular_message() {
            let r = registry();
            assert_eq!(r.parse("Hello world"), ParsedCommand::None("Hello world"));
            assert_eq!(r.parse("  Hello  "), ParsedCommand::None("Hello"));
        }

        #[test]
        fn empty_message() {
            let r = registry();
            assert_eq!(r.parse(""), ParsedCommand::None(""));
            assert_eq!(r.parse("   "), ParsedCommand::None(""));
        }
    }

    mod help_text {
        use super::*;

        #[test]
        fn without_description() {
            let r = CommandRegistry::new(None, Commands::new());
            insta::assert_snapshot!(r.help_text());
        }

        #[test]
        fn with_description() {
            let r = CommandRegistry::new(Some("My cool bot.".into()), Commands::new());
            insta::assert_snapshot!(r.help_text());
        }

        #[test]
        fn with_custom_commands() {
            let commands = Commands::new()
                .register("remind", "Set a reminder")
                .register("list", "List your reminders");
            let r = CommandRegistry::new(None, commands);
            insta::assert_snapshot!(r.help_text());
        }

        #[test]
        fn with_prelude() {
            let r = CommandRegistry::new(None, Commands::new());
            insta::assert_snapshot!(r.help_text_with_prelude(Some("Unknown command: /foo")));
        }

        #[test]
        fn word_style() {
            let commands = Commands::new()
                .style(CommandStyle::Word)
                .register("remind", "Set a reminder")
                .register("list", "List your reminders");
            let r = CommandRegistry::new(None, commands);
            insta::assert_snapshot!(r.help_text());
        }
    }

    #[test]
    fn messages_not_empty() {
        assert!(!messages::ACCESS_DENIED.is_empty());
        assert!(!messages::GENERIC_ERROR.is_empty());
    }
}
