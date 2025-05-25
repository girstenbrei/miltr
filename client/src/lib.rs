#![doc = include_str!("../Readme.md")]

mod codec;

#[cfg(feature = "_fuzzing")]
pub mod fuzzing;

use std::{ops::Deref, sync::Arc};

use asynchronous_codec::Framed;
use futures::{AsyncRead, AsyncWrite, SinkExt, StreamExt};
use miltr_utils::debug;
use paste::paste;
use thiserror::Error;
#[cfg(feature = "tracing")]
use tracing::{instrument, Level};

use miltr_common::{
    actions::{Abort, Action, Quit},
    commands::{
        Body, Command, Connect, Data, EndOfBody, EndOfHeader, Header, Helo, Mail, Recipient,
        Unknown,
    },
    decoding::ServerCommand,
    modifications::{ModificationAction, ModificationResponse},
    optneg::{CompatibilityError, OptNeg},
    ProtocolError,
};

use self::codec::MilterCodec;

/// A milter client using some options and a codec to talk to a milter server
pub struct Client {
    options: Arc<OptNeg>,
    codec: MilterCodec,
}

/// A single milter connection
///
/// This can be created by calling [`Client::connect_via`] to establish
/// a milter session.
///
/// A regular session could use these commands in order:
///
/// - [`Connection::connect`]
/// - [`Connection::helo`]
/// - [`Connection::mail`]
/// - [`Connection::recipient`]
/// - [`Connection::data`]
/// - [`Connection::header`] (multiple)
/// - [`Connection::end_of_header`]
/// - [`Connection::body`] (multiple)
/// - [`Connection::end_of_body`]
///
/// Be careful about the ordering of these commands, milter implementations
/// are designed to expect them in order they appear in the SMTP protocol.
///
/// # Protocol from `OptNeg`
///
/// Depending on what was set by client and server during option negotiation
/// when establishing the connection, commands might either not be sent at all
/// or no response is awaited.
///
/// Assuming [`Protocol::NO_HELO`](miltr_common::optneg::Protocol::NO_HELO) is
/// set during option negotiation, calling [`Connection::helo`] short-circuits
/// to `return Ok(())`.
///
/// If [`Protocol::NR_HELO`](miltr_common::optneg::Protocol::NR_HELO) is set,
/// calling [`Connection::helo`] does not wait for an answer from the milter
/// server, it immediately `return Ok(())` after sending the command.
///
/// Commands behave differently here, see the implementations for
/// [`Protocol::skip_send`](miltr_common::optneg::Protocol::should_skip_send) and
/// [`Protocol::skip_response`](miltr_common::optneg::Protocol::should_skip_response)
/// for details.
pub struct Connection<RW: AsyncRead + AsyncWrite + Unpin> {
    framed: Framed<RW, MilterCodec>,
    options: OptNeg,
}

impl Client {
    /// Create a client which is able to handle connections with the provided
    /// options.
    #[must_use]
    pub fn new(options: OptNeg) -> Self {
        let codec = MilterCodec::new(2_usize.pow(16));

        Self {
            options: Arc::new(options),
            codec,
        }
    }

    /// Option negotiate with the server
    ///
    /// The steps are:
    /// 1. Send our options to the server
    /// 2. Receive it's options back
    /// 3. Merge them into one
    async fn recv_option_negotiation<RW: AsyncRead + AsyncWrite + Unpin>(
        &self,
        framed: &mut Framed<RW, MilterCodec>,
    ) -> Result<OptNeg, ResponseError> {
        let client_options = &self.options;
        framed.send(&client_options.deref().clone().into()).await?;

        let resp = framed
            .next()
            .await
            .ok_or(ResponseError::MissingServerResponse)??;

        let server_options = match resp {
            ServerCommand::OptNeg(optneg) => Ok(optneg),
            command => Err(ResponseError::Unexpected(command)),
        }?;

        let options = server_options.merge_compatible(&self.options)?;

        Ok(options)
    }

