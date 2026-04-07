//! Message handler logic.

use chrono::{DateTime, Utc};
use threema_gateway::protocol::{MessageId, ThreemaId, e2e::delivery_receipt::DeliveryReceipt};

use crate::commands::Commands;

/// Error type for [`MessageHandler`] methods.
///
/// This is a type-erased error: Handler implementations can return any error type that implements
/// [`std::error::Error`].
///
/// The simplest way to do this is by converting a string:
///
/// ```rust
/// use threema_gateway_bot::server::handler::HandlerError;
///
/// let err: HandlerError = "something went wrong".into();
/// let err: HandlerError = format!("bad value: {}", 42).into();
/// ```
pub type HandlerError = Box<dyn std::error::Error + Send + Sync>;

/// Result type for [`MessageHandler`] methods.
pub type HandlerResult<T> = Result<T, HandlerError>;

/// Indicates how a command was classified by the command registry.
///
/// Passed to [`MessageHandler::handle_command`] so the handler can distinguish
/// between commands it explicitly registered and commands that were not recognized.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CommandType {
    /// A command registered via [`Commands::register`](crate::commands::Commands::register).
    Registered,
    /// A command that was not registered.
    ///
    /// Only dispatched to the handler if
    /// [`Commands::handle_unknown`](crate::commands::Commands::handle_unknown) was called.
    Unknown,
}

/// Action returned by handler methods to indicate how the bot should respond.
#[derive(Debug, Clone)]
pub enum Action {
    /// Do nothing (no response to the user).
    Ignore,
    /// Show the auto-generated help text, optionally preceded by a message.
    ShowHelp {
        /// Optional text shown before the help text (e.g. "Unknown command: /foo").
        prelude: Option<String>,
    },
    /// Send these responses to the user.
    Respond(Vec<Response>),
}

/// Trait for implementing custom message handling logic.
///
/// Implement this trait to define how your bot responds to messages.
///
/// Use the [async-trait](https://crates.io/crates/async-trait) crate to implement this trait.
///
/// # Example
///
/// ```rust
/// use async_trait::async_trait;
/// use threema_gateway_bot::server::handler::{
///     Action, HandlerResult, MessageContext, MessageHandler, Response,
/// };
///
/// struct EchoMessageHandler;
///
/// #[async_trait]
/// impl MessageHandler for EchoMessageHandler {
///     async fn handle_text(&self, _ctx: &MessageContext, text: &str) -> HandlerResult<Action> {
///         Ok(Action::Respond(vec![Response::text(text)]))
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync + 'static {
    /// Bot description shown at the top of the help text.
    ///
    /// Return `None` (the default) to omit the description.
    fn description(&self) -> Option<&str> {
        None
    }

    /// Define the commands this bot supports.
    ///
    /// The default returns an empty command set (only the built-in `/help` command).
    fn commands() -> Commands {
        Commands::new()
    }

    /// Handle an incoming text message.
    ///
    /// Called for non-command messages that should be processed by your bot logic.
    async fn handle_text(&self, ctx: &MessageContext, text: &str) -> HandlerResult<Action>;

    /// Handle a command.
    ///
    /// Called when a `/command` is received. The `command` parameter contains the
    /// command name without the leading `/`, and `args` contains any text after
    /// the command. The `command_type` indicates how the command was classified.
    ///
    /// For registered commands (via [`Commands::register`]), this is always called.
    /// For unknown commands, this is only called if [`Commands::handle_unknown`]
    /// was set to `true`.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn commands() -> Commands {
    ///     Commands::new()
    ///         .register("remind", "Set a reminder")
    ///         .register("list", "List your reminders")
    /// }
    ///
    /// async fn handle_command(
    ///     &self,
    ///     ctx: &MessageContext,
    ///     command: &str,
    ///     args: &str,
    ///     command_type: CommandType,
    /// ) -> HandlerResult<Action> {
    ///     match command {
    ///         "remind" => Ok(Action::Respond(self.handle_remind(ctx, args).await?)),
    ///         "list" => Ok(Action::Respond(self.handle_list(ctx).await?)),
    ///         _ => Ok(Action::ShowHelp { prelude: None }),
    ///     }
    /// }
    /// ```
    #[allow(unused)]
    async fn handle_command(
        &self,
        ctx: &MessageContext,
        command: &str,
        args: &str,
        command_type: CommandType,
    ) -> HandlerResult<Action> {
        Ok(Action::ShowHelp { prelude: None })
    }

    /// Handle a classic reaction (thumbs up or thumbs down, sent in a delivery receipt message).
    #[allow(unused)]
    async fn handle_classic_reaction(
        &self,
        ctx: &MessageContext,
        reaction: ClassicReaction,
    ) -> HandlerResult<Action> {
        Ok(Action::Ignore)
    }

    /// Called after a response message is sent to the user.
    ///
    /// This provides the message ID of the sent message, which can be used
    /// for tracking reactions to that message (e.g., for confirmation flows).
    ///
    /// # Arguments
    /// * `ctx` - The original message context (contains user's message_id)
    /// * `sent_message_id` - The ID of the message that was just sent by the bot
    #[allow(unused)]
    async fn on_response_sent(
        &self,
        ctx: &MessageContext,
        sent_message_id: MessageId,
    ) -> HandlerResult<()> {
        Ok(()) // Default: no-op
    }
}

