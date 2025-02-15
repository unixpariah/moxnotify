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
  daemonCargoToml = builtins.fromTOML (builtins.readFile ../moxsignal-daemon/Cargo.toml);
in
rustPlatform.buildRustPackage rec {
  pname = "moxsignal";
  version = daemonCargoToml.package.version;

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
        "moxsignal-daemon"
        "moxsignalctl"
        "Cargo.toml"
        "Cargo.lock"
      ];
  };

  nativeBuildInputs = [
    pkg-config
    vulkan-headers
  ];

  buildInputs = [
    lua5_4
    libxkbcommon
    wayland
    vulkan-loader
    vulkan-validation-layers
  ];

  buildPhase = ''
    cargo build --release --workspace
  '';

  installPhase = ''
    install -Dm755 target/release/moxsignal-daemon $out/bin/moxsignal-daemon
    install -Dm755 target/release/moxsignalctl $out/bin/moxsignalctl
  '';

  postFixup = ''
    for bin in $out/bin/moxsignal-*; do
      patchelf --set-rpath "${lib.makeLibraryPath buildInputs}" $bin
    done
  '';

  meta = with lib; {
    description = "Mox desktop environment notification system";
    homepage = "https://github.com/unixpariah/moxsignal";
    license = licenses.gpl3;
    maintainers = [ maintainers.unixpariah ];
    platforms = platforms.linux;
    mainProgram = "moxsignal-daemon";
  };
}