    /// Handle a single milter connection via the provided RW connection
    ///
    /// # Errors
    /// This fails if an io-error is experienced or option negotiation fails
    pub async fn connect_via<RW: AsyncRead + AsyncWrite + Unpin>(
        &self,
        connection: RW,
    ) -> Result<Connection<RW>, ResponseError> {
        let codec = self.codec.clone();
        let mut framed = Framed::new(connection, codec);
        let options = self.recv_option_negotiation(&mut framed).await?;

        let connection = Connection { framed, options };

        Ok(connection)
    }
}

macro_rules! command {
    (
        $(#[$outer:meta])*
        (into) $variant:ident
    ) => {
        paste! {
            $(#[$outer])*
            pub async fn [<$variant:snake>]<C: Into<[<$variant:camel>]>>(&mut self, command: C) -> Result<(), ResponseError> {
                let command_intoed: [<$variant:camel>] = command.into();
                let command: Command = command_intoed.into();

                self.send_command(command).await
            }
        }
    };
    (
        $(#[$outer:meta])*
        (new) $variant:ident
    ) => {
        paste! {
            $(#[$outer])*
            pub async fn [<$variant:snake>](&mut self) -> Result<(), ResponseError> {
                let command: Command = [<$variant:camel>].into();

                self.send_command(command).await
            }
        }
    };
}

impl<RW: AsyncRead + AsyncWrite + Unpin> Connection<RW> {
    command!(
        /// Send connect information.
        ///
        ///
        /// # Errors
        /// Errors on any response from the milter server that is not Continue
        (into) Connect
    );

    command!(
        /// Handle a client helo
        ///
        ///
        /// # Errors
        /// Errors on any response from the milter server that is not Continue
        (into) Helo
    );

    command!(
        /// Send the sender info
        ///
        ///
        /// # Errors
        /// Errors on any response from the milter server that is not Continue
        (into) Mail
    );

    command!(
        /// Send the recipient info
        ///
        ///
        /// # Errors
        /// Errors on any response from the milter server that is not Continue
        (into) Recipient
    );

    command!(
        /// Indicate that data follows
        ///
        ///
        /// # Errors
        /// Errors on any response from the milter server that is not Continue
        (new) Data
    );

    command!(
        /// Send headers
        ///
        ///
        /// # Errors
        /// Errors on any response from the milter server that is not Continue
        (into) Header
    );

    command!(
        /// Indicate all headers have been sent
        ///
        ///
        /// # Errors
        /// Errors on any response from the milter server that is not Continue
        (new) EndOfHeader
    );

    command!(
        /// Send a body part
        ///
        ///
        /// # Errors
        /// Errors on any response from the milter server that is not Continue
        (into) Body
    );

    // command!(
    //     /// Indicate all body parts have been sent
    //     ///
    //     /// # Errors
    //     /// Errors on any response from the milter server that is not Continue
    //     (new) EndOfBody
    // );

    /// Indicate all body parts have been sent
    ///
    /// # Errors
    /// Errors on any response from the milter server that is not Continue
    pub async fn end_of_body(&mut self) -> Result<ModificationResponse, ResponseError> {
        // First, send the eob command
        let command: Command = EndOfBody.into();
        self.framed.send(&command.into()).await?;

        let mut modification_response_builder = ModificationResponse::builder();
        loop {
            // Receive a response from the server
            let answer = self.receive_answer().await?;

            // Convert it to a command type
            let command: CommandType = answer.try_into()?;

            match command {
                CommandType::Action(action) => {
                    return Ok(modification_response_builder.build(action));
                }
                CommandType::ModificationAction(action) => {
                    modification_response_builder.push(action);
                }
            }
        }
    }

    /// Receive all modification requests from the server
    ///
    /// # Errors
    /// Errors on error regarding server communication
    pub async fn modification(&mut self) -> Result<CommandType, ResponseError> {
        let resp = self.receive_answer().await?;

        CommandType::try_from(resp)
    }

    /// Ask for a graceful connection shutdown
    ///
    /// # Errors
    /// Errors on io or codec Errors
    pub async fn quit(mut self) -> Result<(), ProtocolError> {
        self.framed.send(&Action::Quit(Quit).into()).await?;

        Ok(())
    }

    /// Ask to re-use this connection for a new mail
    ///
    /// # Errors
    /// Errors on any response from the milter server that is not Continue
    pub fn quit_nc(self) -> Result<(), ProtocolError> {
        todo!("Quit_NC Not yet implemented")
    }

    /// Abort processing for the current mail
    ///
    /// # Errors
    /// Errors on io or codec Errors
    pub async fn abort(mut self) -> Result<(), ProtocolError> {
        self.framed.send(&Action::from(Abort).into()).await?;

        Ok(())
    }

    command!(
        /// Send an unknown command to the server.
        ///
        ///
        /// # Errors
        /// Errors on io or codec Errors
        (into) Unknown
    );

    /// Send a command to the server respecting protocol settings
    #[cfg_attr(feature = "tracing", instrument(level = Level::DEBUG, skip(self), fields(%command), err))]
    async fn send_command(&mut self, command: Command) -> Result<(), ResponseError> {
        // Eval skips
        if self.options.protocol.should_skip_send(&command) {
            debug!("Skip sending");
            return Ok(());
        }
        let skip_response = self.options.protocol.should_skip_response(&command);

        // Send it
        debug!("Sending command");
        self.framed.send(&command.into()).await?;

        // Check response
        if skip_response {
            debug!("Skip receiving response");
            return Ok(());
        }
        self.expect_continue().await
    }

    /// Shortcut to fetch an answer from the server
    async fn receive_answer(&mut self) -> Result<ServerCommand, ResponseError> {
        let resp = self
            .framed
            .next()
            .await
            .ok_or(ResponseError::MissingServerResponse)??;

        Ok(resp)
    }
    /// Shortcut expect a Continue answer from the server
    async fn expect_continue(&mut self) -> Result<(), ResponseError> {
        // Receive back answer
        let resp = self.receive_answer().await?;

        // If continue, just continue. Otherwise return an error
        match resp {
            ServerCommand::Continue(_c) => Ok(()),
            command => Err(ResponseError::Unexpected(command)),
        }
    }
}

/// An error for all problems the client could experience
#[derive(Debug, Error)]
pub enum ResponseError {
    /// Anything protocol related
    #[error(transparent)]
    ProtocolError(#[from] ProtocolError),
    /// If there should have been a response
    #[error("Server did not respond to a query")]
    MissingServerResponse,
    /// If there was a response but it was the wrong one
    #[error("Server respond with an unexpected answer")]
    Unexpected(ServerCommand),
    /// If we have a protocol compatibility issue
    #[error(transparent)]
    CompatibilityError(#[from] CompatibilityError),
}

/// The types of commands the server may respond with
pub enum CommandType {
    /// A regular control flow action
    Action(Action),
    /// A data modification action
    ModificationAction(ModificationAction),
}

impl TryFrom<ServerCommand> for CommandType {
    type Error = ResponseError;

    fn try_from(value: ServerCommand) -> Result<Self, Self::Error> {
        match value {
            ServerCommand::OptNeg(value) => Err(ResponseError::Unexpected(value.into())),
            ServerCommand::Abort(value) => Ok(Self::Action(value.into())),
            ServerCommand::Continue(value) => Ok(Self::Action(value.into())),
            ServerCommand::Discard(value) => Ok(Self::Action(value.into())),
            ServerCommand::Reject(value) => Ok(Self::Action(value.into())),
            ServerCommand::Tempfail(value) => Ok(Self::Action(value.into())),
            ServerCommand::Skip(value) => Ok(Self::Action(value.into())),
            ServerCommand::Replycode(value) => Ok(Self::Action(value.into())),
            ServerCommand::AddRecipient(value) => Ok(Self::ModificationAction(value.into())),
            ServerCommand::DeleteRecipient(value) => Ok(Self::ModificationAction(value.into())),
            ServerCommand::ReplaceBody(value) => Ok(Self::ModificationAction(value.into())),
            ServerCommand::AddHeader(value) => Ok(Self::ModificationAction(value.into())),
            ServerCommand::InsertHeader(value) => Ok(Self::ModificationAction(value.into())),
            ServerCommand::ChangeHeader(value) => Ok(Self::ModificationAction(value.into())),
            ServerCommand::Quarantine(value) => Ok(Self::ModificationAction(value.into())),
        }
    }
}
