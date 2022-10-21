{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = [
    pkgs.poppler_utils # pdftohtml(1)
    pkgs.cargo
  ];
}
