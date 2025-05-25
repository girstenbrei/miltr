#![doc = include_str!("../Readme.md")]

mod codec;
mod milter;

#[cfg(feature = "_fuzzing")]
pub mod fuzzing;

use asynchronous_codec::Framed;
pub use milter::{Error, Milter};

use futures::{AsyncRead, AsyncWrite, Future, SinkExt, StreamExt};
use miltr_common::{
    actions::Action,
    decoding::ClientCommand,
    encoding::ServerMessage,
    optneg::{Capability, OptNeg},
};
use miltr_utils::debug;
#[cfg(feature = "tracing")]
use tracing::instrument;

pub(crate) use self::codec::MilterCodec;

/// The entry point to host a milter server
#[derive(Debug)]
pub struct Server<'m, M: Milter> {
    milter: &'m mut M,
    codec: MilterCodec,
    quit_on_abort: bool,
}

impl<'m, M: Milter> Server<'m, M> {
    /// Create a new Server to handle connections
    pub fn new(milter: &'m mut M, quit_on_abort: bool, max_buffer_size: usize) -> Self {
        let codec = MilterCodec::new(max_buffer_size);
        Self {
            milter,
            codec,
            quit_on_abort,
        }
    }

    /// Create a server with defaults working with postfix.
    ///
    /// The main difference is treating the call to `abort` like a call to
    /// `quit`. See [this comment][c] as a source in the postfix docs
    ///
    /// AFAIK, originally there where three use cases individual methods:
    /// 1. Abort
    ///
    ///   The current smtp client that is connected to the milter client
    ///   has finished. Next mail arrives.
    /// 2. Quit
    ///
    ///   The current smtp client that was connected to the milter client
    ///   has quit it's connection and the milter client will now quit this
    ///   connection.
    /// 3. Quit NC
    ///
    ///   The current smtp client that was connected to the milter client
    ///   has quit it's connection but the milter client would like to re-use
    ///   this connection for someone else.
    ///
    /// Different implementation mix them up, making e.g. postfix just always
    /// opening up a new connection for every milter conversation.
    ///
    /// [c]: https://github.com/vdukhovni/postfix/blob/17dbfb9b8b9b483a23ea84dcd272c6d4010ad74b/postfix/src/milter/milter8.c#L387-L392
    #[must_use]
    pub fn default_postfix(milter: &'m mut M) -> Self {
        Self::new(milter, true, 2_usize.pow(16))
    }

    /// Handle a single milter connection.
    ///
    /// # Arguments
    /// - milter: the object implementing [`crate::Milter`]. It's methods will
    ///   be called at the appropriate times.
    ///
    /// # Errors
    /// This basically errors for three cases: Io Problems, Codec Problems and
    /// problems returned by the milter implementation.
    ///
    /// Have a look at [`enum@crate::Error`] for more information.
    #[cfg_attr(feature = "tracing", instrument(skip_all))]
    pub async fn handle_connection<RW: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        socket: RW,
    ) -> Result<(), Error<M::Error>> {
        let mut framed = Framed::new(socket, &mut self.codec);

        let mut options: Option<OptNeg> = Option::None;

        while let Some(command) = framed.next().await {
            let command = command?;
            debug!("Received {}", command);

            match command {
                // First, all the regular smtp related commands
                ClientCommand::Helo(helo) => {
                    Self::notify_respond_answer(self.milter.helo(helo), &mut framed).await?;
                }
                ClientCommand::Connect(connect) => {
                    Self::notify_respond_answer(self.milter.connect(connect), &mut framed).await?;
                }
                ClientCommand::Mail(mail) => {
                    Self::notify_respond_answer(self.milter.mail(mail), &mut framed).await?;
                }
                ClientCommand::Recipient(rcpt) => {
                    Self::notify_respond_answer(self.milter.rcpt(rcpt), &mut framed).await?;
                }
                ClientCommand::Data(_v) => {
                    Self::notify_respond_answer(self.milter.data(), &mut framed).await?;
                }
                ClientCommand::Header(header) => {
                    Self::notify_respond_answer(self.milter.header(header), &mut framed).await?;
                }
                ClientCommand::EndOfHeader(_v) => {
                    Self::notify_respond_answer(self.milter.end_of_header(), &mut framed).await?;
                }
                ClientCommand::Body(body) => {
                    Self::notify_respond_answer(self.milter.body(body), &mut framed).await?;
                }
                ClientCommand::Unknown(unknown) => {
                    Self::notify_respond_answer(self.milter.unknown(unknown), &mut framed).await?;
                }
                // Regular smtp session related commands that need special responses
                ClientCommand::EndOfBody(_v) => {
                    // Notify the milter trait implementation
                    let mut responses = self
                        .milter
                        .end_of_body()
                        .await
                        .map_err(Error::from_app_error)?;

                    // Filter those returned mod requests, keep only those
                    // which have been set by the current capabilities.
                    responses.filter_mods_by_caps(
                        options
                            .as_ref()
                            .map_or(Capability::all(), |o| o.capabilities),
                    );

                    // And send them back
                    let responses: Vec<ServerMessage> = responses.into();
                    for response in responses {
                        debug!("Sending response");
                        framed.send(&response).await?;
                    }
                }
                ClientCommand::Macro(macro_) => {
                    self.milter
                        .macro_(macro_)
                        .await
                        .map_err(Error::from_app_error)?;
                }

                // Control flow cases
                // Option Negotiation
                ClientCommand::OptNeg(opt_neg) => {
                    let response = self.milter.option_negotiation(opt_neg).await?;
                    options = Some(response.clone());
                    framed.send(&response.into()).await?;
                }
                // Abort the current smtp session handling
                ClientCommand::Abort(_v) => {
                    let response = self.milter.abort().await.map_err(Error::from_app_error)?;

                    if self.quit_on_abort {
                        self.milter.quit().await.map_err(Error::from_app_error)?;
                        return Ok(());
                    }
                    framed.send(&response.into()).await?;
                }
                // Quit this connection
                ClientCommand::Quit(_v) => {
                    self.milter.quit().await.map_err(Error::from_app_error)?;
                    return Ok(());
                }
                // Quit and re-use this connection
                ClientCommand::QuitNc(_v) => {
                    self.milter.quit_nc().await.map_err(Error::from_app_error)?;
                }
            }
        }
        Ok(())
    }

    /// Helper function to notify the milter, handle errors and respond
    async fn notify_respond_answer<RW: AsyncRead + AsyncWrite + Unpin>(
        milter_fn: impl Future<Output = Result<impl Into<Action>, M::Error>>,
        framed: &mut Framed<RW, &mut MilterCodec>,
    ) -> Result<(), milter::Error<M::Error>> {
        let response = milter_fn.await.map_err(Error::from_app_error)?;
        let response: Action = response.into();

        framed.send(&response.into()).await?;
        Ok(())
    }
}
