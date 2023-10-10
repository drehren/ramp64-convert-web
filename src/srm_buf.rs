pub(crate) struct SrmBuf {
  data: Box<[u8; 0x48800]>,
}

impl SrmBuf {
  pub fn new() -> Self {
    let mut data = Box::new([0xff; 0x48800]);
    controller_pack::init(&mut data[0x800..0x8800]);
    data.copy_within(0x800..0x8800, 0x8800);
    data.copy_within(0x800..0x8800, 0x10800);
    data.copy_within(0x800..0x8800, 0x18800);
    Self { data }
  }

  pub fn is_empty(&self) -> bool {
    self.eeprom().is_empty()
      && self.sram().is_empty()
      && self.flashram().is_empty()
      && self.controller_pack_iter().all(|cp| cp.is_empty())
  }

  pub fn eeprom(&self) -> Eeprom<'_> {
    Eeprom(&self.data[..0x800])
  }
  pub fn eeprom_mut(&mut self) -> &mut [u8] {
    &mut self.data[..0x800]
  }

  pub fn controller_pack_iter(&self) -> impl Iterator<Item = ControllerPack> {
    self.data[0x800..0x20800].chunks(0x8000).map(ControllerPack)
  }

  pub fn controller_pack_iter_mut(&mut self) -> std::slice::ChunksMut<u8> {
    self.data[0x800..0x20800].chunks_mut(0x8000)
  }

  pub fn full_controller_pack(&self) -> ControllerPack {
    ControllerPack(&self.data[0x800..0x20800])
  }
  pub fn full_controller_pack_mut(&mut self) -> &mut [u8] {
    &mut self.data[0x800..0x20800]
  }

  pub fn sram(&self) -> Sram<'_> {
    Sram(&self.data[0x20800..0x28800])
  }
  pub fn sram_mut(&mut self) -> &mut [u8] {
    &mut self.data[0x20800..0x28800]
  }

  pub fn flashram(&self) -> FlashRam<'_> {
    FlashRam(&self.data[0x28800..0x48800])
  }
  pub fn flashram_mut(&mut self) -> &mut [u8] {
    &mut self.data[0x28800..0x48800]
  }
}

impl AsRef<[u8]> for SrmBuf {
  fn as_ref(&self) -> &[u8] {
    self.data.as_ref()
  }
}

impl AsMut<[u8]> for SrmBuf {
  fn as_mut(&mut self) -> &mut [u8] {
    self.data.as_mut()
  }
}

macro_rules! srm_internal_data {
  ($name:ident, $($is_empty:tt)+) => {
    pub(crate) struct $name<'srm>(&'srm [u8]);
    impl<'srm> $name<'srm> {
      pub(crate) $($is_empty)+

    }
    impl<'srm> AsRef<[u8]> for $name<'srm> {
      fn as_ref(&self) -> &[u8] {
        self.0
      }
    }
  };

  ($name:ident) => {
    srm_internal_data!($name, fn is_empty(&self) -> bool {
      self
      .0
      .iter()
      .rposition(|b| *b != 0xff)
      .is_none()
    });
  };
}

srm_internal_data!(Eeprom);
srm_internal_data!(FlashRam);
srm_internal_data!(Sram);
srm_internal_data!(
  ControllerPack,
  fn is_empty(&self) -> bool {
    controller_pack::is_empty(self.0)
  }
);

impl<'srm> Eeprom<'srm> {
  pub(crate) fn is_4k(&self) -> bool {
    self.0[0x200..].iter().all(|b| b == &0xff)
  }

  pub(crate) fn as_4k(&self) -> Self {
    Eeprom(&self.0[..0x200])
  }
}

mod controller_pack {
  fn checksum1(buf: &[u8]) -> [u8; 2] {
    let mut sum1 = 0u16;
    for half_word in buf[0..24].chunks(2) {
      sum1 = sum1.wrapping_add(u16::from_be_bytes(half_word.try_into().unwrap()));
    }
    sum1
      .wrapping_add(u16::from_be_bytes([buf[24], buf[25]]))
      .wrapping_add(u16::from_be_bytes([buf[26], buf[27]]))
      .to_be_bytes()
  }

  fn checksum2(buf: &[u8]) -> [u8; 2] {
    u16::from_be_bytes([0xff, 0xf2])
      .wrapping_sub(u16::from_be_bytes(buf.try_into().unwrap()))
      .to_be_bytes()
  }

  pub(crate) fn is_empty(mut buf: &[u8]) -> bool {
    const FREE_SPACE: u16 = u16::from_be_bytes([0, 3]);
    buf = &buf[256..512];
    // check the index table, if all unallocated there is no prob
    for v in buf[10..].chunks(2) {
      let val = u16::from_be_bytes(v.try_into().unwrap());
      if val != FREE_SPACE {
        return false;
      }
    }
    true
  }

  pub(crate) fn init(buf: &mut [u8]) {
    const MUPEN64_SERIAL: [u8; 24] = [
      0xff, 0xff, 0xff, 0xff, 0x05, 0x1a, 0x5f, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
      0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];

    buf.fill(0);
    buf[0] = 0x81;
    for (i, b) in buf[0..32].iter_mut().enumerate().skip(1) {
      *b = i as u8;
    }
    buf[32..56].copy_from_slice(&MUPEN64_SERIAL);
    buf[56] = 0xff;
    buf[57] = 0xff;
    buf[58] = 0x01;
    buf[59] = 0xff;
    let s1 = checksum1(&buf[32..64]);
    buf[60..62].copy_from_slice(&s1);
    let s2 = checksum2(&buf[60..62]);
    buf[62..64].copy_from_slice(&s2);

    buf.copy_within(32..64, 96);
    buf.copy_within(32..64, 128);
    buf.copy_within(32..64, 192);

    let table = &mut buf[256..512];
    table[0] = 0;
    table[1] = 113;
    for (i, b) in table[10..].iter_mut().enumerate() {
      *b = ((i % 2) * 3) as u8;
    }

    buf.copy_within(256..512, 512);
  }
}
