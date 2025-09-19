use std::io;

use async_trait::async_trait;
use thiserror::Error;

use miltr_common::{
    actions::{Action, Continue},
    commands::{Body, Connect, Header, Helo, Macro, Mail, Recipient, Unknown},
    modifications::ModificationResponse,
    optneg::OptNeg,
    ProtocolError,
};

/// A trait to implement a working milter server.
///
/// See examples on how to implement this.
#[async_trait]
pub trait Milter: Send {
    /// A user error that might be returned handling this milter communication
    type Error: Send;

    /// Option negotiation for the connection between the miter client and server.
    #[doc(alias = "SMFIC_OPTNEG")]
    #[doc(alias = "xxfi_negotiate")]
    async fn option_negotiation(&mut self, theirs: OptNeg) -> Result<OptNeg, Error<Self::Error>> {
        let mut ours = OptNeg::default();
        ours = ours
            .merge_compatible(&theirs)
            .map_err(ProtocolError::CompatibilityError)?;
        Ok(ours)
    }

    /// A macro sent by the milter client.
    #[doc(alias = "SMFIC_MACRO")]
    async fn macro_(&mut self, _macro: Macro) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Connection information about the smtp connection.
    #[doc(alias = "SMFIC_CONNECT")]
    #[doc(alias = "xxfi_connect")]
    async fn connect(&mut self, _connect_info: Connect) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// The helo name sent by the smtp client.
    #[doc(alias = "SMFIC_HELO")]
    #[doc(alias = "xxfi_helo")]
    async fn helo(&mut self, _helo: Helo) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// The sender this email is from.
    #[doc(alias = "SMFIC_MAIL")]
    #[doc(alias = "from")]
    #[doc(alias = "xxfi_envfrom")]
    async fn mail(&mut self, _mail: Mail) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// A recipient to which this mail is to be transmitted to.
    #[doc(alias = "SMFIC_RCPT")]
    #[doc(alias = "to")]
    #[doc(alias = "xxfi_envrcpt")]
    async fn rcpt(&mut self, _recipient: Recipient) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// Called before data (=body + headers) is sent.
    ///
    /// This allows to first receive sender and receiver, then the rest of the
    /// data.
    #[doc(alias = "SMFIC_DATA")]
    #[doc(alias = "xxfi_data")]
    async fn data(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// A single header with it's name and value.
    ///
    /// Header names are not unique and might be received multiple times.
    #[doc(alias = "SMFIC_HEADER")]
    #[doc(alias = "xxfi_header")]
    async fn header(&mut self, _header: Header) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// Called after all headers have been sent.
    #[doc(alias = "SMFIC_EOH")]
    #[doc(alias = "xxfi_eoh")]
    async fn end_of_header(&mut self) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// A body part was received.
    ///
    /// This may be called multiple times until the whole body was transmitted.
    #[doc(alias = "SMFIC_BODY")]
    #[doc(alias = "xxfi_body")]
    async fn body(&mut self, _body: Body) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// Called after all body parts have been received.
    ///
    /// This is the only stage at which to respond with modifications
    /// to the milter client.
    #[doc(alias = "SMFIC_BODYEOB")]
    #[doc(alias = "xxfi_eom")]
    async fn end_of_body(&mut self) -> Result<ModificationResponse, Self::Error> {
        Ok(ModificationResponse::empty_continue())
    }

    /// A command not matching any Code is received as `unknown`.
    #[doc(alias = "SMFIC_UNKNOWN")]
    #[doc(alias = "xxfi_unknown")]
    async fn unknown(&mut self, _cmd: Unknown) -> Result<Action, Self::Error> {
        Ok(Continue.into())
    }

    /// Reset the message handling to accept a new connection.
    ///
    /// Contrary to it's name, a connection is not aborted here necessarily.
    /// This function is called at the end of every message processing, regardless
    /// of outcome, but the connection is kept open and ready to process the next
    /// message.
    ///
    /// This is the only function not covered by a default. The implementor
    /// needs to reset it's state to handle a new connection.
    ///
    /// See [`Server::default_postfix`](crate::Server::default_postfix).
    #[doc(alias = "SMFIC_ABORT")]
    #[doc(alias = "xxfi_abort")]
    async fn abort(&mut self) -> Result<(), Self::Error>;

    /// Called on quitting a connection from a milter client.
    ///
    /// Some clients (postfix) do not call this method and instead call
    /// `abort` with the expectation the connection is closed.
    ///
    /// See [`Server::default_postfix`](crate::Server::default_postfix).
    #[doc(alias = "SMFIC_QUIT")]
    #[doc(alias = "xxfi_close")]
    async fn quit(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Called when a milter client want's to re-use this milter for a new mail.
    #[doc(alias = "SMFIC_QUIT_NC")]
    async fn quit_nc(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// The main error for this crate encapsulating the different error cases.
#[derive(Debug, Error)]
pub enum Error<ImplError> {
    /// If IO breaks, this will return a [`Error::Io`],
    /// which is a simple [`std::io::Error`]. Check the underlying transport.
    #[error(transparent)]
    Io(#[from] io::Error),

    /// The Codec had problems de/encoding data. This might be
    /// a problem in the implementation or an incompatibility between this crate
    #[error(transparent)]
    Codec(#[from] ProtocolError),

    /// The milter trait implementation returned an error.
    /// This is plumbed through and returned to the call site.
    #[error(transparent)]
    Impl {
        /// The application error patched through
        source: ImplError,
    },
}

impl<AppError> Error<AppError> {
    pub(crate) fn from_app_error(source: AppError) -> Self {
        Self::Impl { source }
    }
}
