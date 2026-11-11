//! Build-381 command 32. Sources: doc 10 gw1, 0x1405F5980 and 0x140DAA240.
//! This codec does not load a character, authorize entry, or create key material.
use bloodlib::packet::{PacketError, MAX_PACKET};

/// Exact native descriptor. The builder bypasses the composer for properties.
/// Its two `?` slots contain eight-byte ability values, not absent fields.
pub const NATIVE_DESCRIPTOR: &str = "hihqQihhhhhhcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccsihhhhcicqqqqqccqcihhhhhhcqcccqcqiqqqqqqqqqqqqcqcccccbhhhicciiiiccccccqcciqqqqqqqqqqqqqqqqqhqqqqhqcqqhhciiqqhqiqcicccci?i?ihhhhiiiihhhhhhhhhiihhhhhhhhhhhhhhhhhiihhiihhhhhihihhhhhhhhhhhhhhhhhhhhhhhiihhihhhhhhhhhhihihhhhhccciciciiqcccccccciiccciccciccciiiiiihhhhiiiihhhhhhhhhhhhhbKCci;CQb;cccccqCQqq;QCch;CQiqqihcccc;Ccicicicicicichhhhhhhhhhc;";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyValue {
    Byte(u8),
    Short(u16),
    Int(u32),
    Long(u64),
    Bit(bool),
    String(Vec<u16>),
    Bytes(Vec<u8>),
}

/// All 422 property IDs, in ID order. There is no default character snapshot.
/// Doc 10 lists every field name and native type. Ability objects supply only
/// their current scalar value. No native object layout is exposed here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatureProperties([PropertyValue; 422]);
impl CreatureProperties {
    pub fn new(values: [PropertyValue; 422]) -> Result<Self, PacketError> {
        for (index, value) in values.iter().enumerate() {
            validate_property(index + 1, value)?;
        }
        Ok(Self(values))
    }
    pub fn get(&self, id: u16) -> Option<&PropertyValue> {
        self.0.get(usize::from(id).checked_sub(1)?)
    }
    pub fn set(&mut self, id: u16, value: PropertyValue) -> Result<(), PacketError> {
        let index = usize::from(id)
            .checked_sub(1)
            .ok_or(PacketError::ValueCount)?;
        if index >= self.0.len() {
            return Err(PacketError::ValueCount);
        }
        validate_property(index + 1, &value)?;
        self.0[index] = value;
        Ok(())
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut index = 0;
        while index < self.0.len() {
            let id = index + 1;
            match property_code(id).expect("validated property ID") {
                'B' => {
                    // Native type 10 retains independent bits in shared storage.
                    let mut packed = 0;
                    for bit in 0..8 {
                        if self.0[index + bit] == PropertyValue::Bit(true) {
                            packed |= 1 << bit;
                        }
                    }
                    bytes.push(packed);
                    index += 8;
                }
                'F' => {
                    // Preserve 381's getter quirk. The same first-member offset
                    // supplies every bit; see 0x140698080..0x1406980BC.
                    bytes.push(if self.0[index] == PropertyValue::Bit(true) {
                        255
                    } else {
                        0
                    });
                    index += 8;
                }
                _ => {
                    match &self.0[index] {
                        PropertyValue::Byte(n) => bytes.push(*n),
                        PropertyValue::Short(n) => bytes.extend(n.to_le_bytes()),
                        PropertyValue::Int(n) => bytes.extend(n.to_le_bytes()),
                        PropertyValue::Long(n) => bytes.extend(n.to_le_bytes()),
                        PropertyValue::String(s) => {
                            let end = s.iter().position(|n| *n == 0).unwrap_or(s.len());
                            bytes.extend((end as u16).to_le_bytes());
                            for unit in &s[..end] {
                                bytes.extend(unit.to_le_bytes());
                            }
                        }
                        PropertyValue::Bytes(b) => {
                            bytes.extend((b.len() as u16).to_le_bytes());
                            bytes.extend(b);
                        }
                        PropertyValue::Bit(_) => unreachable!("validated scalar"),
                    }
                    index += 1;
                }
            }
        }
        bytes
    }
}

