//! A [`Packet`](./struct.Packet.html) struct and associated infrastructure to read an MPEG Transport Stream packet


/// the different values indicating whether a `Packet`'s `adaptation_field()` and `payload()`
/// methods will return `Some` or `None`.
#[derive(Eq, PartialEq, Debug)]
pub enum AdaptationControl {
    /// This value is used if the transport stream packet `adaptation_control` field uses the value
    /// `0b00`, which is not defined by the spec.
    Reserved,
    /// indicates that this packet contains a payload, but not an adaptation field
    PayloadOnly,
    /// indicates that this packet contains an adaptation field, but not a payload
    AdaptationFieldOnly,
    /// indicates that this packet contains both an adaptation field and a payload
    AdaptationFieldAndPayload,
}

impl AdaptationControl {
    #[inline(always)]
    fn from(val: u8) -> AdaptationControl {
        match val {
            0 => AdaptationControl::Reserved,
            1 => AdaptationControl::PayloadOnly,
            2 => AdaptationControl::AdaptationFieldOnly,
            3 => AdaptationControl::AdaptationFieldAndPayload,
            _ => panic!("invalid value {}", val),
        }
    }

    #[inline(always)]
    pub fn has_payload(self) -> bool {
        match self {
            AdaptationControl::Reserved | AdaptationControl::AdaptationFieldOnly => false,
            AdaptationControl::PayloadOnly | AdaptationControl::AdaptationFieldAndPayload => true,
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum TransportScramblingControl {
    NotScrambled,
    Undefined1,
    Undefined2,
    Undefined3,
}

impl TransportScramblingControl {
    fn from(val: u8) -> TransportScramblingControl {
        match val {
            0 => TransportScramblingControl::NotScrambled,
            1 => TransportScramblingControl::Undefined1,
            2 => TransportScramblingControl::Undefined2,
            3 => TransportScramblingControl::Undefined3,
            _ => panic!("invalid value {}", val),
        }
    }
}

/// A collection of fields that may optionally appear within the header of a transport stream
/// `Packet`.
pub struct AdaptationField<'buf> {
    buf: &'buf [u8],
}

impl<'buf> AdaptationField<'buf> {
    pub fn new(buf: &'buf [u8]) -> AdaptationField {
        AdaptationField { buf }
    }

    pub fn discontinuity_indicator(&self) -> bool {
        self.buf[0] & 0b10000000 != 0
    }
}

/// A counter value used within a transport stream to detect discontinuities in a sequence of packets.
/// The continuity counter should increase by one for each packet with a given PID for which
/// `adaptation_control` indicates that a payload should be present.
///
/// See [`Packet.continuity_counter()`](struct.Packet.html#method.continuity_counter)
#[derive(PartialEq,Debug,Clone,Copy)]
pub struct ContinuityCounter {
    val: u8,
}

impl From<u8> for ContinuityCounter {
    #[inline]
    fn from(count: u8) -> ContinuityCounter {
        ContinuityCounter::new(count)
    }
}

impl ContinuityCounter {
    /// Panics if the given value is greater than 15.
    #[inline]
    pub fn new(count: u8) -> ContinuityCounter {
        assert!(count < 0b10000);
        ContinuityCounter { val: count }
    }

    /// Returns this counter's value, which will be between 0 and 15 inclusive.
    #[inline]
    pub fn count(&self) -> u8 {
        self.val
    }

    /// true iff the given `ContinuityCounter` value follows this one.  Note that the maximum counter
    /// value is 15, and the counter 'wraps around':
    ///
    /// ```rust
    /// # use mpeg2ts_reader::packet::ContinuityCounter;
    /// let a = ContinuityCounter::new(0);
    /// let b = ContinuityCounter::new(15);
    /// assert!(a.follows(b));  // after 15, counter wraps around to 0
    /// ```
    #[inline]
    pub fn follows(&self, other: ContinuityCounter) -> bool {
        (other.val + 1) & 0b1111 == self.val
    }
}

/// A transport stream `Packet` is a wrapper around a byte slice which allows the bytes to be
/// interpreted as a packet structure per _ISO/IEC 13818-1, Section 2.4.3.3_.
pub struct Packet<'buf> {
    buf: &'buf [u8],
}

/// The value `0x47`, which must appear in the first byte of every transport stream packet.
pub const SYNC_BYTE: u8 = 0x47;

/// The fixed 188 byte size of a transport stream packet.
pub const PACKET_SIZE: usize = 188;

const FIXED_HEADER_SIZE: usize = 4;
// when AF present, a 1-byte 'length' field precedes the content,
const ADAPTATION_FIELD_OFFSET: usize = FIXED_HEADER_SIZE + 1;

impl<'buf> Packet<'buf> {
    /// returns `true` if the given value is a valid synchronisation byte, the value `0x42`, which
    /// must appear at the start of every transport stream packet.
    #[inline(always)]
    pub fn is_sync_byte(b: u8) -> bool {
        b == SYNC_BYTE
    }

    /// Panics if the given buffer is less than 188 bytes, or if the initial sync-byte does not
    /// have the correct value (`0x47`).  Calling code is expected to have already checked those
    /// conditions.
    #[inline(always)]
    pub fn new(buf: &'buf [u8]) -> Packet {
        assert_eq!(buf.len(),  PACKET_SIZE);
        assert!(Packet::is_sync_byte(buf[0]));
        Packet { buf }
    }

    pub fn transport_error_indicator(&self) -> bool {
        self.buf[1] & 0b10000000 != 0
    }

