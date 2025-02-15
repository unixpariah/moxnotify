{
  pkg-config,
  lua5_4,
  lib,
  rustPlatform,
  libxkbcommon,
  wayland,
  vulkan-headers,
  vulkan-loader,
  vulkan-validation-layers,
}:
let
  cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);
in
rustPlatform.buildRustPackage rec {
  pname = "moxalert";
  version = "${cargoToml.package.version}";
  cargoLock.lockFile = ../Cargo.lock;
  src = lib.fileset.toSource {
    root = ./..;
    fileset = lib.fileset.intersection (lib.fileset.fromSource (lib.sources.cleanSource ./..)) (
      lib.fileset.unions [
        ../src
        ../Cargo.toml
        ../Cargo.lock
      ]
    );
  };

  nativeBuildInputs = [
    pkg-config
    vulkan-headers
  ];

  buildInputs = [
    libxkbcommon
    lua5_4
    libxkbcommon
    vulkan-loader
    vulkan-validation-layers
    wayland
  ];

  postFixup = ''
    patchelf --set-rpath "${lib.makeLibraryPath buildInputs}" $out/bin/moxalert
  '';

  dontPatchELF = false;
  autoPatchelf = true;

  meta = with lib; {
    description = "";
    mainProgram = "moxalert";
    homepage = "https://github.com/unixpariah/moxalert";
    license = licenses.gpl3;
    maintainers = with maintainers; [ unixpariah ];
    platforms = platforms.unix;
  };
}