/// Wire kind for one native property ID. B/F distinguish the two packed runs.
/// The native definition table and all 373 descriptor slots agree (doc 10).
pub fn property_code(id: usize) -> Option<char> {
    if !(1..=422).contains(&id) {
        return None;
    }
    if (166..=173).contains(&id) || (218..=241).contains(&id) {
        return Some('B');
    }
    if (344..=367).contains(&id) {
        return Some('F');
    }
    let mut slot = id - 1;
    if id > 173 {
        slot -= 7;
    }
    if id > 241 {
        slot -= 21;
    }
    if id > 367 {
        slot -= 21;
    }
    NATIVE_DESCRIPTOR
        .as_bytes()
        .get(11 + slot)
        .map(|code| *code as char)
}
fn validate_property(id: usize, value: &PropertyValue) -> Result<(), PacketError> {
    let code = property_code(id).ok_or(PacketError::ValueCount)?;
    let valid = match (code, value) {
        ('c', PropertyValue::Byte(_))
        | ('h', PropertyValue::Short(_))
        | ('i', PropertyValue::Int(_))
        | ('q' | '?', PropertyValue::Long(_))
        | ('B' | 'F', PropertyValue::Bit(_)) => true,
        ('s', PropertyValue::String(s)) => s.len() <= 32,
        ('b', PropertyValue::Bytes(b)) => b.len() <= 10,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(PacketError::ValueType(code))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryHeader {
    pub world_id: u16,
    pub object_id: u64,
    pub jumping_character_state: u8,
    pub chat_address_bits: u32,
    pub chat_port: u16,
    pub chat_world_id: u16,
    pub manage_port: u16,
    pub time_zone_bias: i16,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentBan {
    pub effective_from: i64,
    pub effective_to: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryUser {
    pub game_packet_rights: u16,
    pub content_bans: [ContentBan; 9],
    pub time_based_play: [TimeBasedPlay; 2],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeBasedPlay {
    pub schedule_number: i32,
    pub expire_time_utc: i64,
    pub last_reset_time_utc: i64,
    pub duration: i32,
    pub acknowledged_play_time: i16,
    pub rewards: [u8; 4],
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CraftUnion {
    pub id: u8,
    pub level: i16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlateItem {
    pub item_id: u32,
    pub level: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlatePage {
    pub page: u8,
    pub items: [SlateItem; 6],
    pub stones: [u16; 9],
    pub new_stone_id: u16,
    pub new_stone_position: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnterWorldOk {
    pub header: EntryHeader,
    pub properties: CreatureProperties,
    /// Exact extra-property result. The enclosing field adds its byte count.
    pub extra_properties: Vec<u8>,
    /// Opaque external block. Never generate or log its material here.
    pub opaque_broadcast_block: Vec<u8>,
    /// Slots 1..25. Negative remaining times become zero, as in 0x140618730.
    pub recycle_remaining: [Option<i32>; 25],
    /// Property 2 on pages 1 and 2, without its counted-byte prefix.
    pub acquired_skills: [Vec<u8>; 2],
    pub config_bytes: [u8; 4],
    pub pc_cafe_publisher: u8,
    pub pc_db_id: u64,
    pub content_bans_enabled: bool,
    pub user: Option<EntryUser>,
    pub test_server_enabled: bool,
    pub craft_unions: [CraftUnion; 2],
    pub slate_pages: [SlatePage; 2],
}
impl EnterWorldOk {
    /// Native allocation groups are indivisible. Fragment headers remain outside
    /// the logical body; first mark=total bytes, middle=-1, last continuation=0.
    pub fn encode_fragments(&self) -> Result<Vec<Vec<u8>>, PacketError> {
        let mut out = Fragments::new(32);
        let h = &self.header;
        let mut prefix = h.world_id.to_le_bytes().to_vec();
        prefix.extend(h.object_id.to_le_bytes());
        prefix.push(h.jumping_character_state);
        prefix.extend(h.chat_address_bits.to_le_bytes());
        prefix.extend(h.chat_port.to_le_bytes());
        prefix.extend(h.chat_world_id.to_le_bytes());
        prefix.extend(h.manage_port.to_le_bytes());
        prefix.extend(h.time_zone_bias.to_le_bytes());
        prefix.extend(
            self.user
                .as_ref()
                .map_or(0, |u| u.game_packet_rights)
                .to_le_bytes(),
        );
        out.push(prefix)?;
        out.push(self.properties.encode())?;
        out.push(counted_bytes(&self.extra_properties)?)?;
        out.push(counted_bytes(&self.opaque_broadcast_block)?)?;
        let count = self.recycle_remaining.iter().flatten().count();
        let mut recycle = (count as u16).to_le_bytes().to_vec();
        for (index, remaining) in self.recycle_remaining.iter().enumerate() {
            if let Some(remaining) = remaining {
                recycle.push((index + 1) as u8);
                recycle.extend(remaining.max(&0).to_le_bytes());
            }
        }
        out.push(recycle)?;
        out.push(2u16.to_le_bytes().to_vec())?;
        for (index, skills) in self.acquired_skills.iter().enumerate() {
            let mut page = vec![(index + 1) as u8];
            page.extend(counted_bytes(skills)?);
            out.push(page)?;
        }
        out.push(self.config_bytes.to_vec())?;
        let mut identity = vec![self.pc_cafe_publisher];
        identity.extend(self.pc_db_id.to_le_bytes());
        out.push(identity)?;
        let mut bans = vec![0, 0];
        let mut count = 0u16;
        if let Some(user) = self.user.as_ref().filter(|_| self.content_bans_enabled) {
            for (index, ban) in user.content_bans.iter().enumerate() {
                if ban.effective_from != 0 || ban.effective_to != 0 {
                    count += 1;
                    bans.push(index as u8);
                    bans.extend(ban.effective_from.to_le_bytes());
                    bans.extend(ban.effective_to.to_le_bytes());
                }
            }
        }
        bans[..2].copy_from_slice(&count.to_le_bytes());
        out.push(bans)?;
        out.push(vec![u8::from(self.test_server_enabled)])?;
        out.push(
            (self
                .craft_unions
                .iter()
                .filter(|u| (1..=14).contains(&u.id))
                .count() as u16)
                .to_le_bytes()
                .to_vec(),
        )?;
        for union in self
            .craft_unions
            .iter()
            .filter(|u| (1..=14).contains(&u.id))
        {
            let mut row = vec![union.id];
            row.extend(union.level.to_le_bytes());
            out.push(row)?;
        }
        out.push(
            (if self.user.is_some() { 2u16 } else { 0 })
                .to_le_bytes()
                .to_vec(),
        )?;
        if let Some(user) = &self.user {
            for (index, info) in user.time_based_play.iter().enumerate() {
                let mut row = vec![index as u8];
                row.extend(info.schedule_number.to_le_bytes());
                row.extend(info.expire_time_utc.to_le_bytes());
                row.extend(info.last_reset_time_utc.to_le_bytes());
                row.extend(info.duration.to_le_bytes());
                row.extend(info.acknowledged_play_time.to_le_bytes());
                row.extend(info.rewards);
                out.push(row)?;
            }
        }
        out.push(2u16.to_le_bytes().to_vec())?;
        for page in &self.slate_pages {
            let mut row = vec![page.page];
            for item in &page.items {
                row.extend(item.item_id.to_le_bytes());
                row.push(item.level);
            }
            for stone in page.stones {
                row.extend(stone.to_le_bytes());
            }
            row.extend(page.new_stone_id.to_le_bytes());
            row.push(page.new_stone_position);
            out.push(row)?;
        }
        out.finish()
    }
}
fn counted_bytes(bytes: &[u8]) -> Result<Vec<u8>, PacketError> {
    let mut out = i16::try_from(bytes.len())
        .map_err(|_| PacketError::LengthRange)?
        .to_le_bytes()
        .to_vec();
    out.extend(bytes);
    Ok(out)
}
/// Fragment writer for one command. Each `push` is one indivisible group.
pub struct Fragments {
    command: u16,
    chunks: Vec<Vec<u8>>,
    total: usize,
}
impl Fragments {
    pub fn new(command: u16) -> Self {
        Self {
            command,
            chunks: Vec::new(),
            total: 0,
        }
    }
    pub fn push(&mut self, bytes: Vec<u8>) -> Result<(), PacketError> {
        if bytes.len() > MAX_PACKET - 8 {
            return Err(PacketError::ComposeOverflow);
        }
        self.total = self
            .total
            .checked_add(bytes.len())
            .filter(|n| *n <= i32::MAX as usize)
            .ok_or(PacketError::ComposeOverflow)?;
        if self
            .chunks
            .last()
            .is_none_or(|last| last.len() + bytes.len() > MAX_PACKET)
        {
            self.chunks.push(vec![0; 8]);
        }
        self.chunks
            .last_mut()
            .expect("allocated fragment")
            .extend(bytes);
        Ok(())
    }
    pub fn finish(mut self) -> Result<Vec<Vec<u8>>, PacketError> {
        let last = self
            .chunks
            .len()
            .checked_sub(1)
            .ok_or(PacketError::ComposeOverflow)?;
        for (index, chunk) in self.chunks.iter_mut().enumerate() {
            let size = chunk.len() as u16;
            chunk[..2].copy_from_slice(&size.to_le_bytes());
            chunk[2..4].copy_from_slice(&self.command.to_le_bytes());
            let mark = if index == 0 {
                self.total as i32
            } else if index == last {
                0
            } else {
                -1
            };
            chunk[4..8].copy_from_slice(&mark.to_le_bytes());
        }
        Ok(self.chunks)
    }
}
