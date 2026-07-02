mod container_buf;

use std::str::FromStr;

use container_buf::ContainerBuf;
use flate2::{Decompress, DecompressError};
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

mod imported {
  use js_sys::{JsString, Uint8Array};
  use wasm_bindgen::prelude::*;
  use web_sys::File;

  #[wasm_bindgen(raw_module = "../utils.js")]
  extern "C" {
    pub(crate) fn get_swap_bytes() -> bool;
    pub(crate) fn get_checked(id: JsString) -> bool;
    pub(crate) fn get_file(id: JsString) -> Option<File>;
    pub(crate) fn put_download(data: Uint8Array, file_name: JsString);
    pub(crate) fn get_radio_value(name: JsString) -> Option<JsString>;
  }
}

use imported::get_swap_bytes;

fn get_checked(id: &str) -> bool {
  imported::get_checked(id.into())
}
fn get_file(id: &str) -> Option<web_sys::File> {
  imported::get_file(id.into())
}
fn put_download(data: Uint8Array, file_name: String) {
  imported::put_download(data, file_name.into())
}
fn get_radio_value(name: &str) -> Option<String> {
  imported::get_radio_value(name.into()).map(|s| s.into())
}

#[wasm_bindgen(start)]
pub fn entry_point() -> Result<(), JsValue> {
  Ok(())
}

#[wasm_bindgen]
pub async fn convert(is_create: bool, is_split: bool) -> ConversionResult {
  match (is_create, is_split) {
    (true, false) => {
      if get_checked("retroarch_fmt") {
        if let Some(err) = do_create(ContainerType::RetroArch).await.error {
          return ConversionResult { error: Some(err) };
        }
      }
      if get_checked("bizhawk_fmt") {
        if let Some(err) = do_create(ContainerType::BizHawk).await.error {
          return ConversionResult { error: Some(err) };
        }
      }
      ConversionResult { error: None }
    }
    (false, true) => match get_radio_value("container") {
      Some(container_name) => match ContainerType::from_str(&container_name) {
        Ok(container) => do_split(container).await,
        Err(()) => ConversionResult {
          error: Some("Container invalid value".into()),
        },
      },
      None => ConversionResult {
        error: Some("Container invalid value".into()),
      },
    },
    _ => unreachable!(),
  }
}

#[wasm_bindgen]
pub struct ConversionResult {
  error: Option<String>,
}

#[wasm_bindgen]
impl ConversionResult {
  #[wasm_bindgen(getter)]
  pub fn error(&self) -> Option<String> {
    self.error.clone()
  }
}

fn word_swap(buf: &mut [u8]) {
  for i in (0..buf.len()).step_by(4) {
    buf.swap(i, i + 3);
    buf.swap(i + 1, i + 2);
  }
}

#[derive(Default)]
struct CreateParams {
  battery_file: Option<web_sys::File>,
  controller_paks: [Option<web_sys::File>; 4],
  mupen_pak: Option<web_sys::File>,
}

enum ContainerType {
  RetroArch,
  BizHawk,
}

impl FromStr for ContainerType {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "RetroArch" => Ok(ContainerType::RetroArch),
      "BizHawk" => Ok(ContainerType::BizHawk),
      _ => Err(()),
    }
  }
}

