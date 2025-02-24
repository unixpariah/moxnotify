{
  pkg-config,
  lua5_4,
  lib,
  rustPlatform,
  libxkbcommon,
  wayland,
  vulkan-loader,
  rust-bin,
  makeRustPlatform,
}:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../moxnotify/Cargo.toml);
  nightlyRust = rust-bin.nightly.latest.default;
  nightlyRustPlatform = makeRustPlatform {
    cargo = nightlyRust;
    rustc = nightlyRust;
  };
in
nightlyRustPlatform.buildRustPackage rec {
  pname = "moxnotify";
  version = cargoToml.package.version;

  cargoLock = {
    lockFile = ../Cargo.lock;
    allowBuiltinFetchGit = true;
    outputHashes = { };
  };

  src = lib.cleanSourceWith {
    src = ../.;
    filter =
      path: type:
      let
        relPath = lib.removePrefix (toString ../. + "/") (toString path);
      in
      lib.any (p: lib.hasPrefix p relPath) [
        "moxnotify"
        "mox"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    lua5_4
    libxkbcommon
    wayland
    vulkan-loader
  ];

  buildPhase = ''
    cargo build --release --workspace
  '';

  installPhase = ''
    install -Dm755 target/release/moxnotify $out/bin/moxnotify
    install -Dm755 target/release/mox $out/bin/mox
  '';

  postFixup = ''
    patchelf --set-rpath "${lib.makeLibraryPath buildInputs}" $out/bin/moxnotify
  '';

  dontPatchELF = false;
  autoPatchelf = true;

  meta = with lib; {
    description = "Mox desktop environment notification system";
    homepage = "https://github.com/unixpariah/moxnotify";
    license = licenses.gpl3;
    maintainers = [ maintainers.unixpariah ];
    platforms = platforms.linux;
    mainProgram = "moxnotify";
  };
}
