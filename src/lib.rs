mod srm_buf;

use js_sys::{ArrayBuffer, Uint8Array};
use srm_buf::SrmBuf;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

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

#[wasm_bindgen(start)]
pub fn entry_point() -> Result<(), JsValue> {
  Ok(())
}

#[wasm_bindgen]
pub async fn convert(is_create: bool, is_split: bool) -> Result<ConversionResult, JsValue> {
  match (is_create, is_split) {
    (true, false) => do_create().await,
    (false, true) => do_split().await,
    _ => Err(JsValue::UNDEFINED),
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
  controller_packs: [Option<web_sys::File>; 4],
  mupen_pack: Option<web_sys::File>,
}

async fn do_create() -> Result<ConversionResult, JsValue> {
  let params = CreateParams {
    battery_file: get_file("battery_file"),
    mupen_pack: if get_checked("is_mupen") {
      get_file("controller_pack_mp")
    } else {
      None
    },
    controller_packs: [
      get_file("controller_pack_1"),
      get_file("controller_pack_2"),
      get_file("controller_pack_3"),
      get_file("controller_pack_4"),
    ],
  };

  // srm data
  let mut buf = SrmBuf::new();

  // now try to convert stuff

  let mut file_name = None;

  if let Some(battery) = &params.battery_file {
    file_name = Some(battery.name());

    let battery_arr_buf = JsFuture::from(battery.array_buffer())
      .await
      .and_then(|js| js.dyn_into::<ArrayBuffer>())?;

    let battery_buf_view = Uint8Array::new(&battery_arr_buf);
    // SAFETY: the match ensures an appropriate buffer length
    match battery_buf_view.byte_length() {
      0x1..=0x800 => unsafe { battery_buf_view.raw_copy_to_ptr(buf.eeprom_mut().as_mut_ptr()) },
      0x801..=0x8000 => unsafe { battery_buf_view.raw_copy_to_ptr(buf.sram_mut().as_mut_ptr()) },
      0x8001..=0x20000 => unsafe {
        battery_buf_view.raw_copy_to_ptr(buf.flashram_mut().as_mut_ptr())
      },
      _ => {}
    }
  }
  for (cp, cp_buf) in params
    .controller_packs
    .iter()
    .zip(buf.controller_pack_iter_mut())
  {
    if let Some(cp) = cp {
      if file_name.is_none() {
        file_name = Some(cp.name())
      }
      let array_buf = JsFuture::from(cp.array_buffer())
        .await
        .and_then(|js| js.dyn_into::<ArrayBuffer>())?;
      Uint8Array::new(&array_buf).copy_to(cp_buf);
    }
  }
  if let Some(mp) = &params.mupen_pack {
    if file_name.is_none() {
      file_name = Some(mp.name())
    }
    let arr_buf = JsFuture::from(mp.array_buffer())
      .await
      .and_then(|js| js.dyn_into::<ArrayBuffer>())?;
    Uint8Array::new(&arr_buf).copy_to(buf.full_controller_pack_mut())
  }

  if get_swap_bytes() {
    word_swap(buf.eeprom_mut());
    word_swap(buf.flashram_mut());
  }

  Ok(if let Some(file_name) = file_name {
    download_file(buf.as_ref(), with_extension(&file_name, ".srm"));
    ConversionResult { error: None }
  } else {
    ConversionResult {
      error: Some("No input file(s)".into()),
    }
  })
}

async fn do_split() -> Result<ConversionResult, JsValue> {
  if let Some(srm_file) = get_file("srm_file") {
    let file_name = srm_file.name();
    let arr_buf = JsFuture::from(srm_file.array_buffer())
      .await
      .and_then(|js| js.dyn_into::<ArrayBuffer>())?;
    let uint_buf = Uint8Array::new(&arr_buf);

    let mut srm_buf = SrmBuf::new();
    uint_buf.copy_to(srm_buf.as_mut());

    if srm_buf.is_empty() {
      return Ok(ConversionResult {
        error: Some("Empty SRM".into()),
      });
    }

    let swap_bytes = get_swap_bytes();

    if !srm_buf.eeprom().is_empty() {
      if swap_bytes {
        word_swap(srm_buf.eeprom_mut());
      }
      let eep = if srm_buf.eeprom().is_4k() {
        srm_buf.eeprom().as_4k()
      } else {
        srm_buf.eeprom()
      };
      download_file(eep.as_ref(), with_extension(&file_name, ".eep"));
    } else if !srm_buf.sram().is_empty() {
      download_file(srm_buf.sram().as_ref(), with_extension(&file_name, ".sra"));
    } else if !srm_buf.flashram().is_empty() {
      if swap_bytes {
        word_swap(srm_buf.flashram_mut());
      }
      download_file(
        srm_buf.flashram().as_ref(),
        with_extension(&file_name, ".fla"),
      );
    }

    if get_checked("mupen_out") {
      if !srm_buf.full_controller_pack().is_empty() {
        download_file(
          srm_buf.full_controller_pack().as_ref(),
          with_extension(&file_name, ".mpk"),
        );
      }
    } else {
      for (i, cp) in srm_buf.controller_pack_iter().enumerate() {
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

    Ok(ConversionResult { error: None })
  } else {
    Ok(ConversionResult {
      error: Some("No input file".into()),
    })
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