async fn do_create(container: ContainerType) -> ConversionResult {
  let params = CreateParams {
    battery_file: get_file("battery_file"),
    mupen_pak: if get_checked("is_mupen") {
      get_file("controller_pak_mp")
    } else {
      None
    },
    controller_paks: [
      get_file("controller_pak_1"),
      get_file("controller_pak_2"),
      get_file("controller_pak_3"),
      get_file("controller_pak_4"),
    ],
  };

  // container data
  let mut buf = ContainerBuf::new();

  // now try to convert stuff
  let mut file_name = None;

  if let Some(battery) = &params.battery_file {
    file_name = Some(battery.name());

    let battery_arr_buf = match JsFuture::from(battery.array_buffer())
      .await
      .and_then(|js| js.dyn_into::<ArrayBuffer>())
    {
      Ok(arr) => arr,
      Err(val) => {
        return ConversionResult {
          error: val.as_string(),
        }
      }
    };

    let battery_buf_view = Uint8Array::new(&battery_arr_buf);

    // SAFETY: the match ensures an appropriate buffer length
    match battery_buf_view.byte_length() {
      0x1..=0x800 => unsafe { battery_buf_view.raw_copy_to_ptr(buf.eeprom_mut().as_mut_ptr()) },
      0x801..=0x8000 => {
        let data = match container {
          ContainerType::RetroArch => buf.sram_mut(),
          ContainerType::BizHawk => buf.sram_bizhawk_mut(),
        };
        unsafe { battery_buf_view.raw_copy_to_ptr(data.as_mut_ptr()) }
      }
      0x8001..=0x20000 => {
        let data = match container {
          ContainerType::RetroArch => buf.flashram_mut(),
          ContainerType::BizHawk => buf.flashram_bizhawk_mut(),
        };
        unsafe { battery_buf_view.raw_copy_to_ptr(data.as_mut_ptr()) }
      }
      _ => {}
    }
  }

  for (cp, cp_buf) in params
    .controller_paks
    .iter()
    .zip(buf.controller_pak_iter_mut())
  {
    if let Some(cp) = cp {
      if file_name.is_none() {
        file_name = Some(cp.name())
      }
      let array_buf = match JsFuture::from(cp.array_buffer())
        .await
        .and_then(|js| js.dyn_into::<ArrayBuffer>())
      {
        Ok(arr) => arr,
        Err(val) => {
          return ConversionResult {
            error: val.as_string(),
          }
        }
      };
      Uint8Array::new(&array_buf).copy_to(cp_buf);
    }
  }
  if let Some(mp) = &params.mupen_pak {
    if file_name.is_none() {
      file_name = Some(mp.name())
    }
    let arr_buf = match JsFuture::from(mp.array_buffer())
      .await
      .and_then(|js| js.dyn_into::<ArrayBuffer>())
    {
      Ok(arr) => arr,
      Err(val) => {
        return ConversionResult {
          error: val.as_string(),
        }
      }
    };
    Uint8Array::new(&arr_buf).copy_to(buf.full_controller_pak_mut())
  }

  if get_swap_bytes() {
    if !buf.eeprom().is_empty() {
      word_swap(buf.eeprom_mut());
    }
    match container {
      ContainerType::RetroArch => {
        if !buf.flashram().is_empty() {
          word_swap(buf.flashram_mut());
        }
      }
      ContainerType::BizHawk => {
        if !buf.flashram_bizhawk().is_empty() {
          word_swap(buf.flashram_bizhawk_mut());
        }
      }
    }
  }

  if let Some(file_name) = file_name {
    let ext = match container {
      ContainerType::RetroArch => ".srm",
      ContainerType::BizHawk => ".SaveRAM",
    };
    download_file(buf.as_ref(), with_extension(&file_name, ext));
    ConversionResult { error: None }
  } else {
    ConversionResult {
      error: Some("No input file(s)".into()),
    }
  }
}

fn unrzip(compressed_data: &[u8], data: &mut [u8]) -> Result<(), DecompressError> {
  let mut i = 0;
  let mut o = 0;
  while i + 4 < compressed_data.len() {
    let chunk_size = u32::from_le_bytes(compressed_data[i..i + 4].try_into().unwrap());
    i += 4;

    let mut decompress = Decompress::new(true);
    decompress.decompress(
      &compressed_data[i..i + chunk_size as usize],
      &mut data[o..],
      flate2::FlushDecompress::Sync,
    )?;

    o += decompress.total_out() as usize;
    i += chunk_size as usize;
  }

  Ok(())
}

