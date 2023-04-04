export function get_checked(elem) {
  if (typeof elem === "string" || elem instanceof String) {
    elem = document.getElementById(elem)
  }
  return elem instanceof HTMLInputElement ? elem.checked : false;
}
export function get_swap_bytes() {
  return get_checked("swap_bytes");
}
export function set_hidden(elem, hidden) {
  if (typeof elem === "string" || elem instanceof String) {
    elem = document.getElementById(elem)
  }
  if (elem instanceof HTMLElement) {
    elem.hidden = hidden;
  }
}
export function get_hidden(elem) {
  if (typeof elem === "string" || elem instanceof String) {
    elem = document.getElementById(elem)
  }
  return elem instanceof HTMLElement ? elem.hidden : false;
}
export function get_file(elem) {
  if (typeof elem === "string" || elem instanceof String) {
    elem = document.getElementById(elem);
  }
  return elem instanceof HTMLInputElement ? (elem.files ? elem.files[0] : null) : null;
}
export function put_download(data, file_name) {
  const blob = new Blob([data], { type: "application/octet-stream" });
  const ref = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.innerHTML = file_name;
  a.setAttribute("href", ref);
  a.setAttribute("download", file_name);

  const item = document.createElement("li");
  item.appendChild(a);

  const dl_zone = document.getElementById("download_zone");
  dl_zone.appendChild(item);
}