/// Message context passed to the handler.
#[derive(Debug, Clone)]
pub struct MessageContext {
    /// The incoming message ID.
    pub message_id: MessageId,
    /// Sender's Threema ID.
    pub sender_identity: ThreemaId,
    /// Sender's public nickname.
    pub sender_nickname: Option<String>,
    /// Timestamp when the message was created.
    ///
    /// Note: This value is set by the sender of the message (not the server).
    pub created_at: DateTime<Utc>,
}

/// A classic reaction (i.e. thumbs up or down, sent in a delivery receipt message).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClassicReaction {
    /// Thumbs up / acknowledge
    ThumbsUp,
    /// Thumbs down / decline
    ThumbsDown,
}

impl TryFrom<DeliveryReceipt> for ClassicReaction {
    type Error = String;

    fn try_from(value: DeliveryReceipt) -> std::result::Result<Self, Self::Error> {
        match value {
            DeliveryReceipt::Acknowledged => Ok(Self::ThumbsUp),
            DeliveryReceipt::Declined => Ok(Self::ThumbsDown),
            other => Err(format!("Invalid delivery receipt type: {other:?}")),
        }
    }
}

/// A response towards a user contacting the bot.
#[derive(Debug, Clone)]
pub enum Response {
    /// Text to send back to the user.
    Text(String),
    /// Image displayed inline in the chat (media rendering type).
    Image(FileResponse),
    /// File shown as a downloadable attachment (file rendering type).
    File(FileResponse),
}

/// File/image response data.
#[derive(Debug, Clone)]
pub struct FileResponse {
    /// The file data bytes.
    pub data: Vec<u8>,
    /// The media (MIME) type (e.g., "image/jpeg", "application/pdf").
    pub media_type: String,
    /// Optional file name.
    pub file_name: Option<String>,
    /// Optional caption/description.
    pub caption: Option<String>,
}

impl Response {
    /// Create a simple text response.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Create an image response displayed inline in the chat.
    ///
    /// Note: Media type should be either "image/jpeg" or "image/png".
    pub fn image(data: Vec<u8>, media_type: impl Into<String>, caption: Option<String>) -> Self {
        Self::Image(FileResponse {
            data,
            media_type: media_type.into(),
            file_name: None,
            caption,
        })
    }

    /// Create a file response shown as a downloadable attachment.
    pub fn file(
        data: Vec<u8>,
        media_type: impl Into<String>,
        file_name: Option<String>,
        caption: Option<String>,
    ) -> Self {
        Self::File(FileResponse {
            data,
            media_type: media_type.into(),
            file_name,
            caption,
        })
    }
}
