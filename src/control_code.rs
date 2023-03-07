//! A module which contains [ControlCode] type.

use std::convert::TryFrom;

/// ControlCode represents the standard ASCII control codes [wiki]
///
/// [wiki]: https://en.wikipedia.org/wiki/C0_and_C1_control_codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCode {
    /// Often used as a string terminator, especially in the programming language C.
    Null,
    /// In message transmission, delimits the start of a message header.
    StartOfHeading,
    /// First character of message text, and may be used to terminate the message heading.
    StartOfText,
    /// Often used as a "break" character (Ctrl-C) to interrupt or terminate a program or process.
    EndOfText,
    /// Often used on Unix to indicate end-of-file on a terminal (Ctrl-D).
    EndOfTransmission,
    /// Signal intended to trigger a response at the receiving end, to see if it is still present.
    Enquiry,
    /// Response to an Enquiry, or an indication of successful receipt of a message.
    Acknowledge,
    /// Used for a beep on systems that didn't have a physical bell.
    Bell,
    /// Move the cursor one position leftwards.
    /// On input, this may delete the character to the left of the cursor.
    Backspace,
    /// Position to the next character tab stop.
    HorizontalTabulation,
    /// On Unix, used to mark end-of-line.
    /// In DOS, Windows, and various network standards, LF is used following CR as part of the end-of-line mark.
    LineFeed,
    /// Position the form at the next line tab stop.
    VerticalTabulation,
    /// It appears in some common plain text files as a page break character.
    FormFeed,
    /// Originally used to move the cursor to column zero while staying on the same line.
    CarriageReturn,
    /// Switch to an alternative character set.
    ShiftOut,
    /// Return to regular character set after ShiftOut.
    ShiftIn,
    /// May cause a limited number of contiguously following octets to be interpreted in some different way.
    DataLinkEscape,
    /// A control code which is reserved for device control.
    DeviceControl1,
    /// A control code which is reserved for device control.
    DeviceControl2,
    /// A control code which is reserved for device control.
    DeviceControl3,
    /// A control code which is reserved for device control.
    DeviceControl4,
    /// In multipoint systems, the NAK is used as the not-ready reply to a poll.
    NegativeAcknowledge,
    /// Used in synchronous transmission systems to provide a signal from which synchronous correction may be achieved.
    SynchronousIdle,
    /// Indicates the end of a transmission block of data.
    EndOfTransmissionBlock,
    /// Indicates that the data preceding it are in error or are to be disregarded.
    Cancel,
    /// May mark the end of the used portion of the physical medium.
    EndOfMedium,
    /// Sometimes used to indicate the end of file, both when typing on the terminal and in text files stored on disk.
    Substitute,
    /// The Esc key on the keyboard will cause this character to be sent on most systems.
    /// In systems based on ISO/IEC 2022, even if another set of C0 control codes are used,
    /// this octet is required to always represent the escape character.
    Escape,
    /// Can be used as delimiters to mark fields of data structures.
    /// Also it used for hierarchical levels;
    /// FS == level 4
    FileSeparator,
    /// It used for hierarchical levels;
    /// GS == level 3
    GroupSeparator,
    /// It used for hierarchical levels;
    /// RS == level 2
    RecordSeparator,
    /// It used for hierarchical levels;
    /// US == level 1
    UnitSeparator,
    /// Space is a graphic character. It causes the active position to be advanced by one character position.
    Space,
    /// Usually called backspace on modern machines, and does not correspond to the PC delete key.
    Delete,
}

