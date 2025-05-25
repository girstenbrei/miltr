use std::{
    borrow::BorrowMut,
    ops::{Index, IndexMut},
};

use bytes::{BufMut, BytesMut};
use itertools::Itertools;
use num_enum::IntoPrimitive;

/// Macro stages requested by this milter server
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MacroStages {
    stages: [Vec<String>; MACRO_STAGE_MAX_ID],
}

impl IndexMut<MacroStage> for MacroStages {
    fn index_mut(&mut self, index: MacroStage) -> &mut Self::Output {
        self.stages[index.as_usize()].borrow_mut()
    }
}

impl Index<MacroStage> for MacroStages {
    type Output = Vec<String>;

    fn index(&self, index: MacroStage) -> &Self::Output {
        &self.stages[index.as_usize()]
    }
}

impl MacroStages {
    pub(crate) fn write(&self, buffer: &mut BytesMut) {
        for (index, stage) in self.stages.iter().enumerate() {
            // For empty requests, don't send anything.
            // Postfix would ignore the request either way.
            if stage.is_empty() {
                continue;
            }

            // Write the macro stage
            let macro_stage: MacroStage = index.into();
            let be_bytes: [u8; 4] = u32::to_be_bytes(macro_stage.into());
            buffer.extend_from_slice(&be_bytes);

            // Implement macro requests:
            // Payload of the Options negotiate response is extended to include a structure of
            // <4-byte macro stage ID><space-separated list of symbols>NULL<4-byte macro stage ID><space-separated list of symbols>NULL
            // Ex: \x00\x00\x00\x01j {client_ptr}\x00\x00\x00\x00\x03j {rcpt_addr}\x00
            buffer.extend_from_slice(stage.iter().join(" ").as_bytes());
            buffer.put_u8(0);
        }
    }

    #[must_use]
    pub(crate) fn len(&self) -> usize {
        let mut accumulator = 0;
        for stage in &self.stages {
            // For empty requests, don't send anything.
            // Postfix would ignore the request either way.
            if stage.is_empty() {
                continue;
            }

            accumulator += MacroStage::CODE_SIZE;
            for symbol in stage {
                //                      The space separator
                // The length of the macro string   |
                accumulator += symbol.len() + 1;
            }

            // At the end, one space separator has been added to the accumulator
            // which is one to many. But, at the end, we also need a nullbyte.
            // So the length is correct.
        }

        accumulator
    }

    /// Request `macros` for the `stage` provided.
    pub fn with_stage<S: ToString>(&mut self, stage: MacroStage, macros: &[S]) {
        let stage = &mut self[stage];
        for m in macros {
            stage.push(m.to_string());
        }
    }
}

const MACRO_STAGE_MAX_ID: usize = 9;

/// A macro stage index into [`MacroStages`]
#[derive(Debug, Copy, Clone, IntoPrimitive, PartialEq, Eq)]
#[repr(u32)]
pub enum MacroStage {
    /// `SMFIM_CONNECT`
    Connect = 0,
    /// `SMFIM_HELO`
    Helo = 1,
    /// `SMFIM_ENVFROM`
    MailFrom = 2,
    /// `SMFIM_ENVRCPT`
    RcptTo = 3,
    /// `SMFIM_DATA`
    Data = 4,
    /// `SMFIM_EOB`
    EndOfBody = 5,
    /// `SMFIM_EOH`
    EndOfHeaders = 6,
    /// `SMFIC_EOH`
    Header = 7,
    /// `SMFIM_BODY`
    Body = 8,
    /// `SMFIC_UNKNOWN`
    Unknown = MACRO_STAGE_MAX_ID as u32,
}

impl From<usize> for MacroStage {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::Connect,      // SMFIM_CONNECT
            1 => Self::Helo,         // SMFIM_HELO
            2 => Self::MailFrom,     // SMFIM_ENVFROM
            3 => Self::RcptTo,       // SMFIM_ENVRCPT
            4 => Self::Data,         // SMFIM_DATA
            5 => Self::EndOfBody,    // SMFIM_EOB
            6 => Self::EndOfHeaders, // SMFIM_EOH
            7 => Self::Header,
            8 => Self::Body,
            // The max id should be unknown
            // MACRO_STAGE_MAX_ID => Self::Unknown,
            _ => Self::Unknown,
        }
    }
}

impl From<u32> for MacroStage {
    fn from(value: u32) -> Self {
        Self::from(value as usize)
    }
}

impl MacroStage {
    const CODE_SIZE: usize = 4;

    fn as_usize(self) -> usize {
        let self_u32: u32 = self.into();
        self_u32 as usize
    }
}