    /// a structure larger than a single packet payload needs to be split across multiple packets,
    /// `payload_unit_start()` indicates if this packet payload contains the start of the
    /// structure.  If `false`, this packets payload is a continuation of a structure which began
    /// in an earlier packet within the transport stream.
    #[inline]
    pub fn payload_unit_start_indicator(&self) -> bool {
        self.buf[1] & 0b01000000 != 0
    }

    pub fn transport_priority(&self) -> bool {
        self.buf[1] & 0b00100000 != 0
    }

    /// The sub-stream to which a particular packet belongs is indicated by this Packet Identifier
    /// value.
    #[inline]
    pub fn pid(&self) -> u16 {
        u16::from(self.buf[1] & 0b00011111) << 8 | u16::from(self.buf[2])
    }

    pub fn transport_scrambling_control(&self) -> TransportScramblingControl {
        TransportScramblingControl::from(self.buf[3] >> 6 & 0b11)
    }

    /// The returned enum value indicates if `adaptation_field()`, `payload()` or both will return
    /// something.
    #[inline]
    pub fn adaptation_control(&self) -> AdaptationControl {
        AdaptationControl::from(self.buf[3] >> 4 & 0b11)
    }

    /// Each packet with a given `pid()` value within a transport stream should have a continuity
    /// counter value which increases by 1 from the last counter value seen.  Unexpected continuity
    /// counter values allow the receiver of the transport stream to detect discontinuities in the
    /// stream (e.g. due to data loss during transmission).
    #[inline]
    pub fn continuity_counter(&self) -> ContinuityCounter {
        ContinuityCounter::new(self.buf[3] & 0b00001111)
    }

    fn adaptation_field_length(&self) -> usize {
        self.buf[4] as usize
    }

    /// An `AdaptationField` contains additional packet headers that may be present in the packet.
    pub fn adaptation_field(&self) -> Option<AdaptationField> {
        match self.adaptation_control() {
            AdaptationControl::Reserved | AdaptationControl::PayloadOnly => None,
            AdaptationControl::AdaptationFieldOnly => {
                let len = self.adaptation_field_length();
                if len != (PACKET_SIZE - ADAPTATION_FIELD_OFFSET) {
                    println!(
                        "invalid adaptation_field_length for AdaptationFieldOnly: {}",
                        len
                    );
                    // TODO: Option<Result<AdaptationField>> instead?
                    return None;
                }
                Some(self.mk_af(len))
            }
            AdaptationControl::AdaptationFieldAndPayload => {
                let len = self.adaptation_field_length();
                if len > 182 {
                    println!(
                        "invalid adaptation_field_length for AdaptationFieldAndPayload: {}",
                        len
                    );
                    // TODO: Option<Result<AdaptationField>> instead?
                    return None;
                }
                Some(self.mk_af(len))
            }
        }
    }

    fn mk_af(&self, len: usize) -> AdaptationField {
        AdaptationField::new(
            &self.buf[ADAPTATION_FIELD_OFFSET..ADAPTATION_FIELD_OFFSET + len],
        )
    }

    /// The data contained within the packet, not including the packet headers.
    /// Not all packets have a payload, and `None` is returned if `adaptation_control()` indicates
    /// that no payload is present.  None may also be returned if the packet is malformed.
    /// If `Some` payload is returned, it is guaranteed not to be an empty slice.
    #[inline(always)]
    pub fn payload(&self) -> Option<&'buf [u8]> {
        match self.adaptation_control() {
            AdaptationControl::Reserved | AdaptationControl::AdaptationFieldOnly => None,
            AdaptationControl::PayloadOnly | AdaptationControl::AdaptationFieldAndPayload => self.mk_payload(),
        }
    }

    #[inline]
    fn mk_payload(&self) -> Option<&'buf [u8]> {
        let offset = self.content_offset();
        if offset == self.buf.len() {
            println!("no payload data present");
            None
        } else if offset > self.buf.len() {
            println!("adaptation_field_length {} too large", self.adaptation_field_length());
            None
        } else {
            Some(&self.buf[offset..])
        }
    }

    // borrow a reference to the underlying buffer of this packet
    pub fn buffer(&self) -> &'buf[u8] {
        self.buf
    }

    fn content_offset(&self) -> usize {
        match self.adaptation_control() {
            AdaptationControl::Reserved |
            AdaptationControl::PayloadOnly => FIXED_HEADER_SIZE,
            AdaptationControl::AdaptationFieldOnly |
            AdaptationControl::AdaptationFieldAndPayload => {
                ADAPTATION_FIELD_OFFSET + self.adaptation_field_length()
            }
        }
    }
}

/// trait for objects which process transport stream packets
pub trait PacketConsumer<Ret> {
    fn consume(&mut self, pk: Packet) -> Option<Ret>;
}

#[cfg(test)]
mod test {
    use packet::*;

    #[test]
    #[should_panic]
    fn zero_len() {
        let buf = [0u8; 0];
        Packet::new(&buf[..]);
    }

    #[test]
    fn test_xmas_tree() {
        let mut buf = [0xffu8; self::PACKET_SIZE];
        buf[0] = self::SYNC_BYTE;
        buf[4] = 3;
        let pk = Packet::new(&buf[..]);
        assert_eq!(pk.pid(), 0b1111111111111);
        assert!(pk.transport_error_indicator());
        assert!(pk.payload_unit_start_indicator());
        assert!(pk.transport_priority());
        assert_eq!(
            pk.transport_scrambling_control(),
            TransportScramblingControl::Undefined3
        );
        assert_eq!(
            pk.adaptation_control(),
            AdaptationControl::AdaptationFieldAndPayload
        );
        assert_eq!(pk.continuity_counter().count(), 0b1111);
        assert!(pk.adaptation_field().is_some());
        assert!(pk.adaptation_field().unwrap().discontinuity_indicator());
    }
}