async fn do_split(container: ContainerType) -> ConversionResult {
  if let Some(container_file) = get_file("container_file") {
    let file_name = container_file.name();
    let arr_buf = match JsFuture::from(container_file.array_buffer())
      .await
      .and_then(|js| js.dyn_into::<ArrayBuffer>())
    {
      Ok(arr) => arr,
      Err(val) => {
        return ConversionResult {
          error: val.as_string(),
        }
      }
    };
    let uint_buf = Uint8Array::new(&arr_buf);

    let mut buf = ContainerBuf::new();

    if uint_buf.length() < 0x48800 {
      // we cannot copy this, is this maybe an rzip ?
      let compressed_data = uint_buf.to_vec();

      if b"#RZIPv\x01#" != &compressed_data[..8] {
        return ConversionResult {
          error: Some("SRM is not of the correct size".into()),
        };
      }
      let total_len = (compressed_data.len() > 20)
        .then(|| u64::from_le_bytes(compressed_data[12..20].try_into().unwrap()));
      if total_len != Some(0x48800) {
        return ConversionResult {
          error: Some(format!(
            "Compressed SRM is not of the expected size: {:#?}",
            total_len
          )),
        };
      }

      // check for rzip
      if let Err(err) = unrzip(&compressed_data[20..], buf.as_mut()) {
        return ConversionResult {
          error: Some(format!("Could not extract from RZip file: {err}")),
        };
      }
    } else if uint_buf.length() > 0x48800 {
      return ConversionResult {
        error: Some("SRM is not of the correct size".into()),
      };
    } else {
      uint_buf.copy_to(buf.as_mut());
    }

    if buf.is_empty() {
      return ConversionResult {
        error: Some("Empty SRM".into()),
      };
    }

    let swap_bytes = get_swap_bytes();

    if !buf.eeprom().is_empty() {
      if swap_bytes {
        word_swap(buf.eeprom_mut());
      }
      let eep = if buf.eeprom().is_4k() {
        buf.eeprom().as_4k()
      } else {
        buf.eeprom()
      };
      download_file(eep.as_ref(), with_extension(&file_name, ".eep"));
    }

    match container {
      ContainerType::RetroArch => {
        if !buf.sram().is_empty() {
          download_file(buf.sram().as_ref(), with_extension(&file_name, ".sra"));
        }
      }
      ContainerType::BizHawk => {
        if !buf.sram_bizhawk().is_empty() {
          download_file(
            buf.sram_bizhawk().as_ref(),
            with_extension(&file_name, ".sra"),
          );
        }
      }
    }

    match container {
      ContainerType::RetroArch => {
        if !buf.flashram().is_empty() {
          if swap_bytes {
            word_swap(buf.flashram_mut());
          }
          download_file(buf.flashram().as_ref(), with_extension(&file_name, ".fla"));
        }
      }
      ContainerType::BizHawk => {
        if !buf.flashram_bizhawk().is_empty() {
          if swap_bytes {
            word_swap(buf.flashram_bizhawk_mut());
          }
          download_file(
            buf.flashram_bizhawk().as_ref(),
            with_extension(&file_name, ".fla"),
          );
        }
      }
    }

    if get_checked("mupen_out") {
      if !buf.full_controller_pak().is_empty() {
        download_file(
          buf.full_controller_pak().as_ref(),
          with_extension(&file_name, ".mpk"),
        );
      }
    } else {
      for (i, cp) in buf.controller_pak_iter().enumerate() {
        if cp.is_empty() {
          continue;
        }
        download_file(
          cp.as_ref(),
          with_extension(
            &file_name,
            match i {
              1 => ".mpk2",
              2 => ".mpk3",
              3 => ".mpk4",
              _ => ".mpk",
            },
          ),
        );
      }
    }

    ConversionResult { error: None }
  } else {
    ConversionResult {
      error: Some("No input file".into()),
    }
  }
}

fn with_extension(name: &str, ext: &str) -> String {
  let mut ret = name.to_owned();
  ret.replace_range(name.rfind('.').unwrap_or(name.len()).., ext);
  ret
}

fn download_file(buf: &[u8], file_name: String) {
  let data = {
    let arr = Uint8Array::new_with_length(buf.as_ref().len() as u32);
    arr.copy_from(buf.as_ref());
    arr
  };

  put_download(data, file_name);
}
