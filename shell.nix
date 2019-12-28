{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  name = "rust-env";
  buildInputs = with pkgs; [ rustc cargo rustfmt gcc pkgconfig openssl ];

  RUST_BACKTRACE = 1;
}
