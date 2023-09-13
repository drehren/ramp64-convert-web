# RaMp64 SRM Convert WEB

A bare-bones web to allow phone users with browser with wasm capabilities (all of them?) to also use
the converter.

You can use the github hosted version [here](https://drehren.github.io/ramp64-convert-web/)

For a console application, see [ra_mp64_srm_convert](https://github.com/drehren/ra_mp64_srm_convert).
For a GUI application, see [ramp64-convert-gui](https://github.com/drehren/ramp64-convert-gui).

## Building 

To build this, you'll need [rust](https://www.rust-lang.org) (>=1.66) and [wasm-pack](https://rustwasm.github.io/wasm-pack/) (>=0.11).

If you install wasm-pack by using `cargo install wasm-pack` you'll need to have `perl` and a C/C++ compiler installed as well.

Then, from the root folder of this repo:

```sh
wasm-pack --release --target web -d www/pkg
```

After it completes, any web server can host it from `www`.

## License

MIT