impl ControlCode {
    /// See [ControlCode::Null]
    pub const NUL: ControlCode = ControlCode::Null;
    /// See [ControlCode::StartOfHeading]
    pub const SOH: ControlCode = ControlCode::StartOfHeading;
    /// See [ControlCode::StartOfText]
    pub const STX: ControlCode = ControlCode::StartOfText;
    /// See [ControlCode::EndOfText]
    pub const ETX: ControlCode = ControlCode::EndOfText;
    /// See [ControlCode::EndOfTransmission]
    pub const EOT: ControlCode = ControlCode::EndOfTransmission;
    /// See [ControlCode::Enquiry]
    pub const ENQ: ControlCode = ControlCode::Enquiry;
    /// See [ControlCode::Acknowledge]
    pub const ACK: ControlCode = ControlCode::Acknowledge;
    /// See [ControlCode::Bell]
    pub const BEL: ControlCode = ControlCode::Bell;
    /// See [ControlCode::Backspace]
    pub const BS: ControlCode = ControlCode::Backspace;
    /// See [ControlCode::HorizontalTabulation]
    pub const HT: ControlCode = ControlCode::HorizontalTabulation;
    /// See [ControlCode::LineFeed]
    pub const LF: ControlCode = ControlCode::LineFeed;
    /// See [ControlCode::VerticalTabulation]
    pub const VT: ControlCode = ControlCode::VerticalTabulation;
    /// See [ControlCode::FormFeed]
    pub const FF: ControlCode = ControlCode::FormFeed;
    /// See [ControlCode::CarriageReturn]
    pub const CR: ControlCode = ControlCode::CarriageReturn;
    /// See [ControlCode::ShiftOut]
    pub const SO: ControlCode = ControlCode::ShiftOut;
    /// See [ControlCode::ShiftIn]
    pub const SI: ControlCode = ControlCode::ShiftIn;
    /// See [ControlCode::DataLinkEscape]
    pub const DLE: ControlCode = ControlCode::DataLinkEscape;
    /// See [ControlCode::DeviceControl1]
    pub const DC1: ControlCode = ControlCode::DeviceControl1;
    /// See [ControlCode::DeviceControl2]
    pub const DC2: ControlCode = ControlCode::DeviceControl2;
    /// See [ControlCode::DeviceControl3]
    pub const DC3: ControlCode = ControlCode::DeviceControl3;
    /// See [ControlCode::DeviceControl4]
    pub const DC4: ControlCode = ControlCode::DeviceControl4;
    /// See [ControlCode::NegativeAcknowledge]
    pub const NAK: ControlCode = ControlCode::NegativeAcknowledge;
    /// See [ControlCode::SynchronousIdle]
    pub const SYN: ControlCode = ControlCode::SynchronousIdle;
    /// See [ControlCode::EndOfTransmissionBlock]
    pub const ETB: ControlCode = ControlCode::EndOfTransmissionBlock;
    /// See [ControlCode::Cancel]
    pub const CAN: ControlCode = ControlCode::Cancel;
    /// See [ControlCode::EndOfMedium]
    pub const EM: ControlCode = ControlCode::EndOfMedium;
    /// See [ControlCode::Substitute]
    pub const SUB: ControlCode = ControlCode::Substitute;
    /// See [ControlCode::Escape]
    pub const ESC: ControlCode = ControlCode::Escape;
    /// See [ControlCode::FileSeparator]
    pub const FS: ControlCode = ControlCode::FileSeparator;
    /// See [ControlCode::GroupSeparator]
    pub const GS: ControlCode = ControlCode::GroupSeparator;
    /// See [ControlCode::RecordSeparator]
    pub const RS: ControlCode = ControlCode::RecordSeparator;
    /// See [ControlCode::UnitSeparator]
    pub const US: ControlCode = ControlCode::UnitSeparator;
    /// See [ControlCode::Space]
    pub const SP: ControlCode = ControlCode::Space;
    /// See [ControlCode::Delete]
    pub const DEL: ControlCode = ControlCode::Delete;
}

impl From<ControlCode> for u8 {
    fn from(val: ControlCode) -> Self {
        use ControlCode::*;
        match val {
            Null => 0,
            StartOfHeading => 1,
            StartOfText => 2,
            EndOfText => 3,
            EndOfTransmission => 4,
            Enquiry => 5,
            Acknowledge => 6,
            Bell => 7,
            Backspace => 8,
            HorizontalTabulation => 9,
            LineFeed => 10,
            VerticalTabulation => 11,
            FormFeed => 12,
            CarriageReturn => 13,
            ShiftOut => 14,
            ShiftIn => 15,
            DataLinkEscape => 16,
            DeviceControl1 => 17,
            DeviceControl2 => 18,
            DeviceControl3 => 19,
            DeviceControl4 => 20,
            NegativeAcknowledge => 21,
            SynchronousIdle => 22,
            EndOfTransmissionBlock => 23,
            Cancel => 24,
            EndOfMedium => 25,
            Substitute => 26,
            Escape => 27,
            FileSeparator => 28,
            GroupSeparator => 29,
            RecordSeparator => 30,
            UnitSeparator => 31,
            Space => 32,
            Delete => 127,
        }
    }
}

impl TryFrom<char> for ControlCode {
    type Error = ();

    fn try_from(c: char) -> Result<ControlCode, ()> {
        use ControlCode::*;
        match c {
            '@' => Ok(Null),
            'A' | 'a' => Ok(StartOfHeading),
            'B' | 'b' => Ok(StartOfText),
            'C' | 'c' => Ok(EndOfText),
            'D' | 'd' => Ok(EndOfTransmission),
            'E' | 'e' => Ok(Enquiry),
            'F' | 'f' => Ok(Acknowledge),
            'G' | 'g' => Ok(Bell),
            'H' | 'h' => Ok(Backspace),
            'I' | 'i' => Ok(HorizontalTabulation),
            'J' | 'j' => Ok(LineFeed),
            'K' | 'k' => Ok(VerticalTabulation),
            'L' | 'l' => Ok(FormFeed),
            'M' | 'm' => Ok(CarriageReturn),
            'N' | 'n' => Ok(ShiftOut),
            'O' | 'o' => Ok(ShiftIn),
            'P' | 'p' => Ok(DataLinkEscape),
            'Q' | 'q' => Ok(DeviceControl1),
            'R' | 'r' => Ok(DeviceControl2),
            'S' | 's' => Ok(DeviceControl3),
            'T' | 't' => Ok(DeviceControl4),
            'U' | 'u' => Ok(NegativeAcknowledge),
            'V' | 'v' => Ok(SynchronousIdle),
            'W' | 'w' => Ok(EndOfTransmissionBlock),
            'X' | 'x' => Ok(Cancel),
            'Y' | 'y' => Ok(EndOfMedium),
            'Z' | 'z' => Ok(Substitute),
            '[' => Ok(Escape),
            '\\' => Ok(FileSeparator),
            ']' => Ok(GroupSeparator),
            '^' => Ok(RecordSeparator),
            '_' => Ok(UnitSeparator),
            ' ' => Ok(Space),
            '?' => Ok(Delete),
            _ => Err(()),
        }
    }
}

impl TryFrom<&str> for ControlCode {
    type Error = ();

    fn try_from(c: &str) -> Result<ControlCode, ()> {
        use ControlCode::*;
        match c {
            "^@" => Ok(Null),
            "^A" => Ok(StartOfHeading),
            "^B" => Ok(StartOfText),
            "^C" => Ok(EndOfText),
            "^D" => Ok(EndOfTransmission),
            "^E" => Ok(Enquiry),
            "^F" => Ok(Acknowledge),
            "^G" => Ok(Bell),
            "^H" => Ok(Backspace),
            "^I" => Ok(HorizontalTabulation),
            "^J" => Ok(LineFeed),
            "^K" => Ok(VerticalTabulation),
            "^L" => Ok(FormFeed),
            "^M" => Ok(CarriageReturn),
            "^N" => Ok(ShiftOut),
            "^O" => Ok(ShiftIn),
            "^P" => Ok(DataLinkEscape),
            "^Q" => Ok(DeviceControl1),
            "^R" => Ok(DeviceControl2),
            "^S" => Ok(DeviceControl3),
            "^T" => Ok(DeviceControl4),
            "^U" => Ok(NegativeAcknowledge),
            "^V" => Ok(SynchronousIdle),
            "^W" => Ok(EndOfTransmissionBlock),
            "^X" => Ok(Cancel),
            "^Y" => Ok(EndOfMedium),
            "^Z" => Ok(Substitute),
            "^[" => Ok(Escape),
            "^\\" => Ok(FileSeparator),
            "^]" => Ok(GroupSeparator),
            "^^" => Ok(RecordSeparator),
            "^_" => Ok(UnitSeparator),
            "^ " => Ok(Space),
            "^?" => Ok(Delete),
            _ => Err(()),
        }
    }
}

impl AsRef<str> for ControlCode {
    fn as_ref(&self) -> &str {
        use ControlCode::*;
        match self {
            Null => "^@",
            StartOfHeading => "^A",
            StartOfText => "^B",
            EndOfText => "^C",
            EndOfTransmission => "^D",
            Enquiry => "^E",
            Acknowledge => "^F",
            Bell => "^G",
            Backspace => "^H",
            HorizontalTabulation => "^I",
            LineFeed => "^J",
            VerticalTabulation => "^K",
            FormFeed => "^L",
            CarriageReturn => "^M",
            ShiftOut => "^N",
            ShiftIn => "^O",
            DataLinkEscape => "^P",
            DeviceControl1 => "^Q",
            DeviceControl2 => "^R",
            DeviceControl3 => "^S",
            DeviceControl4 => "^T",
            NegativeAcknowledge => "^U",
            SynchronousIdle => "^V",
            EndOfTransmissionBlock => "^W",
            Cancel => "^X",
            EndOfMedium => "^Y",
            Substitute => "^Z",
            Escape => "^[",
            FileSeparator => "^\\",
            GroupSeparator => "^]",
            RecordSeparator => "^^",
            UnitSeparator => "^_",
            Space => " ",
            Delete => "^?",
        }
    }
}

impl AsRef<[u8]> for ControlCode {
    fn as_ref(&self) -> &[u8] {
        use ControlCode::*;
        match self {
            Null => &[0],
            StartOfHeading => &[1],
            StartOfText => &[2],
            EndOfText => &[3],
            EndOfTransmission => &[4],
            Enquiry => &[5],
            Acknowledge => &[6],
            Bell => &[7],
            Backspace => &[8],
            HorizontalTabulation => &[9],
            LineFeed => &[10],
            VerticalTabulation => &[11],
            FormFeed => &[12],
            CarriageReturn => &[13],
            ShiftOut => &[14],
            ShiftIn => &[15],
            DataLinkEscape => &[16],
            DeviceControl1 => &[17],
            DeviceControl2 => &[18],
            DeviceControl3 => &[19],
            DeviceControl4 => &[20],
            NegativeAcknowledge => &[21],
            SynchronousIdle => &[22],
            EndOfTransmissionBlock => &[23],
            Cancel => &[24],
            EndOfMedium => &[25],
            Substitute => &[26],
            Escape => &[27],
            FileSeparator => &[28],
            GroupSeparator => &[29],
            RecordSeparator => &[30],
            UnitSeparator => &[31],
            Space => &[32],
            Delete => &[127],
        }
    }
}
